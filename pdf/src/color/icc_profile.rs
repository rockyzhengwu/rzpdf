use crate::error::PdfResult;

#[derive(Debug, Clone)]
pub struct ICCProfile {}

impl ICCProfile {
    pub fn try_new(data: &[u8]) -> PdfResult<Self> {
        Ok(ICCProfile {})
    }
}
