use std::io::{Read, Seek};

use crate::{
    color::{
        colorspace::ColorSpace,
        value::{ColorRgb, ColorValue},
    },
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_array::PdfArray},
    pdf_context::PDFContext,
};

#[derive(Debug, Clone)]
pub struct Indexed {
    base: Box<ColorSpace>,
    hival: u8,
    lookup: Vec<u8>,
}

impl Indexed {
    pub fn default_value(&self) -> ColorValue {
        self.base.default_value()
    }

    pub fn try_new<R: Seek + Read>(arr: &PdfArray, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let base = arr
            .get(1)
            .ok_or(PdfError::ColorError("Indexed Base is None".to_string()))?;
        let base = ColorSpace::try_new(base, ctx)?;
        let hival = arr
            .get(2)
            .ok_or(PdfError::ColorError(
                "Indexed Color hival is None".to_string(),
            ))?
            .as_u8()
            .ok_or(PdfError::ColorError(
                "Indexed Color hival is not a as_number".to_string(),
            ))?;
        let lookup_obj = arr.get(3).ok_or(PdfError::ColorError(
            "Indexed color lookup is None".to_string(),
        ))?;
        let lookup = ctx.resolve(lookup_obj)?;
        match lookup {
            PdfObject::PdfStream(stream) => {
                let color = Indexed {
                    base: Box::new(base),
                    hival,
                    lookup: stream.decode_data(ctx)?,
                };
                Ok(color)
            }
            PdfObject::PdfString(s) => {
                let color = Indexed {
                    base: Box::new(base),
                    hival,
                    lookup: s.bytes().to_vec(),
                };
                Ok(color)
            }
            _ => Err(PdfError::ColorError(format!(
                "Indexed lookup need an stream or bytes string got :{:?}",
                lookup
            ))),
        }
    }

    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        let v = value.values()[0];
        let pos = (v.ceil() as usize).max(0).min(self.hival as usize);
        let pos = pos * self.base.number_of_components();
        let mut cvs = Vec::new();
        for i in 0..self.base.number_of_components() {
            let n = self.lookup[pos + i] as f32;
            cvs.push(n / 255.0);
        }
        let value = ColorValue::new(cvs);
        self.base.rgb(&value)
    }

    pub fn number_of_components(&self) -> usize {
        1
    }
}
