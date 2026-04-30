use std::io::Cursor;

use crate::{
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict},
    parser::syntax::{ParseMode, SyntaxParser},
};

pub struct ObjectStreams<'a> {
    dict: &'a PdfDict,
    syntax: SyntaxParser<Cursor<Vec<u8>>>,
    data_length: u64,
}

fn object_stream_entry_is_forbidden_in_strict(obj: &PdfObject) -> bool {
    matches!(obj, PdfObject::PdfStream(_) | PdfObject::PdfReference(_))
}

impl<'a> ObjectStreams<'a> {
    pub fn try_new(dict: &'a PdfDict, data: Vec<u8>) -> PdfResult<Self> {
        Self::try_new_with_mode(dict, data, ParseMode::Compatible)
    }

    pub fn try_new_with_mode(dict: &'a PdfDict, data: Vec<u8>, mode: ParseMode) -> PdfResult<Self> {
        let data_length = data.len();
        let cursor = Cursor::new(data);
        let reader = StreamReader::try_new(cursor)?;
        let syntax = SyntaxParser::with_mode(reader, mode);
        Ok(Self {
            dict,
            syntax,
            data_length: data_length as u64,
        })
    }
    pub fn parse_objects(&mut self) -> PdfResult<Vec<(u32, PdfObject)>> {
        let first = self
            .dict
            .get("First")
            .and_then(PdfObject::as_number)
            .ok_or(PdfError::ParserError(
                "object stream First must be an integer".to_string(),
            ))?
            .as_u64_checked()?;
        let n = self
            .dict
            .get("N")
            .and_then(PdfObject::as_number)
            .ok_or(PdfError::ParserError(
                "object stream N must be an integer".to_string(),
            ))?
            .as_u32_checked()?;
        if first > self.data_length {
            return Err(PdfError::ParserError(
                "object stream First points past stream data".to_string(),
            ));
        }
        let mut objs = Vec::new();
        let mut last_offset = 0;
        for i in 0..n {
            let objnum = self
                .syntax
                .get_object()?
                .as_number()
                .ok_or(PdfError::ParserError(
                    "object stream object number must be an integer".to_string(),
                ))?
                .as_u32_checked()?;
            let relative_offset = self
                .syntax
                .get_object()?
                .as_number()
                .ok_or(PdfError::ParserError(
                    "object stream offset must be an integer".to_string(),
                ))?
                .as_u64_checked()?;
            let offset = relative_offset + first;
            if offset > self.data_length {
                return Err(PdfError::ParserError(format!(
                    "object stream object offset {offset} points past stream data"
                )));
            }
            objs.push((objnum, offset));
            if i > 0 {
                let is_valid = if self.syntax.mode() == ParseMode::Strict {
                    offset > last_offset
                } else {
                    offset >= last_offset
                };
                if !is_valid {
                    return Err(PdfError::ParserError(
                        "object stream offsets must be in increasing order".to_string(),
                    ));
                }
            }
            last_offset = offset;
        }
        let mut res = Vec::new();
        for (objnum, offset) in objs.iter() {
            self.syntax.set_pos(offset.to_owned())?;
            let obj = self.syntax.get_object()?;
            if self.syntax.mode() == ParseMode::Strict
                && object_stream_entry_is_forbidden_in_strict(&obj)
            {
                return Err(PdfError::ParserError(format!(
                    "object stream entry {objnum} must not be a stream object in strict mode"
                )));
            }
            res.push((objnum.to_owned(), obj));
        }
        return Ok(res);
    }
}

#[cfg(test)]
mod tests {
    use super::ObjectStreams;
    use crate::{
        objects::{PdfObject, pdf_dict::PdfDict, pdf_number::PdfNumber},
        parser::syntax::ParseMode,
    };

    #[test]
    fn test_strict_object_stream_rejects_stream_object() {
        let mut dict = PdfDict::default();
        dict.insert(
            "First".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(4.0)),
        );
        dict.insert("N".to_string(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        let data = b"1 0 << /Length 0 >>\nstream\n\nendstream".to_vec();
        let mut object_streams =
            ObjectStreams::try_new_with_mode(&dict, data, ParseMode::Strict).unwrap();
        assert!(object_streams.parse_objects().is_err());
    }

    #[test]
    fn test_strict_object_stream_rejects_reference_object() {
        let mut dict = PdfDict::default();
        dict.insert(
            "First".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(4.0)),
        );
        dict.insert("N".to_string(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        let data = b"1 0 3 0 R".to_vec();
        let mut object_streams =
            ObjectStreams::try_new_with_mode(&dict, data, ParseMode::Strict).unwrap();
        assert!(object_streams.parse_objects().is_err());
    }

    #[test]
    fn test_strict_object_stream_requires_strictly_increasing_offsets() {
        let mut dict = PdfDict::default();
        dict.insert(
            "First".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(8.0)),
        );
        dict.insert("N".to_string(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
        let data = b"1 0 2 0 truefalse".to_vec();
        let mut object_streams =
            ObjectStreams::try_new_with_mode(&dict, data, ParseMode::Strict).unwrap();
        assert!(object_streams.parse_objects().is_err());
    }
}
