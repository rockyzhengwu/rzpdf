//! Explicit transparency-group boundaries, separate from individual paints.
use crate::{
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict},
    page::{clip_path::ClipPath, general_state::BlendMode},
    pdf_context::PDFContext,
};
use std::sync::Arc;

#[derive(Debug, Clone, Copy)]
pub enum SoftMaskKind {
    Alpha,
    Luminosity,
}

#[derive(Debug)]
pub struct SoftMaskPaint {
    pub list: super::display_list::DisplayList,
    pub kind: SoftMaskKind,
    pub background: [f32; 3],
    pub transfer: Option<crate::function::Function>,
    pub(crate) retained_bytes: usize,
    pub(crate) retained_segments: usize,
}
use std::io::{Read, Seek};

#[derive(Debug, Clone)]
pub struct GroupPaint {
    pub isolated: bool,
    pub knockout: bool,
    pub alpha: f32,
    pub blend_mode: BlendMode,
    pub clip: ClipPath,
    pub soft_mask: Option<Arc<SoftMaskPaint>>,
}

#[derive(Debug, Clone, Copy)]
pub(crate) struct GroupParameters {
    pub isolated: bool,
    pub knockout: bool,
}

impl GroupParameters {
    pub(crate) fn parse<R: Read + Seek>(group: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let subtype = group.get("S").map(|v| ctx.resolve(v)).transpose()?;
        if subtype
            .as_ref()
            .and_then(PdfObject::as_name)
            .map(|n| n.name())
            != Some("Transparency")
        {
            return Err(PdfError::UnsupportedFeature("Form group subtype".into()));
        }
        let flag = |key: &str| -> PdfResult<bool> {
            group
                .get(key)
                .map(|v| {
                    ctx.resolve(v)?.as_bool().map(|v| v.value()).ok_or_else(|| {
                        PdfError::PageResourceError(format!("group {key} must be boolean"))
                    })
                })
                .transpose()
                .map(|v| v.unwrap_or(false))
        };
        let isolated = flag("I")?;
        let knockout = flag("K")?;
        if let Some(space) = group.get("CS") {
            if !isolated {
                return Err(PdfError::PageResourceError(
                    "nonisolated group cannot select a blending space".into(),
                ));
            }
            let space = ctx.resolve(space)?;
            if space.as_name().map(|n| n.name()) != Some("DeviceRGB") {
                return Err(PdfError::UnsupportedFeature(
                    "transparency group blending space other than DeviceRGB".into(),
                ));
            }
        }
        Ok(Self { isolated, knockout })
    }
}
