use crate::{
    error::{PdfError, PdfResult},
    page::opcode::is_op,
};
use std::io::{Cursor, Read, Seek};

use crate::{page::operator::Operator, parser::syntax::SyntaxParser};

#[derive(Debug)]
pub struct ContentParser<R: Read + Seek = Cursor<Vec<u8>>> {
    syntax: SyntaxParser<R>,
}

impl<R: Read + Seek> ContentParser<R> {
    pub fn new(syntax: SyntaxParser<R>) -> Self {
        ContentParser { syntax }
    }

    pub fn position(&self) -> u64 {
        self.syntax.position()
    }

    pub fn read_operator(&mut self) -> PdfResult<Option<Operator>> {
        self.read_operator_inner(None, None)
    }

    pub(crate) fn read_operator_with_budget(
        &mut self,
        budget: &crate::budget::OperationBudget,
        retained: &mut crate::budget::MemoryLease,
    ) -> PdfResult<Option<Operator>> {
        self.read_operator_inner(Some(budget), Some(retained))
    }

    fn read_operator_inner(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
        mut retained: Option<&mut crate::budget::MemoryLease>,
    ) -> PdfResult<Option<Operator>> {
        let mut operand_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut operands = Vec::new();
        loop {
            let start = self.syntax.position();
            let word = if let Some(budget) = budget {
                self.syntax.get_next_word_with_budget(budget)
            } else {
                self.syntax.get_next_word()
            };
            match word {
                Ok(word) => {
                    let raw = word.raw();
                    let bare_keyword = !word.is_number()
                        && !matches!(raw, b"true" | b"false" | b"null")
                        && raw.first().is_some_and(|byte| !b"/[]()<>{}".contains(byte));
                    if is_op(raw) || bare_keyword {
                        if matches!(raw, b"ID" | b"EI") {
                            self.syntax.set_pos(start)?;
                            return Err(PdfError::ContentParseError(
                                "ID/EI outside an inline image".into(),
                            ));
                        }
                        if word.is_equal(b"BI") {
                            let offset = self.syntax.position().saturating_sub(2);
                            drop(word);
                            if !operands.is_empty() {
                                self.syntax.set_pos(start)?;
                                return Err(PdfError::ContentParseError(
                                    "BI cannot have ordinary operands".into(),
                                ));
                            }
                            let local = self.syntax.limits().operation_budget();
                            let pool = budget.unwrap_or(&local);
                            match super::inline_image::read(&mut self.syntax, pool) {
                                Ok((image, mut lease)) => {
                                    if let Some(retained) = retained.as_mut() {
                                        retained.absorb(&mut lease)?;
                                    }
                                    return Ok(Some(Operator::from_inline(image, offset)));
                                }
                                Err(error) => {
                                    self.syntax.set_pos(start)?;
                                    return Err(error);
                                }
                            }
                        }
                        let offset = self
                            .syntax
                            .position()
                            .saturating_sub(word.raw().len() as u64);
                        let (opname, mut name_lease) = word.into_reserved_string()?;
                        let op = Operator::new(opname, operands).with_byte_offset(offset);
                        if let (Some(retained), Some(lease)) =
                            (retained.as_mut(), operand_lease.as_mut())
                        {
                            retained.absorb(lease)?;
                        }
                        if let (Some(retained), Some(lease)) =
                            (retained.as_mut(), name_lease.as_mut())
                        {
                            retained.absorb(lease)?;
                        }
                        return Ok(Some(op));
                    } else {
                        // Only operands need object parsing from the original
                        // position. Drop the classification token first.
                        drop(word);
                        self.syntax.set_pos(start)?;
                        if operands.len() >= self.syntax.limits().max_objects {
                            return Err(PdfError::ResourceLimit("content operand count".into()));
                        }
                        if let Some(budget) = budget {
                            if operands.len() == operands.capacity() {
                                let capacity = operands
                                    .capacity()
                                    .saturating_mul(2)
                                    .max(1)
                                    .min(self.syntax.limits().max_objects);
                                let size = std::mem::size_of::<crate::objects::PdfObject>();
                                let mut lease = budget.reserve(
                                    capacity.checked_mul(size).ok_or_else(|| {
                                        PdfError::ResourceLimit("content operand table size".into())
                                    })?,
                                )?;
                                let mut replacement = Vec::new();
                                replacement.try_reserve_exact(capacity).map_err(|_| {
                                    PdfError::ResourceLimit("content operand allocation".into())
                                })?;
                                lease.resize(
                                    replacement.capacity().checked_mul(size).ok_or_else(|| {
                                        PdfError::ResourceLimit(
                                            "content operand table capacity".into(),
                                        )
                                    })?,
                                )?;
                                replacement.append(&mut operands);
                                operands = replacement;
                                operand_lease = Some(lease);
                            }
                        } else {
                            operands.try_reserve(1).map_err(|_| {
                                PdfError::ResourceLimit("content operand allocation".into())
                            })?;
                        }
                        let obj =
                            if let (Some(budget), Some(retained)) = (budget, retained.as_mut()) {
                                let (object, mut lease) =
                                    self.syntax.get_content_object_with_budget(budget)?;
                                retained.absorb(&mut lease)?;
                                object
                            } else {
                                self.syntax.get_object()?
                            };
                        operands.push(obj)
                    }
                }
                Err(PdfError::EndofFile) => {
                    self.syntax.set_pos(start)?;
                    if !operands.is_empty() {
                        return Err(PdfError::ContentParseError(
                            "content ends with operands without an operator".into(),
                        ));
                    }
                    return Ok(None);
                }
                Err(error) => {
                    self.syntax.set_pos(start)?;
                    return Err(error);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use crate::{
        io::stream_reader::StreamReader, page::content_parser::ContentParser,
        parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;

    #[test]
    fn operand_budget_refusal_precedes_malformed_object_parsing() {
        use crate::{budget::OperationBudget, error::PdfError, objects::PdfObject};
        let input = b"(unterminated".as_slice();
        let mut parser = ContentParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(input)).unwrap(),
        ));
        let budget = OperationBudget::new(std::mem::size_of::<PdfObject>() - 1);
        let mut retained = budget.reserve(0).unwrap();
        assert!(matches!(
            parser.read_operator_with_budget(&budget, &mut retained),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(parser.position(), 0);
        assert_eq!(retained.bytes(), 0);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn token_reuse_keeps_eof_and_inline_rejection_positions_stable() {
        use crate::error::PdfError;
        for bytes in [b"q  %tail\n".as_slice(), b"q  %head\n BI".as_slice()] {
            let reader = StreamReader::try_new(Cursor::new(bytes)).unwrap();
            let mut parser = ContentParser::new(SyntaxParser::new(reader));
            assert_eq!(
                parser.read_operator().unwrap().unwrap().byte_offset(),
                Some(0)
            );
            let before = parser.position();
            if bytes.ends_with(b"BI") {
                for _ in 0..2 {
                    assert!(matches!(parser.read_operator(), Err(PdfError::EndofFile)));
                    assert_eq!(parser.position(), before);
                }
            } else {
                for _ in 0..2 {
                    assert!(parser.read_operator().unwrap().is_none());
                    assert_eq!(parser.position(), before);
                }
            }
        }
        let reader = StreamReader::try_new(Cursor::new(b"1".as_slice())).unwrap();
        let mut parser = ContentParser::new(SyntaxParser::new(reader));
        assert!(matches!(
            parser.read_operator(),
            Err(PdfError::ContentParseError(_))
        ));
        assert_eq!(parser.position(), 1);
    }

    #[test]
    fn operand_limit_is_per_operation_and_checked_before_object_parse() {
        use crate::{
            error::PdfError,
            parser::options::{Limits, ParseOptions},
        };
        let options = ParseOptions {
            limits: Limits {
                max_objects: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let reader =
            StreamReader::try_new(Cursor::new(b"1 2 custom 3 4 custom q".to_vec())).unwrap();
        let mut parser = ContentParser::new(SyntaxParser::with_options(reader, options));
        assert_eq!(parser.read_operator().unwrap().unwrap().num_operands(), 2);
        assert_eq!(parser.read_operator().unwrap().unwrap().num_operands(), 2);
        assert_eq!(parser.read_operator().unwrap().unwrap().num_operands(), 0);
        assert!(parser.read_operator().unwrap().is_none());
        let reader = StreamReader::try_new(Cursor::new(b"1 2 (unterminated".to_vec())).unwrap();
        let mut parser = ContentParser::new(SyntaxParser::with_options(reader, options));
        assert!(matches!(
            parser.read_operator(),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn test_content_parser() {
        println!("tet content parser");
        let content: &[u8] = &[
            113, 10, 47, 82, 101, 108, 97, 116, 105, 118, 101, 67, 111, 108, 111, 114, 105, 109,
            101, 116, 114, 105, 99, 32, 114, 105, 32, 10, 47, 71, 83, 50, 32, 103, 115, 10, 66, 84,
            10, 47, 70, 49, 32, 49, 32, 84, 102, 10, 49, 51, 46, 57, 49, 56, 51, 32, 48, 32, 48,
            32, 49, 51, 46, 57, 49, 56, 51, 32, 50, 50, 49, 46, 54, 53, 57, 32, 53, 50, 51, 46, 57,
            54, 54, 49, 32, 84, 109, 10, 47, 67, 115, 56, 32, 99, 115, 32, 49, 32, 115, 99, 110,
            10, 48, 46, 48, 54, 57, 56, 32, 84, 99, 10, 48, 32, 84, 119, 10, 91, 40, 70, 111, 117,
            114, 116, 104, 41, 45, 51, 54, 49, 46, 55, 40, 69, 100, 105, 116, 105, 111, 110, 41,
            93, 84, 74, 10, 47, 70, 50, 32, 49, 32, 84, 102, 10, 50, 56, 46, 51, 51, 51, 55, 32,
            48, 32, 48, 32, 50, 56, 46, 51, 51, 51, 55, 32, 50, 50, 49, 46, 54, 53, 57, 32, 52, 56,
            49, 46, 51, 50, 52, 51, 32, 84, 109, 10, 48, 32, 84, 99, 10, 91, 40, 68, 97, 116, 97,
            41, 45, 50, 52, 48, 46, 49, 40, 83, 116, 114, 117, 99, 116, 117, 114, 101, 115, 41, 93,
            84, 74, 10, 48, 32, 45, 49, 46, 48, 53, 50, 55, 32, 84, 68, 10, 45, 48, 46, 48, 48, 48,
            49, 32, 84, 99, 10, 91, 40, 97, 110, 100, 41, 45, 50, 52, 48, 46, 50, 40, 65, 108, 103,
            111, 114, 105, 116, 104, 109, 41, 93, 84, 74, 10, 84, 42, 10, 48, 32, 84, 99, 10, 91,
            40, 65, 110, 97, 108, 121, 115, 105, 115, 41, 45, 50, 52, 48, 46, 51, 40, 105, 110, 41,
            93, 84, 74, 10, 56, 53, 46, 50, 57, 57, 50, 32, 48, 32, 48, 32, 56, 53, 46, 50, 57, 57,
            50, 32, 50, 56, 48, 46, 51, 49, 52, 32, 51, 53, 49, 46, 48, 49, 56, 55, 32, 84, 109,
            10, 40, 67, 41, 84, 106, 10, 52, 55, 46, 55, 49, 57, 56, 32, 48, 32, 48, 32, 52, 55,
            46, 55, 49, 57, 56, 32, 51, 50, 54, 46, 49, 49, 55, 50, 32, 51, 54, 55, 46, 57, 50, 49,
            32, 84, 109, 10, 40, 43, 43, 41, 84, 106, 10, 69, 84, 10, 81, 10,
        ];
        println!("{}", String::from_utf8(content.to_vec()).unwrap());
        let reader = StreamReader::try_new(Cursor::new(content.to_vec())).unwrap();
        let syntax = SyntaxParser::new(reader);
        let mut parser = ContentParser::new(syntax);
        loop {
            let op = parser.read_operator().unwrap();
            match op {
                Some(op) => {
                    println!("{:?}", op);
                }
                None => {
                    break;
                }
            }
        }
    }

    #[test]
    fn test_parse_tj() {
        let content = b"<0011>Tj abc";
        let reader = StreamReader::try_new(Cursor::new(content.to_vec())).unwrap();
        let syntax = SyntaxParser::new(reader);
        let mut parser = ContentParser::new(syntax);
        let op = parser.read_operator();
        println!("op: {:?}", op);
    }

    #[test]
    fn unknown_operator_preserves_operands_and_offset() {
        let reader =
            StreamReader::try_new(Cursor::new(b" 12 /Name customOp q Q".to_vec())).unwrap();
        let mut parser = ContentParser::new(SyntaxParser::new(reader));
        let op = parser.read_operator().unwrap().unwrap();
        assert_eq!(op.name(), "customOp");
        assert_eq!(op.byte_offset(), Some(10));
        assert_eq!(op.num_operands(), 2);
        assert_eq!(parser.read_operator().unwrap().unwrap().name(), "q");
        assert_eq!(parser.read_operator().unwrap().unwrap().name(), "Q");
        assert!(parser.read_operator().unwrap().is_none());
    }
}
