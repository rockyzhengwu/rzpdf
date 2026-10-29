use pdf::{
    document::{PdfDocument, destination::DestinationView, resource_manager::StandardFont},
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_reference::PdfReference, pdf_stream::PdfStream},
    page::marked_text::{ActualTextSource, MarkedTextOptions},
    parser::{
        options::ParseOptions,
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;
type Doc = PdfDocument<Cursor<Vec<u8>>>;
fn parse(text: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn set(doc: &Doc, id: ObjectId, key: &str, value: PdfObject) {
    let mut d = doc.resolve_object(id).unwrap().into_dict().unwrap();
    d.try_insert(key.into(), value).unwrap();
    doc.update_object(id, PdfObject::PdfDict(d)).unwrap();
}
fn new_doc(options: ParseOptions) -> (Doc, String) {
    let doc = Doc::new_with_options(options).unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    (doc, font)
}
fn strict() -> (Doc, String) {
    new_doc(ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    })
}
fn contents(doc: &Doc, font: &str, body: &str) {
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf 10 10 Td {body} ET").into_bytes(),
    )
    .unwrap();
}
fn text(doc: &Doc, options: MarkedTextOptions) -> String {
    doc.get_page(0)
        .unwrap()
        .extract_marked_text(options)
        .unwrap()
        .text(&Default::default())
        .unwrap()
}
fn raw() -> MarkedTextOptions {
    MarkedTextOptions {
        use_actual_text: false,
        include_artifacts: true,
    }
}
fn two_nodes(doc: &Doc) -> (ObjectId, ObjectId, ObjectId, ObjectId) {
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let root = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let first = doc
        .try_insert_object(parse(&format!(
            "<< /S /P /P {} 0 R /Pg {} 0 R /K 0 >>",
            root.obj_num(),
            page.obj_num()
        )))
        .unwrap();
    let second = doc
        .try_insert_object(parse(&format!(
            "<< /S /P /P {} 0 R /Pg {} 0 R /K 1 >>",
            root.obj_num(),
            page.obj_num()
        )))
        .unwrap();
    doc.update_object(root, parse(&format!("<< /Type /StructTreeRoot /K [{} 0 R {} 0 R] /ParentTree << /Nums [0 [{} 0 R {} 0 R]] >> >>",second.obj_num(),first.obj_num(),first.obj_num(),second.obj_num()))).unwrap();
    set(doc, page, "StructParents", parse("0"));
    set(doc, ObjectId::new(2, 0), "StructTreeRoot", reference(root));
    (root, first, second, page)
}

#[test]
fn actualtext_is_opt_in_while_metadata_preserves_paint_order() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/Span << /ActualText (replacement) >> BDC (ABC) Tj EMC",
    );
    let before = doc.save_to_bytes().unwrap();
    let stamp = doc.stamp();
    assert_eq!(text(&doc, Default::default()), "replacement");
    assert_eq!(text(&doc, raw()), "ABC");
    let paint = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    assert!(paint.diagnostics.is_empty());
    assert_eq!(
        paint
            .characters
            .iter()
            .filter_map(|c| c.unicode.as_deref())
            .collect::<String>(),
        "ABC"
    );
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(before, doc.save_to_bytes().unwrap());
    let compat = Doc::from_bytes_with_mode(before, ParseMode::Compatible).unwrap();
    let report = compat
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    assert!(report.diagnostics.is_empty());
    assert_eq!(
        report
            .characters
            .iter()
            .filter_map(|c| c.unicode.as_deref())
            .collect::<String>(),
        "ABC"
    );
}

#[test]
fn unicode_empty_and_adjacent_replacements_do_not_add_word_breaks() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/Span << /ActualText <FEFF4F60597D> >> BDC (A) Tj EMC /Span << /ActualText () >> BDC (B) Tj EMC /Span << /ActualText ( world) >> BDC (C) Tj EMC",
    );
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap();
    assert_eq!(report.spans.len(), 3);
    assert_eq!(report.spans[1].text.as_deref(), Some(""));
    assert_eq!(report.text(&Default::default()).unwrap(), "你好 world");
    assert_eq!(text(&doc, raw()), "ABC");
}

#[test]
fn replacements_span_tj_quotes_multiple_shows_and_keep_operation_sources() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/Span << /ActualText (joined) >> BDC [(A) -20 (B)] TJ (C) Tj (D) ' 1 2 (E) \" EMC",
    );
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap();
    assert_eq!(report.spans.len(), 1);
    let span = &report.spans[0];
    assert_eq!(span.sources.len(), 4);
    assert!(
        span.sources
            .windows(2)
            .all(|s| s[0].operation_index < s[1].operation_index)
    );
    assert!(matches!(
        span.replacement,
        Some(ActualTextSource::MarkedContent(_))
    ));
    assert_eq!(report.text(&Default::default()).unwrap(), "joined");
    assert_eq!(text(&doc, raw()), "ABCDE");
}

#[test]
fn outer_replacement_is_atomic_and_artifact_filter_is_inherited() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/Span << /ActualText (outer) >> BDC (A) Tj /Span << /ActualText (inner) >> BDC (B) Tj EMC /Artifact BMC (C) Tj EMC EMC /Artifact << /ActualText (hidden) >> BDC /Span BMC (D) Tj EMC EMC (E) Tj",
    );
    assert_eq!(text(&doc, Default::default()), "outerE");
    assert_eq!(
        text(
            &doc,
            MarkedTextOptions {
                include_artifacts: true,
                ..Default::default()
            }
        ),
        "outerhiddenE"
    );
    assert_eq!(
        text(
            &doc,
            MarkedTextOptions {
                use_actual_text: false,
                include_artifacts: false
            }
        ),
        "ABE"
    );
    assert_eq!(text(&doc, raw()), "ABCDE");
}

#[test]
fn empty_graphical_sequence_has_replacement_without_fake_glyph_sources() {
    let (doc, _) = strict();
    doc.replace_page_contents(0,b"/Span << /ActualText (image label) >> BDC 0 0 4 4 re f EMC /Span << /ActualText () >> BDC EMC".to_vec()).unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap();
    assert_eq!(report.spans.len(), 2);
    assert!(report.spans.iter().all(|s| s.sources.is_empty()));
    assert_eq!(report.text(&Default::default()).unwrap(), "image label");
}

#[test]
fn properties_resources_resolve_and_explicit_empty_scope_does_not_inherit() {
    let (doc, font) = strict();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let properties = doc
        .try_insert_object(parse(
            "<< /ActualText (named) /Lang (en-US) /Alt (not replacement) >>",
        ))
        .unwrap();
    let mut resources = doc.get_page(0).unwrap().resource().unwrap();
    resources
        .try_insert(
            "Properties".into(),
            parse(&format!("<< /Props {} 0 R >>", properties.obj_num())),
        )
        .unwrap();
    set(&doc, page, "Resources", PdfObject::PdfDict(resources));
    contents(&doc, &font, "/Span /Props BDC (A) Tj EMC");
    assert_eq!(text(&doc, Default::default()), "named");
    contents(
        &doc,
        &font,
        "/Span << /Alt (wrong) /Lang (fr) >> BDC (A) Tj EMC",
    );
    assert_eq!(text(&doc, Default::default()), "A");
    contents(&doc, &font, "/Span /Missing BDC (A) Tj EMC");
    assert!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text(Default::default())
            .is_err()
    );
}

#[test]
fn invalid_properties_and_marked_text_graphics_boundaries_reject() {
    let (doc, font) = strict();
    for body in [
        "EMC",
        "/Span BMC",
        "/Span << /ActualText null >> BDC EMC",
        "/Span << /ActualText 1 >> BDC EMC",
        "/Span << /MCID -1 >> BDC EMC",
        "/Span << /MCID 0.5 >> BDC EMC",
        "/Span 1 BDC EMC",
        "/Span << /OC 1 >> BDC EMC",
        "/OC BMC EMC",
        "/Span << /Private 1 >> BDC EMC",
        "/P << /ActualText (x) >> BDC EMC",
        "/Span BMC 1 EMC",
    ] {
        contents(&doc, &font, body);
        assert!(
            doc.get_page(0)
                .unwrap()
                .extract_marked_text(Default::default())
                .is_err(),
            "{body}"
        );
    }
    for body in [
        "BT /Span BMC ET EMC",
        "/Span BMC BT EMC ET",
        "q /Span BMC Q EMC",
        "/Span BMC q EMC Q",
    ] {
        doc.replace_page_contents(0, body.as_bytes().to_vec())
            .unwrap();
        assert!(
            doc.get_page(0)
                .unwrap()
                .extract_marked_text(Default::default())
                .is_err(),
            "{body}"
        );
    }
}

#[test]
fn source_snapshot_changes_after_edit_and_reopen_without_mutating_reads() {
    let (doc, font) = strict();
    contents(&doc, &font, "/Span << /ActualText (X) >> BDC (A) Tj EMC");
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap();
    let bytes = doc.save_to_bytes().unwrap();
    let reopened = Doc::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    let again = reopened
        .get_page(0)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap();
    assert_ne!(report.page, again.page);
    assert_eq!(again.text(&Default::default()).unwrap(), "X");
    contents(&doc, &font, "(B) Tj");
    assert!(!doc.is_stamp_current(&report.stamp));
}

#[test]
fn structures_use_declared_order_not_paint_order_and_report_absence() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/P << /MCID 0 >> BDC (A) Tj EMC /P << /MCID 1 >> BDC (B) Tj EMC (Z) Tj",
    );
    assert!(
        doc.extract_text_structure_order(Default::default())
            .unwrap()
            .is_none()
    );
    two_nodes(&doc);
    assert_eq!(text(&doc, Default::default()), "ABZ");
    let report = doc
        .extract_text_structure_order(Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(report.text(&Default::default()).unwrap(), "BA");
    assert_eq!(report.unassociated_spans, 1);
    assert_eq!(report.unextracted_object_references, 0);
}

#[test]
fn structure_element_actualtext_replaces_descendants_and_metadata_once() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/P << /MCID 0 >> BDC /Span << /ActualText (inner) >> BDC (A) Tj EMC EMC /P << /MCID 1 >> BDC (B) Tj EMC",
    );
    let (root, first, second, _) = two_nodes(&doc);
    set(
        &doc,
        first,
        "K",
        parse(&format!("[0 {} 0 R]", second.obj_num())),
    );
    set(&doc, second, "P", reference(first));
    set(&doc, root, "K", reference(first));
    set(&doc, first, "ActualText", parse("(parent)"));
    let report = doc
        .extract_text_structure_order(Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(report.spans.len(), 1);
    assert_eq!(report.text(&Default::default()).unwrap(), "parent");
    assert_eq!(report.spans[0].span.sources.len(), 2);
    assert!(
        matches!(&report.spans[0].span.replacement,Some(ActualTextSource::StructureElement(h)) if h.object_id()==first)
    );
    assert_eq!(
        doc.extract_text_structure_order(raw())
            .unwrap()
            .unwrap()
            .text(&Default::default())
            .unwrap(),
        "AB"
    );
    set(&doc, first, "ActualText", parse("()"));
    assert_eq!(
        doc.extract_text_structure_order(Default::default())
            .unwrap()
            .unwrap()
            .text(&Default::default())
            .unwrap(),
        ""
    );
}

#[test]
fn atomic_actualtext_cannot_be_silently_split_across_element_owners() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/Span << /ActualText (atomic) >> BDC /P << /MCID 0 >> BDC (A) Tj EMC /P << /MCID 1 >> BDC (B) Tj EMC EMC",
    );
    let (root, first, second, _) = two_nodes(&doc);
    assert_eq!(text(&doc, Default::default()), "atomic");
    assert!(matches!(
        doc.extract_text_structure_order(Default::default()),
        Err(PdfError::UnsupportedFeature(_))
    ));
    set(&doc, root, "K", reference(first));
    set(&doc, first, "K", parse("[1 0]"));
    set(
        &doc,
        root,
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [{} 0 R {} 0 R]] >>",
            first.obj_num(),
            first.obj_num()
        )),
    );
    assert_eq!(
        doc.extract_text_structure_order(Default::default())
            .unwrap()
            .unwrap()
            .text(&Default::default())
            .unwrap(),
        "atomic"
    );
    assert_ne!(first, second);
}

#[test]
fn structure_content_actualtext_with_empty_sequence_has_mcid_ownership() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/Span << /MCID 0 /ActualText (empty content) >> BDC EMC /P << /MCID 1 >> BDC (B) Tj EMC",
    );
    two_nodes(&doc);
    assert_eq!(
        doc.extract_text_structure_order(Default::default())
            .unwrap()
            .unwrap()
            .text(&Default::default())
            .unwrap(),
        "Bempty content"
    );
}

#[test]
fn objr_is_reported_not_fabricated_from_alt_or_annotation_appearance() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/P << /MCID 0 >> BDC (A) Tj EMC /P << /MCID 1 >> BDC (B) Tj EMC",
    );
    let (root, first, _, page) = two_nodes(&doc);
    let annot = doc
        .try_insert_object(parse(&format!(
            "<< /Type /Annot /Subtype /Square /Rect [0 0 5 5] /P {} 0 R /StructParent 1 >>",
            page.obj_num()
        )))
        .unwrap();
    set(
        &doc,
        page,
        "Annots",
        parse(&format!("[{} 0 R]", annot.obj_num())),
    );
    set(
        &doc,
        first,
        "K",
        parse(&format!(
            "[0 << /Type /OBJR /Obj {} 0 R >>]",
            annot.obj_num()
        )),
    );
    let second = doc.structure_tree().unwrap().unwrap().elements[0]
        .handle
        .object_id();
    set(
        &doc,
        root,
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [{} 0 R {} 0 R] 1 {} 0 R] >>",
            first.obj_num(),
            second.obj_num(),
            first.obj_num()
        )),
    );
    set(&doc, first, "Alt", parse("(not text)"));
    let report = doc
        .extract_text_structure_order(Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(report.unextracted_object_references, 1);
    assert_eq!(report.text(&Default::default()).unwrap(), "BA");
    set(&doc, first, "ActualText", parse("(replacement)"));
    let report = doc
        .extract_text_structure_order(Default::default())
        .unwrap()
        .unwrap();
    assert_eq!(report.unextracted_object_references, 0);
    assert_eq!(report.text(&Default::default()).unwrap(), "Breplacement");
}

#[test]
fn extraction_cannot_bypass_parenttree_coherence_or_interpreter_diagnostics() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/P << /MCID 0 >> BDC (A) Tj EMC /P << /MCID 1 >> BDC (B) Tj EMC",
    );
    let (root, first, _, _) = two_nodes(&doc);
    set(
        &doc,
        root,
        "ParentTree",
        parse(&format!("<< /Nums [0 [{} 0 R null]] >>", first.obj_num())),
    );
    assert!(
        doc.extract_text_structure_order(Default::default())
            .is_err()
    );
    let (doc, font) = new_doc(Default::default());
    contents(
        &doc,
        &font,
        "/P << /MCID 0 >> BDC (A) Tj EMC /P << /MCID 1 >> BDC (B) Tj EMC Mystery",
    );
    two_nodes(&doc);
    assert!(matches!(
        doc.extract_text_structure_order(Default::default()),
        Err(PdfError::UnsupportedFeature(_))
    ));
}

#[test]
fn capture_limits_apply_before_artifact_or_replacement_suppression() {
    let mut options = ParseOptions::default();
    options.limits.max_text_chars = 5;
    let (doc, font) = new_doc(options);
    contents(&doc, &font, "/Artifact BMC (123456) Tj EMC");
    assert!(matches!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text(Default::default()),
        Err(PdfError::ResourceLimit(_))
    ));
    contents(&doc, &font, "/Span << /ActualText (123456) >> BDC EMC");
    assert!(matches!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text(Default::default()),
        Err(PdfError::ResourceLimit(_))
    ));
    let mut options = ParseOptions::default();
    options.limits.max_depth = 3;
    let (doc, font) = new_doc(options);
    contents(
        &doc,
        &font,
        "/Span BMC /Span BMC /Span BMC /Span BMC EMC EMC EMC EMC",
    );
    assert!(matches!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text(Default::default()),
        Err(PdfError::ResourceLimit(_))
    ));
}

fn form(doc: &Doc, content: &str, properties: Option<&str>) -> ObjectId {
    let mut scope = doc.get_page(0).unwrap().resource().unwrap();
    if let Some(properties) = properties {
        scope
            .try_insert("Properties".into(), parse(properties))
            .unwrap();
    }
    let mut dict = parse("<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>")
        .into_dict()
        .unwrap();
    dict.try_insert("Resources".into(), PdfObject::PdfDict(scope))
        .unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, content.as_bytes().to_vec()).unwrap(),
        ))
        .unwrap();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let mut scope = doc.get_page(0).unwrap().resource().unwrap();
    scope
        .try_insert(
            "XObject".into(),
            parse(&format!("<< /Fm {} 0 R >>", id.obj_num())),
        )
        .unwrap();
    set(doc, page, "Resources", PdfObject::PdfDict(scope));
    id
}

#[test]
fn form_properties_repeated_calls_and_outer_replacement_keep_invocation_sources() {
    let (doc, font) = strict();
    let id = form(
        &doc,
        &format!("/Span /Props BDC BT /{font} 12 Tf (C) Tj ET EMC"),
        Some("<< /Props << /ActualText (child) >> >>"),
    );
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let mut scope = doc.get_page(0).unwrap().resource().unwrap();
    scope
        .try_insert(
            "Properties".into(),
            parse("<< /Props << /ActualText (wrong-page) >> >>"),
        )
        .unwrap();
    set(&doc, page, "Resources", PdfObject::PdfDict(scope));
    doc.replace_page_contents(
        0,
        format!("/Fm Do /Fm Do BT /{font} 12 Tf (Z) Tj ET").into_bytes(),
    )
    .unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap();
    assert_eq!(report.text(&Default::default()).unwrap(), "childchildZ");
    assert_eq!(report.spans[0].sources[0].invocations[0].form_id, Some(id));
    assert_ne!(
        report.spans[0].sources[0].invocations,
        report.spans[1].sources[0].invocations
    );
    doc.replace_page_contents(
        0,
        b"/Span << /ActualText (outer) >> BDC /Fm Do /Fm Do EMC".to_vec(),
    )
    .unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap();
    assert_eq!(report.text(&Default::default()).unwrap(), "outer");
    assert_eq!(report.spans[0].sources.len(), 2);
    assert_eq!(text(&doc, raw()), "CC");
}

#[test]
fn form_mcid_domain_and_structure_order_are_independent_of_page_mcid() {
    let (doc, font) = strict();
    let id = form(
        &doc,
        &format!("/Span << /MCID 0 /ActualText (F) >> BDC BT /{font} 12 Tf (C) Tj ET EMC"),
        None,
    );
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf /P << /MCID 0 >> BDC (A) Tj EMC ET /Fm Do").into_bytes(),
    )
    .unwrap();
    let (root, first, second, _) = two_nodes(&doc);
    set(
        &doc,
        second,
        "K",
        parse(&format!(
            "<< /Type /MCR /Stm {} 0 R /MCID 0 >>",
            id.obj_num()
        )),
    );
    let object = doc.resolve_object(id).unwrap();
    let stream = object.as_stream().unwrap();
    let mut dict = stream.dict().clone();
    dict.try_insert("StructParents".into(), parse("1")).unwrap();
    doc.update_object(
        id,
        PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, stream.raw_data().unwrap().to_vec()).unwrap(),
        ),
    )
    .unwrap();
    set(
        &doc,
        root,
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [{} 0 R] 1 [{} 0 R]] >>",
            first.obj_num(),
            second.obj_num()
        )),
    );
    assert_eq!(text(&doc, Default::default()), "AF");
    assert_eq!(
        doc.extract_text_structure_order(Default::default())
            .unwrap()
            .unwrap()
            .text(&Default::default())
            .unwrap(),
        "FA"
    );
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let stream = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                Default::default(),
                format!("BT /{font} 12 Tf /P << /MCID 0 >> BDC (A) Tj EMC ET /Fm Do /Fm Do")
                    .into_bytes(),
            )
            .unwrap(),
        ))
        .unwrap();
    set(&doc, page, "Contents", reference(stream));
    assert!(
        matches!(doc.extract_text_structure_order(Default::default()),Err(PdfError::UnsupportedFeature(message)) if message.contains("multiple shared Form"))
    );
}

#[test]
fn explicit_empty_form_properties_scope_never_falls_back_to_page() {
    let (doc, _) = strict();
    let id = form(&doc, "/Span /Props BDC EMC", None);
    let mut dict =
        parse("<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] /Resources << >> >>")
            .into_dict()
            .unwrap();
    let mut page_scope = doc.get_page(0).unwrap().resource().unwrap();
    page_scope
        .try_insert(
            "Properties".into(),
            parse("<< /Props << /ActualText (page) >> >>"),
        )
        .unwrap();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    set(&doc, page, "Resources", PdfObject::PdfDict(page_scope));
    doc.update_object(
        id,
        PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict.clone(), b"/Span /Props BDC EMC".to_vec()).unwrap(),
        ),
    )
    .unwrap();
    doc.replace_page_contents(0, b"/Fm Do".to_vec()).unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text(Default::default())
            .is_err()
    );
    dict.remove("Resources");
    doc.update_object(
        id,
        PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, b"/Span /Props BDC EMC".to_vec()).unwrap(),
        ),
    )
    .unwrap();
    assert_eq!(text(&doc, Default::default()), "page");
}

#[test]
fn actualtext_and_text_objects_can_cross_page_contents_chunks() {
    let (doc, font) = strict();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let a = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                Default::default(),
                format!("BT /{font} 12 Tf /Span << /ActualText (combined) >> BDC (A) Tj ")
                    .into_bytes(),
            )
            .unwrap(),
        ))
        .unwrap();
    let b = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(Default::default(), b"(B) Tj EMC ET".to_vec()).unwrap(),
        ))
        .unwrap();
    set(
        &doc,
        page,
        "Contents",
        parse(&format!("[{} 0 R {} 0 R]", a.obj_num(), b.obj_num())),
    );
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap();
    assert_eq!(report.text(&Default::default()).unwrap(), "combined");
    assert_eq!(report.spans[0].sources.len(), 2);
}

#[test]
fn an_unmapped_glyph_is_not_guessed_and_actualtext_can_replace_it() {
    let (doc, font) = strict();
    // Undefined WinAnsi code with an explicit valid width: exercise missing
    // Unicode, not an unrelated missing-width interpreter error.
    let custom = doc.try_insert_object(parse("<< /Type /Font /Subtype /Type1 /BaseFont /Courier /Encoding /WinAnsiEncoding /FirstChar 0 /LastChar 0 /Widths [600] >>")).unwrap();
    let mut fonts = pdf::objects::pdf_dict::PdfDict::default();
    fonts.try_insert(font.clone(), reference(custom)).unwrap();
    let mut resources = doc.get_page(0).unwrap().resource().unwrap();
    resources
        .try_insert("Font".into(), PdfObject::PdfDict(fonts))
        .unwrap();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    set(&doc, page, "Resources", PdfObject::PdfDict(resources));
    contents(
        &doc,
        &font,
        "/Span << /ActualText (known) >> BDC <00> Tj EMC",
    );
    let raw = doc.get_page(0).unwrap().extract_marked_text(raw()).unwrap();
    assert!(raw.spans[0].text.is_none());
    assert!(matches!(
        raw.text(&Default::default()),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert_eq!(text(&doc, Default::default()), "known");
}

#[test]
fn pdf2_structure_artifact_filter_does_not_guess_custom_artifact_roles() {
    let (doc, font) = strict();
    contents(
        &doc,
        &font,
        "/P << /MCID 0 >> BDC (A) Tj EMC /P << /MCID 1 >> BDC (B) Tj EMC",
    );
    let (root, first, _, _) = two_nodes(&doc);
    set(&doc, ObjectId::new(2, 0), "Version", parse("/2.0"));
    let ns = doc
        .try_insert_object(parse("<< /NS (http://iso.org/pdf2/ssn) >>"))
        .unwrap();
    set(
        &doc,
        root,
        "Namespaces",
        parse(&format!("[{} 0 R]", ns.obj_num())),
    );
    set(&doc, first, "NS", reference(ns));
    set(&doc, first, "S", parse("/Artifact"));
    assert_eq!(
        doc.extract_text_structure_order(Default::default())
            .unwrap()
            .unwrap()
            .text(&Default::default())
            .unwrap(),
        "B"
    );
    assert_eq!(
        doc.extract_text_structure_order(raw())
            .unwrap()
            .unwrap()
            .text(&Default::default())
            .unwrap(),
        "BA"
    );
    set(&doc, ns, "NS", parse("(urn:custom)"));
    assert_eq!(
        doc.extract_text_structure_order(Default::default())
            .unwrap()
            .unwrap()
            .text(&Default::default())
            .unwrap(),
        "BA"
    );
}

#[test]
fn cross_page_declared_order_and_document_wide_execution_budget() {
    for max_operations in [100, 10] {
        let mut options = ParseOptions::default();
        options.limits.max_content_operations = max_operations;
        let (doc, font) = new_doc(options);
        doc.add_page(1).unwrap();
        let other_font = doc
            .resources()
            .register_standard_font(1, StandardFont::Courier)
            .unwrap();
        contents(&doc, &font, "/P << /MCID 0 >> BDC (A) Tj EMC");
        doc.replace_page_contents(
            1,
            format!("BT /{other_font} 12 Tf /P << /MCID 0 >> BDC (B) Tj EMC ET").into_bytes(),
        )
        .unwrap();
        let second_page = doc
            .destination(1, DestinationView::Fit)
            .unwrap()
            .page
            .object_id();
        let (root, first, second, _) = two_nodes(&doc);
        set(&doc, second, "Pg", reference(second_page));
        set(&doc, second, "K", parse("0"));
        set(&doc, second_page, "StructParents", parse("1"));
        set(
            &doc,
            root,
            "ParentTree",
            parse(&format!(
                "<< /Nums [0 [{} 0 R] 1 [{} 0 R]] >>",
                first.obj_num(),
                second.obj_num()
            )),
        );
        assert_eq!(text(&doc, Default::default()), "A");
        assert_eq!(
            doc.get_page(1)
                .unwrap()
                .extract_marked_text(Default::default())
                .unwrap()
                .text(&Default::default())
                .unwrap(),
            "B"
        );
        let result = doc.extract_text_structure_order(Default::default());
        if max_operations == 100 {
            assert_eq!(
                result.unwrap().unwrap().text(&Default::default()).unwrap(),
                "BA"
            );
        } else {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        }
    }
}

#[test]
fn nested_form_omitted_resources_use_page_not_immediate_parent() {
    let (doc, font) = strict();
    let child = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                parse("<< /Subtype /Form /BBox [0 0 10 10] >>")
                    .into_dict()
                    .unwrap(),
                format!("/Span /Props BDC BT /{font} 12 Tf (A) Tj ET EMC").into_bytes(),
            )
            .unwrap(),
        ))
        .unwrap();
    let parent=doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(parse(&format!("<< /Subtype /Form /BBox [0 0 10 10] /Resources << /Properties << /Props << /ActualText (wrong) >> >> /XObject << /Child {} 0 R >> >> >>",child.obj_num())).into_dict().unwrap(),b"/Child Do".to_vec()).unwrap())).unwrap();
    let mut resources = doc.get_page(0).unwrap().resource().unwrap();
    resources
        .try_insert(
            "Properties".into(),
            parse("<< /Props << /ActualText (page) >> >>"),
        )
        .unwrap();
    resources
        .try_insert(
            "XObject".into(),
            parse(&format!("<< /Parent {} 0 R >>", parent.obj_num())),
        )
        .unwrap();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    set(&doc, page, "Resources", PdfObject::PdfDict(resources));
    doc.replace_page_contents(0, b"/Parent Do".to_vec())
        .unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap();
    assert_eq!(report.text(&Default::default()).unwrap(), "page");
    assert_eq!(report.spans[0].sources[0].invocations.len(), 2);
}

#[test]
fn retained_sources_and_capture_entries_have_independent_limits() {
    let mut options = ParseOptions::default();
    options.limits.max_decoded_bytes = 1024;
    let (doc, font) = new_doc(options);
    contents(&doc, &font, &format!("({}) Tj", "A".repeat(100)));
    assert!(matches!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text(Default::default()),
        Err(PdfError::ResourceLimit(_))
    ));
    let mut options = ParseOptions::default();
    options.limits.max_objects = 10;
    let (doc, font) = new_doc(options);
    contents(&doc, &font, "/Span BMC (AAAAAAAAAA) Tj EMC");
    assert!(matches!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text(Default::default()),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn optional_content_xobjects_require_the_future_visibility_model() {
    let (doc, font) = strict();
    let id = form(&doc, &format!("BT /{font} 12 Tf (A) Tj ET"), None);
    let value = doc.resolve_object(id).unwrap();
    let stream = value.as_stream().unwrap();
    let mut dict = stream.dict().clone();
    dict.try_insert("OC".into(), parse("<< /Type /OCG /Name (Layer) >>"))
        .unwrap();
    doc.update_object(
        id,
        PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, stream.raw_data().unwrap().to_vec()).unwrap(),
        ),
    )
    .unwrap();
    doc.replace_page_contents(0, b"/Fm Do".to_vec()).unwrap();
    assert!(
        matches!(doc.get_page(0).unwrap().extract_marked_text(Default::default()),Err(PdfError::UnsupportedFeature(message)) if message.contains("optional content"))
    );
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap()
            .characters
            .len(),
        1
    );
}

#[test]
fn hidden_actualtext_still_consumes_the_document_wide_unicode_budget() {
    let mut options = ParseOptions::default();
    options.limits.max_text_chars = 5;
    let (doc, _) = new_doc(options);
    doc.add_page(1).unwrap();
    for i in 0..2 {
        doc.replace_page_contents(
            i,
            b"/Artifact << /MCID 0 /ActualText (1234) >> BDC EMC".to_vec(),
        )
        .unwrap();
    }
    let page2 = doc
        .destination(1, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let (root, first, second, _) = two_nodes(&doc);
    set(&doc, second, "Pg", reference(page2));
    set(&doc, second, "K", parse("0"));
    set(&doc, page2, "StructParents", parse("1"));
    set(
        &doc,
        root,
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [{} 0 R] 1 [{} 0 R]] >>",
            first.obj_num(),
            second.obj_num()
        )),
    );
    for i in 0..2 {
        assert!(
            doc.get_page(i)
                .unwrap()
                .extract_marked_text(Default::default())
                .unwrap()
                .spans
                .is_empty()
        );
    }
    assert!(
        matches!(doc.extract_text_structure_order(Default::default()),Err(PdfError::ResourceLimit(message)) if message.contains("cumulative Unicode"))
    );
}
