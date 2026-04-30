use crate::error::{PdfError, PdfResult};
use crate::parser::parse_utility::hex_to_u8;

#[derive(Debug, PartialEq, Clone, Eq, Hash, Default)]
pub struct PdfName {
    bytes: Vec<u8>,
    name: String,
}

impl PdfName {
    pub fn new(name: String) -> Self {
        let bytes = name.as_bytes().to_vec();
        PdfName { bytes, name }
    }

    pub fn new_from_buffer(buffer: &[u8]) -> PdfResult<Self> {
        if buffer.is_empty() {
            return Ok(PdfName::default());
        }
        let mut i = 1;
        let mut data = Vec::new();
        while i < buffer.len() {
            let c = buffer[i];
            if c == b'#' && i + 2 < buffer.len() {
                let hi = buffer[i + 1];
                let lo = buffer[i + 2];
                if !hi.is_ascii_hexdigit() || !lo.is_ascii_hexdigit() {
                    return Err(PdfError::ParserError(format!(
                        "invalid name escape sequence: {:?}",
                        &buffer[i..=usize::min(i + 2, buffer.len() - 1)]
                    )));
                }
                let v = (hex_to_u8(hi) << 4) | hex_to_u8(lo);
                data.push(v);
                i += 3;
            } else {
                data.push(c);
                i += 1;
            }
        }
        let name = String::from_utf8_lossy(&data).into_owned();
        Ok(PdfName { bytes: data, name })
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }

    pub fn bytes(&self) -> &[u8] {
        self.bytes.as_slice()
    }
}
