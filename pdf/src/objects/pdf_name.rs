use crate::error::{PdfError, PdfResult};

#[derive(Debug, PartialEq, Clone, Eq, Hash, Default)]
pub struct PdfName {
    name: String,
}

impl PdfName {
    pub fn new(name: String) -> Self {
        return PdfName { name };
    }

    pub fn new_from_buffer(buffer: &[u8]) -> Self {
        if buffer.is_empty() {
            return PdfName::default();
        }
        let mut i = 1;
        let mut data = Vec::new();
        while i < buffer.len() {
            let c = buffer[i];
            if c == b'#' && ((c + 2) as usize) < buffer.len() {
                let v = buffer[i + 1] * 16 + buffer[i + 2];
                data.push(v);
                i += 2;
            } else {
                data.push(c);
                i += 1;
            }
        }
        let name = String::from_utf8(data).unwrap();
        PdfName { name }
    }

    pub fn name(&self) -> &str {
        self.name.as_str()
    }
}
