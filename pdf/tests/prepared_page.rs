use pdf::{
    device::trace::TraceDevice,
    document::{PdfDocument, resource_manager::StandardFont},
    error::PdfError,
    page::{annotations::AnnotationRenderOptions, page_object::PageObject},
    parser::options::{Limits, ParseOptions},
};
use std::{io::Cursor, rc::Rc};
type Doc = PdfDocument<Cursor<Vec<u8>>>;
fn fixture() -> Doc {
    let doc = Doc::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf 10 30 Td (Cache sample) Tj ET").into_bytes(),
    )
    .unwrap();
    doc
}
#[test]
fn repeated_preparation_reuses_interpretation_and_replay_matches_uncached() {
    let doc = fixture();
    let first = doc.prepare_page(0, None).unwrap();
    for _ in 0..8 {
        assert!(Rc::ptr_eq(&first, &doc.prepare_page(0, None).unwrap()));
    }
    let mut actual = TraceDevice::new();
    first.replay(&doc, &mut actual).unwrap();
    let mut expected = TraceDevice::new();
    doc.get_page(0).unwrap().display(&mut expected).unwrap();
    assert_eq!(actual.to_xml(), expected.to_xml());
    let stats = doc.render_cache_statistics();
    assert_eq!(stats.page_misses, 1);
    assert_eq!(stats.page_hits, 8);
    assert!(stats.page_estimated_bytes <= 16 * 1024 * 1024);
    assert!(stats.font_hits > 0);
    doc.clear_render_caches();
    let stats = doc.render_cache_statistics();
    assert_eq!((stats.page_entries, stats.font_entries), (0, 0));
    // Explicitly retained snapshots remain usable; clearing cache is not an edit.
    first.replay(&doc, &mut TraceDevice::new()).unwrap();
}
#[test]
fn edit_rollback_and_foreign_document_reject_stale_prepared_pages() {
    let doc = fixture();
    let other = fixture();
    let first = doc.prepare_page(0, None).unwrap();
    assert!(first.replay(&other, &mut TraceDevice::new()).is_err());
    let result: Result<(), PdfError> = doc.edit_transaction(|d| {
        d.replace_page_contents(0, b"q Q".to_vec())?;
        Err(PdfError::DocumentError("rollback".into()))
    });
    assert!(result.is_err());
    assert!(first.replay(&doc, &mut TraceDevice::new()).is_err());
    let current = doc.prepare_page(0, None).unwrap();
    assert!(!Rc::ptr_eq(&first, &current));
    doc.replace_page_contents(0, b"0 0 10 10 re f".to_vec())
        .unwrap();
    assert!(current.replay(&doc, &mut TraceDevice::new()).is_err());
    doc.prepare_page(0, None).unwrap();
    assert_eq!(doc.render_cache_statistics().page_entries, 1);
}
#[test]
fn annotation_options_are_separate_and_cache_is_bounded() {
    let doc = fixture();
    let mut options = AnnotationRenderOptions::default();
    let first = doc.prepare_page(0, Some(options)).unwrap();
    for rotation in [90, 180, 270, 0] {
        options.view_rotation = rotation;
        options.view_zoom = 2.;
        doc.prepare_page(0, Some(options)).unwrap();
    }
    assert_eq!(doc.render_cache_statistics().page_entries, 2);
    assert!(!Rc::ptr_eq(
        &first,
        &doc.prepare_page(0, Some(AnnotationRenderOptions::default()))
            .unwrap()
    ));
    options.view_zoom = f32::NAN;
    assert!(doc.prepare_page(0, Some(options)).is_err());
    assert!(doc.prepare_page(999, None).is_err());
    assert!(doc.render_cache_statistics().page_entries <= 2);
}
#[test]
fn shared_font_objects_survive_independent_interpreters_without_aliasing_names() {
    let doc = fixture();
    let list1 = doc.get_page(0).unwrap().display_list().unwrap();
    let list2 = doc.get_page(0).unwrap().display_list().unwrap();
    fn font(list: &pdf::page::display_list::DisplayList) -> Rc<pdf::font::pdf_font::Font> {
        list.objects()
            .iter()
            .find_map(|o| {
                if let PageObject::Text(t) = o {
                    t.state.text_state.font.clone()
                } else {
                    None
                }
            })
            .unwrap()
    }
    assert!(Rc::ptr_eq(&font(&list1), &font(&list2)));
    let other = fixture();
    let foreign = other.get_page(0).unwrap().display_list().unwrap();
    assert!(!Rc::ptr_eq(&font(&list1), &font(&foreign)));
    doc.clear_render_caches();
    let fresh = doc.get_page(0).unwrap().display_list().unwrap();
    assert!(!Rc::ptr_eq(&font(&list1), &font(&fresh)));
    assert!(doc.render_cache_statistics().font_estimated_bytes <= 4 * 1024 * 1024);
}
#[test]
fn cache_refusal_does_not_change_successful_page_interpretation() {
    let doc = Doc::new_with_options(ParseOptions {
        limits: Limits {
            max_operation_bytes: 1,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"0 0 10 10 re f".to_vec())
        .unwrap();
    let first = doc.prepare_page(0, None).unwrap();
    let second = doc.prepare_page(0, None).unwrap();
    assert!(!Rc::ptr_eq(&first, &second));
    assert_eq!(doc.render_cache_statistics().page_entries, 0);
}

#[test]
fn predefined_cmap_font_is_reused_across_separate_page_interpretations() {
    use pdf::{
        document::destination::DestinationView, io::stream_reader::StreamReader,
        objects::PdfObject, parser::syntax::SyntaxParser,
    };
    fn parse(s: &str) -> PdfObject {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
    }
    let doc = Doc::new().unwrap();
    doc.add_page(0).unwrap();
    let font=doc.try_insert_object(parse("<< /Type /Font /Subtype /Type0 /BaseFont /Missing /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Missing /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor << /Flags 0 >> >>] >>")).unwrap();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let mut dict = doc.resolve_object(page).unwrap().into_dict().unwrap();
    dict.try_insert(
        "Resources".into(),
        parse(&format!("<< /Font << /F {} 0 R >> >>", font.obj_num())),
    )
    .unwrap();
    doc.update_object(page, PdfObject::PdfDict(dict)).unwrap();
    doc.replace_page_contents(0, b"BT /F 12 Tf <0041> Tj ET".to_vec())
        .unwrap();
    doc.get_page(0).unwrap().display_list().unwrap();
    let first = doc.render_cache_statistics();
    assert_eq!(first.font_entries, 1);
    doc.get_page(0).unwrap().display_list().unwrap();
    assert!(doc.render_cache_statistics().font_hits > first.font_hits);
    // A resource edit must not reuse this font's mapping under the same object ID.
    doc.update_object(font,parse("<< /Type /Font /Subtype /Type0 /BaseFont /Changed /Encoding /Identity-V /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Changed /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor << /Flags 0 >> >>] >>")).unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let f = list
        .objects()
        .iter()
        .find_map(|o| {
            if let PageObject::Text(t) = o {
                t.state.text_state.font.as_ref()
            } else {
                None
            }
        })
        .unwrap();
    assert_eq!(f.writting_mode(), pdf::font::WritingMode::Vertical);
}

#[test]
fn equal_font_resource_names_on_different_pages_do_not_alias() {
    let doc = Doc::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let first = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    let second = doc
        .resources()
        .register_standard_font(1, StandardFont::Helvetica)
        .unwrap();
    assert_eq!(first, second);
    for (page, name) in [(0, &first), (1, &second)] {
        doc.replace_page_contents(page, format!("BT /{name} 12 Tf (AB) Tj ET").into_bytes())
            .unwrap();
    }
    for (page, expected) in [(0, "Courier"), (1, "Helvetica"), (0, "Courier")] {
        let list = doc.get_page(page).unwrap().display_list().unwrap();
        let font = list
            .objects()
            .iter()
            .find_map(|o| {
                if let PageObject::Text(t) = o {
                    t.state.text_state.font.as_ref()
                } else {
                    None
                }
            })
            .unwrap();
        assert_eq!(font.name(), expected);
    }
    assert!(doc.render_cache_statistics().font_hits > 0);
}
