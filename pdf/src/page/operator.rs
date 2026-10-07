use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
};

#[derive(Debug, Clone, Default)]
pub struct Operator {
    op: String,
    operands: Vec<PdfObject>,
    byte_offset: Option<u64>,
}
impl Operator {
    pub fn new(op: String, operands: Vec<PdfObject>) -> Self {
        Operator {
            op,
            operands,
            byte_offset: None,
        }
    }

    pub fn with_byte_offset(mut self, offset: u64) -> Self {
        self.byte_offset = Some(offset);
        self
    }

    pub fn byte_offset(&self) -> Option<u64> {
        self.byte_offset
    }

    pub fn name(&self) -> &str {
        self.op.as_str()
    }

    pub fn operand(&self, index: usize) -> PdfResult<&PdfObject> {
        self.operands.get(index).ok_or_else(|| {
            PdfError::ContentParseError(format!(
                "operand {index} missing (operand count {})",
                self.operands.len()
            ))
        })
    }

    pub fn get_as_f32(&self, index: usize) -> PdfResult<f32> {
        if let Some(obj) = self.operands.get(index) {
            let v = obj.as_f32().ok_or_else(|| {
                PdfError::PageOperatorError(format!("operand {index} must be a number"))
            })?;
            if !v.is_finite() {
                return Err(PdfError::PageOperatorError(format!(
                    "{} operand {index} must be finite",
                    self.op
                )));
            }
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

    pub fn get_as_name(&self, index: usize) -> PdfResult<&crate::objects::pdf_name::PdfName> {
        self.operand(index)?.as_name().ok_or_else(|| {
            PdfError::PageOperatorError(format!("{} operand {index} must be a name", self.op))
        })
    }
    pub fn get_as_array(&self, index: usize) -> PdfResult<&crate::objects::pdf_array::PdfArray> {
        self.operand(index)?.as_array().ok_or_else(|| {
            PdfError::PageOperatorError(format!("{} operand {index} must be an array", self.op))
        })
    }
    pub fn get_as_string(&self, index: usize) -> PdfResult<&crate::objects::pdf_string::PdfString> {
        self.operand(index)?.as_string().ok_or_else(|| {
            PdfError::PageOperatorError(format!("{} operand {index} must be a string", self.op))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::pdf_string::PdfString;

    #[test]
    fn invalid_operands_do_not_expand_payload_in_errors() {
        let payload = vec![b'x'; 100_000];
        let op = Operator::new(
            "Tj".into(),
            vec![PdfObject::PdfString(PdfString::new(payload, false))],
        );
        assert!(op.operand(0).is_ok());
        let missing = op.operand(1).unwrap_err().to_string();
        let wrong_type = op.get_as_f32(0).unwrap_err().to_string();
        assert!(missing.len() < 128);
        assert!(wrong_type.len() < 128);
        assert!(!missing.contains("xxxx"));
        assert!(!wrong_type.contains("xxxx"));
    }
}
