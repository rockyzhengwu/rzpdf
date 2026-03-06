use std::io::Cursor;

use crate::{
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference},
    parser::syntax::SyntaxParser,
};

pub struct ObjectStreams<'a> {
    dict: &'a PdfDict,
    syntax: SyntaxParser<Cursor<Vec<u8>>>,
    data_length: u64,
}

impl<'a> ObjectStreams<'a> {
    pub fn try_new(dict: &'a PdfDict, data: Vec<u8>) -> PdfResult<Self> {
        let data_length = data.len();
        let cursor = Cursor::new(data);
        let reader = StreamReader::try_new(cursor)?;
        let syntax = SyntaxParser::new(reader);
        Ok(Self {
            dict,
            syntax,
            data_length: data_length as u64,
        })
    }
    pub fn parse_objects(&mut self) -> PdfResult<Vec<(u32, PdfObject)>> {
        let first = self.dict.get("First").unwrap().as_u64().unwrap();
        let n = self.dict.get("N").unwrap().as_u32().unwrap();
        let mut objs = Vec::new();
        let mut lengths = Vec::new();
        let mut last_offset = 0;
        for i in 0..n {
            let objnum = self.syntax.get_object()?.as_u32().unwrap();
            let offset = self.syntax.get_object()?.as_u64().unwrap() + first;
            objs.push((objnum, offset));
            if i > 0 {
                let length = offset - last_offset;
                lengths.push(length);
            }
            last_offset = offset;
        }
        lengths.push((self.data_length - last_offset) as u64);
        let mut res = Vec::new();
        for (objnum, offset) in objs.iter() {
            self.syntax.set_pos(offset.to_owned())?;
            let obj = self.syntax.get_object()?;
            res.push((objnum.to_owned(), obj));
        }
        return Ok(res);
    }
}
