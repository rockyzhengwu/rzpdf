use std::{
    fmt::Display,
    io::{Read, Seek},
};

use crate::{
    color::{
        cal_gray::CalGray, cal_rgb::CalRgb, device_cmyk::DeviceCmyk, device_gray::DeviceGray,
        device_rgb::DeviceRgb, devicen::DeviceN, icc_based::IccBased, indexed::Indexed, lab::Lab,
        pattern::PatternColorSpace, separation::Separation, value::ColorRgb, value::ColorValue,
    },
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone)]
pub enum ColorSpace {
    DeviceGray(DeviceGray),
    DeviceRgb(DeviceRgb),
    DeviceCmyk(DeviceCmyk),
    CalGray(CalGray),
    CalRgb(CalRgb),
    Lab(Lab),
    IccBased(IccBased),
    Pattern(PatternColorSpace),
    Indexed(Indexed),
    Separation(Separation),
    DeviceN(DeviceN),
}

impl ColorSpace {
    pub(crate) fn visit_profiles(
        &self,
        visitor: &mut impl FnMut(&super::icc_profile::ICCProfile) -> PdfResult<()>,
    ) -> PdfResult<()> {
        match self {
            Self::IccBased(space) => space.visit_profiles(visitor),
            Self::Indexed(space) => space.visit_profiles(visitor),
            Self::Separation(space) => space.visit_profiles(visitor),
            Self::DeviceN(space) => space.visit_profiles(visitor),
            Self::Pattern(space) => space
                .base
                .as_ref()
                .map_or(Ok(()), |base| base.visit_profiles(visitor)),
            _ => Ok(()),
        }
    }
    pub(crate) fn retained_profile_bytes(&self) -> PdfResult<usize> {
        match self {
            Self::IccBased(space) => space.retained_profile_bytes(),
            Self::Indexed(space) => space.retained_profile_bytes(),
            Self::Separation(space) => space.retained_profile_bytes(),
            Self::DeviceN(space) => space.retained_profile_bytes(),
            Self::Pattern(space) => space
                .base
                .as_ref()
                .map(|base| base.retained_profile_bytes())
                .transpose()
                .map(|v| v.unwrap_or(0)),
            _ => Ok(0),
        }
    }
    /// Whether this colorant suppresses paint without suppressing clipping.
    pub fn is_nonpainting(&self) -> bool {
        match self {
            Self::Separation(space) => space.is_nonpainting(),
            Self::DeviceN(space) => space.is_nonpainting(),
            Self::Indexed(space) => space.is_nonpainting(),
            Self::Pattern(space) => space
                .base
                .as_ref()
                .is_some_and(|base| base.is_nonpainting()),
            _ => false,
        }
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        self.rgb_with_context(value, &super::conversion::ColorConversionContext::default())
    }
    pub fn rgb_with_context(
        &self,
        value: &ColorValue,
        context: &super::conversion::ColorConversionContext,
    ) -> PdfResult<ColorRgb> {
        let color = match self {
            ColorSpace::DeviceGray(gray) => gray.rgb(value),
            ColorSpace::DeviceRgb(rgb) => rgb.rgb(value),
            ColorSpace::DeviceCmyk(cmyk) => cmyk.rgb(value),
            ColorSpace::Lab(lab) => lab.rgb(value),
            ColorSpace::IccBased(icc) => return icc.rgb_with_context(value, context),
            ColorSpace::CalGray(cg) => cg.rgb(value),
            ColorSpace::CalRgb(cr) => cr.rgb(value),
            ColorSpace::Indexed(indexed) => return indexed.rgb_with_context(value, context),
            ColorSpace::Separation(sep) => return sep.rgb_with_context(value, context),
            ColorSpace::DeviceN(space) => return space.rgb_with_context(value, context),
            _ => Err(PdfError::UnsupportedFeature(format!(
                "RGB conversion for {self}"
            ))),
        }?;
        context.convert_srgb(color)
    }
    pub fn default_value(&self) -> PdfResult<ColorValue> {
        Ok(match self {
            ColorSpace::DeviceGray(gray) => gray.default_value(),
            ColorSpace::DeviceRgb(rgb) => rgb.default_value(),
            ColorSpace::DeviceCmyk(cmyk) => cmyk.default_value(),
            ColorSpace::Lab(lab) => lab.default_value(),
            ColorSpace::IccBased(icc) => icc.default_value()?,
            ColorSpace::CalGray(cg) => cg.default_value(),
            ColorSpace::CalRgb(cr) => cr.default_value(),
            ColorSpace::Indexed(indexed) => indexed.default_value(),
            ColorSpace::Separation(sep) => sep.default_value(),
            ColorSpace::Pattern(p) => p.default_value(),
            ColorSpace::DeviceN(space) => space.default_value()?,
        })
    }

    pub fn number_of_components(&self) -> PdfResult<usize> {
        Ok(match self {
            ColorSpace::DeviceGray(gray) => gray.number_of_components(),
            ColorSpace::DeviceRgb(rgb) => rgb.number_of_components(),
            ColorSpace::DeviceCmyk(cmyk) => cmyk.number_of_components(),
            ColorSpace::Lab(lab) => lab.number_of_components(),
            ColorSpace::IccBased(icc) => icc.number_of_components(),
            ColorSpace::CalGray(cg) => cg.number_of_components(),
            ColorSpace::CalRgb(cr) => cr.number_of_components(),
            ColorSpace::Indexed(indexed) => indexed.number_of_components(),
            ColorSpace::Separation(sep) => sep.number_of_components(),
            ColorSpace::DeviceN(space) => space.number_of_components(),
            _ => {
                return Err(PdfError::UnsupportedFeature(format!(
                    "Component count for {self}"
                )));
            }
        })
    }
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        ctx.with_color_depth(obj.as_object_id(), || Self::try_new_inner(obj, ctx))
    }

    fn try_new_inner<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        match &obj {
            PdfObject::PdfName(name) => match name.name() {
                "G" | "DeviceGray" => Ok(ColorSpace::DeviceGray(DeviceGray::new())),
                "RGB" | "DeviceRGB" => Ok(ColorSpace::DeviceRgb(DeviceRgb::new())),
                "CMYK" | "DeviceCMYK" => Ok(ColorSpace::DeviceCmyk(DeviceCmyk::new())),
                "Pattern" => Ok(ColorSpace::Pattern(PatternColorSpace::default())),
                _ => Err(PdfError::ColorError(format!(
                    "Color name is error:{:?}",
                    name
                ))),
            },
            PdfObject::PdfArray(array) => {
                let cn = array
                    .get(0)
                    .ok_or(PdfError::ColorError(
                        "ColorSpace array is empty".to_string(),
                    ))?
                    .as_name()
                    .ok_or(PdfError::ColorError(
                        "ColorSpace new need an array".to_string(),
                    ))?;
                match cn.name() {
                    "DeviceGray" => Ok(ColorSpace::DeviceGray(DeviceGray::new())),
                    "DeviceRGB" => Ok(ColorSpace::DeviceRgb(DeviceRgb::new())),
                    "DeviceCMYK" => Ok(ColorSpace::DeviceCmyk(DeviceCmyk::new())),
                    "CalGray" => Ok(ColorSpace::CalGray(CalGray::try_new(&obj, ctx)?)),
                    "CalRGB" => Ok(ColorSpace::CalRgb(CalRgb::try_new(&obj, ctx)?)),
                    "Pattern" => Ok(ColorSpace::Pattern(
                        PatternColorSpace::try_new_with_context(&obj, ctx)?,
                    )),
                    "Indexed" => Ok(ColorSpace::Indexed(Indexed::try_new(array, ctx)?)),
                    "Separation" => Ok(ColorSpace::Separation(Separation::try_new(array, ctx)?)),
                    "DeviceN" => Ok(ColorSpace::DeviceN(DeviceN::try_new(array, ctx)?)),
                    "ICCBased" => Ok(ColorSpace::IccBased(IccBased::try_new(&obj, ctx)?)),
                    "Lab" => Ok(ColorSpace::Lab(Lab::try_new_with_context(&obj, ctx)?)),
                    _ => Err(PdfError::UnsupportedFeature(format!(
                        "Color space {}",
                        cn.name()
                    ))),
                }
            }
            _ => Err(PdfError::ColorError(format!(
                "Parse ColorSpace need an PdfArray or PdfName got :{:?}",
                obj
            ))),
        }
    }
}

impl Display for ColorSpace {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ColorSpace::DeviceGray(_) => write!(f, "DeviceGray"),
            ColorSpace::DeviceRgb(_) => write!(f, "DeviceRGB"),
            ColorSpace::DeviceCmyk(_) => write!(f, "DeviceCMYK"),
            ColorSpace::Lab(_) => write!(f, "Lab"),
            ColorSpace::IccBased(_) => write!(f, "ICCBased"),
            ColorSpace::CalGray(_) => write!(f, "CalGray"),
            ColorSpace::CalRgb(_) => write!(f, "CalRGB"),
            ColorSpace::Indexed(_) => write!(f, "Indexed"),
            ColorSpace::Separation(_) => write!(f, "Separation"),
            ColorSpace::Pattern(_) => write!(f, "Pattern"),
            ColorSpace::DeviceN(_) => write!(f, "DeviceN"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inline_nesting_is_bounded_and_error_or_unwind_releases_reference_scope() {
        use crate::{
            document::Document,
            objects::{pdf_array::PdfArray, pdf_name::PdfName, pdf_reference::PdfReference},
        };
        let doc = Document::new().unwrap();
        let rgb = PdfObject::PdfName(PdfName::new("DeviceRGB".into()));
        let mut nested = rgb.clone();
        for _ in 0..48 {
            nested = PdfObject::PdfArray(PdfArray::from_items(vec![
                PdfObject::PdfName(PdfName::new("Pattern".into())),
                nested,
            ]));
        }
        for _ in 0..2 {
            assert!(matches!(
                ColorSpace::try_new(&nested, doc.ctx()),
                Err(PdfError::ResourceLimit(_))
            ));
            assert!(ColorSpace::try_new(&rgb, doc.ctx()).is_ok());
        }
        let id = doc.try_insert_object(rgb).unwrap();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            doc.ctx()
                .with_color_depth(Some(id), || -> PdfResult<()> { panic!("scope unwind") })
        }));
        assert!(result.is_err());
        let reference = PdfObject::PdfReference(PdfReference::from_id(id));
        assert!(ColorSpace::try_new(&reference, doc.ctx()).is_ok());
    }

    #[test]
    fn unsupported_color_queries_return_errors() {
        let pattern = ColorSpace::Pattern(PatternColorSpace::default());
        assert!(pattern.number_of_components().is_err());
        assert!(pattern.rgb(&ColorValue::new(vec![])).is_err());
    }

    #[test]
    fn device_colors_validate_component_counts_and_finite_values() {
        for (space, count) in [
            (ColorSpace::DeviceGray(DeviceGray::new()), 1),
            (ColorSpace::DeviceRgb(DeviceRgb::new()), 3),
            (ColorSpace::DeviceCmyk(DeviceCmyk::new()), 4),
        ] {
            assert_eq!(space.number_of_components().unwrap(), count);
            assert!(space.rgb(&space.default_value().unwrap()).is_ok());
            assert!(space.rgb(&ColorValue::new(vec![])).is_err());
            assert!(space.rgb(&ColorValue::new(vec![0.0; count + 1])).is_err());
            assert!(space.rgb(&ColorValue::new(vec![f32::NAN; count])).is_err());
        }
    }
}
