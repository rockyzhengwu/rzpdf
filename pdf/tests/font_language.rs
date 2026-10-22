use pdf::{
    document::{
        Document,
        resource_manager::{ResourceKind, StandardFont},
    },
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream, pdf_string::PdfString},
    page::page_object::PageObject,
    parser::syntax::{ParseMode, SyntaxParser},
};
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn set_language(d: &Document, value: &str) {
    let root = pdf::objects::object_id::ObjectId::new(2, 0);
    let mut dict = d.resolve_object(root).unwrap().into_dict().unwrap();
    dict.try_insert(
        "Lang".into(),
        PdfObject::PdfString(PdfString::from_text(value).unwrap()),
    )
    .unwrap();
    d.update_object(root, PdfObject::PdfDict(dict)).unwrap();
}
#[test]
fn catalog_marked_language_scopes_form_inheritance_and_strict_reopen() {
    let d = Document::new().unwrap();
    d.add_page(0).unwrap();
    set_language(&d, "ja-JP");
    let font = d
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let show = |t: &str| format!("BT /{font} 12 Tf 20 40 Td ({t}) Tj ET ");
    let form = d
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            parse("<< /Subtype /Form /BBox [0 0 100 100] >>")
                .into_dict()
                .unwrap(),
            format!(
                "/Span << /Lang (en-GB) >> BDC {} EMC {}",
                show("F"),
                show("G")
            )
            .into_bytes(),
        )))
        .unwrap();
    let form = d
        .resources()
        .register_existing(0, ResourceKind::Form, form)
        .unwrap();
    let code = format!(
        "{} /Span << /Lang (zh-Hant) >> BDC {} q /Artifact BMC Q {} EMC /{form} Do {} EMC {} /Span << /Lang () >> BDC {} EMC /Span << /Lang (not valid) >> BDC {} EMC {}",
        show("A"),
        show("B"),
        show("C"),
        show("H"),
        show("I"),
        show("J"),
        show("K"),
        show("L")
    );
    d.replace_page_contents(0, code.into_bytes()).unwrap();
    let inspect = |d: &Document| {
        d.get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .objects()
            .iter()
            .filter_map(|o| match o {
                PageObject::Text(t) => Some((
                    t.items
                        .iter()
                        .filter_map(|c| c.unicode())
                        .cloned()
                        .collect::<String>(),
                    t.language().map(str::to_owned),
                )),
                _ => None,
            })
            .collect::<Vec<_>>()
    };
    let expected = vec![
        ("A", Some("ja-JP")),
        ("B", Some("zh-Hant")),
        ("C", Some("zh-Hant")),
        ("F", Some("en-GB")),
        ("G", Some("zh-Hant")),
        ("H", Some("zh-Hant")),
        ("I", Some("ja-JP")),
        ("J", None),
        ("K", None),
        ("L", Some("ja-JP")),
    ]
    .into_iter()
    .map(|(s, l)| (s.to_owned(), l.map(str::to_owned)))
    .collect::<Vec<_>>();
    let revision = d.revision();
    assert_eq!(inspect(&d), expected);
    assert_eq!(d.revision(), revision);
    let reopened =
        Document::from_bytes_with_mode(d.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(inspect(&reopened), expected);
}
#[test]
fn oversized_catalog_language_is_ignored_as_a_hint_without_rewriting_source() {
    let d = Document::new().unwrap();
    d.add_page(0).unwrap();
    set_language(&d, &"a".repeat(10000));
    let font = d
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    d.replace_page_contents(0, format!("BT /{font} 12 Tf (A) Tj ET").into_bytes())
        .unwrap();
    let list = d.get_page(0).unwrap().display_list().unwrap();
    assert!(
        list.objects()
            .iter()
            .filter_map(|o| if let PageObject::Text(t) = o {
                Some(t)
            } else {
                None
            })
            .all(|t| t.language().is_none())
    );
    assert_eq!(
        d.resolve_object(pdf::objects::object_id::ObjectId::new(2, 0))
            .unwrap()
            .into_dict()
            .unwrap()
            .get("Lang")
            .unwrap()
            .as_string()
            .unwrap()
            .text_content()
            .unwrap()
            .len(),
        10000
    );
}

#[test]
fn malformed_optional_catalog_hint_does_not_change_painting_semantics() {
    let d = Document::new().unwrap();
    d.add_page(0).unwrap();
    let id = pdf::objects::object_id::ObjectId::new(2, 0);
    let mut root = d.resolve_object(id).unwrap().into_dict().unwrap();
    root.try_insert(
        "Lang".into(),
        PdfObject::PdfString(PdfString::new(vec![0xfe, 0xff, 0xd8, 0], false)),
    )
    .unwrap();
    d.update_object(id, PdfObject::PdfDict(root)).unwrap();
    let font = d
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    d.replace_page_contents(0, format!("BT /{font} 12 Tf (A) Tj ET").into_bytes())
        .unwrap();
    let list = d.get_page(0).unwrap().display_list().unwrap();
    assert!(
        list.objects()
            .iter()
            .filter_map(|o| if let PageObject::Text(t) = o {
                Some(t)
            } else {
                None
            })
            .all(|t| t.language().is_none())
    );
}
