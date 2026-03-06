use crate::error::PdfResult;

#[derive(Debug, Clone)]
pub struct Type1Program {}

impl Type1Program {
    pub fn try_new(bytes: Vec<u8>) -> PdfResult<Self> {
        unimplemented!()
    }
}
