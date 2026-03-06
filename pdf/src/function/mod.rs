use std::io::{Read, Seek};

use exponential::Exponential;
use postscript::PostScriptFunction;
use sampled::Sampled;
use stitching::Stitching;

use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

pub mod exponential;
pub mod postscript;
pub mod sampled;
pub mod stitching;

#[derive(Debug, Clone)]
pub enum Function {
    Type0(Sampled),
    Type2(Exponential),
    Type3(Stitching),
    Type4(PostScriptFunction),
}

pub fn create_function<R: Seek + Read>(
    obj: &PdfObject,
    ctx: &PDFContext<R>,
) -> PdfResult<Function> {
    let t = obj
        .as_dict()
        .unwrap()
        .get("FunctionType")
        .ok_or(PdfError::FunctionError("FunctionType is None".to_string()))?
        .as_u32()
        .ok_or(PdfError::FunctionError(
            "FunctionType need to be a number".to_string(),
        ))?;
    match t {
        0 => {
            let s = Sampled::try_new(obj, ctx)?;
            Ok(Function::Type0(s))
        }
        2 => {
            let function = Exponential::try_new(obj, ctx)?;
            Ok(Function::Type2(function))
        }
        3 => {
            let function = Stitching::try_new(obj, ctx)?;
            Ok(Function::Type3(function))
        }
        4 => {
            let function = PostScriptFunction::try_new(obj, ctx)?;
            Ok(Function::Type4(function))
        }
        _ => Err(PdfError::FunctionError(format!(
            "FunctionType must be in [0,2,3,4] got:{:?}",
            t
        ))),
    }
}

impl Function {
    pub fn eval(&self, inputs: &[f32]) -> PdfResult<Vec<f32>> {
        match self {
            Function::Type0(f) => f.eval(inputs),
            Function::Type2(f) => f.eval(inputs),
            Function::Type3(f) => f.eval(inputs),
            Function::Type4(f) => f.eval(inputs),
        }
    }
}
