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
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        match self {
            ColorSpace::DeviceGray(gray) => gray.rgb(value),
            ColorSpace::DeviceRgb(rgb) => rgb.rgb(value),
            ColorSpace::DeviceCmyk(cmyk) => cmyk.rgb(value),
            ColorSpace::Lab(lab) => lab.rgb(value),
            ColorSpace::IccBased(icc) => icc.rgb(value),
            ColorSpace::CalGray(cg) => cg.rgb(value),
            ColorSpace::CalRgb(cr) => cr.rgb(value),
            ColorSpace::Indexed(indexed) => indexed.rgb(value),
            ColorSpace::Separation(sep) => sep.rgb(value),
            _ => {
                unimplemented!("not implent rgb of colorspace:{:?}", self)
            }
        }
    }
    pub fn default_value(&self) -> ColorValue {
        match self {
            ColorSpace::DeviceGray(gray) => gray.default_value(),
            ColorSpace::DeviceRgb(rgb) => rgb.default_value(),
            ColorSpace::DeviceCmyk(cmyk) => cmyk.default_value(),
            ColorSpace::Lab(lab) => lab.default_value(),
            ColorSpace::IccBased(icc) => icc.default_value(),
            ColorSpace::CalGray(cg) => cg.default_value(),
            ColorSpace::CalRgb(cr) => cr.default_value(),
            ColorSpace::Indexed(indexed) => indexed.default_value(),
            ColorSpace::Separation(sep) => sep.default_value(),
            ColorSpace::Pattern(p) => p.default_value(),
            _ => {
                unimplemented!("not implement default_value of colorspace:{:?}", self)
            }
        }
    }

    pub fn number_of_components(&self) -> usize {
        match self {
            ColorSpace::DeviceGray(gray) => gray.number_of_components(),
            ColorSpace::DeviceRgb(rgb) => rgb.number_of_components(),
            ColorSpace::DeviceCmyk(cmyk) => cmyk.number_of_components(),
            ColorSpace::Lab(lab) => lab.number_of_components(),
            ColorSpace::IccBased(icc) => icc.number_of_components(),
            ColorSpace::CalGray(cg) => cg.number_of_components(),
            ColorSpace::CalRgb(cr) => cr.number_of_components(),
            ColorSpace::Indexed(indexed) => indexed.number_of_components(),
            ColorSpace::Separation(sep) => sep.number_of_components(),
            _ => {
                unimplemented!("not implement number_of_components  : {:?}", self)
            }
        }
    }
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        match obj {
            PdfObject::PdfName(name) => match name.name() {
                "G" | "DeviceGray" => Ok(ColorSpace::DeviceGray(DeviceGray::new())),
                "RGB" | "DeviceRGB" => Ok(ColorSpace::DeviceRgb(DeviceRgb::new())),
                "CMYK" | "DeviceCMYK" => Ok(ColorSpace::DeviceCmyk(DeviceCmyk::new())),
                "Pattern" => Ok(ColorSpace::Pattern(PatternColorSpace::default())),
                _ => {
                    return Err(PdfError::ColorError(format!(
                        "Color name is error:{:?}",
                        name
                    )));
                }
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
                    "CalGray" => Ok(ColorSpace::CalGray(CalGray::try_new(obj, ctx)?)),
                    "CalRGB" => Ok(ColorSpace::CalRgb(CalRgb::try_new(obj, ctx)?)),
                    "Pattern" => Ok(ColorSpace::Pattern(PatternColorSpace::default())),
                    "Indexed" => Ok(ColorSpace::Indexed(Indexed::try_new(array, ctx)?)),
                    "Separation" => Ok(ColorSpace::Separation(Separation::try_new(array, ctx)?)),
                    "ICCBased" => Ok(ColorSpace::IccBased(IccBased::try_new(obj, ctx)?)),
                    "Lab" => Ok(ColorSpace::Lab(Lab::try_new(obj)?)),
                    _ => {
                        unimplemented!()
                    }
                }
            }
            _ => {
                return Err(PdfError::ColorError(format!(
                    "Parse ColorSpace need an PdfArray or PdfName got :{:?}",
                    obj
                )));
            }
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
