use crate::{
    color::shading::Shading,
    error::{PdfError, PdfResult},
    geom::matrix::Matrix,
    objects::{PdfObject, pdf_dict::PdfDict},
    pdf_context::PDFContext,
};
use std::io::{Read, Seek};

#[derive(Debug, Clone)]
pub struct ShadingPattern {
    pub matrix: Matrix,
    pub shading: Shading,
    pub ext_gstate: Option<PdfDict>,
}

impl ShadingPattern {
    pub fn try_new<R: Read + Seek>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        let dict = obj
            .as_dict()
            .ok_or_else(|| PdfError::ColorError("shading pattern must be a dictionary".into()))?;
        if let Some(kind) = dict.get("Type") {
            if ctx.resolve(kind)?.as_name().map(|n| n.name()) != Some("Pattern") {
                return Err(PdfError::ColorError(
                    "shading pattern Type must be Pattern".into(),
                ));
            }
        }
        let kind = dict
            .get("PatternType")
            .map(|v| ctx.resolve(v))
            .transpose()?;
        if kind.as_ref().and_then(PdfObject::as_u8) != Some(2) {
            return Err(PdfError::ColorError("shading PatternType must be 2".into()));
        }
        let matrix = dict
            .get("Matrix")
            .map(|v| Matrix::try_from(&ctx.resolve(v)?))
            .transpose()?
            .unwrap_or_else(Matrix::identity);
        matrix.checked_inverse()?;
        let shading = Shading::try_new(
            dict.get("Shading")
                .ok_or_else(|| PdfError::ColorError("shading pattern missing Shading".into()))?,
            ctx,
        )?;
        let ext_gstate = dict
            .get("ExtGState")
            .map(|v| {
                ctx.resolve(v)?.into_dict().ok_or_else(|| {
                    PdfError::ColorError("pattern ExtGState must be a dictionary".into())
                })
            })
            .transpose()?;
        if ext_gstate
            .as_ref()
            .is_some_and(|state| !crate::page::all_state::supported_ext_gs_keys(state))
        {
            return Err(PdfError::UnsupportedFeature(
                "shading pattern ExtGState entries".into(),
            ));
        }
        Ok(Self {
            matrix,
            shading,
            ext_gstate,
        })
    }
}
