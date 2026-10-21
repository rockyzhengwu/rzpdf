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
    pub(crate) fn retained_profile_bytes(&self) -> PdfResult<usize> {
        self.base.retained_profile_bytes()
    }
    pub fn default_value(&self) -> ColorValue {
        ColorValue::new(vec![0.0])
    }

    pub fn try_new<R: Seek + Read>(arr: &PdfArray, ctx: &PDFContext<R>) -> PdfResult<Self> {
        if arr.len() != 4 {
            return Err(PdfError::ColorError(
                "Indexed requires four array entries".into(),
            ));
        }
        let base_obj = arr
            .get(1)
            .ok_or_else(|| PdfError::ColorError("Missing Indexed base".into()))?;
        let base = ColorSpace::try_new(base_obj, ctx)?;
        if matches!(base, ColorSpace::Indexed(_) | ColorSpace::Pattern(_)) {
            return Err(PdfError::ColorError(
                "Invalid Indexed base color space".into(),
            ));
        }
        let hival_obj = ctx.resolve(
            arr.get(2)
                .ok_or_else(|| PdfError::ColorError("Missing Indexed hival".into()))?,
        )?;
        let hival = hival_obj
            .as_number()
            .and_then(|n| n.as_u32_checked().ok())
            .and_then(|n| u8::try_from(n).ok())
            .ok_or_else(|| {
                PdfError::ColorError("Indexed hival must be an integer from 0 to 255".into())
            })?;
        let lookup_obj = ctx.resolve(
            arr.get(3)
                .ok_or_else(|| PdfError::ColorError("Missing Indexed lookup".into()))?,
        )?;
        let lookup = match lookup_obj {
            PdfObject::PdfStream(stream) => stream.decode_data(ctx)?,
            PdfObject::PdfString(string) => {
                crate::filter::check_output_size(
                    0,
                    string.bytes().len(),
                    ctx.limits().max_decoded_bytes,
                    "Indexed lookup bytes",
                )?;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(string.bytes().len())
                    .map_err(|_| PdfError::ResourceLimit("Indexed lookup allocation".into()))?;
                bytes.extend_from_slice(string.bytes());
                bytes
            }
            _ => {
                return Err(PdfError::ColorError(
                    "Indexed lookup requires a stream or string".into(),
                ));
            }
        };
        let required = (usize::from(hival) + 1) * base.number_of_components()?;
        if lookup.len() < required {
            return Err(PdfError::ColorError(
                "Indexed lookup table is truncated".into(),
            ));
        }
        Ok(Self {
            base: Box::new(base),
            hival,
            lookup,
        })
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        self.rgb_with_context(value, &super::conversion::ColorConversionContext::default())
    }
    pub fn rgb_with_context(
        &self,
        value: &ColorValue,
        context: &super::conversion::ColorConversionContext,
    ) -> PdfResult<ColorRgb> {
        self.base
            .rgb_with_context(&self.base_value(value)?, context)
    }

    pub(crate) fn base(&self) -> &ColorSpace {
        &self.base
    }

    /// Expand the palette before any image Matte unblending. Palette indices
    /// themselves are not preblended colour components (ISO 32000, 11.6.5.3).
    pub(crate) fn base_value(&self, value: &ColorValue) -> PdfResult<ColorValue> {
        if value.value_size() != 1 {
            return Err(PdfError::ColorError("Indexed requires one index".into()));
        }
        let v = *value
            .values()
            .first()
            .ok_or_else(|| PdfError::ColorError("Indexed color requires an index".to_string()))?;
        if !v.is_finite() {
            return Err(PdfError::ColorError(
                "Indexed index must be finite".to_string(),
            ));
        }
        if matches!(*self.base, ColorSpace::Pattern(_) | ColorSpace::Indexed(_)) {
            return Err(PdfError::UnsupportedFeature(
                "Indexed base color space".to_string(),
            ));
        }
        let pos = (v.round() as usize).min(self.hival as usize);
        let components = self.base.number_of_components()?;
        let pos = pos * components;
        let entry = self
            .lookup
            .get(pos..pos + components)
            .ok_or_else(|| PdfError::ColorError("Indexed lookup table is truncated".to_string()))?;
        let lab_ranges = match self.base.as_ref() {
            ColorSpace::Lab(lab) => Some(lab.component_ranges()),
            _ => None,
        };
        let ranges = match self.base.as_ref() {
            ColorSpace::IccBased(icc) => Some(icc.range()),
            _ => lab_ranges.as_ref().map(|ranges| ranges.as_slice()),
        };
        if ranges.is_some_and(|range| range.len() != components * 2) {
            return Err(PdfError::ColorError(
                "Indexed base component range mismatch".into(),
            ));
        }
        let mut cvs = Vec::new();
        cvs.try_reserve_exact(components)
            .map_err(|_| PdfError::ResourceLimit("Indexed component allocation".into()))?;
        for (index, &sample) in entry.iter().enumerate() {
            let (low, high) =
                ranges.map_or((0.0, 1.0), |range| (range[index * 2], range[index * 2 + 1]));
            // Use wide intermediates for finite, widely separated endpoints.
            cvs.push(
                (f64::from(low) + f64::from(sample) / 255.0 * (f64::from(high) - f64::from(low)))
                    as f32,
            );
        }
        Ok(ColorValue::new(cvs))
    }

    pub fn number_of_components(&self) -> usize {
        1
    }
    pub fn is_nonpainting(&self) -> bool {
        self.base.is_nonpainting()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::device_rgb::DeviceRgb;

    #[test]
    fn icc_palette_bytes_use_declared_ranges_before_profile_conversion() {
        use crate::{
            color::icc_based::IccBased,
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::{options::ParseOptions, syntax::SyntaxParser},
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (source, lookup, expected) in [
            (
                "<< /N 1 /Range [0.25 0.75] >>",
                vec![0, 255],
                [[64, 64, 64], [191, 191, 191]],
            ),
            (
                "<< /N 3 /Range [0 0.5 0 1 0 0.25] >>",
                vec![0, 0, 0, 255, 128, 255],
                // The encoded sRGB matrix/TRC, unlike the old Alternate
                // shortcut, puts red just below the 127.5 byte rounding tie.
                [[0, 0, 0], [127, 128, 64]],
            ),
        ] {
            let dict = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            let profile = if dict.get("N").and_then(PdfObject::as_u32) == Some(1) {
                crate::color::icc_profile::ICCProfile::gray_srgb().unwrap()
            } else {
                crate::color::icc_profile::ICCProfile::srgb()
            };
            let icc = IccBased::try_new(
                &PdfObject::PdfStream(PdfStream::new(dict, profile.to_bytes().unwrap())),
                &ctx,
            )
            .unwrap();
            let indexed = Indexed {
                base: Box::new(ColorSpace::IccBased(icc)),
                hival: 1,
                lookup,
            };
            for (index, expected) in expected.into_iter().enumerate() {
                let actual = indexed.rgb(&ColorValue::new(vec![index as f32])).unwrap();
                assert_eq!([actual.r(), actual.g(), actual.b()], expected);
            }
        }
    }

    #[test]
    fn lab_palette_bytes_map_to_l_and_declared_ab_ranges() {
        use crate::{
            io::stream_reader::StreamReader,
            objects::pdf_dict::PdfDict,
            parser::{options::ParseOptions, syntax::SyntaxParser},
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (range, lookup, expected) in [
            (
                "[0 0 0 0]",
                "000000FFFFFF",
                [[0.0, 0.0, 0.0], [100.0, 0.0, 0.0]],
            ),
            (
                "[-20 40 -30 60]",
                "00FF00FF00FF",
                [[0.0, 40.0, -30.0], [100.0, -20.0, 60.0]],
            ),
        ] {
            let source = format!(
                "[/Indexed [/Lab << /WhitePoint [0.95047 1 1.08883] /Range {range} >>] 1 <{lookup}>]"
            );
            let object =
                SyntaxParser::new(StreamReader::try_new(Cursor::new(source.into_bytes())).unwrap())
                    .get_object()
                    .unwrap();
            let indexed = Indexed::try_new(object.as_array().unwrap(), &ctx).unwrap();
            for (index, values) in expected.into_iter().enumerate() {
                let actual = indexed.rgb(&ColorValue::new(vec![index as f32])).unwrap();
                let expected = indexed.base.rgb(&ColorValue::new(values.to_vec())).unwrap();
                assert_eq!(
                    [actual.r(), actual.g(), actual.b()],
                    [expected.r(), expected.g(), expected.b()]
                );
            }
            if range == "[0 0 0 0]" {
                let black = indexed.rgb(&ColorValue::new(vec![0.0])).unwrap();
                let white = indexed.rgb(&ColorValue::new(vec![1.0])).unwrap();
                assert_eq!([black.r(), black.g(), black.b()], [0, 0, 0]);
                assert_eq!([white.r(), white.g(), white.b()], [255, 255, 255]);
            }
        }
    }

    #[test]
    fn indexed_constructor_checks_integer_bounds_and_lookup_length() {
        use crate::{
            document::cross_ref_table::CrossRefTable,
            io::stream_reader::StreamReader,
            objects::pdf_dict::PdfDict,
            parser::{parser::PdfParser, syntax::SyntaxParser},
        };
        use std::{collections::HashMap, io::Cursor};
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
            ColorSpace::try_new(&obj, &ctx)
        };
        assert!(parse("[/Indexed /DeviceRGB 1 <000000FFFFFF>]").is_ok());
        for source in [
            "[/Indexed /DeviceRGB 256 <000000>]",
            "[/Indexed /DeviceRGB -1 <000000>]",
            "[/Indexed /DeviceRGB 1.5 <000000>]",
            "[/Indexed /DeviceRGB 1 <000000>]",
            "[/Indexed /Pattern 0 <00>]",
            "[/Indexed /DeviceGray 0 1]",
            "[/Indexed /DeviceGray 0 <00> null]",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
        let indexed = Indexed {
            base: Box::new(ColorSpace::DeviceRgb(DeviceRgb::new())),
            hival: 1,
            lookup: vec![0, 0, 0, 255, 255, 255],
        };
        assert_eq!(indexed.rgb(&ColorValue::new(vec![0.2])).unwrap().r(), 0);
        assert_eq!(indexed.rgb(&ColorValue::new(vec![0.8])).unwrap().r(), 255);
        assert!(indexed.rgb(&ColorValue::new(vec![0.0, 1.0])).is_err());
        let options = crate::parser::options::ParseOptions {
            limits: crate::parser::options::Limits {
                max_decoded_bytes: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let ctx = PDFContext::try_new(
            PdfParser::new(SyntaxParser::with_options(
                StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
                options,
            )),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        for (source, allowed) in [
            ("[/Indexed /DeviceRGB 0 <010203>]", true),
            ("[/Indexed /DeviceRGB 0 <01020304>]", false),
        ] {
            let obj = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap();
            let result = ColorSpace::try_new(&obj, &ctx);
            if allowed {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
            }
        }
    }

    #[test]
    fn invalid_index_and_truncated_lookup_return_errors() {
        let indexed = Indexed {
            base: Box::new(ColorSpace::DeviceRgb(DeviceRgb::new())),
            hival: 1,
            lookup: vec![0, 0, 0],
        };
        assert!(indexed.rgb(&ColorValue::new(vec![])).is_err());
        assert!(indexed.rgb(&ColorValue::new(vec![f32::NAN])).is_err());
        assert!(indexed.rgb(&ColorValue::new(vec![1.0])).is_err());
        assert!(indexed.rgb(&indexed.default_value()).is_ok());
        assert_eq!(indexed.default_value().values(), &[0.0]);
    }
}
