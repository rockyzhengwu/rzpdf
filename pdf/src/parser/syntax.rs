use std::io::{Read, Seek};

use crate::{
    document::cross_ref_table::{CrossRefTable, ObjectState},
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_bool::PdfBool, pdf_dict::PdfDict,
        pdf_indirect::PdfIndirect, pdf_name::PdfName, pdf_number::PdfNumber,
        pdf_reference::PdfReference, pdf_stream::PdfStream, pdf_string::PdfString,
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
}

impl PdfWord {
    pub fn into_string(self) -> PdfResult<String> {
        String::from_utf8(self.raw)
            .map_err(|e| PdfError::ParserError(format!("PdfWord is not a utf8 string:{:?}", e)))
    }

    pub fn add_char(&mut self, c: u8) {
        self.raw.push(c);
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
        u32::try_from(value).map_err(|_| {
            PdfError::ParserError(format!("PdfWord out of range for u32: {:?}", self.raw))
        })
    }

    pub fn as_u64_checked(&self) -> PdfResult<u64> {
        if !self.is_number {
            return Err(PdfError::ParserError("PdfWord is not number".to_string()));
        }
        let s = std::str::from_utf8(self.raw.as_slice())
            .map_err(|e| PdfError::ParserError(format!("PdfWord is not utf8:{e:?}")))?;
        s.parse::<u64>()
            .map_err(|_| PdfError::ParserError(format!("PdfWord is not u64: {:?}", self.raw)))
    }

    pub fn as_u16_checked(&self) -> PdfResult<u16> {
        let value = self.as_u64_checked()?;
        u16::try_from(value).map_err(|_| {
            PdfError::ParserError(format!("PdfWord out of range for u16: {:?}", self.raw))
        })
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
}

impl<R: Seek + Read> SyntaxParser<R> {
    pub fn is_eof(&self) -> bool {
        self.pos >= self.reader.length()
    }

    pub fn new(reader: StreamReader<R>) -> Self {
        Self::with_mode(reader, ParseMode::Compatible)
    }

    pub fn with_mode(reader: StreamReader<R>, mode: ParseMode) -> Self {
        Self {
            reader,
            pos: 0,
            mode,
            header: None,
            xref_table: None,
        }
    }

    pub fn with_xref_table(mut self, xref_table: CrossRefTable) -> Self {
        self.xref_table = Some(xref_table);
        self
    }

    pub fn position(&self) -> u64 {
        self.pos
    }

    pub fn mode(&self) -> ParseMode {
        self.mode
    }

    pub fn read_header(&mut self) -> PdfResult<&PdfHeader> {
        if self.header.is_none() {
            let saved_pos = self.pos;
            self.set_pos(0)?;
            let line = self.read_line()?;
            if self.mode == ParseMode::Strict {
                let binary_line = self.read_line()?;
                validate_binary_comment_line(&binary_line)?;
            }
            self.set_pos(saved_pos)?;
            self.header = Some(parse_pdf_header(&line)?);
        }
        self.header
            .as_ref()
            .ok_or(PdfError::ParserError("pdf header was not parsed".to_string()))
    }

    pub fn get_indirect_object(&mut self) -> PdfResult<PdfIndirect> {
        let saved_pos = self.pos;
        let obj_word = self.get_next_word()?;
        if !obj_word.is_number() || obj_word.is_empty() {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "get_indirect_object filed get objnum object".to_string(),
            ));
        }
        let objnum = obj_word.as_u32_checked()?;
        let gen_word = self.get_next_word()?;
        if !gen_word.is_number() || gen_word.is_empty() {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "get_indirect_object failed get obj gennum".to_string(),
            ));
        }

        let gennum = gen_word.as_u16_checked()?;

        let obj_word = self.get_next_word()?;
        if !obj_word.is_equal(b"obj") {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "get_indirect_object obj keyword is expected".to_string(),
            ));
        }
        let obj = self.get_object()?;
        let end_word = self.get_next_word()?;
        if !end_word.is_equal(b"endobj") {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "get_indirect_object endobj keyword is expected".to_string(),
            ));
        }
        Ok(PdfIndirect::new(objnum, gennum, obj))
    }

    pub fn get_next_char(&mut self) -> PdfResult<u8> {
        if self.pos >= self.reader.length() {
            return Err(PdfError::EndofFile);
        }
        let byte = self.reader.read_byte()?;
        self.pos += 1;
        Ok(byte)
    }

    fn peek_char_at(&mut self, pos: u64) -> PdfResult<u8> {
        let saved_pos = self.pos;
        self.reader.seek(pos)?;
        let c = self.reader.read_byte()?;
        self.reader.seek(saved_pos)?;
        Ok(c)
    }

    pub fn get_object(&mut self) -> PdfResult<PdfObject> {
        if self.is_eof() {
            return Ok(PdfObject::PdfNull);
        }
        let word = self.get_next_word()?;
        if word.is_number() {
            let saved_pos = self.pos;
            let next = self.get_next_word()?;
            if next.is_number() {
                match self.get_next_word() {
                    Ok(next2) if next2.is_equal(b"R") => {
                        return Ok(PdfObject::PdfReference(PdfReference::new(
                            word.as_u32_checked()?,
                            next.as_u32_checked()?,
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
                return Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(word.raw())?));
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
                array.add_obj(self.get_object()?);
            }
            return Ok(PdfObject::PdfArray(array));
        }
        if word.start_width(b"/") {
            return Ok(PdfObject::PdfName(PdfName::new_from_buffer(word.raw())?));
        }
        if word.is_equal(b"<<") {
            let mut dict = PdfDict::default();
            loop {
                let saved_pos = self.pos;
                let next_word = self.get_next_word()?;
                if next_word.is_equal(b">>") {
                    break;
                }
                if next_word.is_equal(b"endobj") {
                    self.set_pos(saved_pos)?;
                }
                if !next_word.start_width(b"/") {
                    return Err(PdfError::ParserError(format!(
                        "dictionary key must be a name, got {:?}",
                        next_word.raw()
                    )));
                }
                let name = PdfName::new_from_buffer(next_word.raw())?;
                let key = name.name().to_string();
                if self.mode == ParseMode::Strict && dict.contains_key(&key) {
                    return Err(PdfError::ParserError(format!(
                        "duplicate dictionary key is not allowed in strict mode: {key}"
                    )));
                }
                let value = self.get_object()?;
                dict.insert(key, value);
            }
            let saved_pos = self.pos;
            if self.is_eof() {
                return Ok(PdfObject::PdfDict(dict));
            }
            let next_word = self.get_next_word()?;
            if !next_word.is_equal(b"stream") {
                self.set_pos(saved_pos)?;
                return Ok(PdfObject::PdfDict(dict));
            }
            return Ok(PdfObject::PdfStream(self.read_pdf_stream(dict)?));
        }
        Err(PdfError::ParserError(format!(
            "get object invalid word :{:?}",
            word
        )))
    }

    fn read_pdf_stream(&mut self, dict: PdfDict) -> PdfResult<PdfStream> {
        let saved_pos = self.pos;
        self.consume_stream_start()?;

        if let Some(data_len) = self.resolve_stream_length(&dict)? {
            let start_pos = self.pos;
            if data_len + start_pos <= self.reader.length() {
                let data = self.read_block(data_len)?;
                if self.expect_endstream_after_data()? {
                    return Ok(PdfStream::new(dict, data));
                }
            }
        }

        if self.mode == ParseMode::Strict {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "strict mode requires a valid stream Length and matching endstream".to_string(),
            ));
        }

        let end_of_stream = self.find_stream_end_pos()?;
        if let Some(end_pos) = end_of_stream {
            let data_len = end_pos.saturating_sub(self.pos);
            let data = self.read_block(data_len)?;
            return Ok(PdfStream::new(dict, data));
        }
        self.set_pos(saved_pos)?;
        Err(PdfError::ParserError(
            "stream dict has no Length and endstream could not be found".to_string(),
        ))
    }

    fn resolve_stream_length(&mut self, dict: &PdfDict) -> PdfResult<Option<u64>> {
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
                let indirect = self.get_indirect_object()?;
                self.set_pos(saved_pos)?;
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
                if let Ok(nch) = self.peek_char_at(self.pos) {
                    if nch == b'\n' {
                        let _ = self.get_next_char()?;
                    }
                }
                Ok(())
            }
            _ if self.mode == ParseMode::Compatible => {
                self.set_pos(self.pos - 1)?;
                self.to_next_line()
            }
            _ => Err(PdfError::ParserError(
                "stream keyword must be followed by an end-of-line marker".to_string(),
            )),
        }
    }

    fn expect_endstream_after_data(&mut self) -> PdfResult<bool> {
        let saved_pos = self.pos;
        self.consume_optional_single_eol()?;
        let next_word = self.get_next_word()?;
        if next_word.is_equal(b"endstream") {
            return Ok(true);
        }
        self.set_pos(saved_pos)?;
        let next_word = self.get_next_word()?;
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
                if let Ok(nch) = self.peek_char_at(self.pos) {
                    if nch == b'\n' {
                        let _ = self.get_next_char()?;
                    }
                }
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
            (Some(end_s), Some(_)) => Some(end_s),
        };
        if let Some(end_stream_pos) = end_pos {
            let marker_len = self.read_eol_marker(end_stream_pos.saturating_sub(2))?;
            return Ok(Some(end_stream_pos.saturating_sub(marker_len)));
        }
        Ok(None)
    }

    fn find_word_pos(&mut self, tag: &[u8]) -> PdfResult<Option<u64>> {
        let saved_pos = self.pos;
        let tag_len = tag.len();
        loop {
            let mut match_found = true;
            let start_pos = self.pos;
            let mut i = 0;
            while i < tag_len {
                if let Ok(ch) = self.get_next_char() {
                    if ch != tag[i] {
                        match_found = false;
                        break;
                    }
                    i += 1;
                } else {
                    return Ok(None);
                }
            }
            if match_found {
                self.set_pos(saved_pos)?;
                return Ok(Some(start_pos));
            }
            self.set_pos(start_pos + 1)?;
        }
    }

    pub fn set_pos(&mut self, pos: u64) -> PdfResult<()> {
        self.pos = pos;
        self.reader.seek(pos)
    }

    fn read_block(&mut self, len: u64) -> PdfResult<Vec<u8>> {
        if self.pos + len > self.reader.length() {
            return Err(PdfError::ParserError("invalid length of block".to_string()));
        }
        self.pos += len;
        self.reader.read_bytes(len as usize)
    }

    fn to_next_line(&mut self) -> PdfResult<()> {
        while let Ok(ch) = self.get_next_char() {
            if ch == b'\n' {
                break;
            }
            if ch == b'\r' {
                if let Ok(nch) = self.get_next_char() {
                    if nch != b'\n' {
                        self.pos -= 1;
                    }
                }
                break;
            }
        }
        Ok(())
    }

    pub fn read_line(&mut self) -> PdfResult<Vec<u8>> {
        let mut line = Vec::new();
        while let Ok(ch) = self.get_next_char() {
            if is_end_of_line(ch) {
                if ch == b'\r' {
                    if let Ok(next) = self.peek_char_at(self.pos) {
                        if next == b'\n' {
                            let _ = self.get_next_char()?;
                        }
                    }
                }
                break;
            }
            line.push(ch);
        }
        Ok(line)
    }

    fn read_hex_string(&mut self) -> PdfResult<PdfString> {
        let mut bytes = Vec::new();
        let mut high_nibble = None;
        loop {
            let ch = self.get_next_char()?;
            match ch {
                b'>' => {
                    if let Some(hi) = high_nibble.take() {
                        bytes.push(hi << 4);
                    }
                    break;
                }
                _ if is_white_space(ch) => continue,
                _ if ch.is_ascii_hexdigit() => {
                    let nibble = hex_to_u8(ch);
                    if let Some(hi) = high_nibble.take() {
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
        let mut nest_level = 0;
        let mut bytes = Vec::new();
        loop {
            let ch = self.get_next_char()?;
            match ch {
                b'(' => {
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
                    match escaped {
                        b'n' => bytes.push(b'\n'),
                        b'r' => bytes.push(b'\r'),
                        b't' => bytes.push(b'\t'),
                        b'b' => bytes.push(8),
                        b'f' => bytes.push(12),
                        b'(' | b')' | b'\\' => bytes.push(escaped),
                        b'\n' => {}
                        b'\r' => {
                            if let Ok(next) = self.peek_char_at(self.pos) {
                                if next == b'\n' {
                                    let _ = self.get_next_char()?;
                                }
                            }
                        }
                        b'0'..=b'7' => {
                            let mut octal = String::from(escaped as char);
                            for _ in 0..2 {
                                match self.peek_char_at(self.pos) {
                                    Ok(next @ b'0'..=b'7') => {
                                        let _ = self.get_next_char()?;
                                        octal.push(next as char);
                                    }
                                    _ => break,
                                }
                            }
                            let value = u8::from_str_radix(octal.as_str(), 8).map_err(|e| {
                                PdfError::ParserError(format!(
                                    "invalid octal escape in string: {e:?}"
                                ))
                            })?;
                            bytes.push(value);
                        }
                        _ => bytes.push(escaped),
                    }
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
        let byte2 = self.peek_char_at(pos + 1)?;
        if byte1 == b'\r' && byte2 == b'\n' {
            return Ok(2);
        }
        if byte1 == b'\r' || byte1 == b'\n' {
            return Ok(1);
        }
        Ok(0)
    }

    pub fn get_next_word(&mut self) -> PdfResult<PdfWord> {
        self.to_next_word()?;
        let mut word = PdfWord::default();
        let mut c = self.get_next_char()?;
        if is_delimiter(c) {
            word.add_char(c);
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
                    word.add_char(c);
                }
            } else if c == b'<' {
                c = self.get_next_char()?;
                if c == b'<' {
                    word.add_char(c);
                } else {
                    self.set_pos(self.pos - 1)?;
                }
            } else if c == b'>' {
                c = self.get_next_char()?;
                if c == b'>' {
                    word.add_char(c);
                } else {
                    self.set_pos(self.pos - 1)?;
                }
            }
            return Ok(word);
        }
        loop {
            word.add_char(c);
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
            loop {
                c = self.get_next_char()?;
                if is_end_of_line(c) {
                    break;
                }
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
    if !matches!(version, "1.0" | "1.1" | "1.2" | "1.3" | "1.4" | "1.5" | "1.6" | "1.7" | "2.0")
    {
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

fn is_number_token(buf: &[u8]) -> bool {
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
    fn test_read_name() {
        let buffer = "/Name#20One ";
        let mut parser = new_parser(buffer);
        let obj = parser.get_object().unwrap();
        let name = obj.as_name().unwrap();
        assert_eq!(name.name(), "Name One");
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
    }
}
