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

    fn parse(source: &str) -> Lab {
        let obj = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
        )
        .get_object()
        .unwrap();
        Lab::try_new(&obj).unwrap()
    }

    #[test]
    fn lab_neutral_black_white_and_midgray_adapt_to_srgb() {
        for white in ["0.95045593 1 1.08905775", "0.96429568 1 0.8251046", "1 1 1"] {
            let lab = parse(&format!("<< /WhitePoint [{white}] >>"));
            for (l, expected) in [(0., 0.), (100., 1.), (50., 0.4663266), (8., 0.09221228)] {
                let rgb = lab.rgb(&ColorValue::new(vec![l, 0., 0.])).unwrap();
                for component in [rgb.r, rgb.g, rgb.b] {
                    assert!(
                        (component - expected).abs() < 0.00002,
                        "{white}, L={l}: {component}"
                    );
                }
            }
        }
    }

    #[test]
    fn lab_d65_srgb_primaries_match_reference_values() {
        let lab = parse("<< /WhitePoint [0.95047 1 1.08883] /Range [-128 128 -128 128] >>");
        for (value, expected) in [
            ([53.2408, 80.0925, 67.2032], [255u8, 0, 0]),
            ([87.7347, -86.1827, 83.1793], [0, 255, 0]),
            ([32.297, 79.1875, -107.8602], [0, 0, 255]),
        ] {
            let rgb = lab.rgb(&ColorValue::new(value.to_vec())).unwrap();
            for (actual, expected) in [rgb.r(), rgb.g(), rgb.b()].into_iter().zip(expected) {
                assert!(actual.abs_diff(expected) <= 1);
            }
        }
    }

    #[test]
    fn lab_components_and_range_clip_without_nonfinite_output() {
        let lab = parse("<< /WhitePoint [0.96429568 1 0.8251046] /Range [-20 30 -40 50] >>");
        let clipped = lab.rgb(&ColorValue::new(vec![100., 30., -40.])).unwrap();
        let outside = lab.rgb(&ColorValue::new(vec![1000., 500., -500.])).unwrap();
        assert_eq!(
            [clipped.r, clipped.g, clipped.b],
            [outside.r, outside.g, outside.b]
        );
        for input in [
            vec![],
            vec![50.],
            vec![50., 0.],
            vec![50., 0., 0., 0.],
            vec![f32::NAN; 3],
            vec![f32::INFINITY; 3],
        ] {
            assert!(matches!(
                lab.rgb(&ColorValue::new(input)),
                Err(PdfError::ColorError(_))
            ));
        }
        for component in [outside.r, outside.g, outside.b] {
            assert!(component.is_finite() && (0.0..=1.0).contains(&component));
        }
    }

    #[test]
    fn lab_nonzero_black_and_unadaptable_white_are_explicit_errors() {
        let lab = parse("<< /WhitePoint [1 1 1] /BlackPoint [0.01 0.01 0.01] >>");
        assert!(matches!(
            lab.rgb(&ColorValue::new(vec![50., 0., 0.])),
            Err(PdfError::UnsupportedFeature(_))
        ));
        let lab = parse("<< /WhitePoint [100 1 1] >>");
        assert!(matches!(
            lab.rgb(&ColorValue::new(vec![50., 0., 0.])),
            Err(PdfError::ColorError(_))
        ));
    }

    #[test]
    fn indirect_lab_space_converts_after_save_and_strict_reopen() {
        use crate::{
            color::colorspace::ColorSpace, document::PdfDocument,
            objects::pdf_reference::PdfReference, parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        let object = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let y = doc.try_insert_object(object("1")).unwrap();
        let white = doc
            .try_insert_object(object(&format!(
                "[0.96429568 {} {} R 0.8251046]",
                y.obj_num(),
                y.gen_num()
            )))
            .unwrap();
        let parameters = doc
            .try_insert_object(object(&format!(
                "<< /WhitePoint {} {} R >>",
                white.obj_num(),
                white.gen_num()
            )))
            .unwrap();
        let id = doc
            .try_insert_object(object(&format!(
                "[/Lab {} {} R]",
                parameters.obj_num(),
                parameters.gen_num()
            )))
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [&doc, &reopened] {
            let reference = PdfObject::PdfReference(PdfReference::from_id(id));
            let space = ColorSpace::try_new(&reference, doc.ctx()).unwrap();
            let rgb = space.rgb(&ColorValue::new(vec![50., 0., 0.])).unwrap();
            assert_eq!([rgb.r(), rgb.g(), rgb.b()], [119; 3]);
        }
    }

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

    #[test]
    fn lab_context_checks_array_lengths_before_resolving_members() {
        use crate::{document::PdfDocument, objects::pdf_reference::PdfReference};
        let doc = PdfDocument::new().unwrap();
        let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
        doc.update_object(id, PdfObject::PdfReference(PdfReference::from_id(id)))
            .unwrap();
        for key in ["WhitePoint", "BlackPoint", "Range"] {
            let source = if key == "WhitePoint" {
                format!("<< /WhitePoint [{} {} R] >>", id.obj_num(), id.gen_num())
            } else {
                format!(
                    "<< /WhitePoint [1 1 1] /{key} [{} {} R] >>",
                    id.obj_num(),
                    id.gen_num()
                )
            };
            let object =
                SyntaxParser::new(StreamReader::try_new(Cursor::new(source.into_bytes())).unwrap())
                    .get_object()
                    .unwrap();
            assert!(matches!(
                Lab::try_new_with_context(&object, doc.ctx()),
                Err(PdfError::ColorError(_))
            ));
        }
    }

    #[test]
    fn lab_direct_and_context_parameters_share_validation_and_defaults() {
        use crate::document::PdfDocument;
        let doc = PdfDocument::new().unwrap();
        for source in [
            "<< /WhitePoint [0.96429568 1 0.8251046] >>",
            "<< /WhitePoint [1 1 1] /Range [-20 30 -40 50] >>",
            "<< /WhitePoint [1 2 1] >>",
            "<< /WhitePoint [1 1 1] /BlackPoint [-1 0 0] >>",
            "<< /WhitePoint [1 1 1] /Range [1 0 -10 10] >>",
            "<< /WhitePoint [1 1 1] /Range null >>",
        ] {
            let object = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap();
            let direct = Lab::try_new(&object);
            let contextual = Lab::try_new_with_context(&object, doc.ctx());
            match (direct, contextual) {
                (Ok(a), Ok(b)) => {
                    assert_eq!(a.white_point, b.white_point);
                    assert_eq!(a.black_point, b.black_point);
                    assert_eq!(a.range, b.range);
                }
                (Err(PdfError::ColorError(_)), Err(PdfError::ColorError(_))) => {}
                other => panic!("constructor mismatch for {source}: {other:?}"),
            }
        }
    }

    #[test]
    fn lab_arrays_check_direct_and_indirect_color_space_names() {
        use crate::{document::PdfDocument, objects::pdf_reference::PdfReference};
        let doc = PdfDocument::new().unwrap();
        let object = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        for name in [
            "/DeviceRGB",
            "/CalRGB",
            "/Wrong",
            "1",
            "null",
            "(Lab)",
            "true",
        ] {
            let array = object(&format!("[{name} << /WhitePoint [1 1 1] >>]"));
            assert!(matches!(Lab::try_new(&array), Err(PdfError::ColorError(_))));
            assert!(matches!(
                Lab::try_new_with_context(&array, doc.ctx()),
                Err(PdfError::ColorError(_))
            ));
        }
        let good = object("[/Lab << /WhitePoint [1 1 1] >>]");
        assert!(Lab::try_new(&good).is_ok());
        assert!(Lab::try_new_with_context(&good, doc.ctx()).is_ok());
        let name = doc.try_insert_object(object("/Lab")).unwrap();
        let array = object(&format!(
            "[{} {} R << /WhitePoint [1 1 1] >>]",
            name.obj_num(),
            name.gen_num()
        ));
        assert!(Lab::try_new_with_context(&array, doc.ctx()).is_ok());
        assert!(Lab::try_new(&array).is_err()); // No resolver in the direct API.
        doc.update_object(name, object("/DeviceRGB")).unwrap();
        assert!(matches!(
            Lab::try_new_with_context(&array, doc.ctx()),
            Err(PdfError::ColorError(_))
        ));
        doc.update_object(name, PdfObject::PdfReference(PdfReference::from_id(name)))
            .unwrap();
        assert!(Lab::try_new_with_context(&array, doc.ctx()).is_err());
        doc.update_object(name, object("/Lab")).unwrap();
        assert!(Lab::try_new_with_context(&array, doc.ctx()).is_ok());
    }
}

impl Lab {
    pub fn try_new_with_context<R: std::io::Read + std::io::Seek>(
        obj: &PdfObject,
        ctx: &crate::pdf_context::PDFContext<R>,
    ) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        let parameters = match &obj {
            PdfObject::PdfArray(array) if array.len() == 2 => {
                let name = ctx.resolve(array.get(0).ok_or_else(|| {
                    PdfError::ColorError("Missing Lab color space name".into())
                })?)?;
                Self::validate_space_name(&name)?;
                ctx.resolve(
                    array
                        .get(1)
                        .ok_or_else(|| PdfError::ColorError("Missing Lab parameters".into()))?,
                )?
            }
            PdfObject::PdfDict(_) => obj,
            _ => {
                return Err(PdfError::ColorError(
                    "Lab requires a two-element color space array".into(),
                ));
            }
        };
        let dict = parameters
            .as_dict()
            .ok_or_else(|| PdfError::ColorError("Lab parameters must be a dictionary".into()))?;
        let color = Self {
            white_point: super::cal_rgb::read_parameter::<R, 3>(dict, "WhitePoint", ctx)?
                .ok_or_else(|| PdfError::ColorError("Lab WhitePoint is required".into()))?,
            black_point: super::cal_rgb::read_parameter::<R, 3>(dict, "BlackPoint", ctx)?
                .unwrap_or([0.0; 3]),
            range: super::cal_rgb::read_parameter::<R, 4>(dict, "Range", ctx)?
                .unwrap_or([-100.0, 100.0, -100.0, 100.0]),
        };
        color.validate()?;
        Ok(color)
    }

    pub fn try_new(obj: &PdfObject) -> PdfResult<Self> {
        let mut color = Lab::default();
        let parameters =
            match obj {
                PdfObject::PdfArray(array) if array.len() == 2 => {
                    Self::validate_space_name(array.get(0).ok_or_else(|| {
                        PdfError::ColorError("Missing Lab color space name".into())
                    })?)?;
                    array
                        .get(1)
                        .ok_or_else(|| PdfError::ColorError("Missing Lab parameters".into()))?
                }
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

        color.validate()?;
        Ok(color)
    }

    fn validate(&self) -> PdfResult<()> {
        if self
            .white_point
            .iter()
            .chain(self.black_point.iter())
            .chain(self.range.iter())
            .any(|v| !v.is_finite())
            || self.white_point[0] <= 0.0
            || self.white_point[1] != 1.0
            || self.white_point[2] <= 0.0
            || self.black_point.iter().any(|v| *v < 0.0)
            || self.range[0] > self.range[1]
            || self.range[2] > self.range[3]
        {
            return Err(PdfError::ColorError(
                "Invalid Lab white/black point or Range".into(),
            ));
        }
        Ok(())
    }

    fn validate_space_name(name: &PdfObject) -> PdfResult<()> {
        if name.as_name().map(|name| name.name()) != Some("Lab") {
            return Err(PdfError::ColorError(
                "Lab array requires /Lab color space name".into(),
            ));
        }
        Ok(())
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
    pub(crate) fn component_ranges(&self) -> [f32; 6] {
        [
            0.0,
            100.0,
            self.range[0],
            self.range[1],
            self.range[2],
            self.range[3],
        ]
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        if value.value_size() != 3 || value.values().iter().any(|v| !v.is_finite()) {
            return Err(PdfError::ColorError(
                "Lab requires three finite components".into(),
            ));
        }
        if self.black_point != [0.0; 3] {
            return Err(PdfError::UnsupportedFeature(
                "Lab nonzero BlackPoint compensation".into(),
            ));
        }
        let l = f64::from(value.values()[0].clamp(0.0, 100.0));
        let a = f64::from(value.values()[1].clamp(self.range[0], self.range[1]));
        let b = f64::from(value.values()[2].clamp(self.range[2], self.range[3]));
        let y = (l + 16.0) / 116.0;
        let inverse_lab = |v: f64| {
            if v >= 6.0 / 29.0 {
                v * v * v
            } else {
                108.0 / 841.0 * (v - 4.0 / 29.0)
            }
        };
        let white = self.white_point.map(f64::from);
        let xyz = [
            inverse_lab(y + a / 500.0) * white[0],
            inverse_lab(y),
            inverse_lab(y - b / 200.0) * white[2],
        ];
        let multiply = |matrix: [[f64; 3]; 3], vector: [f64; 3]| {
            matrix.map(|row| row[0] * vector[0] + row[1] * vector[1] + row[2] * vector[2])
        };
        // Bradford adaptation to the sRGB D65 white. This is a fixed display
        // policy, not OutputIntent/ICC rendering-intent processing.
        let bradford = [
            [0.8951, 0.2664, -0.1614],
            [-0.7502, 1.7135, 0.0367],
            [0.0389, -0.0685, 1.0296],
        ];
        let source = multiply(bradford, white);
        let destination = multiply(
            bradford,
            [0.3127 / 0.3290, 1.0, (1.0 - 0.3127 - 0.3290) / 0.3290],
        );
        if source.iter().any(|v| !v.is_finite() || *v <= 1e-12) {
            return Err(PdfError::ColorError(
                "Lab white point cannot be adapted".into(),
            ));
        }
        let mut cone = multiply(bradford, xyz);
        for i in 0..3 {
            cone[i] *= destination[i] / source[i];
        }
        let adapted = multiply(
            [
                [
                    0.9869929054667123,
                    -0.14705425642099013,
                    0.15996265166373125,
                ],
                [0.4323052697233945, 0.5183602715367776, 0.0492912282128556],
                [
                    -0.008528664575177328,
                    0.04004282165408487,
                    0.9684866957875502,
                ],
            ],
            cone,
        );
        let linear = multiply(
            [
                [12831.0 / 3959.0, -329.0 / 214.0, -1974.0 / 3959.0],
                [
                    -851781.0 / 878810.0,
                    1648619.0 / 878810.0,
                    36519.0 / 878810.0,
                ],
                [705.0 / 12673.0, -2585.0 / 12673.0, 705.0 / 667.0],
            ],
            adapted,
        );
        if linear.iter().any(|v| !v.is_finite()) {
            return Err(PdfError::ColorError("Lab conversion overflow".into()));
        }
        // Channel clipping is intentional; perceptual gamut mapping remains
        // outside this low-level conversion API.
        let rgb = linear.map(|v| {
            let v = v.clamp(0.0, 1.0);
            (if v <= 0.0031308 {
                12.92 * v
            } else {
                1.055 * v.powf(1.0 / 2.4) - 0.055
            }) as f32
        });
        Ok(ColorRgb::new(rgb[0], rgb[1], rgb[2]))
    }

    pub fn range(&self) -> &[f32] {
        &self.range
    }
}
