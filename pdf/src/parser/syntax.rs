use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_bool::PdfBool, pdf_dict::PdfDict,
        pdf_indirect::PdfIndirect, pdf_name::PdfName, pdf_number::PdfNumber,
        pdf_reference::PdfReference, pdf_stream::PdfStream, pdf_string::PdfString,
    },
    parser::{
        character::{is_delimiter, is_end_of_line, is_number, is_regular, is_white_space},
        parse_utility::{hex_to_u8, real_from_buffer},
    },
};

enum StringStatus {
    Normal,
    Backslash,
    Octal,
    FinishOctal,
    CarriageReturn,
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
        if !self.is_number {
            panic!("PdfWord is not number")
        }
        let mut n: u32 = 0;
        for c in self.raw.iter() {
            if !matches!(c, b'0'..=b'9') {
                panic!("pdfWord is not u32");
            }
            n = n * 10 + (c - 48) as u32;
        }
        n
    }
    pub fn as_u64(&self) -> u64 {
        if !self.is_number {
            panic!("PdfWord is not number")
        }
        let mut n: u64 = 0;
        for c in self.raw.iter() {
            if !matches!(c, b'0'..=b'9') {
                panic!("pdfWord is not u64");
            }
            n = n * 10 + (c - 48) as u64;
        }
        n
    }

    pub fn as_u16(&self) -> u16 {
        if !self.is_number {
            panic!("PdfWord is not number")
        }
        let mut n: u16 = 0;
        for c in self.raw.iter() {
            if !matches!(c, b'0'..=b'9') {
                panic!("pdfWord is not u16");
            }
            n = n * 10 + ((c - 48) as u16);
        }
        n
    }

    pub fn as_f32(&self) -> f32 {
        if !self.is_number {
            panic!("PdfWord is not number")
        }
        real_from_buffer(self.raw.as_slice())
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
}

impl<R: Seek + Read> SyntaxParser<R> {
    pub fn is_eof(&self) -> bool {
        self.pos >= self.reader.length()
    }

    pub fn new(reader: StreamReader<R>) -> Self {
        SyntaxParser { reader, pos: 0 }
    }

    pub fn position(&self) -> u64 {
        self.pos
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
        let objnum = obj_word.as_u32();
        let gen_word = self.get_next_word()?;
        if !gen_word.is_number() || gen_word.is_empty() {
            self.set_pos(saved_pos)?;
            return Err(PdfError::ParserError(
                "get_indirect_object failed get obj gennum".to_string(),
            ));
        }

        let gennum = gen_word.as_u16();

        let obj_word = self.get_next_word()?;
        if !obj_word.is_equal(b"obj") {
            return Err(PdfError::ParserError(
                "get_indirect_object obj keyword is expected".to_string(),
            ));
        }
        let obj = self.get_object()?;
        let indirect = PdfIndirect::new(objnum, gennum, obj);
        Ok(indirect)
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
            let next2 = self.get_next_word()?;
            if next.is_number() && next2.is_equal(b"R") {
                return Ok(PdfObject::PdfReference(PdfReference::new(
                    word.as_u32(),
                    next.as_u32(),
                )));
            } else {
                self.set_pos(saved_pos)?;
                let number = PdfNumber::new(word.as_f32());
                return Ok(PdfObject::PdfNumber(number));
            }
        }
        if word.is_equal(b"true") || word.is_equal(b"false") {
            let bool = PdfObject::PdfBool(PdfBool::new(word.raw()));
            return Ok(bool);
        }
        if word.is_equal(b"null") {
            return Ok(PdfObject::PdfNull);
        }
        if word.is_equal(b"(") {
            let s = self.read_string()?;
            return Ok(PdfObject::PdfString(s));
        }
        if word.is_equal(b"<") {
            let s = self.read_hex_string()?;
            return Ok(PdfObject::PdfString(s));
        }
        if word.is_equal(b"[") {
            let mut array = PdfArray::default();
            loop {
                let next_word = self.peek_word()?;
                if next_word.is_equal(b"]") {
                    self.get_next_word()?;
                    break;
                } else {
                    let obj = self.get_object()?;
                    array.add_obj(obj);
                }
            }
            return Ok(PdfObject::PdfArray(array));
        }

        if word.start_width(b"/") {
            let name = PdfName::new_from_buffer(&word.raw);
            return Ok(PdfObject::PdfName(name));
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
                    continue;
                }
                let name = PdfName::new_from_buffer(&next_word.raw);
                let key = name.name().to_string();
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
            let stream = self.read_pdf_stream(dict)?;
            return Ok(PdfObject::PdfStream(stream));
        }
        return Err(PdfError::ParserError(format!(
            "get object invalid word :{:?}",
            word
        )));
    }

    fn read_pdf_stream(&mut self, dict: PdfDict) -> PdfResult<PdfStream> {
        let saved_pos = self.pos;
        if let Some(len_obj) = dict.get("Length") {
            match len_obj {
                PdfObject::PdfNumber(len_num) => {
                    let data_len = len_num.get_u64();
                    self.to_next_line()?;
                    let start_pos = self.pos;
                    if data_len + start_pos < self.reader.length() {
                        // invalid lenght
                        let data = self.read_block(data_len)?;
                        self.to_next_line()?;
                        let next_word = self.get_next_word()?;
                        if next_word.is_equal(b"endstream") {
                            let stream = PdfStream::new(dict, data);
                            return Ok(stream);
                        }
                    }
                }
                _ => {
                    //panic!("stream length need to be number or reference");
                }
            }
        }
        self.to_next_line()?;
        let end_of_stream = self.find_stream_end_pos()?;
        if let Some(end_pos) = end_of_stream {
            let data_len = end_pos - saved_pos;
            let data = self.read_block(data_len)?;
            let stream = PdfStream::new(dict, data);
            return Ok(stream);
        } else {
            panic!("Stream dict has no Length and can't found the end of stream");
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
            let marker_len = self.read_eol_marker(end_stream_pos - 2)?;
            return Ok(Some(end_stream_pos - marker_len));
        } else {
            Ok(None)
        }
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
                    } else {
                        i += 1;
                    }
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
            return Err(PdfError::ParserError(format!("invalid length of block")));
        }
        self.pos += len as u64;
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

    fn read_hex_string(&mut self) -> PdfResult<PdfString> {
        let mut ch = self.get_next_char()?;
        let mut bytes = Vec::new();
        let mut code = 0;
        let mut is_first = true;
        loop {
            if ch == b'>' {
                break;
            }
            if ch.is_ascii_hexdigit() {
                let val = hex_to_u8(ch);
                if is_first {
                    code = val * 16;
                } else {
                    code = code + val;
                    bytes.push(code);
                }
                is_first = !is_first;
            } else {
                bytes.push(ch.to_owned());
            }
            ch = self.get_next_char()?
        }
        Ok(PdfString::new(bytes, true))
    }

    fn read_string(&mut self) -> PdfResult<PdfString> {
        let mut nest_level: i32 = 0;
        let mut status: StringStatus = StringStatus::Normal;
        let mut ch = self.get_next_char()?;
        let mut bytes = Vec::new();
        let mut esc_octal = String::new();
        loop {
            match status {
                StringStatus::Normal => match ch {
                    b'(' => {
                        bytes.push(ch.to_owned());
                        nest_level += 1;
                    }
                    b')' => {
                        if nest_level == 0 {
                            return Ok(PdfString::new(bytes, false));
                        }
                        bytes.push(ch.to_owned());
                        nest_level -= 1;
                    }
                    b'\\' => {
                        status = StringStatus::Backslash;
                    }
                    _ => {
                        bytes.push(ch.to_owned());
                    }
                },
                StringStatus::Backslash => {
                    match ch {
                        b'0'..=b'7' => {
                            status = StringStatus::Octal;
                            esc_octal.push(ch.to_owned() as char);
                        }
                        b'\r' => status = StringStatus::CarriageReturn,
                        b'n' => {
                            status = StringStatus::Normal;
                            bytes.push(b'\n')
                        }
                        b'r' => {
                            status = StringStatus::Normal;
                            bytes.push(b'\r')
                        }
                        b't' => {
                            status = StringStatus::Normal;
                            bytes.push(b'\t')
                        }
                        b'b' => {
                            status = StringStatus::Normal;
                            bytes.push(8)
                        }
                        b'f' => {
                            status = StringStatus::Normal;
                            bytes.push(12)
                        }
                        b'\n' => {
                            status = StringStatus::Normal;
                        } //donothing
                        _ => {
                            status = StringStatus::Normal;
                            bytes.push(ch.to_owned())
                        }
                    }
                }
                StringStatus::Octal => match ch {
                    b'0'..=b'7' => {
                        esc_octal.push(ch.to_owned() as char);
                        status = StringStatus::FinishOctal;
                    }
                    _ => {
                        let v = u8::from_str_radix(esc_octal.as_str(), 8).unwrap();
                        bytes.push(v);
                        esc_octal.clear();
                        status = StringStatus::Normal;
                    }
                },
                StringStatus::FinishOctal => {
                    status = StringStatus::Normal;
                    match ch {
                        b'0'..=b'7' => {
                            esc_octal.push(ch.to_owned() as char);
                            let v = u8::from_str_radix(esc_octal.as_str(), 8).unwrap();
                            esc_octal.clear();
                            bytes.push(v);
                        }
                        _ => {
                            let v = u8::from_str_radix(esc_octal.as_str(), 8).unwrap();
                            esc_octal.clear();
                            bytes.push(v);
                        }
                    }
                }
                StringStatus::CarriageReturn => {
                    status = StringStatus::Normal;
                    if ch != b'\n' {
                        continue;
                    }
                }
            }
            ch = self.get_next_char()?;
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
        if byte1 == b'\r' || byte2 == b'\n' {
            return Ok(1);
        }
        Ok(0)
    }

    pub fn get_next_word(&mut self) -> PdfResult<PdfWord> {
        self.to_next_word()?;
        let mut word = PdfWord::default();
        let mut c = self.get_next_char()?;
        if is_delimiter(c) {
            word.is_number = false;
            word.add_char(c);
            if c == b'/' {
                loop {
                    c = self.get_next_char()?;
                    if !is_regular(c) && !is_number(c) {
                        self.set_pos(self.pos - 1)?;
                        return Ok(word);
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
            if is_number(c) {
                word.is_number = true;
            }
            c = self.get_next_char()?;
            if is_delimiter(c) || is_white_space(c) {
                self.set_pos(self.pos - 1)?;
                break;
            }
        }
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

#[cfg(test)]
mod tests {
    use std::io::Cursor;

    use crate::{io::stream_reader::StreamReader, parser::syntax::SyntaxParser};
    fn new_parser(buffer: &str) -> SyntaxParser<Cursor<&str>> {
        let inner = Cursor::new(buffer);
        let reader = StreamReader::try_new(inner).unwrap();
        SyntaxParser::new(reader)
    }

    #[test]
    fn test_read_name() {
        let buffer = "/Name1 ";
        let mut parser = new_parser(buffer);
        let obj = parser.get_object().unwrap();
        let name = obj.as_name().unwrap();
        assert_eq!(name.name(), "Name1");
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
        // need be dict
        let dict = obj.as_dict().unwrap();
        println!("{:?}", dict);
        let w = dict.get("W");
        println!("{:?}", w);
    }
    #[test]
    fn test_read_string() {
        let buffer = r#"(Strings may contain balanced parentheses() and
special characters(*!&}^% and so on).)"#;
        let mut parser = new_parser(buffer);
        let obj = parser.get_object().unwrap();
        let s = obj.into_string().unwrap();
        println!("{:?}", s.get_content());
    }
    #[test]
    fn test_get_next_word() {
        let buffer = "[this is a single [array] <<dict>>]";
        let mut parser = new_parser(buffer);
        let word = parser.get_next_word().unwrap();
        println!("{:?}", word);
        let word = parser.get_next_word().unwrap();
        println!("{:?}", word);

        let word = parser.get_next_word().unwrap();
        println!("{:?}", word);
    }
}
