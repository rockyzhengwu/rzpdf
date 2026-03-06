use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::document::cross_ref_table::{CrossRefTable, ObjectInfo, ObjectState};

use crate::filter::apply_filter;
use crate::objects::PdfObject;
use crate::{error::PdfResult, parser::syntax::SyntaxParser};

pub fn parse_xref<R: Seek + Read>(syntax: &mut SyntaxParser<R>) -> PdfResult<CrossRefTable> {
    let last_xref_offset = find_start_xref(syntax)?;
    syntax.set_pos(last_xref_offset)?;
    let word = syntax.peek_word()?;
    if word.is_equal(b"xref") {
        load_xref_table(syntax, last_xref_offset)
    } else {
        load_xref_stream(syntax, last_xref_offset)
    }
}
fn load_xref_stream<R: Seek + Read>(
    syntax: &mut SyntaxParser<R>,
    xref_offset: u64,
) -> PdfResult<CrossRefTable> {
    let mut cross_ref_table = parse_xref_stream(syntax, xref_offset)?;
    let mut prev_offset = match cross_ref_table.trailer().get("Prev") {
        Some(obj) => obj.as_number().unwrap().get_u64(),
        None => 0,
    };
    loop {
        if prev_offset == 0 {
            break;
        }
        let prev_xref = parse_xref_stream(syntax, prev_offset)?;
        if let Some(prev) = prev_xref.trailer().get("Prev") {
            prev_offset = prev.as_number().unwrap().get_u64();
        } else {
            prev_offset = 0;
        }
        cross_ref_table.merge(prev_xref);
    }
    Ok(cross_ref_table)
}

pub fn parse_xref_stream<R: Seek + Read>(
    syntax: &mut SyntaxParser<R>,
    pos: u64,
) -> PdfResult<CrossRefTable> {
    syntax.set_pos(pos)?;
    // TODO handle unwrap
    let indirect = syntax.get_indirect_object()?;
    let stream = indirect.obj().as_stream().unwrap();
    let dict = stream.dict();
    let wobj = dict.get("W").unwrap().as_array().unwrap();
    let size = dict.get("Size").unwrap().as_number().unwrap().get_u32();
    let mut w = Vec::new();
    for v in wobj {
        w.push(v.as_number().unwrap().get_u32());
    }
    let index = match dict.get("Index") {
        Some(a) => {
            let a = a.as_array().unwrap();
            let mut res = Vec::new();
            for v in a {
                res.push(v.as_number().unwrap().get_u32());
            }
            res
        }
        None => vec![0, size],
    };

    let raw_data = stream.raw_data();
    let filter = dict.get("Filter");
    let decode_params = dict.get("DecodeParms");
    let buffer = match (filter, decode_params) {
        (None, None) => raw_data.to_vec(),
        (Some(&PdfObject::PdfName(ref filter_name)), None) => {
            apply_filter(filter_name.name(), raw_data, None)?
        }
        (Some(&PdfObject::PdfName(ref name)), Some(&PdfObject::PdfDict(ref param))) => {
            apply_filter(name.name(), raw_data, Some(&param))?
        }
        (Some(&PdfObject::PdfArray(ref names)), None) => {
            let mut data = raw_data.to_vec();
            for name in names.into_iter() {
                data = apply_filter(name.as_name().unwrap().name(), data.as_slice(), None)?;
            }
            data
        }
        (Some(&PdfObject::PdfArray(ref names)), Some(&PdfObject::PdfArray(ref params))) => {
            let mut data = raw_data.to_vec();
            for (i, name) in names.into_iter().enumerate() {
                data = apply_filter(
                    name.as_name().unwrap().name(),
                    data.as_slice(),
                    params.get(i).unwrap().as_dict(),
                )?;
            }
            data
        }
        _ => {
            panic!("Xref stream filter error");
        }
    };
    let mut entries = HashMap::new();
    let mut bptr = 0;

    for v in index.chunks(2) {
        let start = v[0];
        let length = v[1];
        for num in start..(start + length) {
            let t = if w[0] > 0 {
                let mut t = 0_u32;
                for _ in 0..w[0] {
                    t = (t << 8) + buffer[bptr] as u32;
                    bptr += 1;
                }
                t
            } else {
                1_u32
            };

            let mut offset = 0;
            for _ in 0..w[1] {
                offset = (offset << 8) + buffer[bptr] as usize;
                bptr += 1;
            }
            let mut gennum = 0;
            for _ in 0..w[2] {
                gennum = (gennum << 8) + buffer[bptr] as u32;
                bptr += 1;
            }
            match t {
                0 => {
                    entries.insert(
                        num as u32,
                        ObjectInfo::new(num, offset as u64, gennum, ObjectState::Free),
                    );
                }
                1 => {
                    entries.insert(
                        num as u32,
                        ObjectInfo::new(num, offset as u64, gennum, ObjectState::Normal),
                    );
                }
                2 => {
                    entries.insert(
                        num as u32,
                        ObjectInfo::new(num, offset as u64, gennum, ObjectState::Compressed),
                    );
                }
                _ => {
                    return Err(crate::error::PdfError::ParserError(format!(
                        "parse xref stream xref entry type must 1,2 or 3 got :{}",
                        t
                    )));
                }
            }
        }
    }
    let trailer = dict.to_owned();
    let cross_ref_table = CrossRefTable::new(entries, trailer);
    Ok(cross_ref_table)
}

fn load_xref_table<R: Read + Seek>(
    syntax: &mut SyntaxParser<R>,
    xref_offset: u64,
) -> PdfResult<CrossRefTable> {
    if let Some(mut cross_ref_table) = parse_xref_section(syntax, xref_offset)? {
        let mut prev_offset = 0;
        if let Some(prev) = cross_ref_table.trailer().get("Prev") {
            // TODO handle unwrap
            prev_offset = prev.as_number().unwrap().get_u64();
        }
        loop {
            if prev_offset != 0 {
                if let Some(prev_cross_ref_table) = parse_xref_section(syntax, prev_offset)? {
                    if let Some(prev) = prev_cross_ref_table.trailer().get("Prev") {
                        prev_offset = prev.as_number().unwrap().get_u64();
                    }
                    cross_ref_table.merge(prev_cross_ref_table);
                }
            } else {
                break;
            }
        }
        return Ok(cross_ref_table);
    } else {
        // TODO rebuild cross ref table
        panic!("fild parse xref is None");
    }
}
fn parse_xref_section<R: Seek + Read>(
    syntax: &mut SyntaxParser<R>,
    pos: u64,
) -> PdfResult<Option<CrossRefTable>> {
    syntax.set_pos(pos)?;
    let next_word = syntax.get_next_word()?;
    if !next_word.is_equal(b"xref") {
        return Ok(None);
    }
    let mut objects = HashMap::new();
    let mut next_word = syntax.get_next_word()?;
    loop {
        if !next_word.is_number() {
            break;
        }
        let start_objnum = next_word.as_u32();
        next_word = syntax.get_next_word()?;
        if !next_word.is_number() {
            break;
        }
        let obj_count = next_word.as_u32();
        let objs = parse_xref_sub_section(syntax, start_objnum, obj_count)?;
        objects.extend(objs);
    }
    if !next_word.is_equal(b"trailer") {
        return Ok(None);
    }
    if let Some(trailer) = syntax.get_object()?.into_dict() {
        let xref = CrossRefTable::new(objects, trailer);
        return Ok(Some(xref));
    }
    Ok(None)
}

fn parse_xref_sub_section<R: Seek + Read>(
    syntax: &mut SyntaxParser<R>,
    start_objnum: u32,
    count: u32,
) -> PdfResult<HashMap<u32, ObjectInfo>> {
    let mut objects = HashMap::new();
    for i in 0..count {
        let offset = syntax.get_next_word()?.as_u64();
        // point to 0 offset is invalid
        if offset == 0 {
            syntax.get_next_word()?;
            syntax.get_next_word()?;
            continue;
        }
        let gennum = syntax.get_next_word()?.as_u32();
        let t = syntax.get_next_word()?;
        let mut state = ObjectState::Normal;
        if t.is_equal(b"f") {
            state = ObjectState::Free;
        }
        let objnum = start_objnum + i;
        let objinfo = ObjectInfo::new(objnum, offset, gennum, state);
        objects.insert(objnum, objinfo);
    }
    Ok(objects)
}

fn find_start_xref<R: Seek + Read>(syntax: &mut SyntaxParser<R>) -> PdfResult<u64> {
    let limit = syntax.file_size().min(4096);
    let pos = syntax.file_size() - limit;
    syntax.set_pos(pos)?;
    let tag = "startxref".as_bytes();
    let mut cur = 0;
    let mut i = 0;
    while i < limit {
        if cur == tag.len() {
            break;
        }
        let ch = syntax.get_next_char()?;
        if ch == tag[cur] {
            cur += 1;
        } else if ch == tag[0] {
            cur = 1;
        } else {
            cur = 0
        }
        i += 1;
    }
    //let startxref = pos + i - tag.len() as u64;
    //self.syntax.set_pos(startxref)?;
    let offsetobj = syntax.get_next_word()?;
    assert!(offsetobj.is_number());
    let offset = offsetobj.as_u64();
    Ok(offset)
}

#[derive(Debug)]
pub struct PdfParser<R: Seek + Read> {
    syntax: SyntaxParser<R>,
}

impl<R: Seek + Read> PdfParser<R> {
    pub fn new(syntax: SyntaxParser<R>) -> Self {
        PdfParser { syntax }
    }

    pub fn parse_indirect_obj(&mut self, pos: u64) -> PdfResult<PdfObject> {
        self.syntax.set_pos(pos)?;
        let indirect = self.syntax.get_indirect_object()?;
        return Ok(indirect.to_obj());
    }
}

#[cfg(test)]
mod tests {
    use super::{find_start_xref, parse_xref};
    use crate::io::stream_reader::StreamReader;
    use crate::parser::parser::PdfParser;
    use crate::parser::syntax::SyntaxParser;

    use std::io::{BufReader, Cursor};
    fn new_parser(buffer: &str) -> PdfParser<Cursor<&str>> {
        let inner = Cursor::new(buffer);
        let reader = StreamReader::try_new(inner).unwrap();
        let syntax = SyntaxParser::new(reader);
        let parser = PdfParser::new(syntax);
        return parser;
    }

    #[test]
    fn test_find_start_xref() {
        let buffer = r#" end obj \r\n startxref 1000 \r\n %EOF "#;
        let inner = Cursor::new(buffer);
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::new(reader);
        let startxref = find_start_xref(&mut syntax).unwrap() as usize;
        assert_eq!(startxref, 1000);
    }

    #[test]
    fn test_parse_xref() {
        let f = std::fs::File::open("tests/pdfs/xref_stream.pdf").unwrap();
        let buffreader = BufReader::new(f);
        let reader = StreamReader::try_new(buffreader).unwrap();
        let mut syntax = SyntaxParser::new(reader);
        let cross_ref_table = parse_xref(&mut syntax).unwrap();
        println!("{:?}", cross_ref_table);
    }
}
