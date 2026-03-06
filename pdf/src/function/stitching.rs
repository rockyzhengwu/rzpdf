use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    function::create_function,
    objects::PdfObject,
    pdf_context::PDFContext,
};

use super::Function;

#[derive(Debug, Clone, Default)]
pub struct Stitching {
    domain: Vec<f32>,
    functions: Vec<Function>,
    bounds: Vec<f32>,
    encode: Vec<f32>,
    m: usize,
    n: usize,
}

impl Stitching {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let mut function = Stitching::default();

        let domain = obj
            .as_dict()
            .unwrap()
            .get("Domain")
            .ok_or(PdfError::FunctionError(
                "Type3 Function Domain is None".to_string(),
            ))?
            .as_array()
            .ok_or(PdfError::FunctionError(
                "Type3 function domain is not an array".to_string(),
            ))?;
        for v in domain.into_iter() {
            let value = v.as_f32().ok_or(PdfError::FunctionError(
                "Type3 Domain element is not number".to_string(),
            ))?;
            function.domain.push(value);
        }
        function.m = function.domain.len() / 2;
        if function.m != 1 {
            return Err(PdfError::FunctionError(
                "Type3 function with more than one input".to_string(),
            ))?;
        }
        let functions = obj
            .as_dict()
            .unwrap()
            .get("Functions")
            .ok_or(PdfError::FunctionError(
                "Type3 Function Functions is required".to_string(),
            ))?
            .as_array()
            .ok_or(PdfError::FunctionError(
                "Type3 Functions is not array".to_string(),
            ))?;
        for f in functions.into_iter() {
            let ff = create_function(f, ctx)?;
            function.functions.push(ff);
        }
        if let Some(encode) = obj.as_dict().unwrap().get("Encode") {
            let enc_array = encode.as_array().ok_or(PdfError::FunctionError(
                "Type3 function Encode is not array".to_string(),
            ))?;
            for v in enc_array.into_iter() {
                let vv = v.as_f32().ok_or(PdfError::FunctionError(
                    "Type3 Function Encode element is not number".to_string(),
                ))?;
                function.encode.push(vv);
            }
        } else {
            return Err(PdfError::FunctionError(
                "Type3 function Encode is rquired".to_string(),
            ));
        }
        Ok(function)
    }

    pub fn eval(&self, inputs: &[f32]) -> PdfResult<Vec<f32>> {
        unimplemented!()
    }
}
