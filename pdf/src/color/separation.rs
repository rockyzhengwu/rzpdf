use std::io::{Read, Seek};

use crate::{
    color::{
        colorspace::ColorSpace,
        value::{ColorRgb, ColorValue},
    },
    error::{PdfError, PdfResult},
    function::{Function, create_function},
    objects::pdf_array::PdfArray,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone)]
pub struct Separation {
    name: String,
    alternate_space: Box<ColorSpace>,
    tint_transform: Function,
}

impl Separation {
    pub(crate) fn retained_profile_bytes(&self) -> PdfResult<usize> {
        self.alternate_space.retained_profile_bytes()
    }
    pub fn is_nonpainting(&self) -> bool {
        self.name == "None"
    }
    pub fn try_new<R: Seek + Read>(arr: &PdfArray, ctx: &PDFContext<R>) -> PdfResult<Self> {
        if arr.len() != 4 {
            return Err(PdfError::ColorError(
                "Separation requires four array entries".into(),
            ));
        }
        let family = ctx.resolve(
            arr.get(0)
                .ok_or_else(|| PdfError::ColorError("missing Separation family".into()))?,
        )?;
        if family
            .as_name()
            .is_none_or(|name| name.name() != "Separation")
        {
            return Err(PdfError::ColorError(
                "Separation array must start with /Separation".into(),
            ));
        }
        let name_obj = ctx.resolve(arr.get(1).ok_or(PdfError::ColorError(
            "Separation Colorspace name is Nonne".to_string(),
        ))?)?;
        let name = name_obj
            .as_name()
            .ok_or(PdfError::ColorError(
                "Separation colorspace Name is not a pdfName object".to_string(),
            ))?
            .name();
        let alternate_space = arr.get(2).ok_or(PdfError::ColorError(
            "Separation ColorSpace alternate_space is None".to_string(),
        ))?;
        let alternate = ColorSpace::try_new(alternate_space, ctx)?;
        if matches!(
            alternate,
            ColorSpace::Pattern(_)
                | ColorSpace::Indexed(_)
                | ColorSpace::Separation(_)
                | ColorSpace::DeviceN(_)
        ) {
            return Err(PdfError::ColorError(
                "Invalid Separation alternate space".into(),
            ));
        }
        let tint_transform = arr.get(3).ok_or(PdfError::ColorError(
            "Separation colorspace initransform is None".to_string(),
        ))?;
        let ts = ctx.resolve(tint_transform)?;
        let tint_transform = create_function(&ts, ctx)?;
        if tint_transform.dimensions() != (1, alternate.number_of_components()?) {
            return Err(PdfError::ColorError(
                "Separation tint-transform dimensions do not match".into(),
            ));
        }
        Ok(Self {
            name: name.to_string(),
            alternate_space: Box::new(alternate),
            tint_transform,
        })
    }

    pub fn default_value(&self) -> ColorValue {
        ColorValue::new(vec![1.0])
    }

    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        self.rgb_with_context(value, &super::conversion::ColorConversionContext::default())
    }
    pub fn rgb_with_context(
        &self,
        value: &ColorValue,
        context: &super::conversion::ColorConversionContext,
    ) -> PdfResult<ColorRgb> {
        if value.value_size() != 1 || !value.values()[0].is_finite() {
            return Err(PdfError::ColorError(
                "Separation requires one finite tint".into(),
            ));
        }
        let tint = value.values()[0].clamp(0.0, 1.0);
        match self.name.as_str() {
            "All" => {
                let gray = 1.0 - tint;
                return context.convert_srgb(ColorRgb::new(gray, gray, gray));
            }
            "None" => {
                return Err(PdfError::UnsupportedFeature(
                    "None Separation requires paint suppression, not an RGB color".into(),
                ));
            }
            _ => {}
        }
        let values = self.tint_transform.eval(&[tint])?;
        self.alternate_space
            .rgb_with_context(&ColorValue::new(values), context)
    }

    pub fn number_of_components(&self) -> usize {
        1
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        objects::{PdfObject, pdf_dict::PdfDict},
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::{collections::HashMap, io::Cursor};

    #[test]
    fn direct_constructor_rejects_wrong_family_even_with_valid_parameters() {
        let doc = crate::document::PdfDocument::new().unwrap();
        for family in ["/Lab", "/DeviceN", "null", "1", "(Separation)"] {
            let object = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(
                    format!(
                        "[{family} /Spot /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]"
                    )
                    .into_bytes(),
                ))
                .unwrap(),
            )
            .get_object()
            .unwrap();
            assert!(matches!(
                Separation::try_new(object.as_array().unwrap(), doc.ctx()),
                Err(PdfError::ColorError(_))
            ));
        }
    }

    #[test]
    fn separation_stitching_tint_transform_survives_save_and_reopen() {
        use crate::{document::PdfDocument, parser::syntax::ParseMode};
        let doc = PdfDocument::new().unwrap();
        let object = SyntaxParser::new(StreamReader::try_new(Cursor::new(
            b"[/Separation /Spot /DeviceRGB << /FunctionType 3 /Domain [0 1] /Bounds [0.5] /Encode [0 1 0 1] /Functions [<< /FunctionType 2 /Domain [0 1] /N 1 /C0 [1 0 0] /C1 [0 1 0] >> << /FunctionType 2 /Domain [0 1] /N 1 /C0 [0 0 1] /C1 [1 1 1] >>] >>]".to_vec(),
        )).unwrap()).get_object().unwrap();
        let id = doc.try_insert_object(object).unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [&doc, &reopened] {
            let object = doc
                .ctx()
                .resolve_owned(&PdfObject::PdfReference(
                    crate::objects::pdf_reference::PdfReference::from_id(id),
                ))
                .unwrap();
            let color = ColorSpace::try_new(&object, doc.ctx()).unwrap();
            for (tint, expected) in [(0., [255, 0, 0]), (0.5, [0, 0, 255]), (1., [255, 255, 255])] {
                let rgb = color.rgb(&ColorValue::new(vec![tint])).unwrap();
                assert_eq!([rgb.r(), rgb.g(), rgb.b()], expected);
            }
        }
    }

    #[test]
    fn separation_tints_and_cyclic_colors_return_results() {
        let parser = PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let ctx = PDFContext::try_new(
            parser,
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let good =
            parse("[/Separation /Spot /DeviceGray << /FunctionType 2 /N 1 /Domain [0 1] >>]");
        let color = ColorSpace::try_new(&good, &ctx).unwrap();
        assert!(color.rgb(&ColorValue::new(vec![0.5])).is_ok());
        assert!(color.rgb(&ColorValue::new(vec![])).is_err());
        assert!(color.rgb(&ColorValue::new(vec![f32::NAN])).is_err());
        for source in [
            "[/Separation]",
            "[/Separation 1 /DeviceGray null]",
            "[/Separation /Spot /Pattern null]",
        ] {
            assert!(ColorSpace::try_new(&parse(source), &ctx).is_err());
        }
        let id = ctx.insert_object(PdfObject::PdfNull);
        let source = format!("[/Indexed {} {} R 0 (x)]", id.obj_num(), id.gen_num());
        let cyclic = parse(&source);
        ctx.update_object(id, cyclic.clone()).unwrap();
        for _ in 0..2 {
            assert!(matches!(
                ColorSpace::try_new(&cyclic, &ctx),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert!(ColorSpace::try_new(&good, &ctx).is_ok());
    }
}
