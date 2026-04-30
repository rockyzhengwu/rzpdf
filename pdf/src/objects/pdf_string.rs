use crate::error::PdfResult;

#[derive(Debug, PartialEq, Clone)]
pub struct PdfString {
    bytes: Vec<u8>,
    is_hex: bool,
}

impl PdfString {
    pub fn new(bytes: Vec<u8>, is_hex: bool) -> Self {
        Self { bytes, is_hex }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn raw_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn get_content(&self) -> PdfResult<String> {
        String::from_utf8(self.bytes.clone()).map_err(|e| {
            crate::error::PdfError::ObjectError(format!(
                "PdfString content convert utf8 error:{:?}",
                e
            ))
        })
    }
}
