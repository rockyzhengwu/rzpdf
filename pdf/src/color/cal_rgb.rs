use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::objects::PdfObject;
use crate::pdf_context::PDFContext;

use super::value::{ColorRgb, ColorValue};

pub(super) fn read_parameter<R: Seek + Read, const N: usize>(
    dict: &crate::objects::pdf_dict::PdfDict,
    key: &str,
    ctx: &PDFContext<R>,
) -> PdfResult<Option<[f32; N]>> {
    let Some(value) = dict.get(key) else {
        return Ok(None);
    };
    let value = ctx.resolve(value)?;
    let array = value
        .as_array()
        .ok_or_else(|| PdfError::ColorError(format!("Color parameter {key} requires an array")))?;
    if array.len() != N {
        return Err(PdfError::ColorError(format!(
            "Color parameter {key} requires {N} values"
        )));
    }
    let mut values = [0.0; N];
    for (i, entry) in array.into_iter().enumerate() {
        values[i] = ctx
            .resolve(entry)?
            .as_f32()
            .filter(|v| v.is_finite())
            .ok_or_else(|| {
                PdfError::ColorError(format!("Color parameter {key} requires finite numbers"))
            })?;
    }
    Ok(Some(values))
}

#[derive(Debug, Clone)]
pub struct CalRgb {
    gamma: [f32; 3],
    white_point: [f32; 3],
    black_point: [f32; 3],
    matrix: [f32; 9],
}

impl Default for CalRgb {
    fn default() -> Self {
        CalRgb {
            gamma: [1.0, 1.0, 1.0],
            white_point: [1.0, 1.0, 1.0],
            black_point: [0.0, 0.0, 0.0],
            matrix: [1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        objects::pdf_dict::PdfDict,
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::{collections::HashMap, io::Cursor};

    #[test]
    fn calrgb_invalid_parameters_and_components_return_errors() {
        let parser = PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let ctx = PDFContext::try_new(
            parser,
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let parse = |source: &str| {
            let obj = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap();
            CalRgb::try_new(&obj, &ctx)
        };
        let color = parse("[/CalRGB << /WhitePoint [0.95 1 1.09] /Gamma [2 2 2] >>]").unwrap();
        assert!(color.rgb(&color.default_value()).is_ok());
        assert!(color.rgb(&ColorValue::new(vec![])).is_err());
        assert!(color.rgb(&ColorValue::new(vec![f32::NAN; 3])).is_err());
        for source in [
            "null",
            "[/CalRGB 1]",
            "<< >>",
            "<< /WhitePoint [1 1 1] /Gamma [] >>",
            "<< /WhitePoint [1 1 1] /Gamma [0 1 1] >>",
            "<< /WhitePoint [1 /bad 1] >>",
            "<< /WhitePoint [1 2 1] >>",
            "<< /WhitePoint [1 1 1] /Matrix [1] >>",
            "<< /WhitePoint [1 1 1] /BlackPoint [-1 0 0] >>",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }
}

impl CalRgb {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        let parameters = match &obj {
            PdfObject::PdfArray(array) if array.len() == 2 => ctx.resolve(
                array
                    .get(1)
                    .ok_or_else(|| PdfError::ColorError("Missing CalRGB parameters".into()))?,
            )?,
            PdfObject::PdfDict(_) => obj,
            _ => {
                return Err(PdfError::ColorError(
                    "CalRGB requires a two-element array or parameter dictionary".into(),
                ));
            }
        };
        let dict = parameters
            .as_dict()
            .ok_or_else(|| PdfError::ColorError("CalRGB parameters must be a dictionary".into()))?;
        let mut color = CalRgb::default();
        color.white_point = read_parameter::<R, 3>(dict, "WhitePoint", ctx)?
            .ok_or_else(|| PdfError::ColorError("CalRGB requires WhitePoint".into()))?;
        if let Some(value) = read_parameter::<R, 3>(dict, "Gamma", ctx)? {
            color.gamma = value;
        }
        if let Some(value) = read_parameter::<R, 3>(dict, "BlackPoint", ctx)? {
            color.black_point = value;
        }
        if let Some(value) = read_parameter::<R, 9>(dict, "Matrix", ctx)? {
            color.matrix = value;
        }
        if color.white_point[0] <= 0.0
            || color.white_point[1] != 1.0
            || color.white_point[2] <= 0.0
            || color.black_point.iter().any(|v| *v < 0.0)
            || color.gamma.iter().any(|v| *v <= 0.0)
        {
            return Err(PdfError::ColorError(
                "Invalid CalRGB white/black point or Gamma".into(),
            ));
        }
        Ok(color)
    }
    pub fn default_value(&self) -> ColorValue {
        ColorValue::new(vec![0.0, 0.0, 0.0])
    }

    pub fn number_of_components(&self) -> usize {
        3
    }

    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        if value.value_size() != 3 || value.values().iter().any(|v| !v.is_finite()) {
            return Err(PdfError::ColorError(
                "CalRGB requires three finite components".into(),
            ));
        }
        let a = value.values()[0].clamp(0.0, 1.0);
        let b = value.values()[1].clamp(0.0, 1.0);
        let c = value.values()[2].clamp(0.0, 1.0);
        let gr = a.powf(self.gamma[0]);
        let gg = b.powf(self.gamma[1]);
        let gb = c.powf(self.gamma[2]);

        let x = self.matrix[0] * gr + self.matrix[3] * gg + self.matrix[6] * gb;
        let y = self.matrix[1] * gr + self.matrix[4] * gg + self.matrix[7] * gb;
        let z = self.matrix[2] * gr + self.matrix[5] * gg + self.matrix[8] * gb;
        let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
        let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
        let b = 0.0557 * x - 0.2040 * y + 1.0570 * z;
        if !r.is_finite() || !g.is_finite() || !b.is_finite() {
            return Err(PdfError::ColorError("CalRGB conversion overflow".into()));
        }
        Ok(ColorRgb::new(r, g, b))
    }
}
