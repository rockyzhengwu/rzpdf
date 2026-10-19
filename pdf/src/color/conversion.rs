//! Explicit output and ICC fallback policy, independent of a drawing backend.
use super::{icc_profile::ICCProfile, value::ColorRgb};
use crate::error::{PdfError, PdfResult};

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum RenderingIntent {
    Perceptual,
    #[default]
    RelativeColorimetric,
    Saturation,
    AbsoluteColorimetric,
}
impl RenderingIntent {
    pub fn from_pdf_name(name: &str) -> Option<Self> {
        Some(match name {
            "Perceptual" => Self::Perceptual,
            "RelativeColorimetric" => Self::RelativeColorimetric,
            "Saturation" => Self::Saturation,
            "AbsoluteColorimetric" => Self::AbsoluteColorimetric,
            _ => return None,
        })
    }
    pub(crate) fn backend(self) -> moxcms::RenderingIntent {
        match self {
            Self::Perceptual => moxcms::RenderingIntent::Perceptual,
            Self::RelativeColorimetric => moxcms::RenderingIntent::RelativeColorimetric,
            Self::Saturation => moxcms::RenderingIntent::Saturation,
            Self::AbsoluteColorimetric => moxcms::RenderingIntent::AbsoluteColorimetric,
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IccFallback {
    #[default]
    Reject,
    /// Explicit display fallback, never an ICC-accurate conversion claim.
    Alternate,
}

#[derive(Debug, Clone)]
pub struct ColorConversionContext {
    pub rendering_intent: RenderingIntent,
    pub icc_fallback: IccFallback,
    output: ICCProfile,
}
impl Default for ColorConversionContext {
    fn default() -> Self {
        Self {
            rendering_intent: RenderingIntent::default(),
            icc_fallback: IccFallback::Reject,
            output: ICCProfile::srgb(),
        }
    }
}
impl ColorConversionContext {
    pub fn with_output_profile(output: ICCProfile) -> PdfResult<Self> {
        if output.number_of_components() != 3 {
            return Err(PdfError::UnsupportedFeature(
                "output colour target must be RGB".into(),
            ));
        }
        Ok(Self {
            output,
            ..Self::default()
        })
    }
    pub fn output_profile(&self) -> &ICCProfile {
        &self.output
    }
    pub fn with_intent(&self, intent: RenderingIntent) -> Self {
        Self {
            rendering_intent: intent,
            ..self.clone()
        }
    }
    pub(crate) fn convert_srgb(&self, color: ColorRgb) -> PdfResult<ColorRgb> {
        if self.output.is_builtin_srgb() {
            return Ok(color);
        }
        ICCProfile::srgb().convert(&[color.r, color.g, color.b], self)
    }
}
