use std::io::{Read, Seek};

use super::value::ColorValue;
use crate::error::{PdfError, PdfResult};
use crate::objects::PdfObject;
use crate::pdf_context::PDFContext;

pub mod tiling;
use tiling::TilingPattern;

#[derive(Debug, Clone, Default)]
pub struct PatternColorSpace {}

impl PatternColorSpace {
    pub fn try_new(obj: &PdfObject) -> PdfResult<Self> {
        unimplemented!()
    }

    pub fn default_value(&self) -> ColorValue {
        ColorValue::default()
    }
}

#[derive(Debug, Clone)]
pub enum Pattern {
    Tiling(TilingPattern),
    Shading,
}

impl Pattern {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let pt = obj.get_attr("PatternType").ok_or(PdfError::ColorError(
            "Pattern has no Pattern type ".to_string(),
        ))?;
        let pt = pt
            .as_u32()
            .ok_or(PdfError::ColorError("Color error".to_string()))?;
        match pt {
            1 => {
                let p = TilingPattern::try_new(obj, ctx)?;
                return Ok(Pattern::Tiling(p));
            }
            _ => {
                unimplemented!("Shading pattern not implement")
            }
        }
    }
}
