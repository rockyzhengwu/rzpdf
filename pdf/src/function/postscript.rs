use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone, Default)]
pub struct PostScriptFunction {
    domain: Vec<f32>,
    range: Vec<f32>,
}

impl PostScriptFunction {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let mut function = PostScriptFunction::default();

        let domain = obj
            .as_dict()
            .unwrap()
            .get("Domain")
            .ok_or(PdfError::FunctionError(
                "Type4 Function Domain is None".to_string(),
            ))?
            .as_array()
            .ok_or(PdfError::FunctionError(
                "Type4 function domain is not an array".to_string(),
            ))?;
        for v in domain.into_iter() {
            let value = v.as_f32().ok_or(PdfError::FunctionError(
                "Type4 Domain element is not number".to_string(),
            ))?;
            function.domain.push(value);
        }

        let range = obj
            .as_dict()
            .unwrap()
            .get("Range")
            .ok_or(PdfError::FunctionError("Type4 Range is None".to_string()))?
            .as_array()
            .ok_or(PdfError::FunctionError(
                "Type3 Function Range is not an array".to_string(),
            ))?;
        for v in range.into_iter() {
            let value = v.as_f32().ok_or(PdfError::FunctionError(
                "Type4 Function range element is not an number".to_string(),
            ))?;
            function.range.push(value);
        }
        let data = obj.as_stream().unwrap().decode_data(ctx)?;

        Ok(function)
    }

    pub fn eval(&self, _inputs: &[f32]) -> PdfResult<Vec<f32>> {
        let mut _stack: Vec<f32> = Vec::with_capacity(100);
        unimplemented!()
    }
}
