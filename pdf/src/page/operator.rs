use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
};

#[derive(Debug, Clone, Default)]
pub struct Operator {
    op: String,
    operands: Vec<PdfObject>,
}
impl Operator {
    pub fn new(op: String, operands: Vec<PdfObject>) -> Self {
        Operator { op, operands }
    }

    pub fn name(&self) -> &str {
        self.op.as_str()
    }

    pub fn operand(&self, index: usize) -> PdfResult<&PdfObject> {
        self.operands
            .get(index)
            .ok_or(PdfError::ContentParseError(format!(
                "{:?}, can't have enough operands {:?}",
                self.op, self.operands
            )))
    }

    pub fn get_as_f32(&self, index: usize) -> PdfResult<f32> {
        if let Some(obj) = self.operands.get(index) {
            let v = obj.as_f32().ok_or(PdfError::PageOperatorError(format!(
                "expect f32 got {:?}",
                obj
            )))?;
            Ok(v)
        } else {
            Err(PdfError::PageOperatorError(
                "Get operand error out  of index".to_string(),
            ))
        }
    }

    pub fn num_operands(&self) -> usize {
        self.operands.len()
    }
}
