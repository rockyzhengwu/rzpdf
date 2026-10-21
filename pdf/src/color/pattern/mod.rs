use std::io::{Read, Seek};

use super::colorspace::ColorSpace;
use super::value::ColorValue;
use crate::error::{PdfError, PdfResult};
use crate::objects::PdfObject;
use crate::pdf_context::PDFContext;
use std::sync::Arc;

pub mod shading;
pub mod tiling;
use tiling::TilingPattern;

#[derive(Debug, Clone, Default)]
pub struct PatternColorSpace {
    pub(crate) base: Option<Box<ColorSpace>>,
    pub(crate) paint: Option<Arc<PatternPaint>>,
}

/// Immutable interpretation snapshot in page display coordinates.
#[derive(Debug)]
pub struct PatternPaint {
    pub list: crate::page::display_list::DisplayList,
    pub(crate) retained_bytes: usize,
    pub(crate) retained_segments: usize,
    pub(crate) tiling: bool,
    /// Single-cell snapshot and its placement in page display coordinates.
    pub tile_matrix: Option<crate::geom::matrix::Matrix>,
}

impl PatternPaint {
    pub fn is_tiling(&self) -> bool {
        self.tiling
    }
}

impl PatternColorSpace {
    /// Construct a direct definition; referenced base spaces require a context.
    pub fn try_new(obj: &PdfObject) -> PdfResult<Self> {
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(Default::default(), Default::default());
        Self::try_new_with_context(obj, &ctx)
    }

    pub fn default_value(&self) -> ColorValue {
        ColorValue::new(Vec::new())
    }

    pub fn paint(&self) -> Option<&PatternPaint> {
        self.paint.as_deref()
    }

    pub fn try_new_with_context<R: Read + Seek>(
        obj: &PdfObject,
        ctx: &PDFContext<R>,
    ) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        if obj.as_name().is_some_and(|name| name.name() == "Pattern") {
            return Ok(Self::default());
        }
        let array = obj
            .as_array()
            .ok_or_else(|| PdfError::ColorError("Pattern requires an array".into()))?;
        if !(1..=2).contains(&array.len()) {
            return Err(PdfError::ColorError(
                "Pattern requires zero or one base color space".into(),
            ));
        }
        if array
            .get(0)
            .map(|v| ctx.resolve(v))
            .transpose()?
            .as_ref()
            .and_then(PdfObject::as_name)
            .map(|n| n.name())
            != Some("Pattern")
        {
            return Err(PdfError::ColorError(
                "Pattern array must begin with Pattern".into(),
            ));
        }
        let base = array
            .get(1)
            .map(|value| ColorSpace::try_new(value, ctx))
            .transpose()?;
        if matches!(base, Some(ColorSpace::Pattern(_))) {
            return Err(PdfError::ColorError(
                "Pattern base cannot be Pattern".into(),
            ));
        }
        Ok(Self {
            base: base.map(Box::new),
            paint: None,
        })
    }
}

#[derive(Debug, Clone)]
#[expect(
    clippy::large_enum_variant,
    reason = "preserve public pattern variants and inline budget accounting"
)]
pub enum Pattern {
    Tiling(TilingPattern),
    Shading(shading::ShadingPattern),
}

impl Pattern {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let resolved = ctx.resolve(obj)?;
        let obj = &resolved;
        let pt = obj.get_attr("PatternType").ok_or(PdfError::ColorError(
            "Pattern has no Pattern type ".to_string(),
        ))?;
        let pt = ctx.resolve(pt)?;
        let pt = pt
            .as_u32()
            .ok_or(PdfError::ColorError("Color error".to_string()))?;
        match pt {
            1 => {
                let p = TilingPattern::try_new(obj, ctx)?;
                Ok(Pattern::Tiling(p))
            }
            2 => Ok(Pattern::Shading(shading::ShadingPattern::try_new(
                obj, ctx,
            )?)),
            _ => Err(PdfError::UnsupportedFeature(format!("PatternType {pt}"))),
        }
    }
}
