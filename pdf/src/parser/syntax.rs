use std::io::{Read, Seek};

use crate::{
    document::cross_ref_table::{CrossRefTable, ObjectState},
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{
        pdf_array::PdfArray, pdf_bool::PdfBool, pdf_dict::PdfDict, pdf_indirect::PdfIndirect,
        pdf_name::PdfName, pdf_number::PdfNumber, pdf_reference::PdfReference,
        pdf_stream::PdfStream, pdf_string::PdfString, PdfObject,
    },
    parser::{
        character::{is_delimiter, is_end_of_line, is_number, is_regular, is_white_space},
        parse_utility::hex_to_u8,
    },
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ParseMode {
    Strict,
    Compatible,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PdfHeader {
    version: String,
}

impl PdfHeader {
    pub fn version(&self) -> &str {
        self.version.as_str()
    }
}

#[derive(Debug, Default)]
pub struct PdfWord {
    raw: Vec<u8>,
    is_number: bool,
    budget: Option<crate::budget::OperationBudget>,
    lease: Option<crate::budget::MemoryLease>,
}

fn reserve_string_byte(
    bytes: &mut Vec<u8>,
    budget: Option<&crate::budget::OperationBudget>,
    owner: &mut Option<crate::budget::MemoryLease>,
) -> PdfResult<()> {
    if let Some(budget) = budget {
        if bytes.len() == bytes.capacity() {
            let capacity = bytes.capacity().saturating_mul(2).max(1);
            let mut lease = budget.reserve(capacity)?;
            let mut replacement = Vec::new();
            replacement
                .try_reserve_exact(capacity)
                .map_err(|_| PdfError::ResourceLimit("string allocation".into()))?;
            lease.resize(replacement.capacity())?;
            replacement.extend_from_slice(bytes);
            *bytes = replacement;
            *owner = Some(lease);
        }
    } else {
        bytes
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("string allocation".into()))?;
    }
    Ok(())
}

impl PdfWord {
    pub(crate) fn into_reserved_string(
        self,
    ) -> PdfResult<(String, Option<crate::budget::MemoryLease>)> {
        let name = String::from_utf8(self.raw).map_err(|e| {
            PdfError::ParserError(format!("PdfWord is not a utf8 string: {}", e.utf8_error()))
        })?;
        Ok((name, self.lease))
    }
    pub fn into_string(self) -> PdfResult<String> {
        String::from_utf8(self.raw).map_err(|e| {
            PdfError::ParserError(format!("PdfWord is not a utf8 string: {}", e.utf8_error()))
        })
    }

    pub fn add_char(&mut self, c: u8) {
        self.raw.push(c);
    }
    pub fn try_add_char(&mut self, c: u8) -> PdfResult<()> {
        if let Some(budget) = &self.budget {
            if self.raw.len() == self.raw.capacity() {
                let capacity = self
                    .raw
                    .capacity()
                    .checked_mul(2)
                    .unwrap_or(usize::MAX)
                    .max(1);
                let mut lease = budget.reserve(capacity)?;
                let mut replacement = Vec::new();
                replacement
                    .try_reserve_exact(capacity)
                    .map_err(|_| PdfError::ResourceLimit("token allocation".into()))?;
                lease.resize(replacement.capacity())?;
                replacement.extend_from_slice(&self.raw);
                self.raw = replacement;
                self.lease = Some(lease);
            }
            self.raw.push(c);
            return Ok(());
        }
        self.raw
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("token allocation".into()))?;
        self.raw.push(c);
        Ok(())
    }

    pub fn is_empty(&self) -> bool {
        self.raw.is_empty()
    }

    pub fn raw(&self) -> &[u8] {
        self.raw.as_slice()
    }

    pub fn is_number(&self) -> bool {
        self.is_number
    }

    pub fn is_equal(&self, s: &[u8]) -> bool {
        self.raw == s
    }

    pub fn as_u32(&self) -> u32 {
        self.as_u32_checked().expect("PdfWord is not u32")
    }

    pub fn as_u64(&self) -> u64 {
        self.as_u64_checked().expect("PdfWord is not u64")
    }

    pub fn as_u16(&self) -> u16 {
        self.as_u16_checked().expect("PdfWord is not u16")
    }

    pub fn as_u32_checked(&self) -> PdfResult<u32> {
        let value = self.as_u64_checked()?;
        u32::try_from(value)
            .map_err(|_| PdfError::ParserError("PdfWord out of range for u32".into()))
    }

    pub fn as_u64_checked(&self) -> PdfResult<u64> {
        if !self.is_number {
            return Err(PdfError::ParserError("PdfWord is not number".to_string()));
        }
        let s = std::str::from_utf8(self.raw.as_slice())
            .map_err(|e| PdfError::ParserError(format!("PdfWord is not utf8:{e:?}")))?;
        s.parse::<u64>()
            .map_err(|_| PdfError::ParserError("PdfWord is not u64".into()))
    }

    pub fn as_u16_checked(&self) -> PdfResult<u16> {
        let value = self.as_u64_checked()?;
        u16::try_from(value)
            .map_err(|_| PdfError::ParserError("PdfWord out of range for u16".into()))
    }

    pub fn start_width(&self, prefix: &[u8]) -> bool {
        if prefix.len() > self.raw.len() {
            return false;
        }
        for (i, c) in prefix.iter().enumerate() {
            if c != &self.raw[i] {
                return false;
            }
        }
        true
    }
}

#[derive(Debug)]
pub struct SyntaxParser<R: Seek + Read> {
    pos: u64,
    reader: StreamReader<R>,
    mode: ParseMode,
    header: Option<PdfHeader>,
    xref_table: Option<CrossRefTable>,
    limits: super::options::Limits,
    object_depth: usize,
    input_end: u64,
    pub(crate) diagnostics: super::diagnostic::DiagnosticLog,
    work_remaining: Option<u64>,
    diagnostic_object: Option<crate::objects::object_id::ObjectId>,
}

impl<R: Seek + Read> SyntaxParser<R> {
    pub fn is_eof(&self) -> bool {
        self.pos >= self.input_end
    }

    pub fn new(reader: StreamReader<R>) -> Self {
        Self::with_mode(reader, ParseMode::Compatible)
    }

    pub fn with_mode(reader: StreamReader<R>, mode: ParseMode) -> Self {
        Self::with_options(
            reader,
            super::options::ParseOptions {
                mode,
                ..Default::default()
            },
        )
    }

    pub fn with_options(reader: StreamReader<R>, options: super::options::ParseOptions) -> Self {
        let input_end = reader.length();
        Self {
            reader,
            pos: 0,
            mode: options.mode,
            header: None,
            xref_table: None,
            limits: options.limits,
            object_depth: 0,
            input_end,
            diagnostics: super::diagnostic::DiagnosticLog::new(options.limits.max_operation_bytes),
            work_remaining: None,
            diagnostic_object: None,
        }
    }

    pub fn with_xref_table(mut self, xref_table: CrossRefTable) -> Self {
        self.xref_table = Some(xref_table);
        self
    }

    pub fn position(&self) -> u64 {
        self.pos
    }

    pub(crate) fn limit_work(&mut self, steps: u64) { self.work_remaining = Some(steps); }
    pub(crate) fn remaining_work(&self) -> Option<u64> { self.work_remaining }
    pub(crate) fn finish_work(&mut self) { self.work_remaining = None; }
    fn charge_work(&mut self, steps: u64) -> PdfResult<()> {
        if let Some(remaining) = &mut self.work_remaining {
            *remaining = remaining.checked_sub(steps).ok_or_else(|| PdfError::ResourceLimit("recovery scan steps".into()))?;
        }
        Ok(())
    }
    pub(crate) fn report(&self, offset: u64, object: Option<crate::objects::object_id::ObjectId>, reason: super::diagnostic::StructureReason) -> PdfResult<()> {
        self.diagnostics.record(super::diagnostic::StructureDiagnostic { offset, object: object.or(self.diagnostic_object), reason })
    }

    pub fn mode(&self) -> ParseMode {
        self.mode
    }

    pub fn limits(&self) -> super::options::Limits {
        self.limits
    }

    pub fn read_header(&mut self) -> PdfResult<&PdfHeader> {
        if self.header.is_none() {
            let saved_pos = self.pos;
            self.set_pos(0)?;
            let line = self.read_line()?;
            if self.mode == ParseMode::Strict {
                let binary_line = self.read_line()?;
                validate_binary_comment_line(&binary_line)?;
            } else {
                // Inspect without a line allocation or an additional token-size
                // restriction on the first body line in Compatible mode.
                let mut binary = false;
                if !self.is_eof() && self.get_next_char()? == b'%' {
                    let mut high_bits = 0;
                    while !self.is_eof() {
                        let byte = self.get_next_char()?;
                        if is_end_of_line(byte) { break; }
                        high_bits += usize::from(byte >= 128);
                        if high_bits == 4 { binary = true; break; }
                    }
                }
                if !binary {
                    self.report(0, None, super::diagnostic::StructureReason::CompatibleHeader)?;
                }
            }
            self.set_pos(saved_pos)?;
            self.header = Some(parse_pdf_header(&line)?);
        }
        self.header.as_ref().ok_or(PdfError::ParserError(
            "pdf header was not parsed".to_string(),
        ))
    }

    pub fn get_indirect_object(&mut self) -> PdfResult<PdfIndirect> {
        self.get_indirect_object_inner(None)
            .map(|(object, _)| object)
    }

    pub(crate) fn get_indirect_object_with_budget(
        &mut self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfIndirect, crate::budget::MemoryLease)> {
        let (object, lease) = self.get_indirect_object_inner(Some(budget))?;
        Ok((object, lease.unwrap()))
    }

    fn get_indirect_object_inner(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(PdfIndirect, Option<crate::budget::MemoryLease>)> {
        let saved_pos = self.pos;
        let obj_word = self.get_next_word_inner(budget)?;
        if !obj_word.is_number() || obj_word.is_empty() {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "get_indirect_object filed get objnum object".to_string(),
            ));
        }
        let objnum = obj_word.as_u32_checked()?;
        let gen_word = self.get_next_word_inner(budget)?;
        if !gen_word.is_number() || gen_word.is_empty() {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "get_indirect_object failed get obj gennum".to_string(),
            ));
        }

        let gennum = gen_word.as_u16_checked()?;

        let obj_word = self.get_next_word_inner(budget)?;
        if !obj_word.is_equal(b"obj") {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "get_indirect_object obj keyword is expected".to_string(),
            ));
        }
        let previous_object = self.diagnostic_object.replace(crate::objects::object_id::ObjectId::new(objnum, u32::from(gennum)));
        let result = (|| {
        let (lease, obj) = if let Some(budget) = budget {
            let (object, lease) = self.get_content_object_with_budget(budget)?;
            (Some(lease), object)
        } else {
            (None, self.get_object()?)
        };
        let end_word = self.get_next_word_inner(budget)?;
        if !end_word.is_equal(b"endobj") {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "get_indirect_object endobj keyword is expected".to_string(),
            ));
        }
        Ok((PdfIndirect::new(objnum, gennum, obj), lease))
        })();
        self.diagnostic_object = previous_object;
        result
    }

    pub fn get_next_char(&mut self) -> PdfResult<u8> {
        if self.pos >= self.input_end {
            return Err(PdfError::EndofFile);
        }
        self.charge_work(1)?;
        let byte = self.reader.read_byte()?;
        self.pos += 1;
        Ok(byte)
    }

    fn peek_char_at(&mut self, pos: u64) -> PdfResult<u8> {
        if pos >= self.input_end {
            return Err(PdfError::EndofFile);
        }
        self.charge_work(1)?;
        let saved_pos = self.pos;
        self.reader.seek(pos)?;
        let result = self.reader.read_byte();
        self.reader.seek(saved_pos)?;
        result
    }

    pub fn get_object(&mut self) -> PdfResult<PdfObject> {
        if self.object_depth >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit(format!(
                "object nesting at byte {}",
                self.pos
            )));
        }
        self.object_depth += 1;
        let result = self.get_object_inner();
        self.object_depth -= 1;
        result
    }

    pub(crate) fn get_object_until(&mut self, end: u64) -> PdfResult<PdfObject> {
        self.get_object_until_inner(end, None)
            .map(|(object, _)| object)
    }

    pub(crate) fn get_object_until_with_budget(
        &mut self,
        end: u64,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
        let (object, lease) = self.get_object_until_inner(end, Some(budget))?;
        Ok((object, lease.unwrap()))
    }

    fn get_object_until_inner(
        &mut self,
        end: u64,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(PdfObject, Option<crate::budget::MemoryLease>)> {
        if end <= self.pos || end > self.input_end {
            return Err(PdfError::ParserError(
                "invalid object input boundary".into(),
            ));
        }
        let saved_end = self.input_end;
        self.input_end = end;
        let result = (|| {
            let (lease, object) = if let Some(budget) = budget {
                let (object, lease) = self.get_content_object_with_budget(budget)?;
                (Some(lease), object)
            } else {
                (None, self.get_object()?)
            };
            let mut in_comment = false;
            while !self.is_eof() {
                let ch = self.get_next_char()?;
                if in_comment {
                    if is_end_of_line(ch) {
                        in_comment = false;
                    }
                } else if ch == b'%' {
                    in_comment = true;
                } else if !is_white_space(ch) {
                    return Err(PdfError::ParserError(format!(
                        "unexpected trailing object data at byte {}",
                        self.pos - 1
                    )));
                }
            }
            Ok((object, lease))
        })();
        self.input_end = saved_end;
        result
    }

    fn get_object_inner(&mut self) -> PdfResult<PdfObject> {
        if self.is_eof() {
            return Err(PdfError::EndofFile);
        }
        let word = self.get_next_word()?;
        if word.is_number() {
            let saved_pos = self.pos;
            let next = match self.get_next_word() {
                Ok(next) => next,
                Err(PdfError::EndofFile) => {
                    self.set_pos(saved_pos)?;
                    return Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                        word.raw(),
                    )?));
                }
                Err(error) => return Err(error),
            };
            if next.is_number() {
                match self.get_next_word() {
                    Ok(next2) if next2.is_equal(b"R") => {
                        return Ok(PdfObject::PdfReference(PdfReference::new(
                            word.as_u32_checked()?,
                            u32::from(next.as_u16_checked()?),
                        )));
                    }
                    Ok(_) | Err(PdfError::EndofFile) => {
                        self.set_pos(saved_pos)?;
                        return Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                            word.raw(),
                        )?));
                    }
                    Err(err) => return Err(err),
                }
            } else {
                self.set_pos(saved_pos)?;
                return Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                    word.raw(),
                )?));
            }
        }
        if word.is_equal(b"true") || word.is_equal(b"false") {
            return Ok(PdfObject::PdfBool(PdfBool::new(word.raw())));
        }
        if word.is_equal(b"null") {
            return Ok(PdfObject::PdfNull);
        }
        if word.is_equal(b"(") {
            return Ok(PdfObject::PdfString(self.read_string()?));
        }
        if word.is_equal(b"<") {
            return Ok(PdfObject::PdfString(self.read_hex_string()?));
        }
        if word.is_equal(b"[") {
            let mut array = PdfArray::default();
            loop {
                let next_word = self.peek_word()?;
                if next_word.is_equal(b"]") {
                    self.get_next_word()?;
                    break;
                }
                if array.len() >= self.limits.max_objects {
                    return Err(PdfError::ResourceLimit("array element count".into()));
                }
                array.try_add_obj(self.get_object()?)?;
            }
            return Ok(PdfObject::PdfArray(array));
        }
        if word.start_width(b"/") {
            return Ok(PdfObject::PdfName(PdfName::new_from_buffer(word.raw())?));
        }
        if word.is_equal(b"<<") {
            let mut dict = PdfDict::default();
            let mut entry_count = 0usize;
            loop {
                let saved_pos = self.pos;
                let next_word = self.get_next_word()?;
                if next_word.is_equal(b">>") {
                    break;
                }
                if entry_count >= self.limits.max_objects {
                    return Err(PdfError::ResourceLimit("dictionary entry count".into()));
                }
                entry_count += 1;
                if next_word.is_equal(b"endobj") {
                    self.set_pos(saved_pos)?;
                }
                if !next_word.start_width(b"/") {
                    return Err(PdfError::ParserError(format!(
                        "dictionary key must be a name at byte {}",
                        saved_pos
                    )));
                }
                let name = PdfName::new_from_buffer(next_word.raw())?;
                let key = name.name().to_string();
                if self.mode == ParseMode::Strict && dict.contains_key(&key) {
                    return Err(PdfError::ParserError(format!(
                        "duplicate dictionary key is not allowed in strict mode at byte {saved_pos}"
                    )));
                }
                if self.mode == ParseMode::Compatible && dict.contains_key(&key) {
                    self.report(saved_pos, None, super::diagnostic::StructureReason::DuplicateDictionaryKey)?;
                }
                let value = self.get_object()?;
                dict.try_insert(key, value)?;
            }
            let saved_pos = self.pos;
            if self.is_eof() {
                return Ok(PdfObject::PdfDict(dict));
            }
            let next_word = match self.get_next_word() {
                Ok(word) => word,
                Err(PdfError::EndofFile) => {
                    self.set_pos(saved_pos)?;
                    return Ok(PdfObject::PdfDict(dict));
                }
                Err(error) => return Err(error),
            };
            if !next_word.is_equal(b"stream") {
                self.set_pos(saved_pos)?;
                return Ok(PdfObject::PdfDict(dict));
            }
            return Ok(PdfObject::PdfStream(self.read_pdf_stream(dict)?));
        }
        Err(PdfError::ParserError(format!(
            "invalid object token ending at byte {}",
            self.pos
        )))
    }

    fn read_pdf_stream(&mut self, dict: PdfDict) -> PdfResult<PdfStream> {
        self.read_pdf_stream_inner(dict, None)
            .map(|(stream, _)| stream)
    }

    fn read_pdf_stream_inner(
        &mut self,
        dict: PdfDict,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(PdfStream, Option<crate::budget::MemoryLease>)> {
        let saved_pos = self.pos;
        self.consume_stream_start()?;
        let data_start = self.pos;

        if let Some(data_len) = self.resolve_stream_length(&dict, budget)? {
            let start_pos = self.pos;
            if data_len <= self.input_end.saturating_sub(start_pos) {
                let (data, lease) = self.read_stream_block(data_len, budget)?;
                if self.expect_endstream_after_data(budget)? {
                    return Ok((PdfStream::new(dict, data), lease));
                }
            }
        }

        if self.mode == ParseMode::Strict {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "strict mode requires a valid stream Length and matching endstream".to_string(),
            ));
        }

        self.set_pos(data_start)?;
        self.report(data_start, None, super::diagnostic::StructureReason::StreamBoundarySearch)?;
        let end_of_stream = self.find_stream_end_pos()?;
        if let Some(end_pos) = end_of_stream {
            let data_len = end_pos.saturating_sub(self.pos);
            let (data, lease) = self.read_stream_block(data_len, budget)?;
            return Ok((PdfStream::new(dict, data), lease));
        }
        self.set_pos(saved_pos)?;
        Err(PdfError::ParserError(
            "stream dict has no Length and endstream could not be found".to_string(),
        ))
    }

    fn resolve_stream_length(
        &mut self,
        dict: &PdfDict,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<Option<u64>> {
        let Some(len_obj) = dict.get("Length") else {
            return Ok(None);
        };
        match len_obj {
            PdfObject::PdfNumber(len_num) => Ok(Some(len_num.as_u64_checked()?)),
            PdfObject::PdfReference(reference) => {
                let Some(xref_table) = self.xref_table.as_ref() else {
                    return Ok(None);
                };
                let Some(info) = xref_table.lookup_id(reference.id()) else {
                    return Ok(None);
                };
                if matches!(info.state(), ObjectState::Compressed) {
                    return Ok(None);
                }
                let saved_pos = self.pos;
                self.set_pos(info.offset())?;
                let indirect = self.get_indirect_object_inner(budget);
                self.set_pos(saved_pos)?;
                let (indirect, _lease) = indirect?;
                if indirect.id() != reference.id() {
                    return Err(PdfError::InvalidReference(format!(
                        "stream Length reference {:?} points to {:?}",
                        reference.id(),
                        indirect.id()
                    )));
                }
                let number = indirect.obj().as_number().ok_or(PdfError::ParserError(
                    "stream Length indirect object must resolve to a number".to_string(),
                ))?;
                Ok(Some(number.as_u64_checked()?))
            }
            _ => Ok(None),
        }
    }

    fn consume_stream_start(&mut self) -> PdfResult<()> {
        let next = self.get_next_char()?;
        match next {
            b'\n' => Ok(()),
            b'\r' => {
                self.consume_optional_lf()?;
                Ok(())
            }
            _ if self.mode == ParseMode::Compatible => {
                self.report(self.pos - 1, None, super::diagnostic::StructureReason::StreamStartWhitespace)?;
                self.set_pos(self.pos - 1)?;
                self.to_next_line()
            }
            _ => Err(PdfError::ParserError(
                "stream keyword must be followed by an end-of-line marker".to_string(),
            )),
        }
    }

    fn expect_endstream_after_data(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<bool> {
        let saved_pos = self.pos;
        self.consume_optional_single_eol()?;
        let next_word = self.get_next_word_inner(budget)?;
        if next_word.is_equal(b"endstream") {
            return Ok(true);
        }
        self.set_pos(saved_pos)?;
        drop(next_word);
        let next_word = self.get_next_word_inner(budget)?;
        if next_word.is_equal(b"endstream") {
            return Ok(true);
        }
        self.set_pos(saved_pos)?;
        Ok(false)
    }

    fn consume_optional_single_eol(&mut self) -> PdfResult<()> {
        let next = match self.get_next_char() {
            Ok(ch) => ch,
            Err(PdfError::EndofFile) => return Ok(()),
            Err(err) => return Err(err),
        };
        match next {
            b'\n' => Ok(()),
            b'\r' => {
                self.consume_optional_lf()?;
                Ok(())
            }
            _ => self.set_pos(self.pos - 1),
        }
    }

    fn find_stream_end_pos(&mut self) -> PdfResult<Option<u64>> {
        let start_of_end_stream = self.find_word_pos(b"endstream")?;
        let start_of_end_obj = self.find_word_pos(b"endobj")?;
        if start_of_end_stream.is_none() && start_of_end_obj.is_none() {
            return Ok(None);
        }
        let end_pos = match (start_of_end_stream, start_of_end_obj) {
            (None, None) => None,
            (Some(end_s), None) => Some(end_s),
            (None, Some(end_o)) => Some(end_o),
            (Some(end_s), Some(end_o)) => Some(end_s.min(end_o)),
        };
        if let Some(end_stream_pos) = end_pos {
            let mut marker_len = 0;
            if end_stream_pos > self.pos {
                match self.peek_char_at(end_stream_pos - 1)? {
                    b'\r' => marker_len = 1,
                    b'\n' => {
                        marker_len = 1;
                        if end_stream_pos - self.pos >= 2
                            && self.peek_char_at(end_stream_pos - 2)? == b'\r'
                        {
                            marker_len = 2;
                        }
                    }
                    _ => {}
                }
            }
            return Ok(Some(end_stream_pos.saturating_sub(marker_len)));
        }
        Ok(None)
    }

    fn consume_optional_lf(&mut self) -> PdfResult<()> {
        match self.peek_char_at(self.pos) {
            Ok(b'\n') => {
                self.get_next_char()?;
            }
            Ok(_) | Err(PdfError::EndofFile) => {}
            Err(err) => return Err(err),
        }
        Ok(())
    }

    fn find_word_pos(&mut self, tag: &[u8]) -> PdfResult<Option<u64>> {
        let saved_pos = self.pos;
        let tag_len = tag.len();
        loop {
            let mut match_found = true;
            let start_pos = self.pos;
            if start_pos.saturating_sub(saved_pos) > self.limits.max_raw_stream_bytes as u64 {
                self.set_pos(saved_pos)?;
                return Err(PdfError::ResourceLimit(
                    "stream marker search distance".into(),
                ));
            }
            let mut i = 0;
            while i < tag_len {
                let ch = match self.get_next_char() {
                    Ok(ch) => ch,
                    Err(err) => {
                        self.set_pos(saved_pos)?;
                        return match err {
                            PdfError::EndofFile => Ok(None),
                            other => Err(other),
                        };
                    }
                };
                if ch != tag[i] {
                    match_found = false;
                    break;
                }
                i += 1;
            }
            if match_found {
                self.set_pos(saved_pos)?;
                return Ok(Some(start_pos));
            }
            self.set_pos(start_pos + 1)?;
        }
    }

    pub fn set_pos(&mut self, pos: u64) -> PdfResult<()> {
        self.reader.seek(pos)?;
        self.pos = pos;
        Ok(())
    }

    fn read_block(&mut self, len: u64) -> PdfResult<Vec<u8>> {
        self.read_stream_block(len, None).map(|(bytes, _)| bytes)
    }

    fn read_stream_block(
        &mut self,
        len: u64,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(Vec<u8>, Option<crate::budget::MemoryLease>)> {
        if len > self.limits.max_raw_stream_bytes as u64 {
            return Err(PdfError::ResourceLimit("raw stream byte size".into()));
        }
        if len > self.input_end.saturating_sub(self.pos) {
            return Err(PdfError::ParserError("invalid length of block".to_string()));
        }
        let size =
            usize::try_from(len).map_err(|_| PdfError::ResourceLimit("input block size".into()))?;
        self.charge_work(len)?;
        let result = if let Some(budget) = budget {
            self.reader
                .read_bytes_with_budget(size, budget)
                .map(|bytes| {
                    let (bytes, lease) = bytes.into_reserved_parts();
                    (bytes, Some(lease))
                })
        } else {
            self.reader.read_bytes(size).map(|bytes| (bytes, None))
        };
        match result {
            Ok(bytes) => {
                self.pos += len;
                Ok(bytes)
            }
            Err(err) => {
                self.reader.seek(self.pos)?;
                Err(err)
            }
        }
    }

    fn to_next_line(&mut self) -> PdfResult<()> {
        // These callers discard the line: retain neither bytes nor capacity.
        let mut length = 0usize;
        loop {
            let ch = match self.get_next_char() {
                Ok(ch) => ch,
                Err(PdfError::EndofFile) => return Ok(()),
                Err(error) => return Err(error),
            };
            if is_end_of_line(ch) {
                if ch == b'\r' {
                    self.consume_optional_lf()?;
                }
                return Ok(());
            }
            if length >= self.limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("line byte size".into()));
            }
            length += 1;
        }
    }

    pub fn read_line(&mut self) -> PdfResult<Vec<u8>> {
        let mut line = Vec::new();
        loop {
            let ch = match self.get_next_char() {
                Ok(ch) => ch,
                Err(PdfError::EndofFile) => break,
                Err(err) => return Err(err),
            };
            if is_end_of_line(ch) {
                if ch == b'\r' {
                    self.consume_optional_lf()?;
                }
                break;
            }
            if line.len() >= self.limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("line byte size".into()));
            }
            line.try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("line allocation".into()))?;
            line.push(ch);
        }
        Ok(line)
    }

    fn read_hex_string(&mut self) -> PdfResult<PdfString> {
        self.read_hex_string_inner(None, &mut None)
    }

    pub(crate) fn get_string_with_budget(
        &mut self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfString, crate::budget::MemoryLease)> {
        let word = self.get_next_word_with_budget(budget)?;
        let hex = word.is_equal(b"<");
        if !hex && !word.is_equal(b"(") {
            return Err(PdfError::ParserError(
                "expected string opening token".into(),
            ));
        }
        drop(word);
        let mut lease = Some(budget.reserve(0)?);
        let string = if hex {
            self.read_hex_string_inner(Some(budget), &mut lease)?
        } else {
            self.read_string_inner(Some(budget), &mut lease)?
        };
        Ok((string, lease.unwrap()))
    }

    pub(crate) fn get_name_with_budget(
        &mut self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfName, crate::budget::MemoryLease)> {
        let word = self.get_next_word_with_budget(budget)?;
        PdfName::new_from_buffer_with_budget(word.raw(), budget)
    }

    /// Recursively track content operand tables and scalar payloads.
    /// Embedded stream bytes still use the legacy reader.
    pub(crate) fn get_content_object_with_budget(
        &mut self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
        if self.object_depth >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit("object nesting".into()));
        }
        let start = self.pos;
        let word = self.get_next_word_with_budget(budget)?;
        if word.is_equal(b"true") || word.is_equal(b"false") {
            return Ok((
                PdfObject::PdfBool(PdfBool::new(word.raw())),
                budget.reserve(0)?,
            ));
        }
        if word.is_equal(b"null") {
            return Ok((PdfObject::PdfNull, budget.reserve(0)?));
        }
        let array = word.is_equal(b"[");
        let dictionary = word.is_equal(b"<<");
        let number = word.is_number();
        let name = word.start_width(b"/");
        let string = word.is_equal(b"(") || word.is_equal(b"<");
        drop(word);
        if !(array || dictionary || number || name || string) {
            return Err(PdfError::ParserError(format!(
                "invalid object token ending at byte {}",
                self.pos
            )));
        }
        self.set_pos(start)?;
        if number {
            return self.get_number_with_budget(budget);
        }
        if name {
            let (name, lease) = self.get_name_with_budget(budget)?;
            return Ok((PdfObject::PdfName(name), lease));
        }
        if string {
            let (string, lease) = self.get_string_with_budget(budget)?;
            return Ok((PdfObject::PdfString(string), lease));
        }
        if dictionary {
            drop(self.get_next_word_with_budget(budget)?);
            self.object_depth += 1;
            let result = (|| {
                let mut builder = crate::objects::pdf_dict::BudgetedDictBuilder::new(
                    budget,
                    self.limits.max_objects,
                )?;
                let mut entry_count = 0;
                loop {
                    let start = self.pos;
                    let word = self.get_next_word_with_budget(budget)?;
                    if word.is_equal(b">>") {
                        break;
                    }
                    if entry_count >= self.limits.max_objects {
                        return Err(PdfError::ResourceLimit("dictionary entry count".into()));
                    }
                    entry_count += 1;
                    if word.is_equal(b"endobj") {
                        self.set_pos(start)?;
                    }
                    let (name, lease) = PdfName::new_from_buffer_with_budget(word.raw(), budget)?;
                    drop(word);
                    let (key, key_lease) = name.into_budgeted_key(lease)?;
                    if self.mode == ParseMode::Strict && builder.contains_key(&key) {
                        return Err(PdfError::ParserError(
                            "duplicate dictionary key is not allowed in strict mode".into(),
                        ));
                    }
                    if self.mode == ParseMode::Compatible && builder.contains_key(&key) {
                        self.report(start, None, super::diagnostic::StructureReason::DuplicateDictionaryKey)?;
                    }
                    let (value, value_lease) = self.get_content_object_with_budget(budget)?;
                    builder.insert(key, value, key_lease, value_lease)?;
                }
                let saved_pos = self.pos;
                let stream = match self.get_next_word_with_budget(budget) {
                    Ok(word) => word.is_equal(b"stream"),
                    Err(PdfError::EndofFile) => false,
                    Err(error) => return Err(error),
                };
                let (dict, mut lease) = builder.finish()?;
                if stream {
                    let (stream, mut data_lease) =
                        self.read_pdf_stream_inner(dict, Some(budget))?;
                    lease.absorb(data_lease.as_mut().unwrap())?;
                    Ok((PdfObject::PdfStream(stream), lease))
                } else {
                    self.set_pos(saved_pos)?;
                    Ok((PdfObject::PdfDict(dict), lease))
                }
            })();
            self.object_depth -= 1;
            return result;
        }
        drop(self.get_next_word_with_budget(budget)?);
        self.object_depth += 1;
        let result = (|| {
            let mut payload = budget.reserve(0)?;
            let mut table = budget.reserve(0)?;
            let mut items = Vec::new();
            loop {
                let start = self.pos;
                let next = self.get_next_word_with_budget(budget)?;
                if next.is_equal(b"]") {
                    break;
                }
                drop(next);
                self.set_pos(start)?;
                if items.len() >= self.limits.max_objects {
                    return Err(PdfError::ResourceLimit("array element count".into()));
                }
                if items.len() == items.capacity() {
                    let capacity = items
                        .capacity()
                        .saturating_mul(2)
                        .max(1)
                        .min(self.limits.max_objects);
                    let size = std::mem::size_of::<PdfObject>();
                    let mut replacement_lease = budget.reserve(
                        capacity
                            .checked_mul(size)
                            .ok_or_else(|| PdfError::ResourceLimit("array table size".into()))?,
                    )?;
                    let mut replacement = Vec::new();
                    replacement
                        .try_reserve_exact(capacity)
                        .map_err(|_| PdfError::ResourceLimit("array table allocation".into()))?;
                    replacement_lease.resize(
                        replacement.capacity().checked_mul(size).ok_or_else(|| {
                            PdfError::ResourceLimit("array table capacity".into())
                        })?,
                    )?;
                    replacement.append(&mut items);
                    items = replacement;
                    table = replacement_lease;
                }
                let (object, mut lease) = self.get_content_object_with_budget(budget)?;
                items.push(object);
                payload.absorb(&mut lease)?;
            }
            payload.absorb(&mut table)?;
            Ok((PdfObject::PdfArray(PdfArray::from_items(items)), payload))
        })();
        self.object_depth -= 1;
        result
    }

    /// Preserve object-parser reference lookahead while tracking all numeric
    /// tokens. A number takes ownership of its token rather than copying it.
    pub(crate) fn get_number_with_budget(
        &mut self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
        if self.object_depth >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit("object nesting".into()));
        }
        let word = self.get_next_word_with_budget(budget)?;
        if !word.is_number() {
            return Err(PdfError::ParserError("expected number token".into()));
        }
        let saved_pos = self.pos;
        let reference = match self.get_next_word_with_budget(budget) {
            Ok(next) if next.is_number() => match self.get_next_word_with_budget(budget) {
                Ok(next2) if next2.is_equal(b"R") => Some(PdfReference::new(
                    word.as_u32_checked()?,
                    u32::from(next.as_u16_checked()?),
                )),
                Ok(_) | Err(PdfError::EndofFile) => None,
                Err(error) => return Err(error),
            },
            Ok(_) | Err(PdfError::EndofFile) => None,
            Err(error) => return Err(error),
        };
        if let Some(reference) = reference {
            return Ok((PdfObject::PdfReference(reference), budget.reserve(0)?));
        }
        self.set_pos(saved_pos)?;
        let (lexeme, lease) = word.into_reserved_string()?;
        let number = PdfNumber::from_owned_lexeme(lexeme)?;
        Ok((PdfObject::PdfNumber(number), lease.unwrap()))
    }

    fn read_hex_string_inner(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
        lease: &mut Option<crate::budget::MemoryLease>,
    ) -> PdfResult<PdfString> {
        let mut bytes = Vec::new();
        let start = self.pos;
        let mut high_nibble = None;
        loop {
            let ch = self.get_next_char()?;
            if self.pos.saturating_sub(start) > self.limits.max_token_bytes as u64 {
                return Err(PdfError::ResourceLimit("hex string token size".into()));
            }
            match ch {
                b'>' => {
                    if let Some(hi) = high_nibble.take() {
                        reserve_string_byte(&mut bytes, budget, lease)?;
                        bytes.push(hi << 4);
                    }
                    break;
                }
                _ if is_white_space(ch) => continue,
                _ if ch.is_ascii_hexdigit() => {
                    let nibble = hex_to_u8(ch);
                    if let Some(hi) = high_nibble.take() {
                        reserve_string_byte(&mut bytes, budget, lease)?;
                        bytes.push((hi << 4) | nibble);
                    } else {
                        high_nibble = Some(nibble);
                    }
                }
                _ => {
                    return Err(PdfError::ParserError(format!(
                        "hex string contains invalid character: {:?}",
                        ch as char
                    )));
                }
            }
        }
        Ok(PdfString::new(bytes, true))
    }

    fn read_string(&mut self) -> PdfResult<PdfString> {
        self.read_string_inner(None, &mut None)
    }

    fn read_string_inner(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
        lease: &mut Option<crate::budget::MemoryLease>,
    ) -> PdfResult<PdfString> {
        let mut nest_level = 0;
        let start = self.pos;
        let mut bytes = Vec::new();
        loop {
            let ch = self.get_next_char()?;
            if self.pos.saturating_sub(start) > self.limits.max_token_bytes as u64 {
                return Err(PdfError::ResourceLimit("literal string token size".into()));
            }
            if ch == b')' && nest_level == 0 {
                return Ok(PdfString::new(bytes, false));
            }
            if ch != b'\\' {
                reserve_string_byte(&mut bytes, budget, lease)?;
            }
            match ch {
                b'(' => {
                    if nest_level >= self.limits.max_depth {
                        return Err(PdfError::ResourceLimit(
                            "literal string nesting depth".into(),
                        ));
                    }
                    nest_level += 1;
                    bytes.push(ch);
                }
                b')' => {
                    if nest_level == 0 {
                        return Ok(PdfString::new(bytes, false));
                    }
                    nest_level -= 1;
                    bytes.push(ch);
                }
                b'\\' => {
                    let escaped = self.get_next_char()?;
                    if !matches!(escaped, b'\n' | b'\r') {
                        reserve_string_byte(&mut bytes, budget, lease)?;
                    }
                    match escaped {
                        b'n' => bytes.push(b'\n'),
                        b'r' => bytes.push(b'\r'),
                        b't' => bytes.push(b'\t'),
                        b'b' => bytes.push(8),
                        b'f' => bytes.push(12),
                        b'(' | b')' | b'\\' => bytes.push(escaped),
                        b'\n' => {}
                        b'\r' => {
                            self.consume_optional_lf()?;
                        }
                        b'0'..=b'7' => {
                            let mut value = escaped - b'0';
                            for _ in 0..2 {
                                match self.peek_char_at(self.pos) {
                                    Ok(next @ b'0'..=b'7') => {
                                        let _ = self.get_next_char()?;
                                        // PDF ignores high-order overflow in octal escapes.
                                        value = value.wrapping_mul(8).wrapping_add(next - b'0');
                                    }
                                    _ => break,
                                }
                            }
                            bytes.push(value);
                        }
                        _ => bytes.push(escaped),
                    }
                }
                b'\r' => {
                    if matches!(self.peek_char_at(self.pos), Ok(b'\n')) {
                        self.get_next_char()?;
                    }
                    bytes.push(b'\n');
                }
                _ => bytes.push(ch),
            }
        }
    }

    pub fn peek_word(&mut self) -> PdfResult<PdfWord> {
        let saved_pos = self.pos;
        let word = self.get_next_word();
        self.set_pos(saved_pos)?;
        word
    }

    pub fn read_eol_marker(&mut self, pos: u64) -> PdfResult<u64> {
        let byte1 = self.peek_char_at(pos)?;
        if byte1 == b'\r' {
            if pos >= self.input_end.saturating_sub(1) {
                return Ok(1);
            }
            match self.peek_char_at(pos.saturating_add(1)) {
                Ok(b'\n') => return Ok(2),
                Ok(_) | Err(PdfError::EndofFile) => return Ok(1),
                Err(err) => return Err(err),
            }
        }
        if byte1 == b'\n' {
            return Ok(1);
        }
        Ok(0)
    }

    pub fn get_next_word(&mut self) -> PdfResult<PdfWord> {
        self.get_next_word_inner(None)
    }

    pub(crate) fn get_next_word_with_budget(
        &mut self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<PdfWord> {
        self.get_next_word_inner(Some(budget))
    }

    fn get_next_word_inner(
        &mut self,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<PdfWord> {
        self.to_next_word()?;
        let mut word = PdfWord::default();
        if let Some(budget) = budget {
            word.lease = Some(budget.reserve(0)?);
            word.budget = Some(budget.clone());
        }
        let mut c = self.get_next_char()?;
        if self.limits.max_token_bytes == 0 {
            return Err(PdfError::ResourceLimit("token byte size".into()));
        }
        if is_delimiter(c) {
            word.try_add_char(c)?;
            if c == b'/' {
                loop {
                    c = match self.get_next_char() {
                        Ok(ch) => ch,
                        Err(PdfError::EndofFile) => break,
                        Err(err) => return Err(err),
                    };
                    if !is_regular(c) && !is_number(c) && c != b'#' {
                        self.set_pos(self.pos - 1)?;
                        break;
                    }
                    if word.raw().len() >= self.limits.max_token_bytes {
                        return Err(PdfError::ResourceLimit("name token byte size".into()));
                    }
                    word.try_add_char(c)?;
                }
            } else if c == b'<' {
                c = self.get_next_char()?;
                if c == b'<' {
                    if word.raw().len() >= self.limits.max_token_bytes {
                        return Err(PdfError::ResourceLimit("delimiter token byte size".into()));
                    }
                    word.try_add_char(c)?;
                } else {
                    self.set_pos(self.pos - 1)?;
                }
            } else if c == b'>' {
                c = self.get_next_char()?;
                if c == b'>' {
                    if word.raw().len() >= self.limits.max_token_bytes {
                        return Err(PdfError::ResourceLimit("delimiter token byte size".into()));
                    }
                    word.try_add_char(c)?;
                } else {
                    self.set_pos(self.pos - 1)?;
                }
            }
            return Ok(word);
        }
        loop {
            if word.raw().len() >= self.limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("word token byte size".into()));
            }
            word.try_add_char(c)?;
            c = match self.get_next_char() {
                Ok(ch) => ch,
                Err(PdfError::EndofFile) => break,
                Err(err) => return Err(err),
            };
            if is_delimiter(c) || is_white_space(c) {
                self.set_pos(self.pos - 1)?;
                break;
            }
        }
        word.is_number = is_number_token(word.raw());
        Ok(word)
    }

    fn to_next_word(&mut self) -> PdfResult<()> {
        let mut c = self.get_next_char()?;
        loop {
            while is_white_space(c) {
                c = self.get_next_char()?;
            }
            if c != b'%' {
                break;
            }
            let comment_start = self.pos - 1;
            let mut comment_prefix = [0u8; 5];
            let mut prefix_count = 0;
            loop {
                c = self.get_next_char()?;
                if prefix_count < comment_prefix.len() { comment_prefix[prefix_count] = c; prefix_count += 1; }
                if is_end_of_line(c) {
                    break;
                }
            }
            if self.work_remaining.is_some() && (&comment_prefix[..4] == b"%EOF" || (&comment_prefix[..4] == b"PDF-" && comment_start != 0)) {
                return Err(PdfError::UnsupportedFeature("recovery refuses embedded revision/header markers".into()));
            }
        }
        self.set_pos(self.pos - 1)?;
        Ok(())
    }

    pub fn file_size(&self) -> u64 {
        self.reader.length()
    }
}

fn parse_pdf_header(line: &[u8]) -> PdfResult<PdfHeader> {
    let prefix = b"%PDF-";
    if line.len() < prefix.len() + 3 || &line[..prefix.len()] != prefix {
        return Err(PdfError::ParserError(
            "file header is not a valid PDF header".to_string(),
        ));
    }
    let version = std::str::from_utf8(&line[prefix.len()..])
        .map_err(|e| PdfError::ParserError(format!("invalid header version: {e:?}")))?;
    if !matches!(
        version,
        "1.0" | "1.1" | "1.2" | "1.3" | "1.4" | "1.5" | "1.6" | "1.7" | "2.0"
    ) {
        return Err(PdfError::ParserError(format!(
            "unsupported pdf header version: {version}"
        )));
    }
    Ok(PdfHeader {
        version: version.to_string(),
    })
}

fn validate_binary_comment_line(line: &[u8]) -> PdfResult<()> {
    if line.first() != Some(&b'%') {
        return Err(PdfError::ParserError(
            "strict mode requires a binary comment line immediately after the PDF header"
                .to_string(),
        ));
    }
    let high_bit_count = line[1..].iter().filter(|byte| **byte >= 128).count();
    if high_bit_count < 4 {
        return Err(PdfError::ParserError(
            "strict mode binary comment line must contain at least four bytes with the high bit set"
                .to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn is_number_token(buf: &[u8]) -> bool {
    if buf.is_empty() {
        return false;
    }
    let mut i = 0;
    if matches!(buf[0], b'+' | b'-') {
        i += 1;
    }
    if i >= buf.len() {
        return false;
    }

    let mut digits_before = 0;
    while i < buf.len() && buf[i].is_ascii_digit() {
        digits_before += 1;
        i += 1;
    }

    let mut digits_after = 0;
    if i < buf.len() && buf[i] == b'.' {
        i += 1;
        while i < buf.len() && buf[i].is_ascii_digit() {
            digits_after += 1;
            i += 1;
        }
    }

    i == buf.len() && (digits_before > 0 || digits_after > 0)
}

#[cfg(test)]
mod tests {
    #[test]
    fn delimiter_tokens_respect_zero_and_single_byte_limits() {
        use crate::{
            io::stream_reader::StreamReader,
            parser::options::{Limits, ParseOptions},
        };
        for (limit, source, allowed) in [
            (0, "[", false),
            (1, "<<", false),
            (1, ">>", false),
            (1, "/a", false),
            (1, "[", true),
            (2, "<<", true),
        ] {
            let options = ParseOptions {
                limits: Limits {
                    max_token_bytes: limit,
                    ..Default::default()
                },
                ..Default::default()
            };
            let reader =
                StreamReader::try_new(std::io::Cursor::new(source.as_bytes().to_vec())).unwrap();
            let result = super::SyntaxParser::with_options(reader, options).get_next_word();
            if allowed {
                assert!(result.is_ok());
            } else {
                assert!(matches!(
                    result,
                    Err(crate::error::PdfError::ResourceLimit(_))
                ));
            }
        }
    }
    #[test]
    fn word_errors_do_not_expand_large_payloads() {
        let word = super::PdfWord {
            raw: vec![b'9'; 100_000],
            is_number: true,
            ..Default::default()
        };
        assert!(word.as_u64_checked().unwrap_err().to_string().len() < 128);
        let word = super::PdfWord {
            raw: vec![0xff; 100_000],
            is_number: false,
            ..Default::default()
        };
        assert!(word.into_string().unwrap_err().to_string().len() < 160);
    }
    use std::io::Cursor;

    use crate::{
        document::cross_ref_table::{CrossRefTable, ObjectInfo, ObjectState},
        io::stream_reader::StreamReader,
        parser::syntax::{ParseMode, SyntaxParser},
    };

    fn new_parser(buffer: &str) -> SyntaxParser<Cursor<&str>> {
        let inner = Cursor::new(buffer);
        let reader = StreamReader::try_new(inner).unwrap();
        SyntaxParser::new(reader)
    }

    #[test]
    fn invalid_budgeted_object_tokens_match_ordinary_errors_without_fallback() {
        use crate::budget::OperationBudget;
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            for source in [
                "unknown",
                "obj",
                "endobj",
                "]",
                ")",
                ">>",
                " % comment\nunsupported",
            ] {
                let make = || {
                    SyntaxParser::with_mode(
                        StreamReader::try_new(Cursor::new(source)).unwrap(),
                        mode,
                    )
                };
                let mut ordinary = make();
                let expected = ordinary.get_object().unwrap_err().to_string();
                let pool = OperationBudget::new(65536);
                let mut budgeted = make();
                assert_eq!(
                    budgeted
                        .get_content_object_with_budget(&pool)
                        .unwrap_err()
                        .to_string(),
                    expected
                );
                assert_eq!(budgeted.position(), ordinary.position());
                assert_eq!(pool.live_bytes(), 0);
                assert!(pool.peak_bytes() > 0);
            }
        }
        let source = "unsupported".repeat(128);
        let pool = OperationBudget::new(32);
        let mut parser = new_parser(&source);
        assert!(matches!(
            parser.get_content_object_with_budget(&pool),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn discarded_lines_match_retained_line_positions_and_limits() {
        for source in ["", "abc", "abc\nnext", "abc\rnext", "abc\r\nnext", "\nnext"] {
            let mut retained = new_parser(source);
            retained.read_line().unwrap();
            let mut skipped = new_parser(source);
            skipped.to_next_line().unwrap();
            assert_eq!(skipped.position(), retained.position(), "{source}");
        }
        for source in ["abc", "abcd", "abc\r\nnext"] {
            let mut retained = new_parser(source);
            let mut skipped = new_parser(source);
            retained.limits.max_token_bytes = 3;
            skipped.limits.max_token_bytes = 3;
            assert_eq!(skipped.to_next_line().is_ok(), retained.read_line().is_ok());
            assert_eq!(skipped.position(), retained.position());
        }
        let mut parser = new_parser("  ignored\r\nabc");
        parser.consume_stream_start().unwrap();
        assert_eq!(parser.position(), 11);
    }

    #[test]
    fn budgeted_stream_payloads_preserve_length_and_recovery_semantics() {
        let budget = crate::budget::OperationBudget::new(16384);
        for source in [
            "<< /Length 3 >> stream\nabc\nendstream",
            "<< /Length 3 >> stream ignored\r\nabc\nendstream",
            "<< /Length 1 >> stream\nabc\nendstream",
            "<<>> stream\nabc\nendstream",
        ] {
            let mut legacy = new_parser(source);
            let expected = legacy.get_object().unwrap();
            let mut parser = new_parser(source);
            let (object, lease) = parser.get_content_object_with_budget(&budget).unwrap();
            assert_eq!(object, expected);
            assert_eq!(parser.position(), legacy.position());
            assert_eq!(object.as_stream().unwrap().raw_data(), b"abc");
            let expected_bytes = if source.starts_with("<<>>") {
                3
            } else {
                std::mem::size_of::<(String, crate::objects::PdfObject)>() + 10
            };
            assert_eq!(budget.live_bytes(), expected_bytes);
            drop(object);
            drop(lease);
            assert_eq!(budget.live_bytes(), 0);
        }
        let neighbor = budget.reserve(3).unwrap();
        let mut parser = new_parser("<< /Length 1 >> stream\nabc\nendstream");
        parser.mode = ParseMode::Strict;
        assert!(parser.get_content_object_with_budget(&budget).is_err());
        assert_eq!(parser.object_depth, 0);
        assert_eq!(budget.live_bytes(), 3);
        let mut parser = new_parser("abcdef");
        let small = crate::budget::OperationBudget::new(3);
        assert!(parser.read_stream_block(4, Some(&small)).is_err());
        assert_eq!(parser.position(), 0);
        assert_eq!(small.live_bytes(), 0);
        let (bytes, lease) = parser.read_stream_block(3, Some(&small)).unwrap();
        assert_eq!(bytes, b"abc");
        assert_eq!(small.live_bytes(), 3);
        drop(bytes);
        drop(lease);
        assert_eq!(small.live_bytes(), 0);
        drop(neighbor);
    }

    #[test]
    fn dictionary_count_and_depth_limits_restore_state_and_quota() {
        let budget = crate::budget::OperationBudget::new(16384);
        let neighbor = budget.reserve(3).unwrap();
        for source in ["<< /A null /B true >>", "<< /A null /A true >>"] {
            let mut parser = new_parser(source);
            parser.limits.max_objects = 1;
            assert!(matches!(
                parser.get_content_object_with_budget(&budget),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
            assert_eq!(parser.object_depth, 0);
            assert_eq!(budget.live_bytes(), 3);
            parser.set_pos(0).unwrap();
            parser.limits.max_objects = 2;
            let (object, lease) = parser.get_content_object_with_budget(&budget).unwrap();
            drop(object);
            drop(lease);
            assert_eq!(budget.live_bytes(), 3);
        }
        let mut parser = new_parser("<< /A << /B null >> >>");
        parser.limits.max_depth = 2;
        assert!(matches!(
            parser.get_content_object_with_budget(&budget),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(parser.object_depth, 0);
        assert_eq!(budget.live_bytes(), 3);
        parser.set_pos(0).unwrap();
        parser.limits.max_depth = 3;
        let (object, lease) = parser.get_content_object_with_budget(&budget).unwrap();
        drop(object);
        drop(lease);
        assert_eq!(budget.live_bytes(), 3);
        let mut parser = new_parser("<<>>");
        parser.limits.max_objects = 0;
        let (object, lease) = parser.get_content_object_with_budget(&budget).unwrap();
        assert_eq!(object.as_dict().unwrap().into_iter().len(), 0);
        assert_eq!(budget.live_bytes(), 3);
        drop(object);
        drop(lease);
        drop(neighbor);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn budgeted_dictionaries_preserve_nested_values_and_replacement_semantics() {
        let budget = crate::budget::OperationBudget::new(16384);
        for source in [
            "<<>>",
            "<< /A (abc) /B [12 /C] >> op",
            "<< /Z << /X true >> /A null >>",
            "<< /A (abcdefgh) /A (x) >>",
            "<< /A#23B 1 >>",
        ] {
            let mut legacy = new_parser(source);
            let expected = legacy.get_object().unwrap();
            let mut parser = new_parser(source);
            let (object, lease) = parser.get_content_object_with_budget(&budget).unwrap();
            assert_eq!(object, expected, "{source}");
            assert_eq!(parser.position(), legacy.position(), "{source}");
            if source == "<<>>" {
                assert_eq!(budget.live_bytes(), 0);
            }
            if source == "<< /A (abcdefgh) /A (x) >>" {
                assert_eq!(
                    budget.live_bytes(),
                    std::mem::size_of::<(String, crate::objects::PdfObject)>() + 2
                );
            }
            if source == "<< /A (abc) /B [12 /C] >> op" {
                assert_eq!(
                    budget.live_bytes(),
                    2 * std::mem::size_of::<(String, crate::objects::PdfObject)>()
                        + 2 * std::mem::size_of::<crate::objects::PdfObject>()
                        + 10
                );
            }
            drop(object);
            drop(lease);
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn dictionary_finalization_peak_and_parse_failures_preserve_neighbor() {
        let pair = std::mem::size_of::<(String, crate::objects::PdfObject)>();
        let entry = std::mem::size_of::<(
            String,
            crate::objects::PdfObject,
            crate::budget::MemoryLease,
            crate::budget::MemoryLease,
        )>();
        let budget = crate::budget::OperationBudget::new(entry + pair + 1);
        let neighbor = budget.reserve(1).unwrap();
        assert!(new_parser("<< /A null >>")
            .get_content_object_with_budget(&budget)
            .is_err());
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let (object, lease) = new_parser("<< /A null >>")
            .get_content_object_with_budget(&budget)
            .unwrap();
        assert_eq!(budget.live_bytes(), pair + 1);
        drop(object);
        drop(lease);
        let budget = crate::budget::OperationBudget::new(16384);
        let neighbor = budget.reserve(3).unwrap();
        for source in [
            "<< /A (abc) /B (unterminated",
            "<< /A true /#gg null >>",
            "<< /A null",
            "<< /A null /A false >>",
        ] {
            let mut parser = new_parser(source);
            parser.mode = ParseMode::Strict;
            assert!(parser.get_content_object_with_budget(&budget).is_err());
            assert_eq!(parser.object_depth, 0);
            assert_eq!(budget.live_bytes(), 3);
        }
        drop(neighbor);
    }

    #[test]
    fn budgeted_arrays_preserve_nested_payloads_and_charge_tables() {
        let budget = crate::budget::OperationBudget::new(8192);
        for source in [
            "[]",
            "[[(abc) /A#ff] 12 true null]",
            "[1 0 R [<61> -.5 false]]",
        ] {
            let mut legacy = new_parser(source);
            let expected = legacy.get_object().unwrap();
            let mut parser = new_parser(source);
            let (object, lease) = parser.get_content_object_with_budget(&budget).unwrap();
            assert_eq!(format!("{object:?}"), format!("{expected:?}"));
            assert_eq!(parser.position(), legacy.position());
            if source == "[]" {
                assert_eq!(budget.live_bytes(), 0);
            } else if source == "[[(abc) /A#ff] 12 true null]" {
                assert_eq!(
                    budget.live_bytes(),
                    6 * std::mem::size_of::<crate::objects::PdfObject>() + 12
                );
            }
            drop(object);
            drop(lease);
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn budgeted_array_growth_and_errors_release_only_owned_quota() {
        let size = std::mem::size_of::<crate::objects::PdfObject>();
        let budget = crate::budget::OperationBudget::new(6 * size);
        let neighbor = budget.reserve(1).unwrap();
        let mut parser = new_parser("[null null null]");
        assert!(parser.get_content_object_with_budget(&budget).is_err());
        assert_eq!(budget.live_bytes(), 1);
        assert_eq!(parser.object_depth, 0);
        drop(neighbor);
        parser.set_pos(0).unwrap();
        let (object, lease) = parser.get_content_object_with_budget(&budget).unwrap();
        assert_eq!(budget.live_bytes(), 4 * size);
        drop(object);
        drop(lease);
        assert_eq!(budget.live_bytes(), 0);
        let neighbor = budget.reserve(1).unwrap();
        for source in ["[(abc) [1 (unterminated]", "[1 /#gg]", "[true", "[[null]"] {
            let mut parser = new_parser(source);
            assert!(parser.get_content_object_with_budget(&budget).is_err());
            assert_eq!(parser.object_depth, 0);
            assert_eq!(budget.live_bytes(), 1);
        }
        let mut parser = new_parser("[[null]]");
        parser.limits.max_depth = 2;
        assert!(parser.get_content_object_with_budget(&budget).is_err());
        assert_eq!(parser.object_depth, 0);
        assert_eq!(budget.live_bytes(), 1);
        let mut parser = new_parser("[null null]");
        parser.limits.max_objects = 1;
        assert!(matches!(
            parser.get_content_object_with_budget(&budget),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(parser.object_depth, 0);
        assert_eq!(budget.live_bytes(), 1);
        parser.set_pos(0).unwrap();
        parser.limits.max_objects = 2;
        let (object, lease) = parser.get_content_object_with_budget(&budget).unwrap();
        assert_eq!(object.as_array().unwrap().len(), 2);
        assert_eq!(budget.live_bytes(), 1 + 2 * size);
        drop(object);
        drop(lease);
        assert_eq!(budget.live_bytes(), 1);
    }

    #[test]
    fn budgeted_numeric_objects_preserve_lexemes_references_and_position() {
        let budget = crate::budget::OperationBudget::new(4096);
        for source in [
            "1",
            "+.5 op",
            "-0 2 op",
            "12345 6 R",
            "1 65535 R",
            "0.12345678901234567890 Tj",
        ] {
            let mut legacy = new_parser(source);
            let expected = legacy.get_object().unwrap();
            let mut parser = new_parser(source);
            let (object, lease) = parser.get_number_with_budget(&budget).unwrap();
            assert_eq!(format!("{object:?}"), format!("{expected:?}"), "{source}");
            assert_eq!(parser.position(), legacy.position(), "{source}");
            if object.as_reference().is_some() {
                assert_eq!(lease.bytes(), 0);
            } else {
                let length = object.as_number().unwrap().lexeme().len();
                assert_eq!(lease.bytes(), length.next_power_of_two());
            }
            drop(object);
            drop(lease);
            assert_eq!(budget.live_bytes(), 0);
        }
        for source in ["-1 0 R", "1.5 0 R", "1 65536 R", "4294967296 0 R", "/Name"] {
            assert!(new_parser(source).get_number_with_budget(&budget).is_err());
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn numeric_lookahead_peak_rejection_preserves_neighbor() {
        let budget = crate::budget::OperationBudget::new(5);
        let neighbor = budget.reserve(1).unwrap();
        // First token, second token, and R coexist: 2 + 2 + 1 bytes.
        assert!(new_parser("12 34 R")
            .get_number_with_budget(&budget)
            .is_err());
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let (object, lease) = new_parser("12 34 R")
            .get_number_with_budget(&budget)
            .unwrap();
        assert!(object.as_reference().is_some());
        assert_eq!(budget.live_bytes(), 0);
        drop(object);
        drop(lease);
    }

    #[test]
    fn budgeted_word_growth_and_string_transfer_keep_capacity_charged() {
        let budget = crate::budget::OperationBudget::new(12);
        let neighbor = budget.reserve(1).unwrap();
        assert!(matches!(
            new_parser("abcde").get_next_word_with_budget(&budget),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let word = new_parser("abcde")
            .get_next_word_with_budget(&budget)
            .unwrap();
        assert_eq!(word.raw(), b"abcde");
        let (name, lease) = word.into_reserved_string().unwrap();
        let lease = lease.unwrap();
        assert_eq!(name, "abcde");
        assert_eq!(budget.live_bytes(), name.capacity());
        assert_eq!(lease.bytes(), name.capacity());
        drop(name);
        drop(lease);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn budgeted_word_errors_release_only_their_own_reservations() {
        use crate::{
            budget::{BudgetedBytes, OperationBudget},
            error::PdfError,
            parser::options::{Limits, ParseOptions},
        };
        let budget = OperationBudget::new(32);
        let neighbor = BudgetedBytes::copy_from_slice(&budget, b"neighbor").unwrap();
        let baseline = budget.live_bytes();
        let reader = StreamReader::try_new(Cursor::new([0xffu8, 0xfe])).unwrap();
        let mut parser = SyntaxParser::new(reader);
        let word = parser.get_next_word_with_budget(&budget).unwrap();
        assert!(budget.live_bytes() > baseline);
        assert!(matches!(
            word.into_reserved_string(),
            Err(PdfError::ParserError(_))
        ));
        assert_eq!(budget.live_bytes(), baseline);
        let reader = StreamReader::try_new(Cursor::new(b"abcd".as_slice())).unwrap();
        let options = ParseOptions {
            limits: Limits {
                max_token_bytes: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut parser = SyntaxParser::with_options(reader, options);
        assert!(matches!(
            parser.get_next_word_with_budget(&budget),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), baseline);
        assert_eq!(neighbor.as_slice(), b"neighbor");
        let word = new_parser("ok").get_next_word_with_budget(&budget).unwrap();
        drop(word);
        assert_eq!(budget.live_bytes(), baseline);
        drop(neighbor);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn budgeted_strings_do_not_allocate_for_empty_output_or_hex_whitespace() {
        let budget = crate::budget::OperationBudget::new(1);
        for source in ["()", "<>", "< \n\t >", "(\\\n)", "(\\\r)", "(\\\r\n)"] {
            let (string, lease) = new_parser(source).get_string_with_budget(&budget).unwrap();
            assert!(string.bytes().is_empty());
            assert_eq!(lease.bytes(), 0);
            assert_eq!(budget.live_bytes(), 0);
        }
        let (string, lease) = new_parser("< F >").get_string_with_budget(&budget).unwrap();
        assert_eq!(string.bytes(), [0xf0]);
        assert_eq!(lease.bytes(), 1);
        drop(string);
        drop(lease);
        assert_eq!(budget.live_bytes(), 0);
        let budget = crate::budget::OperationBudget::new(6);
        for source in ["(abcd)", "(abcd\\\n)", "(abcd\\\r\n)"] {
            let (string, lease) = new_parser(source).get_string_with_budget(&budget).unwrap();
            assert_eq!(string.bytes(), b"abcd");
            assert_eq!(lease.bytes(), 4);
            drop(string);
            drop(lease);
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn string_budget_growth_and_parse_errors_preserve_neighbor_reservation() {
        use crate::{budget::OperationBudget, error::PdfError};
        let budget = OperationBudget::new(12);
        let neighbor = budget.reserve(1).unwrap();
        for source in ["(abcde)", "<6162636465>"] {
            assert!(matches!(
                new_parser(source).get_string_with_budget(&budget),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(budget.live_bytes(), 1);
        }
        drop(neighbor);
        for source in ["(abcde)", "<6162636465>"] {
            let (string, lease) = new_parser(source).get_string_with_budget(&budget).unwrap();
            assert_eq!(string.bytes(), b"abcde");
            assert_eq!(lease.bytes(), 8);
            assert_eq!(budget.live_bytes(), 8);
            drop(string);
            drop(lease);
            assert_eq!(budget.live_bytes(), 0);
        }
        let budget = OperationBudget::new(64);
        let neighbor = budget.reserve(3).unwrap();
        for source in ["(unterminated", "<6162G>", "<6162"] {
            assert!(new_parser(source).get_string_with_budget(&budget).is_err());
            assert_eq!(budget.live_bytes(), 3);
        }
        drop(neighbor);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn numbers_at_eof_and_reference_lookahead_preserve_position() {
        for source in [
            "42",
            "42 ",
            "42 % trailing comment",
            "-1",
            "1.5",
            "4294967295",
        ] {
            for mode in [ParseMode::Strict, ParseMode::Compatible] {
                let mut parser = SyntaxParser::with_mode(
                    StreamReader::try_new(Cursor::new(source)).unwrap(),
                    mode,
                );
                let number = parser.get_object().unwrap().into_number().unwrap();
                assert_eq!(number.lexeme(), source.split_whitespace().next().unwrap());
            }
        }
        let mut parser = new_parser("12 0");
        assert_eq!(
            parser.get_object().unwrap().as_number().unwrap().lexeme(),
            "12"
        );
        assert_eq!(
            parser.get_object().unwrap().as_number().unwrap().lexeme(),
            "0"
        );
        let mut parser = new_parser("12 0 R");
        assert!(parser.get_object().unwrap().as_reference().is_some());
        assert!(new_parser("-1 0 R").get_object().is_err());
        assert!(new_parser("1.5 0 R").get_object().is_err());
    }

    #[test]
    fn test_read_name() {
        let buffer = "/Name#20One ";
        let mut parser = new_parser(buffer);
        let obj = parser.get_object().unwrap();
        let name = obj.as_name().unwrap();
        assert_eq!(name.name(), "Name One");
    }

    #[test]
    fn eof_is_not_null_and_reference_generation_is_bounded() {
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            for source in ["", " ", "% comment"] {
                let mut parser = SyntaxParser::with_mode(
                    StreamReader::try_new(Cursor::new(source)).unwrap(),
                    mode,
                );
                assert!(matches!(
                    parser.get_object(),
                    Err(crate::error::PdfError::EndofFile)
                ));
            }
            for source in ["1 65536 R", "1 4294967295 R", "1 -1 R", "1 1.5 R"] {
                let mut parser = SyntaxParser::with_mode(
                    StreamReader::try_new(Cursor::new(source)).unwrap(),
                    mode,
                );
                assert!(parser.get_object().is_err(), "{source}");
            }
            for source in ["1 0 R", "1 65535 R"] {
                let mut parser = SyntaxParser::with_mode(
                    StreamReader::try_new(Cursor::new(source)).unwrap(),
                    mode,
                );
                assert!(parser.get_object().unwrap().as_reference().is_some());
            }
            let mut parser =
                SyntaxParser::with_mode(StreamReader::try_new(Cursor::new("null")).unwrap(), mode);
            assert_eq!(
                parser.get_object().unwrap(),
                crate::objects::PdfObject::PdfNull
            );
            assert!(matches!(
                parser.get_object(),
                Err(crate::error::PdfError::EndofFile)
            ));
        }
    }

    #[test]
    fn bounded_object_parsing_restores_input_end_after_success_and_error() {
        let mut parser = new_parser("true false");
        assert!(parser.get_object_until(5).is_ok());
        assert!(matches!(
            parser.get_object().unwrap(),
            crate::objects::PdfObject::PdfBool(_)
        ));
        let mut parser = new_parser("(a) true");
        assert!(parser.get_object_until(2).is_err());
        parser.set_pos(4).unwrap();
        assert!(matches!(
            parser.get_object().unwrap(),
            crate::objects::PdfObject::PdfBool(_)
        ));
        parser.set_pos(0).unwrap();
        assert!(parser.get_object_until(0).is_err());
    }

    #[test]
    fn test_read_dict() {
        let buffer = r#"<</Type /Example
/Subtype /DictionaryExample
/W [1 2 3]
/Version 0.01
/IntegerItem 12
/StringItem (a string)
/Subdictionary << /Item1 0.4
/Item2 true
/LastItem (not!)
/VeryLastItem (OK)
>>
>>"#;
        let mut parser = new_parser(buffer);
        let obj = parser.get_object().unwrap();
        let dict = obj.as_dict().unwrap();
        assert!(dict.get("Subdictionary").is_some());
    }

    #[test]
    fn test_read_string() {
        let buffer = "(line1\\\r\nline2\\053)";
        let mut parser = new_parser(buffer);
        let obj = parser.get_object().unwrap();
        let s = obj.into_string().unwrap();
        assert_eq!(s.get_content().unwrap(), "line1line2+");
    }

    #[test]
    fn io_failures_preserve_parser_position() {
        use std::io::{self, Read, Seek, SeekFrom};
        struct FaultyReader(Cursor<Vec<u8>>);
        impl Read for FaultyReader {
            fn read(&mut self, buf: &mut [u8]) -> io::Result<usize> {
                if self.0.position() == 1 {
                    return Err(io::Error::other("injected read failure"));
                }
                // Partial reads exercise rollback after read_exact consumes a prefix.
                let count = buf.len().min(1);
                self.0.read(&mut buf[..count])
            }
        }
        impl Seek for FaultyReader {
            fn seek(&mut self, from: SeekFrom) -> io::Result<u64> {
                if from == SeekFrom::Start(99) {
                    return Err(io::Error::other("injected seek failure"));
                }
                self.0.seek(from)
            }
        }
        let mut parser = SyntaxParser::new(
            StreamReader::try_new(FaultyReader(Cursor::new(b"abc".to_vec()))).unwrap(),
        );
        assert!(parser.peek_char_at(1).is_err());
        assert_eq!(parser.position(), 0);
        assert!(parser.set_pos(99).is_err());
        assert_eq!(parser.position(), 0);
        assert!(parser.read_block(3).is_err());
        assert_eq!(parser.position(), 0);
        assert!(parser.read_line().is_err());
        assert_eq!(parser.position(), 1);
        parser.set_pos(0).unwrap();
        assert!(parser.to_next_line().is_err());
        parser.set_pos(0).unwrap();
        assert_eq!(parser.get_next_char().unwrap(), b'a');
        parser.set_pos(2).unwrap();
        assert_eq!(parser.read_block(1).unwrap(), b"c");
        assert_eq!(parser.position(), 3);
        assert!(matches!(
            parser.peek_char_at(u64::MAX),
            Err(crate::error::PdfError::EndofFile)
        ));
        let mut parser = SyntaxParser::new(
            StreamReader::try_new(FaultyReader(Cursor::new(b"\rX".to_vec()))).unwrap(),
        );
        assert!(parser.consume_stream_start().is_err());
        assert_eq!(parser.position(), 1);
        parser.set_pos(0).unwrap();
        assert!(parser.consume_optional_single_eol().is_err());
        assert_eq!(parser.position(), 1);
    }

    #[test]
    fn marker_search_restores_position() {
        for source in ["data endobj", "data endst", "data", ""] {
            let mut parser = new_parser(source);
            assert_eq!(parser.find_word_pos(b"endstream").unwrap(), None);
            assert_eq!(parser.position(), 0);
            assert_eq!(
                parser.find_word_pos(b"endobj").unwrap(),
                source.find("endobj").map(|p| p as u64)
            );
            assert_eq!(parser.position(), 0);
        }
    }

    #[test]
    fn line_reading_preserves_byte_after_single_cr() {
        for source in ["a\rb", "a\r\nb", "a\nb"] {
            let mut parser = new_parser(source);
            assert_eq!(parser.read_line().unwrap(), b"a");
            assert_eq!(parser.get_next_char().unwrap(), b'b');
            parser.set_pos(0).unwrap();
            parser.to_next_line().unwrap();
            assert_eq!(parser.get_next_char().unwrap(), b'b');
        }
        for source in ["a", "a\r", "a\n", "a\r\n"] {
            let mut parser = new_parser(source);
            assert_eq!(parser.read_line().unwrap(), b"a");
            assert!(parser.is_eof());
        }
    }

    #[test]
    fn eol_marker_handles_single_byte_at_eof() {
        for (source, expected) in [("\r", 1), ("\n", 1), ("\r\n", 2), ("x", 0)] {
            assert_eq!(new_parser(source).read_eol_marker(0).unwrap(), expected);
        }
        assert!(new_parser("x").read_eol_marker(u64::MAX).is_err());
    }

    #[test]
    fn literal_string_octal_and_line_endings() {
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            for (source, expected) in [
                (
                    r"(\777\400\377\0053\53\7\8)",
                    &[255, 0, 255, 5, b'3', b'+', 7, b'8'][..],
                ),
                ("(a\rb\r\nc\nd)", &b"a\nb\nc\nd"[..]),
                ("(a\\\rb\\\r\nc\\\nd)", &b"abcd"[..]),
                (r"(a\rb\nc)", &b"a\rb\nc"[..]),
                (r"(a(b\)c)d)", &b"a(b)c)d"[..]),
            ] {
                let mut parser = SyntaxParser::with_mode(
                    StreamReader::try_new(Cursor::new(source)).unwrap(),
                    mode,
                );
                assert_eq!(
                    parser.get_object().unwrap().into_string().unwrap().bytes(),
                    expected
                );
            }
            for source in ["(", "(abc\\", "(abc\\\r", "(a(b)"] {
                let mut parser = SyntaxParser::with_mode(
                    StreamReader::try_new(Cursor::new(source)).unwrap(),
                    mode,
                );
                assert!(parser.get_object().is_err());
            }
        }
    }

    #[test]
    fn test_read_hex_string() {
        let buffer = "<48 65 6c6c6f2>";
        let mut parser = new_parser(buffer);
        let obj = parser.get_object().unwrap();
        let s = obj.into_string().unwrap();
        assert_eq!(s.raw_bytes(), b"Hello ".to_vec());
    }

    #[test]
    fn test_get_next_word() {
        let buffer = "[this is a single [array] <<dict>>]";
        let mut parser = new_parser(buffer);
        assert_eq!(parser.get_next_word().unwrap().raw(), b"[");
        assert_eq!(parser.get_next_word().unwrap().raw(), b"this");
        assert_eq!(parser.get_next_word().unwrap().raw(), b"is");
    }

    #[test]
    fn test_number_grammar() {
        let buffer = "+12 -.5 12. abc";
        let mut parser = new_parser(buffer);
        assert!(parser.get_next_word().unwrap().is_number());
        assert!(parser.get_next_word().unwrap().is_number());
        assert!(parser.get_next_word().unwrap().is_number());
        assert!(!parser.get_next_word().unwrap().is_number());
    }

    #[test]
    fn test_header_read() {
        let buffer = "%PDF-1.7\r\n%\u{80}\u{80}\u{80}\u{80}\r\n";
        let inner = Cursor::new(buffer.as_bytes());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut parser = SyntaxParser::with_mode(reader, ParseMode::Strict);
        let header = parser.read_header().unwrap();
        assert_eq!(header.version(), "1.7");
    }

    #[test]
    fn test_strict_header_requires_binary_comment_line() {
        let buffer = "%PDF-1.7\r\n%abc\r\n";
        let inner = Cursor::new(buffer.as_bytes());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut parser = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(parser.read_header().is_err());
    }

    #[test]
    fn test_strict_duplicate_dict_key_rejected() {
        let buffer = "<< /Type /A /Type /B >>";
        let inner = Cursor::new(buffer.as_bytes());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut parser = SyntaxParser::with_mode(reader, ParseMode::Strict);
        assert!(parser.get_object().is_err());
    }

    #[test]
    fn test_name_preserves_raw_bytes() {
        let buffer = "/A#ffB";
        let mut parser = new_parser(buffer);
        let obj = parser.get_object().unwrap();
        let name = obj.as_name().unwrap();
        assert_eq!(name.bytes(), b"A\xffB");
    }

    #[test]
    fn binary_name_bytes_are_not_whitespace() {
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let reader = StreamReader::try_new(Cursor::new(b"/A\x80\xffB /Next")).unwrap();
            let mut parser = SyntaxParser::with_mode(reader, mode);
            assert_eq!(
                parser.get_object().unwrap().as_name().unwrap().bytes(),
                b"A\x80\xffB"
            );
            assert_eq!(
                parser.get_object().unwrap().as_name().unwrap().bytes(),
                b"Next"
            );
            let reader = StreamReader::try_new(Cursor::new(b"<ab\xffcd>")).unwrap();
            assert!(SyntaxParser::with_mode(reader, mode).get_object().is_err());
        }
    }

    #[test]
    fn truncated_name_escapes_fail_in_names_and_dictionary_keys() {
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            for source in ["/A#", "/A#0 ", "<< /A# 1 >>", "<< /A#0 1 >>"] {
                let reader = StreamReader::try_new(Cursor::new(source)).unwrap();
                assert!(
                    SyntaxParser::with_mode(reader, mode).get_object().is_err(),
                    "{source}"
                );
            }
        }
    }

    #[test]
    fn indirect_length_budget_restores_position_and_releases_temporary_objects() {
        let mut dict = crate::objects::pdf_dict::PdfDict::default();
        dict.insert(
            "Length".into(),
            crate::objects::PdfObject::PdfReference(
                crate::objects::pdf_reference::PdfReference::new(1, 0),
            ),
        );
        let budget = crate::budget::OperationBudget::new(1024);
        let neighbor = budget.reserve(3).unwrap();
        for source in [
            "1 0 obj 5 endobj",
            "2 0 obj 5 endobj",
            "1 0 obj (abc) endobj",
            "1 0 obj 5 wrong",
            "1 0 obj (unterminated",
        ] {
            let xref = CrossRefTable::new(
                std::collections::HashMap::from([(
                    1,
                    ObjectInfo::new(1, 0, 0, ObjectState::Normal),
                )]),
                Default::default(),
            );
            let mut parser = new_parser(source).with_xref_table(xref);
            parser.set_pos(2).unwrap();
            let result = parser.resolve_stream_length(&dict, Some(&budget));
            if source == "1 0 obj 5 endobj" {
                assert_eq!(result.unwrap(), Some(5));
            } else {
                assert!(result.is_err());
            }
            assert_eq!(parser.position(), 2);
            assert_eq!(parser.object_depth, 0);
            assert_eq!(budget.live_bytes(), 3);
        }
        let xref = CrossRefTable::new(
            std::collections::HashMap::from([(1, ObjectInfo::new(1, 0, 0, ObjectState::Normal))]),
            Default::default(),
        );
        let mut parser = new_parser("1 0 obj 5 endobj").with_xref_table(xref);
        parser.set_pos(2).unwrap();
        let small = crate::budget::OperationBudget::new(1);
        assert!(parser.resolve_stream_length(&dict, Some(&small)).is_err());
        assert_eq!(parser.position(), 2);
        assert_eq!(small.live_bytes(), 0);
        drop(neighbor);
    }

    #[test]
    fn test_stream_length_indirect_reference() {
        let buffer = b"<< /Length 1 0 R >>\nstream\nabcde\nendstream\n1 0 obj\n5\nendobj\n";
        let object1_offset = buffer
            .windows(b"1 0 obj".len())
            .position(|slice| slice == b"1 0 obj")
            .unwrap() as u64;

        let inner = Cursor::new(buffer.to_vec());
        let reader = StreamReader::try_new(inner).unwrap();
        let mut xref = CrossRefTable::new_empty();
        xref.merge(CrossRefTable::new(
            std::collections::HashMap::from([(
                1,
                ObjectInfo::new(1, object1_offset, 0, ObjectState::Normal),
            )]),
            Default::default(),
        ));
        let mut parser = SyntaxParser::new(reader).with_xref_table(xref);
        let stream = parser.get_object().unwrap().into_stream().unwrap();
        assert_eq!(stream.raw_data(), b"abcde");
        parser.set_pos(0).unwrap();
        let budget = crate::budget::OperationBudget::new(4096);
        let (object, lease) = parser.get_content_object_with_budget(&budget).unwrap();
        assert_eq!(object.as_stream().unwrap(), &stream);
        assert_eq!(
            budget.live_bytes(),
            std::mem::size_of::<(String, crate::objects::PdfObject)>() + 6 + 5
        );
        drop(object);
        drop(lease);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn invalid_indirect_length_restores_stream_position() {
        let source = "<< /Length 1 0 R >> trailing 1 0 obj (unterminated";
        let offset = source.find("1 0 obj").unwrap() as u64;
        let xref = CrossRefTable::new(
            std::collections::HashMap::from([(
                1,
                ObjectInfo::new(1, offset, 0, ObjectState::Normal),
            )]),
            Default::default(),
        );
        let mut parser = new_parser(source).with_xref_table(xref);
        let dict = parser.get_object().unwrap().into_dict().unwrap();
        let saved = parser.position();
        assert!(parser.resolve_stream_length(&dict, None).is_err());
        assert_eq!(parser.position(), saved);
        assert_eq!(parser.get_next_word().unwrap().raw(), b"trailing");
    }

    #[test]
    fn stream_fallback_restarts_at_data_and_trims_only_adjacent_eol() {
        for length in ["", "/Length 1", "/Length 2", "/Length 999"] {
            for separator in ["\r", "\n", "\r\n", ""] {
                let source = format!("<< {length} >> stream\nabc{separator}endstream");
                let stream = new_parser(&source)
                    .get_object()
                    .unwrap()
                    .into_stream()
                    .unwrap();
                assert_eq!(stream.raw_data(), b"abc", "{source:?}");
            }
        }
        let source = "<< >> stream\n\nabc\nendstream";
        assert_eq!(
            new_parser(source)
                .get_object()
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"\nabc"
        );
        let source = "<< /Length 4 >> stream\nabc\rendstream";
        assert_eq!(
            new_parser(source)
                .get_object()
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"abc\r"
        );
    }
}
