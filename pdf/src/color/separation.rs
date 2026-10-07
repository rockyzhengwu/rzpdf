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

impl Separation {
    pub fn try_new<R: Seek + Read>(arr: &PdfArray, ctx: &PDFContext<R>) -> PdfResult<Self> {
        if arr.len() != 4 {
            return Err(PdfError::ColorError(
                "Separation requires four array entries".into(),
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
        Ok(Self {
            name: name.to_string(),
            alternate_space: Box::new(alternate),
            tint_transform,
        })
    }

    pub fn default_value(&self) -> ColorValue {
        return ColorValue::new(vec![1.0]);
    }

    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        if value.value_size() != 1 || !value.values()[0].is_finite() {
            return Err(PdfError::ColorError(
                "Separation requires one finite tint".into(),
            ));
        }
        let values = self
            .tint_transform
            .eval(&[value.values()[0].clamp(0.0, 1.0)])?;
        self.alternate_space.rgb(&ColorValue::new(values))
    }

    pub fn number_of_components(&self) -> usize {
        1
    }
}
