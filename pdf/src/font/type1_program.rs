use crate::error::{PdfError, PdfResult};

#[derive(Debug, Clone)]
pub struct Type1Program {}

impl Type1Program {
    pub fn try_new(_bytes: Vec<u8>) -> PdfResult<Self> {
        Err(PdfError::UnsupportedFeature(
            "Type1 font program parser".into(),
        ))
    }
}
