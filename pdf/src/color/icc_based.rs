use std::io::{Read, Seek};

use crate::{
    color::{colorspace::ColorSpace, icc_profile::ICCProfile},
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

use super::device_cmyk::DeviceCmyk;
use super::device_gray::DeviceGray;
use super::device_rgb::DeviceRgb;
use super::value::{ColorRgb, ColorValue};

#[derive(Debug, Clone)]
pub struct IccBased {
    n: u8,
    alternate: Box<ColorSpace>,
    range: Vec<f32>,
    profile: Option<ICCProfile>,
}

impl Default for IccBased {
    fn default() -> Self {
        IccBased {
            n: 1,
            alternate: Box::new(ColorSpace::DeviceGray(DeviceGray::new())),
            range: Vec::new(),
            profile: None,
        }
    }
}

impl IccBased {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let color_stream = match obj {
            PdfObject::PdfArray(array) => {
                if array.len() < 2 {
                    return Err(PdfError::ColorError(
                        "IccBased Color array need 2 param at least".to_string(),
                    ));
                }
                let cd = ctx
                    .resolve(array.get(1).unwrap())?
                    .as_stream()
                    .ok_or(PdfError::ColorError(format!("IccBased need Streamgot",)))?;
                cd
            }
            PdfObject::PdfStream(s) => s,
            _ => {
                return Err(PdfError::ColorError("Bad IccBased Color".to_string()));
            }
        };

        let mut color = IccBased::default();
        if let Some(n) = color_stream.dict().get("N") {
            let nv = n.as_u32().ok_or(PdfError::ColorError(format!(
                "IccBased N is not a number:{:?}",
                n
            )))?;
            if !matches!(nv, 1 | 3 | 4) {
                return Err(PdfError::ColorError(format!(
                    "IccBased N is must 1,3,or 4 got: {:?}",
                    nv
                )));
            }
            color.n = nv as u8;
        } else {
            return Err(PdfError::ColorError(format!(
                "IccBased color need a N parameter "
            )));
        }
        if let Some(alt) = color_stream.dict().get("Alternate") {
            let altc = ColorSpace::try_new(alt, ctx)?;
            color.alternate = Box::new(altc);
        } else {
            match color.n {
                1 => {
                    color.alternate = Box::new(ColorSpace::DeviceGray(DeviceGray::new()));
                }
                3 => {
                    color.alternate = Box::new(ColorSpace::DeviceRgb(DeviceRgb::new()));
                }
                4 => {
                    color.alternate = Box::new(ColorSpace::DeviceCmyk(DeviceCmyk::new()));
                }
                _ => { // donothing}
                }
            }
        }
        match color_stream.dict().get("Range") {
            Some(r) => {
                let ra = r.as_array().ok_or(PdfError::ColorError(format!(
                    "IccBased Color need an array got :{:?}",
                    r
                )))?;
                if (ra.len() as u8) != (color.n * 2_u8) {
                    return Err(PdfError::ColorError(format!(
                        "IccBased range array element is not valid got:{:?}",
                        ra.len()
                    )));
                }
                for i in 0..color.n * 2 {
                    let v = ra.get(i as usize).unwrap().as_f32().unwrap();
                    color.range.push(v);
                }
            }
            None => {
                for _ in 0..color.n {
                    color.range.push(0.0);
                    color.range.push(1.0);
                }
            }
        }

        Ok(color)
    }

    pub fn default_value(&self) -> ColorValue {
        match self.n {
            1 => ColorValue::new(vec![0.0]),
            3 => ColorValue::new(vec![0.0, 0.0, 0.0]),
            4 => ColorValue::new(vec![0.0, 0.0, 0.0, 0.0]),
            _ => {
                panic!("IccBased color n must be 1, 3, or 4")
            }
        }
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        self.alternate.rgb(value)
    }

    pub fn number_of_components(&self) -> usize {
        self.n as usize
    }
    pub fn range(&self) -> &[f32] {
        self.range.as_slice()
    }
}
