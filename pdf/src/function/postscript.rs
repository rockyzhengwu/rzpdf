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
        let obj = ctx.resolve(obj)?;
        let stream = obj
            .as_stream()
            .ok_or_else(|| PdfError::FunctionError("Type4 requires a stream".to_string()))?;

        let domain = obj
            .get_attr("Domain")
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
            .get_attr("Range")
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
        let _data = stream.decode_data(ctx)?;
        if function.domain.is_empty()
            || function.range.is_empty()
            || function.domain.len() % 2 != 0
            || function.range.len() % 2 != 0
            || function
                .domain
                .chunks_exact(2)
                .chain(function.range.chunks_exact(2))
                .any(|p| !p[0].is_finite() || !p[1].is_finite() || p[0] > p[1])
        {
            return Err(PdfError::FunctionError(
                "Type4 requires finite ordered Domain/Range pairs".to_string(),
            ));
        }

        Ok(function)
    }

    pub fn eval(&self, _inputs: &[f32]) -> PdfResult<Vec<f32>> {
        let mut _stack: Vec<f32> = Vec::with_capacity(100);
        Err(PdfError::UnsupportedFeature(
            "PostScript function evaluation".into(),
        ))
    }
}
