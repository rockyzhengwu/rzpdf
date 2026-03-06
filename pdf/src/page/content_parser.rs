use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
    page::opcode::is_op,
};
use std::io::Cursor;

use crate::{page::operator::Operator, parser::syntax::SyntaxParser};

#[derive(Debug)]
pub struct ContentParser {
    syntax: SyntaxParser<Cursor<Vec<u8>>>,
}

impl ContentParser {
    pub fn new(syntax: SyntaxParser<Cursor<Vec<u8>>>) -> Self {
        ContentParser { syntax }
    }

    pub fn read_operator(&mut self) -> PdfResult<Option<Operator>> {
        let mut operands = Vec::new();
        loop {
            let word = self.syntax.peek_word();
            match word {
                Ok(word) => {
                    if is_op(word.raw()) {
                        if word.is_equal(b"BI") {
                            self.syntax.get_next_word()?;
                            let mut image_objects = PdfDict::default();
                            loop {
                                let next_word = self.syntax.peek_word()?;
                                if next_word.is_equal(b"ID") {
                                    self.syntax.get_next_word()?;
                                    break;
                                }
                                let key = self.syntax.get_object()?.into_name().unwrap();
                                let value = self.syntax.get_object()?;
                                image_objects.insert(key.name().to_string(), value);
                            }
                            // TODO read Image dict
                        }
                        let opword = self.syntax.get_next_word()?;
                        let opname = opword.into_string()?;
                        let op = Operator::new(opname, operands.clone());
                        operands.clear();
                        return Ok(Some(op));
                    } else {
                        let obj = self.syntax.get_object()?;
                        operands.push(obj)
                    }
                }
                Err(PdfError::EndofFile) => return Ok(None),
                _ => {
                    return Err(PdfError::ContentParseError(
                        "Content Parser read operation error".to_string(),
                    ));
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
}
