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

/// Display approximation through the required alternate/tint transform.
/// Native spot plates, overprint and NChannel custom blending are not modeled.
#[derive(Debug, Clone)]
pub struct DeviceN {
    components: usize,
    all_none: bool,
    alternate: Box<ColorSpace>,
    transform: Function,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::PdfDocument, io::stream_reader::StreamReader, parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;

    fn parse(source: &str) -> crate::objects::PdfObject {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
    }

    #[test]
    fn constructor_checks_family_before_parameters_and_resolves_its_tag() {
        use crate::objects::{PdfObject, pdf_name::PdfName};
        let doc = PdfDocument::new().unwrap();
        for tag in ["/Separation", "/Lab", "/Unknown", "1", "null", "(DeviceN)"] {
            let object = parse(&format!(
                "[{tag} [/Spot] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]"
            ));
            assert!(
                matches!(
                    DeviceN::try_new(object.as_array().unwrap(), doc.ctx()),
                    Err(PdfError::ColorError(_))
                ),
                "{tag}"
            );
        }
        let tag = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("DeviceN".into())))
            .unwrap();
        let object = parse(&format!(
            "[{} {} R [/Spot] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
            tag.obj_num(),
            tag.gen_num()
        ));
        assert!(DeviceN::try_new(object.as_array().unwrap(), doc.ctx()).is_ok());
        doc.update_object(tag, PdfObject::PdfName(PdfName::new("Lab".into())))
            .unwrap();
        assert!(matches!(
            DeviceN::try_new(object.as_array().unwrap(), doc.ctx()),
            Err(PdfError::ColorError(_))
        ));
    }

    #[test]
    fn component_slots_and_name_bytes_have_exact_boundaries_and_recover() {
        use crate::parser::options::{Limits, ParseOptions};
        let make = |bytes| {
            PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_decoded_bytes: bytes,
                    max_objects: 2,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap()
        };
        let slots = std::mem::size_of::<String>();
        let source = format!(
            "[/DeviceN [/{}] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
            "S".repeat(slots)
        );
        let object = parse(&source);
        let doc = make(slots);
        assert!(ColorSpace::try_new(&object, doc.ctx()).is_ok());
        let too_long = parse(&format!(
            "[/DeviceN [/{}] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
            "S".repeat(slots + 1)
        ));
        for _ in 0..2 {
            assert!(matches!(
                ColorSpace::try_new(&too_long, doc.ctx()),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert!(ColorSpace::try_new(&object, doc.ctx()).is_ok());
        let too_small = make(slots - 1);
        assert!(matches!(
            ColorSpace::try_new(&object, too_small.ctx()),
            Err(PdfError::ResourceLimit(_))
        ));
        let many =
            parse("[/DeviceN [/A /B /C] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]");
        assert!(matches!(
            ColorSpace::try_new(&many, doc.ctx()),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(ColorSpace::try_new(&object, doc.ctx()).is_ok());
    }

    #[test]
    fn indirect_parameters_and_color_cycles_recover_after_repair() {
        use crate::objects::{PdfObject, pdf_name::PdfName, pdf_reference::PdfReference};
        let doc = PdfDocument::new().unwrap();
        let spot = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("Spot".into())))
            .unwrap();
        let names = doc
            .try_insert_object(parse(&format!("[{} {} R]", spot.obj_num(), spot.gen_num())))
            .unwrap();
        let alternate = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("DeviceGray".into())))
            .unwrap();
        let transform = doc
            .try_insert_object(parse("<< /FunctionType 2 /Domain [0 1] /N 1 >>"))
            .unwrap();
        let subtype = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("DeviceN".into())))
            .unwrap();
        let attributes = doc
            .try_insert_object(parse(&format!(
                "<< /Subtype {} {} R >>",
                subtype.obj_num(),
                subtype.gen_num()
            )))
            .unwrap();
        let source = format!(
            "[/DeviceN {} {} R {} {} R {} {} R {} {} R]",
            names.obj_num(),
            names.gen_num(),
            alternate.obj_num(),
            alternate.gen_num(),
            transform.obj_num(),
            transform.gen_num(),
            attributes.obj_num(),
            attributes.gen_num()
        );
        let object = parse(&source);
        let id = doc.try_insert_object(object.clone()).unwrap();
        let old = ColorSpace::try_new(&object, doc.ctx()).unwrap();
        assert_eq!(old.rgb(&ColorValue::new(vec![0.5])).unwrap().r(), 128);
        doc.update_object(spot, PdfObject::PdfReference(PdfReference::from_id(spot)))
            .unwrap();
        for _ in 0..2 {
            assert!(ColorSpace::try_new(&object, doc.ctx()).is_err());
        }
        doc.update_object(spot, PdfObject::PdfName(PdfName::new("Spot".into())))
            .unwrap();
        assert!(ColorSpace::try_new(&object, doc.ctx()).is_ok());
        doc.update_object(
            alternate,
            PdfObject::PdfReference(PdfReference::from_id(id)),
        )
        .unwrap();
        // Ordinary resolve intentionally follows one object reference, so an
        // alias object is rejected as a non-color-space, not recursively chased.
        assert!(matches!(
            ColorSpace::try_new(&object, doc.ctx()),
            Err(PdfError::ColorError(_))
        ));
        doc.update_object(alternate, object.clone()).unwrap();
        for _ in 0..2 {
            assert!(matches!(
                ColorSpace::try_new(&object, doc.ctx()),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        doc.update_object(
            alternate,
            PdfObject::PdfName(PdfName::new("DeviceGray".into())),
        )
        .unwrap();
        assert!(ColorSpace::try_new(&object, doc.ctx()).is_ok());
        // Previously created immutable models remain usable after source edits.
        assert_eq!(old.rgb(&ColorValue::new(vec![0.5])).unwrap().r(), 128);
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        let saved = reopened.resolve_object(id).unwrap();
        let color = ColorSpace::try_new(&saved, reopened.ctx()).unwrap();
        assert_eq!(color.rgb(&ColorValue::new(vec![0.5])).unwrap().r(), 128);
    }

    #[test]
    fn single_component_queries_clip_and_reject_nonfinite_or_wrong_counts() {
        let doc = PdfDocument::new().unwrap();
        let object = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"[/DeviceN [/Spot] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap();
        let space = ColorSpace::try_new(&object, doc.ctx()).unwrap();
        assert_eq!(space.number_of_components().unwrap(), 1);
        assert_eq!(space.default_value().unwrap().values(), &[1.0]);
        for (tint, expected) in [(-1.0, 0), (0.5, 128), (2.0, 255)] {
            assert_eq!(
                space.rgb(&ColorValue::new(vec![tint])).unwrap().r(),
                expected
            );
        }
        for values in [vec![], vec![0.0, 1.0], vec![f32::NAN], vec![f32::INFINITY]] {
            assert!(matches!(
                space.rgb(&ColorValue::new(values)),
                Err(PdfError::ColorError(_))
            ));
        }
    }
}

impl DeviceN {
    pub fn is_nonpainting(&self) -> bool {
        self.all_none
    }
    pub fn try_new<R: Read + Seek>(array: &PdfArray, ctx: &PDFContext<R>) -> PdfResult<Self> {
        if !matches!(array.len(), 4 | 5) {
            return Err(PdfError::ColorError(
                "DeviceN requires four or five entries".into(),
            ));
        }
        let tag = ctx.resolve(
            array
                .get(0)
                .ok_or_else(|| PdfError::ColorError("missing DeviceN family".into()))?,
        )?;
        if tag.as_name().is_none_or(|name| name.name() != "DeviceN") {
            return Err(PdfError::ColorError(
                "DeviceN array must start with /DeviceN".into(),
            ));
        }
        let names = ctx.resolve(
            array
                .get(1)
                .ok_or_else(|| PdfError::ColorError("missing DeviceN names".into()))?,
        )?;
        let names = names
            .as_array()
            .ok_or_else(|| PdfError::ColorError("DeviceN names must be an array".into()))?;
        let count = names.len();
        if count == 0 {
            return Err(PdfError::ColorError("DeviceN names cannot be empty".into()));
        }
        if count > ctx.limits().max_objects
            || count
                .checked_mul(std::mem::size_of::<String>())
                .is_none_or(|n| n > ctx.limits().max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit("DeviceN components".into()));
        }
        let mut resolved_names = Vec::<String>::new();
        resolved_names
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("DeviceN names allocation".into()))?;
        let mut bytes = 0usize;
        for name in names {
            let object = ctx.resolve(name)?;
            let name = object
                .as_name()
                .ok_or_else(|| PdfError::ColorError("DeviceN component must be a name".into()))?
                .name();
            if name == "All" {
                return Err(PdfError::ColorError(
                    "All is not a DeviceN component".into(),
                ));
            }
            bytes = bytes
                .checked_add(name.len())
                .filter(|n| *n <= ctx.limits().max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("DeviceN name bytes".into()))?;
            let mut copied = String::new();
            copied
                .try_reserve_exact(name.len())
                .map_err(|_| PdfError::ResourceLimit("DeviceN name allocation".into()))?;
            copied.push_str(name);
            resolved_names.push(copied);
        }
        resolved_names.sort_unstable();
        if resolved_names
            .windows(2)
            .any(|pair| pair[0] == pair[1] && pair[0] != "None")
        {
            return Err(PdfError::ColorError(
                "DeviceN component names must be distinct".into(),
            ));
        }
        let all_none = resolved_names.iter().all(|name| name == "None");
        if let Some(attributes) = array.get(4) {
            let attributes = ctx.resolve(attributes)?;
            let attributes = attributes.as_dict().ok_or_else(|| {
                PdfError::ColorError("DeviceN attributes must be a dictionary".into())
            })?;
            if let Some(subtype) = attributes.get("Subtype") {
                let subtype = ctx.resolve(subtype)?;
                let subtype = subtype
                    .as_name()
                    .ok_or_else(|| PdfError::ColorError("DeviceN Subtype must be a name".into()))?
                    .name();
                if !matches!(subtype, "DeviceN" | "NChannel")
                    || (subtype == "NChannel" && resolved_names.iter().any(|name| name == "None"))
                {
                    return Err(PdfError::ColorError(
                        "invalid DeviceN subtype/components".into(),
                    ));
                }
            }
        }
        let alternate = ColorSpace::try_new(
            array
                .get(2)
                .ok_or_else(|| PdfError::ColorError("missing DeviceN alternate".into()))?,
            ctx,
        )?;
        if matches!(
            alternate,
            ColorSpace::Pattern(_)
                | ColorSpace::Indexed(_)
                | ColorSpace::Separation(_)
                | ColorSpace::DeviceN(_)
        ) {
            return Err(PdfError::ColorError(
                "invalid DeviceN alternate color space".into(),
            ));
        }
        let transform = create_function(
            array
                .get(3)
                .ok_or_else(|| PdfError::ColorError("missing DeviceN transform".into()))?,
            ctx,
        )?;
        if transform.dimensions() != (count, alternate.number_of_components()?) {
            return Err(PdfError::ColorError(
                "DeviceN tint-transform dimensions do not match".into(),
            ));
        }
        Ok(Self {
            components: count,
            all_none,
            alternate: Box::new(alternate),
            transform,
        })
    }

    pub fn number_of_components(&self) -> usize {
        self.components
    }

    pub fn default_value(&self) -> PdfResult<ColorValue> {
        let mut values = Vec::new();
        values
            .try_reserve_exact(self.components)
            .map_err(|_| PdfError::ResourceLimit("DeviceN default allocation".into()))?;
        values.resize(self.components, 1.0);
        Ok(ColorValue::new(values))
    }

    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        if value.value_size() != self.components || value.values().iter().any(|v| !v.is_finite()) {
            return Err(PdfError::ColorError(
                "DeviceN requires one finite tint per component".into(),
            ));
        }
        if self.all_none {
            return Err(PdfError::UnsupportedFeature(
                "all-None DeviceN requires paint suppression, not an RGB color".into(),
            ));
        }
        let mut tints = Vec::new();
        tints
            .try_reserve_exact(self.components)
            .map_err(|_| PdfError::ResourceLimit("DeviceN tint allocation".into()))?;
        tints.extend(value.values().iter().map(|v| v.clamp(0.0, 1.0)));
        let values = self.transform.eval(&tints)?;
        if values.len() != self.alternate.number_of_components()?
            || values.iter().any(|v| !v.is_finite())
        {
            return Err(PdfError::ColorError(
                "invalid DeviceN transform output".into(),
            ));
        }
        self.alternate.rgb(&ColorValue::new(values))
    }
}
