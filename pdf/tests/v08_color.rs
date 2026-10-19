use pdf::{
    color::{
        conversion::{ColorConversionContext, IccFallback, RenderingIntent},
        icc_based::IccBased,
        icc_profile::ICCProfile,
        value::ColorValue,
    },
    document::PdfDocument,
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::{page_builder::PageBuilder, page_object::PageObject},
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;
#[path = "../examples/support/v08_cmyk.rs"]
mod cmyk;
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
#[test]
fn ri_and_extgstate_intent_are_scoped_and_survive_strict_save() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page_with_builder(
        0,
        PageBuilder::new().size(20., 20.).resources(
            parse("<< /ExtGState << /R << /RI /Saturation >> >> >>")
                .into_dict()
                .unwrap(),
        ),
    )
    .unwrap();
    doc.replace_page_contents(0,b"/Perceptual ri 0 0 1 1 re f q /R gs 1 0 1 1 re f Q 2 0 1 1 re f /AbsoluteColorimetric ri 3 0 1 1 re f".to_vec()).unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for doc in [&doc, &reopened] {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let intents = list
            .objects()
            .iter()
            .filter_map(|o| {
                if let PageObject::Path(p) = o {
                    Some(p.graphic_state().general_state.rendering_intent)
                } else {
                    None
                }
            })
            .collect::<Vec<_>>();
        assert_eq!(
            intents,
            [
                RenderingIntent::Perceptual,
                RenderingIntent::Saturation,
                RenderingIntent::Perceptual,
                RenderingIntent::AbsoluteColorimetric
            ]
        );
    }
}
#[test]
fn icc_profile_is_not_replaced_by_alternate_and_invalid_registration_is_atomic() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let make = |n, bytes| {
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    parse(&format!("<< /N {n} /Alternate /DeviceRGB >>"))
                        .into_dict()
                        .unwrap(),
                    bytes,
                )
                .unwrap(),
            ))
            .unwrap();
        parse(&format!("[/ICCBased {} {} R]", id.obj_num(), id.gen_num()))
    };
    let data = moxcms::ColorProfile::new_adobe_rgb().encode().unwrap();
    let object = make(3, data.clone());
    let name = doc
        .resources()
        .register_color_space(0, object.clone())
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("/{name} cs 0.2 0.5 0.8 sc 0 0 1 1 re f").into_bytes(),
    )
    .unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!()
    };
    let color = &path.graphic_state().color_state;
    let rgb = color.fill_color_space.rgb(&color.fill_color_value).unwrap();
    assert!((rgb.r - 0.2).abs() > 0.02);
    for object in [
        make(3, vec![]),
        make(3, data[..100].to_vec()),
        make(4, data),
    ] {
        let revision = doc.revision();
        let bytes = doc.save_to_bytes().unwrap();
        assert!(doc.resources().register_color_space(0, object).is_err());
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.save_to_bytes().unwrap(), bytes);
    }
    assert!(doc.resources().register_color_space(0, object).is_ok());
}
#[test]
fn fallback_requires_an_explicit_policy_and_cmyk_lut_uses_all_colorants() {
    let legacy = IccBased::default();
    let value = ColorValue::new(vec![0.5]);
    assert!(legacy.rgb(&value).is_err());
    let mut context = ColorConversionContext::default();
    context.icc_fallback = IccFallback::Alternate;
    let rgb = legacy.rgb_with_context(&value, &context).unwrap();
    assert_eq!([rgb.r, rgb.g, rgb.b], [0.5; 3]);
    let profile = ICCProfile::try_new(&cmyk::profile()).unwrap();
    assert_eq!(profile.number_of_components(), 4);
    for component in 0..4 {
        let mut values = [0.2; 4];
        let a = profile
            .convert(&values, &ColorConversionContext::default())
            .unwrap();
        values[component] = 0.8;
        let b = profile
            .convert(&values, &ColorConversionContext::default())
            .unwrap();
        assert!(
            [a.r - b.r, a.g - b.g, a.b - b.b]
                .iter()
                .any(|v| v.abs() > 0.05)
        );
    }
}
