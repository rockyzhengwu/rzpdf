use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

use super::value::{ColorRgb, ColorValue};

#[derive(Debug, Clone)]
pub struct CalGray {
    gamma: f32,
    white_point: [f32; 3],
    black_point: [f32; 3],
}

impl Default for CalGray {
    fn default() -> Self {
        CalGray {
            gamma: 1.0,
            white_point: [1.0, 1.0, 1.0],
            black_point: [0.0, 0.0, 0.0],
        }
    }
}

impl CalGray {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        let parameters = match &obj {
            PdfObject::PdfArray(array) if array.len() == 2 => ctx.resolve(
                array
                    .get(1)
                    .ok_or_else(|| PdfError::ColorError("Missing CalGray parameters".into()))?,
            )?,
            PdfObject::PdfDict(_) => obj,
            _ => {
                return Err(PdfError::ColorError(
                    "CalGray requires a two-element array or dictionary".into(),
                ));
            }
        };
        let dict = parameters.as_dict().ok_or_else(|| {
            PdfError::ColorError("CalGray parameters must be a dictionary".into())
        })?;
        let mut color = CalGray {
            white_point: super::cal_rgb::read_parameter::<R, 3>(dict, "WhitePoint", ctx)?
                .ok_or_else(|| PdfError::ColorError("CalGray requires WhitePoint".into()))?,
            ..Default::default()
        };
        if let Some(value) = super::cal_rgb::read_parameter::<R, 3>(dict, "BlackPoint", ctx)? {
            color.black_point = value;
        }
        if let Some(gamma) = dict.get("Gamma") {
            color.gamma = ctx
                .resolve(gamma)?
                .as_f32()
                .filter(|v| v.is_finite() && *v > 0.0)
                .ok_or_else(|| {
                    PdfError::ColorError("CalGray Gamma must be positive and finite".into())
                })?;
        }
        if color.white_point[0] <= 0.0
            || color.white_point[1] != 1.0
            || color.white_point[2] <= 0.0
            || color.black_point.iter().any(|v| *v < 0.0)
        {
            return Err(PdfError::ColorError(
                "Invalid CalGray white/black point".into(),
            ));
        }
        Ok(color)
    }
    pub fn number_of_components(&self) -> usize {
        1
    }

    pub fn default_value(&self) -> ColorValue {
        ColorValue::new(vec![0.0])
    }

    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        if value.value_size() != 1 || !value.values()[0].is_finite() {
            return Err(PdfError::ColorError(
                "CalGray requires one finite component".into(),
            ));
        }
        let a = value.values()[0].clamp(0.0, 1.0);
        let x = self.white_point[0] * a.powf(self.gamma);
        let y = self.white_point[1] * a.powf(self.gamma);
        let z = self.white_point[2] * a.powf(self.gamma);
        let r = 3.2406 * x - 1.5372 * y - 0.4986 * z;
        let g = -0.9689 * x + 1.8758 * y + 0.0415 * z;
        let b = 0.0557 * x - 0.2040 * y + 1.0570 * z;
        if !r.is_finite() || !g.is_finite() || !b.is_finite() {
            return Err(PdfError::ColorError("CalGray conversion overflow".into()));
        }
        Ok(ColorRgb::new(r, g, b))
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
    fn calgray_rejects_bad_parameters_and_component_values() {
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
            CalGray::try_new(&obj, &ctx)
        };
        let color = parse("[/CalGray << /WhitePoint [0.95 1 1.09] /Gamma 2 >>]").unwrap();
        assert!(color.rgb(&color.default_value()).is_ok());
        assert!(color.rgb(&ColorValue::new(vec![])).is_err());
        assert!(color.rgb(&ColorValue::new(vec![f32::NAN])).is_err());
        assert!(color.rgb(&ColorValue::new(vec![0.0, 1.0])).is_err());
        for source in [
            "null",
            "[/CalGray]",
            "[/CalGray 1]",
            "<< >>",
            "<< /WhitePoint [1 /bad 1] >>",
            "<< /WhitePoint [1 2 1] >>",
            "<< /WhitePoint [1 1 1] /Gamma 0 >>",
            "<< /WhitePoint [1 1 1] /Gamma /bad >>",
            "<< /WhitePoint [1 1 1] /BlackPoint [-1 0 0] >>",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }
}
