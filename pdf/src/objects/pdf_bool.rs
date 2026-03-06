#[derive(Debug, PartialEq, Clone)]
pub struct PdfBool {
    value: bool,
}

impl PdfBool {
    pub fn new(bytes: &[u8]) -> Self {
        if bytes == b"true" {
            return PdfBool { value: true };
        }
        PdfBool { value: false }
    }

    pub fn value(&self) -> bool {
        self.value
    }
}
