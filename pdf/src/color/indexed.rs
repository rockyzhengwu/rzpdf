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
        if matches!(
            *self.base,
            ColorSpace::Pattern(_) | ColorSpace::DeviceN(_) | ColorSpace::Indexed(_)
        ) {
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
        let cvs = entry.iter().map(|n| *n as f32 / 255.0).collect();
        let value = ColorValue::new(cvs);
        self.base.rgb(&value)
    }

    pub fn number_of_components(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::device_rgb::DeviceRgb;

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
