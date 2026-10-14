use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::document::cross_ref_table::{CrossRefTable, ObjectInfo, ObjectState};

use crate::filter::{apply_filters, FilterContext};
use crate::objects::PdfObject;
use crate::{
    error::{PdfError, PdfResult},
    parser::syntax::SyntaxParser,
};

pub fn parse_xref<R: Seek + Read>(syntax: &mut SyntaxParser<R>) -> PdfResult<CrossRefTable> {
    parse_xref_with_offset(syntax).map(|(xref, _)| xref)
}

/// Preserve the actual latest revision address before merging older revisions.
pub(crate) fn parse_xref_with_offset<R: Seek + Read>(
    syntax: &mut SyntaxParser<R>,
) -> PdfResult<(CrossRefTable, u64)> {
    let last_xref_offset = find_start_xref(syntax)?;
    let xref = load_xref_chain(syntax, last_xref_offset)?;
    if syntax.mode() == crate::parser::syntax::ParseMode::Strict {
        validate_strict_xref_table(&xref)?;
    }
    Ok((xref, last_xref_offset))
}
fn load_xref_chain<R: Seek + Read>(
    syntax: &mut SyntaxParser<R>,
    offset: u64,
) -> PdfResult<CrossRefTable> {
    let mut visited = std::collections::HashSet::new();
    let mut next = Some(offset);
    let mut result: Option<CrossRefTable> = None;
    while let Some(offset) = next {
        if visited.len() >= syntax.limits().max_depth {
            return Err(PdfError::ResourceLimit("xref history depth".into()));
        }
        if !visited.insert(offset) {
            return Err(PdfError::InvalidReference(format!(
                "cycle in xref history at {offset}"
            )));
        }
        syntax.set_pos(offset)?;
        let is_table = syntax.peek_word()?.is_equal(b"xref");
        let mut revision = if is_table {
            parse_xref_section(syntax, offset)?
                .ok_or_else(|| PdfError::ParserError("invalid xref table".into()))?
        } else {
            parse_xref_stream(syntax, offset)?
        };
        next = revision
            .trailer()
            .get("Prev")
            .map(|value| {
                value
                    .as_number()
                    .ok_or_else(|| PdfError::ParserError("Prev must be an integer".into()))?
                    .as_u64_checked()
            })
            .transpose()?
            .filter(|offset| *offset != 0);
        if is_table {
            if let Some(stream_offset) = revision.trailer().get("XRefStm") {
                let stream_offset = stream_offset
                    .as_number()
                    .ok_or_else(|| PdfError::ParserError("XRefStm must be an integer".into()))?
                    .as_u64_checked()?;
                if !visited.insert(stream_offset) {
                    return Err(PdfError::InvalidReference("cycle in hybrid xref".into()));
                }
                // Supplemental stream overrides table entries in the same revision.
                revision.merge(parse_xref_stream(syntax, stream_offset)?);
            }
        }
        if let Some(current) = &mut result {
            current.merge_older(revision);
        } else {
            result = Some(revision);
        }
        if result.as_ref().unwrap().objects().len() > syntax.limits().max_objects {
            return Err(PdfError::ResourceLimit("xref object count".into()));
        }
    }
    result.ok_or_else(|| PdfError::ParserError("missing xref history".into()))
}

pub fn parse_xref_stream<R: Seek + Read>(
    syntax: &mut SyntaxParser<R>,
    pos: u64,
) -> PdfResult<CrossRefTable> {
    syntax.set_pos(pos)?;
    let indirect = syntax.get_indirect_object()?;
    let stream = indirect.obj().as_stream().ok_or(PdfError::ParserError(
        "xref stream must be a stream".to_string(),
    ))?;
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
        .ok_or(PdfError::ParserError(
            "xref stream W must be an array".to_string(),
        ))?;
    let size = dict
        .get("Size")
        .and_then(PdfObject::as_number)
        .ok_or(PdfError::ParserError(
            "xref stream Size must be a number".to_string(),
        ))?
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
    if w[0] > 4 || w[1] > 8 || w[2] > 4 {
        return Err(PdfError::ParserError(
            "xref field widths exceed supported integer sizes".into(),
        ));
    }
    if size as usize > syntax.limits().max_objects {
        return Err(PdfError::ResourceLimit("xref Size".into()));
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
    let mut entry_count = 0usize;
    for pair in index.chunks_exact(2) {
        let end = pair[0]
            .checked_add(pair[1])
            .ok_or_else(|| PdfError::ParserError("xref Index range overflow".into()))?;
        if end > size {
            return Err(PdfError::ParserError("xref Index exceeds Size".into()));
        }
        entry_count = entry_count
            .checked_add(pair[1] as usize)
            .ok_or_else(|| PdfError::ResourceLimit("xref entry count overflow".into()))?;
        if entry_count > syntax.limits().max_objects {
            return Err(PdfError::ResourceLimit("xref entry count".into()));
        }
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
                max_decoded_bytes: syntax.limits().max_decoded_bytes,
                max_image_pixels: syntax.limits().max_image_pixels,
                crypt_already_processed: false,
            },
        )
        .map_err(|e| PdfError::ParserError(format!("xref stream filter decode failed: {e}")))?,
        None => {
            if raw_data.len() > syntax.limits().max_decoded_bytes {
                return Err(PdfError::ResourceLimit("xref stream bytes".into()));
            }
            raw_data.to_vec()
        }
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

            let mut offset = 0u64;
            for _ in 0..w[1] {
                offset = (offset << 8) + buffer[bptr] as u64;
                bptr += 1;
            }
            let mut gennum = 0;
            for _ in 0..w[2] {
                gennum = (gennum << 8) + buffer[bptr] as u32;
                bptr += 1;
            }
            match t {
                0 => {
                    // Compact xref streams commonly encode the reserved free
                    // object zero as (0, 0, 0), including qpdf. It is never
                    // allocatable; retain its semantic sentinel generation.
                    // Do not normalize live entries or other generations.
                    let gennum = if num == 0 && offset == 0 && gennum == 0 {
                        65535
                    } else {
                        gennum
                    };
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
        if obj_count as usize > syntax.limits().max_objects.saturating_sub(objects.len()) {
            return Err(PdfError::ResourceLimit("xref subsection count".into()));
        }
        start_objnum
            .checked_add(obj_count)
            .ok_or_else(|| PdfError::ParserError("xref subsection range overflow".into()))?;
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
        next_word = syntax.get_next_word()?;
    }
    if !next_word.is_equal(b"trailer") {
        return Ok(None);
    }
    if let Some(trailer) = syntax.get_object()?.into_dict() {
        if syntax.mode() == crate::parser::syntax::ParseMode::Strict
            && trailer.get("Size").is_none()
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
    if syntax.mode() == crate::parser::syntax::ParseMode::Strict {
        let remainder = syntax.read_line()?;
        if remainder.iter().any(|byte| !matches!(byte, b' ' | b'\t')) {
            return Err(PdfError::ParserError(
                "xref subsection header must end before entries".into(),
            ));
        }
    }
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
    if !(line.len() == 18 || (line.len() == 19 && line[18] == b' ')) {
        return Err(PdfError::ParserError(format!(
            "strict mode requires xref entry fields and optional trailing space, got {} bytes",
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
    window
        .windows(b"%%EOF".len())
        .rposition(|slice| slice == b"%%EOF")
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
    if zero.gennum() != 65535 {
        return Err(PdfError::ParserError(
            "strict mode requires object 0 free entry to have generation 65535".to_string(),
        ));
    }
    // Object zero points to the first reusable free entry, not necessarily
    // back to itself. Bound traversal without allocating a visited table.
    let mut next = zero.offset();
    let mut traversed = 0;
    while next != 0 {
        if traversed >= xref.objects().len() {
            return Err(PdfError::InvalidReference("cycle in xref free list".into()));
        }
        traversed += 1;
        let number = u32::try_from(next).map_err(|_| PdfError::InvalidReference("xref free-list number overflow".into()))?;
        let info = xref.lookup(&number).ok_or_else(|| PdfError::InvalidReference("missing xref free-list entry".into()))?;
        if !matches!(info.state(), ObjectState::Free) || info.gennum() > 65535 {
            return Err(PdfError::InvalidReference("invalid xref free-list entry".into()));
        }
        next = info.offset();
    }
    Ok(())
}

#[derive(Debug)]
pub struct PdfParser<R: Seek + Read> {
    syntax: SyntaxParser<R>,
}

impl<R: Seek + Read> PdfParser<R> {
    pub fn mode(&self) -> super::syntax::ParseMode {
        self.syntax.mode()
    }
    pub fn limits(&self) -> super::options::Limits {
        self.syntax.limits()
    }
    pub fn new(syntax: SyntaxParser<R>) -> Self {
        PdfParser { syntax }
    }

    pub fn parse_indirect_obj(&mut self, pos: u64) -> PdfResult<PdfObject> {
        self.syntax.set_pos(pos)?;
        let indirect = self.syntax.get_indirect_object()?;
        return Ok(indirect.to_obj());
    }

    pub fn parse_indirect_obj_checked(
        &mut self,
        pos: u64,
        expected: crate::objects::object_id::ObjectId,
    ) -> PdfResult<PdfObject> {
        self.syntax.set_pos(pos)?;
        let indirect = self.syntax.get_indirect_object()?;
        if indirect.id() != expected {
            return Err(PdfError::InvalidReference(format!(
                "xref for {expected:?} at byte {pos} points to {:?}",
                indirect.id()
            )));
        }
        Ok(indirect.to_obj())
    }

    pub(crate) fn parse_indirect_obj_checked_with_budget(
        &mut self,
        pos: u64,
        expected: crate::objects::object_id::ObjectId,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
        self.syntax.set_pos(pos)?;
        let (lease, indirect) = {
            let (indirect, lease) = self.syntax.get_indirect_object_with_budget(budget)?;
            (lease, indirect)
        };
        if indirect.id() != expected {
            return Err(PdfError::InvalidReference(format!(
                "xref for {expected:?} at byte {pos} points to {:?}",
                indirect.id()
            )));
        }
        Ok((indirect.to_obj(), lease))
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

    fn parse_test_xref(
        bytes: Vec<u8>,
    ) -> crate::error::PdfResult<crate::document::cross_ref_table::CrossRefTable> {
        let reader = StreamReader::try_new(Cursor::new(bytes)).unwrap();
        parse_xref(&mut SyntaxParser::new(reader))
    }

    #[test]
    fn prev_cycle_returns_invalid_reference() {
        let pdf = b"%PDF-1.7\nxref\n0 1\n0000000000 65535 f\ntrailer\n<< /Size 1 /Prev 9 >>\nstartxref\n9\n%%EOF\n";
        assert!(matches!(
            parse_test_xref(pdf.to_vec()),
            Err(crate::error::PdfError::InvalidReference(_))
        ));
    }

    #[test]
    fn newest_stream_entries_override_older_table_entries() {
        let mut pdf = b"%PDF-1.7\nxref\n0 2\n0000000000 65535 f\n0000000011 00000 n\ntrailer\n<< /Size 2 >>\n".to_vec();
        let offset = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /XRef /Size 3 /Index [1 1] /W [1 1 1] /Prev 9 /Length 3 >>\nstream\n\x01\x63\x00\nendstream\nendobj\n");
        pdf.extend_from_slice(format!("startxref\n{offset}\n%%EOF\n").as_bytes());
        let xref = parse_test_xref(pdf).unwrap();
        assert_eq!(xref.lookup(&1).unwrap().offset(), 99);
        assert!(matches!(
            xref.lookup(&0).unwrap().state(),
            ObjectState::Free
        ));
    }

    #[test]
    fn hybrid_stream_overrides_same_revision_table() {
        let mut pdf = b"%PDF-1.7\n".to_vec();
        let stream_offset = pdf.len();
        pdf.extend_from_slice(b"2 0 obj\n<< /Type /XRef /Size 3 /Index [1 1] /W [1 1 1] /Length 3 >>\nstream\n\x02\x07\x00\nendstream\nendobj\n");
        let table_offset = pdf.len();
        pdf.extend_from_slice(format!("xref\n0 2\n0000000000 65535 f\n0000000011 00000 n\ntrailer\n<< /Size 3 /XRefStm {stream_offset} >>\nstartxref\n{table_offset}\n%%EOF\n").as_bytes());
        let xref = parse_test_xref(pdf).unwrap();
        assert!(matches!(
            xref.lookup(&1).unwrap().state(),
            ObjectState::Compressed
        ));
        assert_eq!(xref.lookup(&1).unwrap().offset(), 7);
    }

    #[test]
    fn oversized_xref_subsection_and_width_return_errors() {
        let pdf = b"%PDF-1.7\nxref\n0 4294967295\ntrailer\n<< /Size 1 >>\nstartxref\n9\n%%EOF\n";
        assert!(matches!(
            parse_test_xref(pdf.to_vec()),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        let pdf = b"%PDF-1.7\n1 0 obj\n<< /Type /XRef /Size 1 /W [4294967295 1 1] /Length 0 >>\nstream\n\nendstream\nendobj\nstartxref\n9\n%%EOF\n";
        assert!(matches!(
            parse_test_xref(pdf.to_vec()),
            Err(crate::error::PdfError::ParserError(_))
        ));
    }

    #[test]
    fn test_find_last_startxref() {
        let buffer =
            b"%PDF-1.7\n1 0 obj\n(startxref)\nendobj\nstartxref\n42\n%%EOF\nstartxref\n99\n%%EOF";
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
    fn multiple_xref_subsections_advance_in_strict_and_compatible_modes() {
        let bytes = b"%PDF-1.7\nxref\n0 1\n0000000000 65535 f \n3 1\n0000000123 00007 n \ntrailer\n<< /Size 4 >>\nstartxref\n9\n%%EOF\n";
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let reader = StreamReader::try_new(Cursor::new(bytes)).unwrap();
            let table = parse_xref(&mut SyntaxParser::with_mode(reader, mode)).unwrap();
            assert_eq!(table.lookup(&3).unwrap().offset(), 123);
            assert_eq!(table.lookup(&3).unwrap().gennum(), 7);
        }
    }

    #[test]
    fn strict_free_list_accepts_a_chain_and_rejects_cycles_live_and_missing_targets() {
        for (target, entry, accepts) in [
            (1, "0000000000 00008 f ", true),
            (1, "0000000001 00008 f ", false),
            (1, "0000000010 00008 n ", false),
            (3, "0000000000 00008 f ", false),
        ] {
            let bytes = format!("%PDF-1.7\nxref\n0 2\n{target:010} 65535 f \n{entry}\ntrailer\n<< /Size 2 >>\nstartxref\n9\n%%EOF\n");
            let reader = StreamReader::try_new(Cursor::new(bytes.into_bytes())).unwrap();
            assert_eq!(parse_xref(&mut SyntaxParser::with_mode(reader, ParseMode::Strict)).is_ok(), accepts);
        }
    }

    #[test]
    fn compact_xref_stream_reserved_zero_is_normalized_but_other_generations_are_not() {
        for (generation, accepts) in [(0u8, true), (1, false)] {
            let mut bytes = b"1 0 obj\n<< /Type /XRef /Size 1 /W [1 1 1] /Length 3 >>\nstream\n".to_vec();
            bytes.extend_from_slice(&[0, 0, generation]);
            bytes.extend_from_slice(b"\nendstream\nendobj\nstartxref\n0\n%%EOF\n");
            let reader = StreamReader::try_new(Cursor::new(bytes)).unwrap();
            let result = parse_xref(&mut SyntaxParser::with_mode(reader, ParseMode::Strict));
            assert_eq!(result.is_ok(), accepts);
            if let Ok(table) = result { assert_eq!(table.lookup(&0).unwrap().gennum(), 65535); }
        }
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
