use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::document::cross_ref_table::{CrossRefTable, ObjectInfo, ObjectState};

use crate::filter::{FilterContext, apply_filters};
use crate::objects::PdfObject;
use crate::{
    error::{PdfError, PdfResult},
    parser::syntax::SyntaxParser,
};

pub fn parse_xref<R: Seek + Read>(syntax: &mut SyntaxParser<R>) -> PdfResult<CrossRefTable> {
    let last_xref_offset = find_start_xref(syntax)?;
    syntax.set_pos(last_xref_offset)?;
    let word = syntax.peek_word()?;
    let xref = if word.is_equal(b"xref") {
        load_xref_table(syntax, last_xref_offset)
    } else {
        load_xref_stream(syntax, last_xref_offset)
    }?;
    if syntax.mode() == crate::parser::syntax::ParseMode::Strict {
        validate_strict_xref_table(&xref)?;
    }
    Ok(xref)
}
fn load_xref_stream<R: Seek + Read>(
    syntax: &mut SyntaxParser<R>,
    xref_offset: u64,
) -> PdfResult<CrossRefTable> {
    let mut cross_ref_table = parse_xref_stream(syntax, xref_offset)?;
    let mut prev_offset = match cross_ref_table.trailer().get("Prev") {
        Some(obj) => obj
            .as_number()
            .ok_or(PdfError::ParserError("Prev must be a number".to_string()))?
            .as_u64_checked()?,
        None => 0,
    };
    loop {
        if prev_offset == 0 {
            break;
        }
        let prev_xref = parse_xref_stream(syntax, prev_offset)?;
        if let Some(prev) = prev_xref.trailer().get("Prev") {
            prev_offset = prev
                .as_number()
                .ok_or(PdfError::ParserError("Prev must be a number".to_string()))?
                .as_u64_checked()?;
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
    let indirect = syntax.get_indirect_object()?;
    let stream = indirect
        .obj()
        .as_stream()
        .ok_or(PdfError::ParserError("xref stream must be a stream".to_string()))?;
    let dict = stream.dict();
    if syntax.mode() == crate::parser::syntax::ParseMode::Strict {
        let ty = dict
            .get("Type")
            .and_then(PdfObject::as_name)
            .ok_or(PdfError::ParserError(
                "strict mode requires xref stream Type to be /XRef".to_string(),
            ))?;
        if ty.name() != "XRef" {
            return Err(PdfError::ParserError(
                "strict mode requires xref stream Type to be /XRef".to_string(),
            ));
        }
    }
    let wobj = dict
        .get("W")
        .and_then(PdfObject::as_array)
        .ok_or(PdfError::ParserError("xref stream W must be an array".to_string()))?;
    let size = dict
        .get("Size")
        .and_then(PdfObject::as_number)
        .ok_or(PdfError::ParserError("xref stream Size must be a number".to_string()))?
        .as_u32_checked()?;
    let mut w = Vec::new();
    for v in wobj {
        w.push(
            v.as_number()
                .ok_or(PdfError::ParserError(
                    "xref stream W items must be numbers".to_string(),
                ))?
                .as_u32_checked()?,
        );
    }
    if w.len() != 3 {
        return Err(PdfError::ParserError(
            "xref stream W must contain exactly three integers".to_string(),
        ));
    }
    let index = match dict.get("Index") {
        Some(a) => {
            let a = a.as_array().ok_or(PdfError::ParserError(
                "xref stream Index must be an array".to_string(),
            ))?;
            let mut res = Vec::new();
            for v in a {
                res.push(
                    v.as_number()
                        .ok_or(PdfError::ParserError(
                            "xref stream Index items must be numbers".to_string(),
                        ))?
                        .as_u32_checked()?,
                );
            }
            res
        }
        None => vec![0, size],
    };
    if index.len() % 2 != 0 {
        return Err(PdfError::ParserError(
            "xref stream Index must contain pairs of integers".to_string(),
        ));
    }

    let raw_data = stream.raw_data();
    let buffer = match dict.get("Filter") {
        Some(filter) => apply_filters(
            filter,
            dict.get("DecodeParms"),
            raw_data,
            FilterContext {
                stream_dict: Some(dict),
                jbig2_globals: None,
            },
        )
        .map_err(|e| PdfError::ParserError(format!("xref stream filter decode failed: {e}")))?,
        None => raw_data.to_vec(),
    };
    let mut entries = HashMap::new();
    let mut bptr = 0;
    let entry_size = (w[0] + w[1] + w[2]) as usize;
    if entry_size == 0 {
        return Err(PdfError::ParserError(
            "xref stream W entry width must not be all zeros".to_string(),
        ));
    }

    for v in index.chunks(2) {
        let start = v[0];
        let length = v[1];
        for num in start..(start + length) {
            if bptr + entry_size > buffer.len() {
                return Err(PdfError::ParserError(
                    "xref stream data ended before all entries were read".to_string(),
                ));
            }
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
                    return Err(PdfError::ParserError(format!(
                        "xref stream entry type must be 0, 1, or 2, got {t}"
                    )));
                }
            }
        }
    }
    if syntax.mode() == crate::parser::syntax::ParseMode::Strict && bptr != buffer.len() {
        return Err(PdfError::ParserError(
            "strict mode requires xref stream data length to exactly match the decoded entries"
                .to_string(),
        ));
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
            prev_offset = prev
                .as_number()
                .ok_or(PdfError::ParserError("Prev must be a number".to_string()))?
                .as_u64_checked()?;
        }
        loop {
            if prev_offset != 0 {
                if let Some(prev_cross_ref_table) = parse_xref_section(syntax, prev_offset)? {
                    if let Some(prev) = prev_cross_ref_table.trailer().get("Prev") {
                        prev_offset = prev
                            .as_number()
                            .ok_or(PdfError::ParserError("Prev must be a number".to_string()))?
                            .as_u64_checked()?;
                    }
                    cross_ref_table.merge(prev_cross_ref_table);
                }
            } else {
                break;
            }
        }
        return Ok(cross_ref_table);
    } else {
        Err(crate::error::PdfError::ParserError(
            "failed to parse xref table at startxref offset".to_string(),
        ))
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
        let start_objnum = next_word.as_u32_checked()?;
        next_word = syntax.get_next_word()?;
        if !next_word.is_number() {
            break;
        }
        let obj_count = next_word.as_u32_checked()?;
        if syntax.mode() == crate::parser::syntax::ParseMode::Strict && obj_count == 0 {
            return Err(PdfError::ParserError(
                "strict mode requires xref subsection counts to be greater than zero".to_string(),
            ));
        }
        let objs = parse_xref_sub_section(syntax, start_objnum, obj_count)?;
        if syntax.mode() == crate::parser::syntax::ParseMode::Strict {
            for objnum in objs.keys() {
                if objects.contains_key(objnum) {
                    return Err(PdfError::ParserError(format!(
                        "strict mode forbids duplicate xref entries for object {objnum}"
                    )));
                }
            }
        }
        objects.extend(objs);
    }
    if !next_word.is_equal(b"trailer") {
        return Ok(None);
    }
    if let Some(trailer) = syntax.get_object()?.into_dict() {
        if syntax.mode() == crate::parser::syntax::ParseMode::Strict && trailer.get("Size").is_none()
        {
            return Err(PdfError::ParserError(
                "strict mode requires trailer dictionary to contain Size".to_string(),
            ));
        }
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
        let (offset, gennum, state) = if syntax.mode() == crate::parser::syntax::ParseMode::Strict {
            parse_xref_entry_strict(syntax)?
        } else {
            let offset = syntax.get_next_word()?.as_u64_checked()?;
            let gennum = syntax.get_next_word()?.as_u32_checked()?;
            let t = syntax.get_next_word()?;
            let state = if t.is_equal(b"n") {
                ObjectState::Normal
            } else if t.is_equal(b"f") {
                ObjectState::Free
            } else {
                return Err(PdfError::ParserError(format!(
                    "xref table entry type must be 'n' or 'f', got {:?}",
                    t.raw()
                )));
            };
            (offset, gennum, state)
        };
        let objnum = start_objnum + i;
        let objinfo = ObjectInfo::new(objnum, offset, gennum, state);
        objects.insert(objnum, objinfo);
    }
    Ok(objects)
}

fn parse_xref_entry_strict<R: Seek + Read>(
    syntax: &mut SyntaxParser<R>,
) -> PdfResult<(u64, u32, ObjectState)> {
    let line = syntax.read_line()?;
    if line.len() != 18 {
        return Err(PdfError::ParserError(format!(
            "strict mode requires xref entries to be exactly 18 bytes before EOL, got {}",
            line.len()
        )));
    }
    if line[10] != b' ' || line[16] != b' ' {
        return Err(PdfError::ParserError(
            "strict mode requires xref entry fields to be space-separated".to_string(),
        ));
    }
    let offset = std::str::from_utf8(&line[0..10])
        .map_err(|e| PdfError::ParserError(format!("xref offset is not utf8: {e:?}")))?
        .parse::<u64>()
        .map_err(|_| PdfError::ParserError("xref offset must be 10 decimal digits".to_string()))?;
    let gennum = std::str::from_utf8(&line[11..16])
        .map_err(|e| PdfError::ParserError(format!("xref generation is not utf8: {e:?}")))?
        .parse::<u32>()
        .map_err(|_| {
            PdfError::ParserError("xref generation number must be 5 decimal digits".to_string())
        })?;
    let state = match line[17] {
        b'n' => ObjectState::Normal,
        b'f' => ObjectState::Free,
        _ => {
            return Err(PdfError::ParserError(
                "strict mode requires xref entry state to be 'n' or 'f'".to_string(),
            ));
        }
    };
    Ok((offset, gennum, state))
}

fn find_start_xref<R: Seek + Read>(syntax: &mut SyntaxParser<R>) -> PdfResult<u64> {
    let limit = syntax.file_size().min(1024 * 1024);
    let pos = syntax.file_size() - limit;
    syntax.set_pos(pos)?;
    let tag = b"startxref";
    let mut window = Vec::with_capacity(limit as usize);
    for _ in 0..limit {
        window.push(syntax.get_next_char()?);
    }
    if syntax.mode() == crate::parser::syntax::ParseMode::Strict {
        validate_final_eof_marker(&window)?;
    }
    let Some(rel) = window.windows(tag.len()).rposition(|slice| slice == tag) else {
        return Err(PdfError::ParserError(
            "startxref keyword was not found near end of file".to_string(),
        ));
    };
    if syntax.mode() == crate::parser::syntax::ParseMode::Strict {
        let eof_rel = find_last_eof_marker(&window).ok_or(PdfError::ParserError(
            "strict mode requires a final %%EOF marker near the end of the file".to_string(),
        ))?;
        if rel >= eof_rel {
            return Err(PdfError::ParserError(
                "strict mode requires startxref to appear before the final %%EOF marker"
                    .to_string(),
            ));
        }
    }
    syntax.set_pos(pos + rel as u64 + tag.len() as u64)?;
    let offsetobj = syntax.get_next_word()?;
    if !offsetobj.is_number() {
        return Err(PdfError::ParserError(
            "startxref must be followed by a numeric byte offset".to_string(),
        ));
    }
    let offset = offsetobj.as_u64_checked()?;
    Ok(offset)
}

fn find_last_eof_marker(window: &[u8]) -> Option<usize> {
    window.windows(b"%%EOF".len()).rposition(|slice| slice == b"%%EOF")
}

fn validate_final_eof_marker(window: &[u8]) -> PdfResult<()> {
    let eof_pos = find_last_eof_marker(window).ok_or(PdfError::ParserError(
        "strict mode requires a final %%EOF marker near the end of the file".to_string(),
    ))?;
    let trailing = &window[eof_pos + b"%%EOF".len()..];
    if trailing
        .iter()
        .any(|byte| !matches!(byte, b'\x00' | b'\t' | b'\n' | b'\x0C' | b'\r' | b' '))
    {
        return Err(PdfError::ParserError(
            "strict mode requires %%EOF to be the final non-whitespace marker in the file"
                .to_string(),
        ));
    }
    Ok(())
}

fn validate_strict_xref_table(xref: &CrossRefTable) -> PdfResult<()> {
    let zero = xref.lookup(&0).ok_or(PdfError::ParserError(
        "strict mode requires cross-reference data to contain object 0".to_string(),
    ))?;
    if !matches!(zero.state(), ObjectState::Free) {
        return Err(PdfError::ParserError(
            "strict mode requires object 0 to be a free cross-reference entry".to_string(),
        ));
    }
    if zero.offset() != 0 {
        return Err(PdfError::ParserError(
            "strict mode requires object 0 free entry to have offset 0".to_string(),
        ));
    }
    if zero.gennum() != 65535 {
        return Err(PdfError::ParserError(
            "strict mode requires object 0 free entry to have generation 65535".to_string(),
        ));
    }
    Ok(())
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
    use super::{find_start_xref, parse_xref, parse_xref_stream};
    use crate::document::cross_ref_table::ObjectState;
    use crate::io::stream_reader::StreamReader;
    use crate::parser::parser::PdfParser;
    use crate::parser::syntax::{ParseMode, SyntaxParser};

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
    fn test_find_last_startxref() {
        let buffer = b"%PDF-1.7\n1 0 obj\n(startxref)\nendobj\nstartxref\n42\n%%EOF\nstartxref\n99\n%%EOF";
        let inner = Cursor::new(buffer.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::new(reader);
        let startxref = find_start_xref(&mut syntax).unwrap();
        assert_eq!(startxref, 99);
    }

    #[test]
    fn test_parse_xref() {
        let f = std::fs::File::open("../tests/path-test.pdf").unwrap();
        let buffreader = BufReader::new(f);
        let reader = StreamReader::try_new(buffreader).unwrap();
        let mut syntax = SyntaxParser::new(reader);
        assert!(parse_xref(&mut syntax).is_ok());
    }

    #[test]
    fn test_parse_xref_keeps_free_entries() {
        let body = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\n";
        let xref_offset = body.len();
        let mut pdf = body.to_vec();
        pdf.extend_from_slice(b"xref\n0 2\n0000000000 65535 f\n0000000009 00000 n\ntrailer\n<< /Size 2 >>\nstartxref\n");
        pdf.extend_from_slice(xref_offset.to_string().as_bytes());
        pdf.extend_from_slice(b"\n%%EOF\n");

        let inner = Cursor::new(pdf);
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::new(reader);
        let xref = parse_xref(&mut syntax).unwrap();

        let free = xref.lookup(&0).unwrap();
        assert!(matches!(free.state(), ObjectState::Free));
        let in_use = xref.lookup(&1).unwrap();
        assert!(matches!(in_use.state(), ObjectState::Normal));
    }

    #[test]
    fn test_strict_xref_trailer_requires_size() {
        let pdf = b"%PDF-1.7\nxref\n0 1\n0000000000 65535 f\ntrailer\n<< >>\nstartxref\n9\n%%EOF\n";
        let inner = Cursor::new(pdf.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(parse_xref(&mut syntax).is_err());
    }

    #[test]
    fn test_strict_xref_stream_requires_type_xref() {
        let pdf = b"1 0 obj\n<< /Size 1 /W [1 1 1] /Length 3 >>\nstream\n\x01\x00\x00\nendstream\nendobj\n";
        let inner = Cursor::new(pdf.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(parse_xref_stream(&mut syntax, 0).is_err());
    }

    #[test]
    fn test_strict_xref_stream_rejects_trailing_bytes() {
        let pdf = b"1 0 obj\n<< /Type /XRef /Size 1 /W [1 1 1] /Length 4 >>\nstream\n\x01\x00\x00\xff\nendstream\nendobj\n";
        let inner = Cursor::new(pdf.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(parse_xref_stream(&mut syntax, 0).is_err());
    }

    #[test]
    fn test_strict_find_startxref_requires_final_eof() {
        let buffer = b"%PDF-1.7\nstartxref\n12\n%%EOF\njunk";
        let inner = Cursor::new(buffer.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(find_start_xref(&mut syntax).is_err());
    }

    #[test]
    fn test_strict_parse_xref_rejects_zero_count_subsection() {
        let pdf = b"%PDF-1.7\nxref\n0 0\ntrailer\n<< /Size 1 >>\nstartxref\n9\n%%EOF\n";
        let inner = Cursor::new(pdf.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(parse_xref(&mut syntax).is_err());
    }

    #[test]
    fn test_strict_parse_xref_rejects_bad_entry_width() {
        let pdf = b"%PDF-1.7\nxref\n0 1\n000000000 65535 f\ntrailer\n<< /Size 1 >>\nstartxref\n9\n%%EOF\n";
        let inner = Cursor::new(pdf.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(parse_xref(&mut syntax).is_err());
    }

    #[test]
    fn test_strict_parse_xref_requires_object_zero_free_65535() {
        let pdf = b"%PDF-1.7\nxref\n0 1\n0000000000 00000 n\ntrailer\n<< /Size 1 >>\nstartxref\n9\n%%EOF\n";
        let inner = Cursor::new(pdf.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(parse_xref(&mut syntax).is_err());
    }

    #[test]
    fn test_strict_parse_xref_stream_requires_object_zero_free_65535() {
        let xref_stream = b"1 0 obj\n<< /Type /XRef /Size 1 /W [1 1 2] /Length 4 >>\nstream\n\x01\x00\x00\x00\nendstream\nendobj\n";
        let mut pdf = xref_stream.to_vec();
        pdf.extend_from_slice(b"startxref\n0\n%%EOF\n");
        let inner = Cursor::new(pdf.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut syntax = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(parse_xref(&mut syntax).is_err());
    }
}
