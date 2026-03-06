#[derive(Debug, Clone, PartialEq, Hash, Eq)]
pub struct PdfReference {
    objnum: u32,
    gennum: u32,
}

impl PdfReference {
    pub fn new(objnum: u32, gennum: u32) -> Self {
        Self { objnum, gennum }
    }
    pub fn objnum(&self) -> u32 {
        self.objnum
    }
}
