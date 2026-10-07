use crate::color::value::ColorValue;
use crate::error::{PdfError, PdfResult};
use crate::objects::PdfObject;

use super::value::ColorRgb;

#[derive(Debug, Clone)]
pub struct Lab {
    white_point: [f32; 3],
    black_point: [f32; 3],
    range: [f32; 4],
}

impl Default for Lab {
    fn default() -> Self {
        Lab {
            white_point: [1.0, 1.0, 1.0],
            black_point: [0.0, 0.0, 0.0],
            range: [-100.0, 100.0, -100.0, 100.0],
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{io::stream_reader::StreamReader, parser::syntax::SyntaxParser};
    use std::io::Cursor;

    #[test]
    fn lab_color_array_and_invalid_parameters_return_results() {
        let parse = |source: &str| {
            let obj = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap();
            Lab::try_new(&obj)
        };
        let lab = parse("[/Lab << /WhitePoint [0.95 1 1.09] /Range [10 20 -30 -10] >>]").unwrap();
        assert_eq!(lab.default_value().values(), &[0.0, 10.0, -10.0]);
        for source in [
            "null",
            "[/Lab]",
            "[/Lab 1]",
            "<< >>",
            "<< /WhitePoint [1 1] >>",
            "<< /WhitePoint [1 /bad 1] >>",
            "<< /WhitePoint [1 2 1] >>",
            "<< /WhitePoint [1 1 1] /Range [1 0 0 1] >>",
            "<< /WhitePoint [1 1 1] /BlackPoint [-1 0 0] >>",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
    }
}

impl Lab {
    pub fn try_new_with_context<R: std::io::Read + std::io::Seek>(
        obj: &PdfObject,
        ctx: &crate::pdf_context::PDFContext<R>,
    ) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        let parameters = match &obj {
            PdfObject::PdfArray(array) if array.len() == 2 => ctx.resolve(
                array
                    .get(1)
                    .ok_or_else(|| PdfError::ColorError("Missing Lab parameters".into()))?,
            )?,
            PdfObject::PdfDict(_) => obj,
            _ => {
                return Err(PdfError::ColorError(
                    "Lab requires a two-element color space array".into(),
                ));
            }
        };
        let mut dict = parameters
            .into_dict()
            .ok_or_else(|| PdfError::ColorError("Lab parameters must be a dictionary".into()))?;
        for key in ["WhitePoint", "BlackPoint", "Range"] {
            if let Some(value) = dict.get(key) {
                let mut resolved = ctx.resolve(value)?;
                if let PdfObject::PdfArray(array) = &mut resolved {
                    let mut values = crate::objects::pdf_array::PdfArray::default();
                    for value in &*array {
                        values.add_obj(ctx.resolve(value)?);
                    }
                    *array = values;
                }
                dict.insert(key.into(), resolved);
            }
        }
        Self::try_new(&PdfObject::PdfDict(dict))
    }

    pub fn try_new(obj: &PdfObject) -> PdfResult<Self> {
        let mut color = Lab::default();
        let parameters = match obj {
            PdfObject::PdfArray(array) if array.len() == 2 => array
                .get(1)
                .ok_or_else(|| PdfError::ColorError("Missing Lab parameters".into()))?,
            _ => obj,
        };
        let dict = parameters
            .as_dict()
            .ok_or_else(|| PdfError::ColorError("Lab parameters must be a dictionary".into()))?;
        if let Some(r) = dict.get("Range") {
            let ra = r.as_array().ok_or(PdfError::ColorError(format!(
                "Lab Range need an array got:{:?}",
                r
            )))?;
            if ra.len() != 4 {
                return Err(PdfError::ColorError(format!(
                    "Lab Range need 4 elements :{:?}",
                    ra
                )));
            }
            for i in 0..4 {
                color.range[i] =
                    ra.get(i)
                        .unwrap()
                        .as_f32()
                        .ok_or(PdfError::ColorError(format!(
                            "Lab Range element need number got:{:?}",
                            ra.get(i)
                        )))?;
            }
        }

        match dict.get("WhitePoint") {
            Some(wh) => {
                let wha = wh.as_array().ok_or(PdfError::ColorError(format!(
                    "Lab  WhitePoint is not an array got:{:?}",
                    obj
                )))?;
                if wha.len() != 3 {
                    return Err(PdfError::ColorError(format!(
                        "Lab WhitePoint need 3 elements :{:?}",
                        wha
                    )));
                }
                for i in 0..3 {
                    let v = wha
                        .get(i)
                        .unwrap()
                        .as_f32()
                        .ok_or(PdfError::ColorError(format!(
                            "Lab WhitePoint {:?} value is not an number:{:?}",
                            i,
                            wha.get(i)
                        )))?;
                    color.white_point[i] = v;
                }
            }
            None => {
                return Err(PdfError::ColorError(
                    "Lab WhitePoint is required".to_string(),
                ));
            }
        }
        if let Some(bp) = dict.get("BlackPoint") {
            let bpa = bp.as_array().ok_or(PdfError::ColorError(format!(
                "Lab BlackPoint is not an array got:{:?}",
                obj
            )))?;
            if bpa.len() != 3 {
                return Err(PdfError::ColorError(format!(
                    "Lab BlackPoint need 3 elements :{:?}",
                    bpa
                )));
            }
            for i in 0..3 {
                let v = bpa
                    .get(i)
                    .unwrap()
                    .as_f32()
                    .ok_or(PdfError::ColorError(format!(
                        "Lab BlackPoint {:?} value is not an number:{:?}",
                        i,
                        bpa.get(i)
                    )))?;
                color.black_point[i] = v;
            }
        }

        if color
            .white_point
            .iter()
            .chain(color.black_point.iter())
            .chain(color.range.iter())
            .any(|v| !v.is_finite())
            || color.white_point[0] <= 0.0
            || color.white_point[1] != 1.0
            || color.white_point[2] <= 0.0
            || color.black_point.iter().any(|v| *v < 0.0)
            || color.range[0] > color.range[1]
            || color.range[2] > color.range[3]
        {
            return Err(PdfError::ColorError(
                "Invalid Lab white/black point or Range".into(),
            ));
        }
        Ok(color)
    }

    pub fn default_value(&self) -> ColorValue {
        let mut default = vec![0.0, 0.0, 0.0];
        if self.range[0] > 0.0 {
            default[1] = self.range[0];
        } else if self.range[1] < 0.0 {
            default[1] = self.range[1];
        }

        if self.range[2] > 0.0 {
            default[2] = self.range[2];
        } else if self.range[3] < 0.0 {
            default[2] = self.range[3];
        }
        ColorValue::new(default)
    }

    pub fn number_of_components(&self) -> usize {
        3
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        Err(PdfError::UnsupportedFeature("Lab color conversion".into()))
    }

    pub fn range(&self) -> &[f32] {
        &self.range
    }
}
