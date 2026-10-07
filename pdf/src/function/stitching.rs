use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
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
        let mut remaining = ctx.limits().max_objects;
        if remaining == 0 {
            return Err(PdfError::ResourceLimit(
                "PDF function total nodes".to_string(),
            ));
        }
        remaining -= 1;
        Self::try_new_at_depth(obj, ctx, 0, &mut remaining)
    }

    pub(crate) fn try_new_at_depth<R: Seek + Read>(
        obj: &PdfObject,
        ctx: &PDFContext<R>,
        depth: usize,
        remaining_nodes: &mut usize,
    ) -> PdfResult<Self> {
        if depth >= ctx.limits().max_depth {
            return Err(PdfError::ResourceLimit("PDF function nesting".to_string()));
        }
        let obj = ctx.resolve(obj)?;
        let dict = obj
            .as_dict()
            .ok_or_else(|| PdfError::FunctionError("Type3 requires a dictionary".to_string()))?;
        let array_values = |key: &str| -> PdfResult<Vec<f32>> {
            let value = ctx.resolve(
                dict.get(key)
                    .ok_or_else(|| PdfError::FunctionError(format!("Type3 requires {key}")))?,
            )?;
            let array = value
                .as_array()
                .ok_or_else(|| PdfError::FunctionError(format!("{key} must be an array")))?;
            array
                .into_iter()
                .map(|v| {
                    ctx.resolve(v)?
                        .as_f32()
                        .filter(|n| n.is_finite())
                        .ok_or_else(|| {
                            PdfError::FunctionError(format!("{key} must contain finite numbers"))
                        })
                })
                .collect()
        };
        let domain = array_values("Domain")?;
        if domain.len() != 2 || domain[0] >= domain[1] {
            return Err(PdfError::FunctionError(
                "Type3 requires a nonempty ordered Domain pair".to_string(),
            ));
        }
        let entries = ctx
            .resolve(dict.get("Functions").ok_or_else(|| {
                PdfError::FunctionError("Type3 requires Functions".to_string())
            })?)?;
        let entries = entries
            .as_array()
            .ok_or_else(|| PdfError::FunctionError("Functions must be an array".to_string()))?;
        if entries.len() == 0 {
            return Err(PdfError::FunctionError(
                "Functions must not be empty".to_string(),
            ));
        }
        if entries.len() > ctx.limits().max_objects {
            return Err(PdfError::ResourceLimit("Type3 function count".to_string()));
        }
        if entries.len() > *remaining_nodes {
            return Err(PdfError::ResourceLimit(
                "PDF function total nodes".to_string(),
            ));
        }
        let encode = array_values("Encode")?;
        let bounds = array_values("Bounds")?;
        if encode.len() != entries.len() * 2
            || bounds.len() != entries.len() - 1
            || bounds.windows(2).any(|p| p[0] >= p[1])
            || bounds.iter().any(|b| *b <= domain[0] || *b >= domain[1])
        {
            return Err(PdfError::FunctionError(
                "Invalid Type3 Bounds/Encode dimensions or ordering".to_string(),
            ));
        }
        let functions = entries
            .into_iter()
            .map(|f| super::create_function_at_depth(f, ctx, depth + 1, remaining_nodes))
            .collect::<PdfResult<Vec<_>>>()?;
        Ok(Self {
            domain,
            functions,
            bounds,
            encode,
            m: 1,
            n: 0,
        })
    }
    pub fn eval(&self, inputs: &[f32]) -> PdfResult<Vec<f32>> {
        Err(PdfError::UnsupportedFeature(
            "stitching function evaluation".into(),
        ))
    }
}
