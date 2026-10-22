#[derive(Default, Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlendMode {
    #[default]
    Normal,
    Multiply,
    Screen,
    Overlay,
    Darken,
    Lighten,
    ColorDodge,
    ColorBurn,
    HardLight,
    SoftLight,
    Difference,
    Exclusion,
    Hue,
    Saturation,
    Color,
    Luminosity,
}

impl BlendMode {
    pub fn from_pdf_name(name: &str) -> Option<Self> {
        Some(match name {
            "Normal" | "Compatible" => Self::Normal,
            "Multiply" => Self::Multiply,
            "Screen" => Self::Screen,
            "Overlay" => Self::Overlay,
            "Darken" => Self::Darken,
            "Lighten" => Self::Lighten,
            "ColorDodge" => Self::ColorDodge,
            "ColorBurn" => Self::ColorBurn,
            "HardLight" => Self::HardLight,
            "SoftLight" => Self::SoftLight,
            "Difference" => Self::Difference,
            "Exclusion" => Self::Exclusion,
            "Hue" => Self::Hue,
            "Saturation" => Self::Saturation,
            "Color" => Self::Color,
            "Luminosity" => Self::Luminosity,
            _ => return None,
        })
    }
}

pub(crate) fn resolve_blend_mode<R: std::io::Read + std::io::Seek>(
    value: &crate::objects::PdfObject,
    ctx: &crate::pdf_context::PDFContext<R>,
) -> crate::error::PdfResult<Option<BlendMode>> {
    resolve_blend_mode_using(value, ctx.limits(), &mut |object| ctx.resolve_owned(object))
}

pub(crate) fn resolve_blend_mode_using<O: std::borrow::Borrow<crate::objects::PdfObject>>(
    value: &crate::objects::PdfObject,
    limits: crate::parser::options::Limits,
    resolve: &mut impl FnMut(&crate::objects::PdfObject) -> crate::error::PdfResult<O>,
) -> crate::error::PdfResult<Option<BlendMode>> {
    use crate::{error::PdfError, objects::PdfObject};
    let name_mode = |value: &PdfObject| {
        let name = value
            .as_name()
            .ok_or_else(|| PdfError::PageResourceError("blend mode must be a name".into()))?;
        if name.name().len() > limits.max_token_bytes {
            return Err(PdfError::ResourceLimit("blend mode name bytes".into()));
        }
        Ok(BlendMode::from_pdf_name(name.name()))
    };
    let owned = resolve(value)?;
    match owned.borrow() {
        PdfObject::PdfName(_) => name_mode(owned.borrow()),
        PdfObject::PdfArray(array) => {
            if array.len() > limits.max_objects {
                return Err(PdfError::ResourceLimit("blend mode array entries".into()));
            }
            let mut result = None;
            for value in array {
                let element = resolve(value)?;
                let mode = name_mode(element.borrow())?;
                if result.is_none() {
                    result = mode;
                }
            }
            // ISO 32000-1 11.6.3: unrecognized preference arrays use Normal.
            Ok(Some(result.unwrap_or(BlendMode::Normal)))
        }
        _ => Err(PdfError::PageResourceError(
            "blend mode must be a name or name array".into(),
        )),
    }
}

#[derive(Debug, Clone)]
pub struct GeneralState {
    /// Retained print-device parameters; RGB consumers must choose their own
    /// overprint simulation policy rather than losing these during parsing.
    pub stroke_overprint: bool,
    pub fill_overprint: bool,
    pub overprint_mode: u8,
    pub stroke_adjustment: bool,
    pub smoothness: f32,
    /// Printer conversion functions apply to RGB -> CMYK conversion, not RGB
    /// raster output. Retain them so a print backend can evaluate them.
    pub black_generation: Option<std::sync::Arc<crate::function::Function>>,
    pub undercolor_removal: Option<std::sync::Arc<crate::function::Function>>,
    pub blend_mode: BlendMode,
    pub stroke_alpha: f32,
    pub fill_alpha: f32,
    pub flat_ness: f32,
    pub soft_mask: Option<std::sync::Arc<super::transparency::SoftMaskPaint>>,
    pub rendering_intent: crate::color::conversion::RenderingIntent,
}

impl Default for GeneralState {
    fn default() -> Self {
        Self {
            stroke_overprint: false,
            fill_overprint: false,
            overprint_mode: 0,
            stroke_adjustment: false,
            smoothness: 0.,
            black_generation: None,
            undercolor_removal: None,
            blend_mode: BlendMode::Normal,
            stroke_alpha: 1.0,
            fill_alpha: 1.0,
            flat_ness: 0.0,
            soft_mask: None,
            rendering_intent: Default::default(),
        }
    }
}

#[cfg(test)]
mod budget_tests {
    use super::*;
    use crate::{
        budget::OperationBudget,
        error::PdfResult,
        objects::{PdfObject, pdf_array::PdfArray, pdf_name::PdfName},
        parser::options::Limits,
    };

    fn parse(value: &PdfObject, pool: &OperationBudget) -> PdfResult<Option<BlendMode>> {
        let mut lease = pool.reserve(0)?;
        resolve_blend_mode_using(value, Limits::default(), &mut |object| {
            let (copy, mut child) =
                crate::objects::clone_object_with_budget(object, pool, Limits::default(), 0)?;
            lease.absorb(&mut child)?;
            Ok(copy)
        })
    }

    #[test]
    fn preference_validation_and_budget_failure_release_all_copies() {
        let name = |value: &str| PdfObject::PdfName(PdfName::new(value.into()));
        let value = PdfObject::PdfArray(PdfArray::from_items(vec![
            name("Unknown"),
            name("Multiply"),
            name("Screen"),
        ]));
        let probe = OperationBudget::new(65536);
        assert_eq!(parse(&value, &probe).unwrap(), Some(BlendMode::Multiply));
        let peak = probe.peak_bytes();
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(parse(&value, &pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        assert_eq!(parse(&value, &pool).unwrap(), Some(BlendMode::Multiply));
        assert_eq!(pool.live_bytes(), 0);
        let invalid = PdfObject::PdfArray(PdfArray::from_items(vec![
            name("Normal"),
            PdfObject::PdfNull,
        ]));
        assert!(matches!(
            parse(&invalid, &probe),
            Err(crate::error::PdfError::PageResourceError(_))
        ));
        assert_eq!(probe.live_bytes(), 0);
        assert_eq!(parse(&name("Unknown"), &probe).unwrap(), None);
        assert_eq!(
            parse(
                &PdfObject::PdfArray(PdfArray::from_items(vec![name("Unknown")])),
                &probe
            )
            .unwrap(),
            Some(BlendMode::Normal)
        );
        assert_eq!(probe.live_bytes(), 0);
    }
}
