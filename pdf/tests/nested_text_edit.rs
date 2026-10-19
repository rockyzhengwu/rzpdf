use pdf::{
    document::{
        owned::Document,
        resource_manager::{ResourceKind, StandardFont},
    },
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

#[test]
fn nested_range_operation_budget_rejects_overlap_without_mutation() {
    for array in [false, true] {
        for succeeds in [false, true] {
            let doc = Document::new_with_options(pdf::parser::options::ParseOptions {
                limits: pdf::parser::options::Limits {
                    // The pool now includes the rebuilt leaf program, not
                    // just encoding bytes and range metadata.
                    max_operation_bytes: 131072,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
            doc.add_page(0).unwrap();
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            if array {
                // Top-level editing also charges font resource copies.
                let doc = Document::new_with_options(pdf::parser::options::ParseOptions {
                    limits: pdf::parser::options::Limits {
                        // Top-level binding now includes owned font resource copies.
                        max_operation_bytes: 131072,
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .unwrap();
                doc.add_page(0).unwrap();
                let font = doc
                    .resources()
                    .register_standard_font(0, StandardFont::Helvetica)
                    .unwrap();
                doc.replace_page_contents(
                    0,
                    format!("BT /{font} 12 Tf [(A) 25 (B)] TJ ET").into_bytes(),
                )
                .unwrap();
                let root = doc.get_page(0).unwrap().content_program().unwrap();
                let revision = doc.revision();
                let replacement = if succeeds {
                    "XYZ".to_owned()
                } else {
                    "X".repeat(131072)
                };
                let result =
                    doc.replace_page_text_array_range(0, &root, 2, (0, 1)..(2, 0), &replacement);
                assert_eq!(result.is_ok(), succeeds);
                if !succeeds {
                    assert!(matches!(
                        result,
                        Err(pdf::error::PdfError::ResourceLimit(_))
                    ));
                    assert_eq!(doc.revision(), revision);
                    assert_eq!(
                        doc.get_page(0)
                            .unwrap()
                            .content_program()
                            .unwrap()
                            .source_bytes(),
                        root.source_bytes()
                    );
                    doc.replace_page_text_array_range(0, &root, 2, (0, 0)..(2, 1), "X")
                        .unwrap();
                }
            }
            let mut dictionary = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(
                    b"<< /Subtype /Form /BBox [0 0 100 100] >>".to_vec(),
                ))
                .unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            dictionary.insert(
                "Resources".into(),
                PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
            );
            let form = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(
                        dictionary,
                        format!(
                            "BT /{font} 12 Tf {} ET",
                            if array { "[(A) 25 (B)] TJ" } else { "(AB) Tj" }
                        )
                        .into_bytes(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, form)
                .unwrap();
            doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
                .unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let report = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            let source = report.characters[0].nested_source.as_ref().unwrap();
            let revision = doc.revision();
            let resources = doc.get_page(0).unwrap().resource().unwrap();
            let replacement = if succeeds {
                "XYZ".to_owned()
            } else {
                "X".repeat(131072)
            };
            let result = if array {
                doc.replace_nested_text_array_range(0, &root, source, (0, 1)..(2, 0), &replacement)
            } else {
                doc.replace_nested_text_range(0, &root, source, None, 1..1, &replacement)
            };
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert!(matches!(
                    result,
                    Err(pdf::error::PdfError::ResourceLimit(_))
                ));
                assert_eq!(doc.revision(), revision);
                assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
                let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
                assert_eq!(unchanged.source_bytes(), root.source_bytes());
                assert_eq!(unchanged.stream_object_id(0), root.stream_object_id(0));
                if array {
                    doc.replace_nested_text_array_range(0, &root, source, (0, 0)..(2, 1), "X")
                        .unwrap();
                } else {
                    doc.replace_nested_text_range(0, &root, source, None, 0..2, "X")
                        .unwrap();
                }
            }
            let report = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            assert_eq!(
                report
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                if succeeds { "AXYZBAB" } else { "XAB" }
            );
        }
    }
}

#[test]
fn cross_item_unicode_range_preserves_adjustments_and_shared_forms() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let body = format!("BT /{font} 12 Tf [(ABCD) -12.00 %keep\n(EF) +3 (GH)] TJ ET");
    doc.replace_page_contents(0, body.as_bytes().to_vec())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let initial_resources = doc.get_page(0).unwrap().resource().unwrap();
    let initial_revision = doc.revision();
    for range in [(1, 0)..(4, 1), (0, 5)..(4, 1), (4, 1)..(0, 1)] {
        assert!(
            doc.replace_page_text_array_range(0, &root, 2, range, "X")
                .is_err()
        );
        assert_eq!(doc.revision(), initial_revision);
        assert_eq!(
            doc.get_page(0).unwrap().resource().unwrap(),
            initial_resources
        );
    }
    let rollback: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_page_text_array_range(0, &root, 2, (0, 1)..(4, 1), "X")?;
        let temporary = editing
            .get_page(0)?
            .extract_characters_paint_order_report()?;
        assert_eq!(
            temporary
                .characters
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AXH"
        );
        Err(pdf::error::PdfError::ObjectError(
            "injected top-level cross-item rollback".into(),
        ))
    });
    assert!(rollback.is_err());
    let refreshed = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(refreshed.source_bytes(), root.source_bytes());
    assert_eq!(refreshed.stream_object_id(0), root.stream_object_id(0));
    assert_eq!(
        doc.get_page(0).unwrap().resource().unwrap(),
        initial_resources
    );
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap()
            .characters
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "ABCDEFGH"
    );
    assert!(
        doc.replace_page_text_array_range(0, &root, 2, (0, 1)..(4, 1), "é€")
            .is_err()
    );
    let root = refreshed;
    doc.replace_page_text_array_range(0, &root, 2, (0, 1)..(4, 1), "é€")
        .unwrap();
    let edited = doc.get_page(0).unwrap().content_program().unwrap();
    let bytes = edited.source_bytes();
    assert!(
        bytes
            .windows(b"-12.00 %keep\n<> +3 <48>".len())
            .any(|part| part == b"-12.00 %keep\n<> +3 <48>")
    );
    // Preserve the top-level edited page for strict-save verification below.
    doc.copy_page(0, 2).unwrap();
    let mut dictionary = SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            b"<< /Subtype /Form /BBox [0 0 100 100] >>".to_vec(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    dictionary.insert(
        "Resources".into(),
        PdfObject::PdfDict(doc.get_page(1).unwrap().resource().unwrap()),
    );
    let form = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dictionary, body.into_bytes()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    doc.copy_page(0, 2).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    let source = report.characters[0].nested_source.as_ref().unwrap();
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let revision = doc.revision();
    for range in [(1, 0)..(4, 1), (0, 5)..(4, 1), (4, 1)..(0, 1)] {
        assert!(
            doc.replace_nested_text_array_range(0, &root, source, range, "X")
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    }
    let rollback: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_nested_text_array_range(0, &root, source, (0, 1)..(4, 1), "X")?;
        let temporary = editing
            .get_page(0)?
            .extract_characters_paint_order_report()?;
        assert_eq!(
            temporary
                .characters
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AXHABCDEFGH"
        );
        Err(pdf::error::PdfError::ObjectError(
            "injected cross-item rollback".into(),
        ))
    });
    assert!(rollback.is_err());
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    let refreshed = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(refreshed.source_bytes(), root.source_bytes());
    assert_eq!(refreshed.stream_object_id(0), root.stream_object_id(0));
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap()
            .characters
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "ABCDEFGHABCDEFGH"
    );
    assert!(
        doc.replace_nested_text_array_range(0, &root, source, (0, 1)..(4, 1), "é€")
            .is_err()
    );
    let root = refreshed;
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    let source = report.characters[0].nested_source.as_ref().unwrap();
    doc.replace_nested_text_array_range(0, &root, source, (0, 1)..(4, 1), "é€")
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for (page, expected) in [
            (0, "Aé€HABCDEFGH"),
            (1, "ABCDEFGH"),
            (2, "ABCDEFGHABCDEFGH"),
            (3, "Aé€H"),
        ] {
            let report = document
                .get_page(page)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            assert!(report.diagnostics.is_empty());
            assert_eq!(
                report
                    .characters
                    .iter()
                    .map(|ch| ch.unicode.as_deref().unwrap())
                    .collect::<String>(),
                expected
            );
        }
    }
}

#[test]
fn nested_composite_text_range_rejects_split_code_and_preserves_other_calls() {
    use pdf::objects::{pdf_dict::PdfDict, pdf_reference::PdfReference};
    fn dictionary(bytes: &[u8]) -> PdfDict {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(bytes.to_vec())).unwrap())
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap()
    }
    let doc = Document::new_with_options(pdf::parser::options::ParseOptions {
        limits: pdf::parser::options::Limits {
            max_text_chars: 4,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    let encoding = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
        PdfDict::default(), b"2 begincodespacerange <00> <7F> <8000> <80FF> endcodespacerange 2 begincidchar <41> 7 <8001> 8 endcidchar".to_vec()).unwrap())).unwrap();
    let unicode = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
        PdfDict::default(), b"2 begincodespacerange <00> <7F> <8000> <80FF> endcodespacerange 2 beginbfchar <41> <4E2D> <8001> <6587> endbfchar".to_vec()).unwrap())).unwrap();
    let mut font = dictionary(b"<< /Type /Font /Subtype /Type0 /BaseFont /Test /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Test /CIDSystemInfo << /Registry (Test) /Ordering (Test) /Supplement 0 >> /DW 500 >>] >>");
    font.insert(
        "Encoding".into(),
        PdfObject::PdfReference(PdfReference::from_id(encoding)),
    );
    font.insert(
        "ToUnicode".into(),
        PdfObject::PdfReference(PdfReference::from_id(unicode)),
    );
    let id = doc.try_insert_object(PdfObject::PdfDict(font)).unwrap();
    let font_name = doc
        .resources()
        .register_existing(0, ResourceKind::Font, id)
        .unwrap();
    let mut form = dictionary(b"<< /Subtype /Form /BBox [0 0 100 100] >>");
    form.insert(
        "Resources".into(),
        PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
    );
    let body = format!("BT /{font_name} 12 Tf <418001> Tj ET");
    let form = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(form, body.as_bytes().to_vec()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    let source = report.characters[0].nested_source.as_ref().unwrap();
    let revision = doc.revision();
    for range in [1..2, 2..3, 2..2] {
        assert!(matches!(
            doc.replace_nested_text_range(0, &root, source, None, range, "中"),
            Err(pdf::error::PdfError::ObjectError(_))
        ));
        assert_eq!(doc.revision(), revision);
    }
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .encode_text_for_font(&font_name, "中文中文")
            .unwrap()
            .len(),
        6
    );
    // Two old characters and four replacement characters each fit separately,
    // but replacing just one old character would retain five in the string.
    assert!(matches!(
        doc.replace_nested_text_range(0, &root, source, None, 1..3, "中文中文"),
        Err(pdf::error::PdfError::ResourceLimit(_))
    ));
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(unchanged.source_bytes(), root.source_bytes());
    assert_eq!(unchanged.stream_object_id(0), root.stream_object_id(0));
    doc.replace_nested_text_range(0, &root, source, None, 1..3, "中")
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for (page, expected) in [(0, "中中中文"), (1, "中文中文")] {
            let report = document
                .get_page(page)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            assert_eq!(
                report
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                expected
            );
            assert_eq!(
                report
                    .characters
                    .iter()
                    .map(|ch| ch.code_length)
                    .collect::<Vec<_>>(),
                if page == 0 {
                    vec![1, 1, 1, 2]
                } else {
                    vec![1, 2, 1, 2]
                }
            );
        }
        assert_eq!(
            document
                .resolve_object(form)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            body.as_bytes()
        );
    }
    let mut array_form = dictionary(b"<< /Subtype /Form /BBox [0 0 100 100] >>");
    array_form.insert(
        "Resources".into(),
        PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
    );
    let array_body = format!("BT /{font_name} 12 Tf [<8001> 25 %keep\n() -10 <8001>] TJ ET");
    let array_form = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(array_form, array_body.as_bytes().to_vec()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, array_form)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    doc.copy_page(0, 2).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    let source = report.characters[0].nested_source.as_ref().unwrap();
    let revision = doc.revision();
    for range in [(0, 1)..(4, 0), (0, 0)..(4, 1), (0, 1)..(0, 1)] {
        assert!(matches!(
            doc.replace_nested_text_array_range(0, &root, source, range, "中"),
            Err(pdf::error::PdfError::ObjectError(_))
        ));
        assert_eq!(doc.revision(), revision);
    }
    assert!(matches!(
        doc.replace_nested_text_array_range(0, &root, source, (0, 0)..(4, 0), "中文中文"),
        Err(pdf::error::PdfError::ResourceLimit(_))
    ));
    assert_eq!(doc.revision(), revision);
    doc.replace_nested_text_array_range(0, &root, source, (0, 0)..(4, 0), "中")
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for (page, expected, lengths) in [
            (0, "中文文文", vec![1, 2, 2, 2]),
            (2, "文文文文", vec![2, 2, 2, 2]),
        ] {
            let report = document
                .get_page(page)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            assert_eq!(
                report
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                expected
            );
            assert_eq!(
                report
                    .characters
                    .iter()
                    .map(|ch| ch.code_length)
                    .collect::<Vec<_>>(),
                lengths
            );
        }
        assert_eq!(
            document
                .resolve_object(array_form)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            array_body.as_bytes()
        );
    }
}

#[test]
fn empty_nested_text_can_be_populated_without_character_event() {
    use pdf::page::{
        content_program::ContentProgram,
        source_provenance::{FormInvocation, NestedDrawSource},
    };
    for body in [b"BT () Tj ET".as_slice(), b"BT <> Tj ET".as_slice()] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let dictionary = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Subtype /Form /BBox [0 0 100 100] /Resources << >> >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let form = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dictionary, body.to_vec()).unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, form)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{font} 12 Tf ET /{name} Do /{name} Do").into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        assert!(
            doc.get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap()
                .characters
                .is_empty()
        );
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let leaf = ContentProgram::from_decoded_bytes(body.to_vec(), Default::default()).unwrap();
        let source = NestedDrawSource {
            invocations: vec![FormInvocation {
                form_id: Some(form),
                operation_index: 3,
                operator_byte_offset: root.operations()[3].operator.byte_offset(),
            }],
            operation_index: 1,
            operator_byte_offset: leaf.operations()[1].operator.byte_offset(),
            geometry_operation_indices: vec![],
        };
        doc.replace_nested_text(0, &root, &source, "é€").unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let report = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap();
        let source = report.characters[0].nested_source.as_ref().unwrap();
        let revision = doc.revision();
        assert!(
            doc.replace_nested_text_range(0, &root, source, None, 2..3, "A")
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let rollback: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
            editing.replace_nested_text_range(0, &root, source, None, 1..2, "A")?;
            assert_eq!(
                editing
                    .get_page(0)?
                    .extract_characters_paint_order_report()?
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "éA"
            );
            Err(pdf::error::PdfError::ObjectError(
                "injected range rollback".into(),
            ))
        });
        assert!(rollback.is_err());
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        let refreshed = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(refreshed.source_bytes(), root.source_bytes());
        assert_eq!(refreshed.stream_object_id(0), root.stream_object_id(0));
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap()
                .characters
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "é€"
        );
        assert!(
            doc.replace_nested_text_range(0, &root, source, None, 1..2, "A")
                .is_err()
        );
        doc.replace_nested_text_range(0, &refreshed, source, None, 1..2, "A")
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let report = document
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            assert_eq!(
                report
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "éA"
            );
            assert_eq!(
                report
                    .characters
                    .iter()
                    .map(|ch| ch.charcode)
                    .collect::<Vec<_>>(),
                [0xe9, 0x41]
            );
            assert!(
                report
                    .characters
                    .iter()
                    .all(|ch| ch.font_name.as_deref() == Some("Helvetica"))
            );
            assert!(
                document
                    .get_page(1)
                    .unwrap()
                    .extract_characters_paint_order_report()
                    .unwrap()
                    .characters
                    .is_empty()
            );
            assert_eq!(
                document
                    .resolve_object(form)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .raw_data(),
                body
            );
        }
    }
}

#[test]
fn nested_unicode_replacement_inherits_caller_font_across_scoped_q_restore() {
    fn dict(text: &str) -> pdf::objects::pdf_dict::PdfDict {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap()
    }
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let helvetica = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let courier = doc
        .resources()
        .register_standard_font(1, StandardFont::Courier)
        .unwrap();
    assert_eq!(helvetica, courier);
    let scope = doc.get_page(1).unwrap().resource().unwrap();
    let mut leaf_dict = dict("<< /Subtype /Form /BBox [0 0 100 100] >>");
    leaf_dict.insert("Resources".into(), PdfObject::PdfDict(scope.clone()));
    let leaf = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(leaf_dict, b"BT (AB) Tj ET".to_vec()).unwrap(),
        ))
        .unwrap();
    let mut outer_dict = dict(&format!(
        "<< /Subtype /Form /BBox [0 0 100 100] /Resources << /XObject << /Child {} 0 R >> >> >>",
        leaf.obj_num()
    ));
    let mut outer_scope = outer_dict
        .get("Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .clone();
    outer_scope.insert("Font".into(), scope.get("Font").unwrap().clone());
    outer_dict.insert("Resources".into(), PdfObject::PdfDict(outer_scope));
    let outer = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                outer_dict,
                format!("q BT /{courier} 12 Tf ET Q /Child Do").into_bytes(),
            )
            .unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, outer)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{helvetica} 12 Tf ET /{name} Do /{name} Do").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 2).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    assert!(
        report
            .characters
            .iter()
            .all(|ch| ch.font_name.as_deref() == Some("Helvetica"))
    );
    let source = report.characters[0].nested_source.as_ref().unwrap();
    let revision = doc.revision();
    assert!(doc.replace_nested_text(0, &root, source, "中").is_err());
    assert_eq!(doc.revision(), revision);
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let rollback: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_nested_text(0, &root, source, "XY")?;
        assert_eq!(
            editing
                .get_page(0)?
                .extract_characters_paint_order_report()?
                .characters
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "XYAB"
        );
        Err(pdf::error::PdfError::ObjectError(
            "injected nested Unicode rollback".into(),
        ))
    });
    assert!(rollback.is_err());
    let restored = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(restored.source_bytes(), root.source_bytes());
    assert_eq!(restored.stream_object_id(0), root.stream_object_id(0));
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap()
            .characters
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "ABAB"
    );
    assert!(doc.replace_nested_text(0, &root, source, "XY").is_err());
    doc.replace_nested_text(0, &restored, source, "é€").unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for (page, expected) in [(0, "é€AB"), (2, "ABAB")] {
            let report = document
                .get_page(page)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            assert_eq!(
                report
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                expected
            );
            assert!(
                report
                    .characters
                    .iter()
                    .all(|ch| ch.font_name.as_deref() == Some("Helvetica"))
            );
            if page == 0 {
                assert_eq!(report.characters[0].charcode, 0xe9);
                assert_eq!(report.characters[1].charcode, 0x80);
                assert_eq!(report.characters[0].code_length, 1);
                assert_eq!(report.characters[1].code_length, 1);
                let root = document.get_page(0).unwrap().content_program().unwrap();
                let copied = document
                    .get_page(0)
                    .unwrap()
                    .nested_content_program(
                        &root,
                        report.characters[0].nested_source.as_ref().unwrap(),
                    )
                    .unwrap();
                assert_eq!(
                    copied.operations()[1]
                        .operator
                        .get_as_string(0)
                        .unwrap()
                        .bytes(),
                    [0xe9, 0x80]
                );
            }
        }
        assert_eq!(
            document
                .resolve_object(leaf)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"BT (AB) Tj ET"
        );
    }
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    let source = report.characters[0].nested_source.as_ref().unwrap();
    assert_eq!(source.invocations.len(), 2);
    doc.replace_nested_text_range(0, &root, source, None, 0..1, "A")
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for (page, expected) in [(0, "A€AB"), (2, "ABAB")] {
            let report = document
                .get_page(page)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            assert_eq!(
                report
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                expected
            );
            assert!(
                report
                    .characters
                    .iter()
                    .all(|ch| ch.font_name.as_deref() == Some("Helvetica"))
            );
        }
        assert_eq!(
            document
                .resolve_object(leaf)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"BT (AB) Tj ET"
        );
    }
    for (range, text, expected) in [(2..2, "Z", "A€ZAB"), (0..3, "", "AB")] {
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let report = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap();
        let source = report.characters[0].nested_source.as_ref().unwrap();
        doc.replace_nested_text_range(0, &root, source, None, range, text)
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            for (page, expected) in [(0, expected), (2, "ABAB")] {
                let report = document
                    .get_page(page)
                    .unwrap()
                    .extract_characters_paint_order_report()
                    .unwrap();
                assert_eq!(
                    report
                        .characters
                        .iter()
                        .filter_map(|ch| ch.unicode.as_deref())
                        .collect::<String>(),
                    expected
                );
            }
            assert_eq!(
                document
                    .resolve_object(leaf)
                    .unwrap()
                    .into_stream()
                    .unwrap()
                    .raw_data(),
                b"BT (AB) Tj ET"
            );
        }
    }
}

#[test]
fn nested_tj_and_quotes_clear_one_event_preserving_operator_state_and_other_calls() {
    fn dictionary(source: &str) -> pdf::objects::pdf_dict::PdfDict {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap()
    }
    for (show, operator) in [
        ("[(A) 25 % keep\n(B) -10] TJ", "TJ"),
        ("(AB) '", "'"),
        ("2 1 (AB) \"", "\""),
    ] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let mut dict = dictionary("<< /Subtype /Form /BBox [0 0 100 100] >>");
        dict.insert(
            "Resources".into(),
            PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
        );
        let body = format!("BT /{font} 12 Tf 14 TL {show} 1 0 0 1 20 20 Tm (C) Tj ET");
        let leaf = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, body.as_bytes().to_vec()).unwrap(),
            ))
            .unwrap();
        let outer = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(dictionary(&format!("<< /Subtype /Form /BBox [0 0 100 100] /Resources << /XObject << /Child {} 0 R >> >> >>",leaf.obj_num())),b"/Child Do".to_vec()).unwrap())).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, outer)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
            .unwrap();
        doc.copy_page(0, 1).unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let report = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap();
        let source = report.characters[0].nested_source.as_ref().unwrap();
        assert_eq!(source.invocations.len(), 2);
        assert_eq!(source.operation_index, 3);
        doc.remove_nested_text_draw(0, &root, source).unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let mut page = document.get_page(0).unwrap();
            let root = page.content_program().unwrap();
            let report = page.extract_characters_paint_order_report().unwrap();
            assert_eq!(
                report
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "CABC"
            );
            let remaining = report.characters[0].nested_source.as_ref().unwrap();
            assert_ne!(remaining.invocations[0].form_id, Some(outer));
            assert_ne!(remaining.invocations[1].form_id, Some(leaf));
            let copied = page.nested_content_program(&root, remaining).unwrap();
            let edited = &copied.operations()[3].operator;
            assert_eq!(edited.name(), operator);
            assert_eq!(copied.operations()[2].operator.name(), "TL");
            assert_eq!(
                copied.operations()[2].operator.operand(0).unwrap().as_f32(),
                Some(14.)
            );
            let list = document.get_page(0).unwrap().display_list().unwrap();
            let pdf::page::page_object::PageObject::Text(neighbor) = &list.objects()[0] else {
                panic!("missing independent C text")
            };
            assert_eq!(
                neighbor.state.text_state.word_space,
                if operator == "\"" { 2. } else { 0. }
            );
            assert_eq!(
                neighbor.state.text_state.char_space,
                if operator == "\"" { 1. } else { 0. }
            );
            if operator == "TJ" {
                let array = edited.get_as_array(0).unwrap();
                assert_eq!(array.get(1).unwrap().as_f32(), Some(25.));
                assert_eq!(array.get(3).unwrap().as_f32(), Some(-10.));
                assert!(
                    array
                        .get(0)
                        .unwrap()
                        .as_string()
                        .unwrap()
                        .bytes()
                        .is_empty()
                );
                assert!(
                    array
                        .get(2)
                        .unwrap()
                        .as_string()
                        .unwrap()
                        .bytes()
                        .is_empty()
                );
                assert!(
                    std::str::from_utf8(copied.source_bytes())
                        .unwrap()
                        .contains("% keep\n")
                );
            } else {
                if operator == "\"" {
                    assert_eq!(edited.operand(0).unwrap().as_f32(), Some(2.));
                    assert_eq!(edited.operand(1).unwrap().as_f32(), Some(1.));
                }
                assert!(
                    edited
                        .get_as_string(if operator == "\"" { 2 } else { 0 })
                        .unwrap()
                        .bytes()
                        .is_empty()
                );
            }
            assert_eq!(
                document
                    .get_page(1)
                    .unwrap()
                    .extract_characters_paint_order_report()
                    .unwrap()
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "ABCABC"
            );
            assert_eq!(
                document
                    .resolve_object(leaf)
                    .unwrap()
                    .into_stream()
                    .unwrap()
                    .raw_data(),
                body.as_bytes()
            );
        }
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let report = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap();
        let source = report.characters[0].nested_source.as_ref().unwrap();
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let rollback: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
            editing.replace_nested_text_codes(0, &root, source, b"D")?;
            assert_eq!(
                editing
                    .get_page(0)?
                    .extract_characters_paint_order_report()?
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "DABC"
            );
            Err(pdf::error::PdfError::ObjectError(
                "injected nested replacement rollback".into(),
            ))
        });
        assert!(rollback.is_err());
        let restored = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(restored.source_bytes(), root.source_bytes());
        assert_eq!(restored.stream_object_id(0), root.stream_object_id(0));
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap()
                .characters
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "CABC"
        );
        assert!(
            doc.replace_nested_text_codes(0, &root, source, b"D")
                .is_err()
        );
        doc.replace_nested_text_codes(0, &restored, source, b"D")
            .unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let report = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap();
        let mut source = report.characters[0].nested_source.as_ref().unwrap().clone();
        let copied = doc
            .get_page(0)
            .unwrap()
            .nested_content_program(&root, &source)
            .unwrap();
        source.operation_index = 3;
        source.operator_byte_offset = copied.operations()[3].operator.byte_offset();
        if operator == "TJ" {
            let revision = doc.revision();
            for item in [1, usize::MAX] {
                assert!(matches!(
                    doc.replace_nested_text_array_item(0, &root, &source, item, "中文"),
                    Err(pdf::error::PdfError::ObjectError(_))
                ));
                assert_eq!(doc.revision(), revision);
            }
            assert!(
                doc.replace_nested_text_array_item_codes(0, &root, &source, 1, b"Z")
                    .is_err()
            );
            assert_eq!(doc.revision(), revision);
            doc.replace_nested_text_array_item(0, &root, &source, 0, "Z")
                .unwrap();
        } else {
            doc.replace_nested_text(0, &root, &source, "Z").unwrap();
        }
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            for (index, expected) in [(0, "ZDABC"), (1, "ABCABC")] {
                assert_eq!(
                    document
                        .get_page(index)
                        .unwrap()
                        .extract_characters_paint_order_report()
                        .unwrap()
                        .characters
                        .iter()
                        .filter_map(|ch| ch.unicode.as_deref())
                        .collect::<String>(),
                    expected
                );
            }
            assert_eq!(
                document
                    .resolve_object(leaf)
                    .unwrap()
                    .into_stream()
                    .unwrap()
                    .raw_data(),
                body.as_bytes()
            );
        }
        for (range, text, expected) in [(1..1, "é", "ZéDABC"), (0..1, "Y", "YéDABC")] {
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let report = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            let source = report.characters[0].nested_source.as_ref().unwrap();
            let item = (operator == "TJ").then_some(0);
            doc.replace_nested_text_range(0, &root, source, item, range, text)
                .unwrap();
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for document in [&doc, &reopened] {
                for (page, expected) in [(0, expected), (1, "ABCABC")] {
                    let report = document
                        .get_page(page)
                        .unwrap()
                        .extract_characters_paint_order_report()
                        .unwrap();
                    assert_eq!(
                        report
                            .characters
                            .iter()
                            .filter_map(|ch| ch.unicode.as_deref())
                            .collect::<String>(),
                        expected
                    );
                }
                let root = document.get_page(0).unwrap().content_program().unwrap();
                let report = document
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order_report()
                    .unwrap();
                let leaf_program = document
                    .get_page(0)
                    .unwrap()
                    .nested_content_program(
                        &root,
                        report.characters[0].nested_source.as_ref().unwrap(),
                    )
                    .unwrap();
                let edited = &leaf_program.operations()[3].operator;
                assert_eq!(edited.name(), operator);
                if operator == "TJ" {
                    let array = edited.get_as_array(0).unwrap();
                    assert_eq!(array.get(1).unwrap().as_f32(), Some(25.));
                    assert_eq!(array.get(3).unwrap().as_f32(), Some(-10.));
                } else if operator == "\"" {
                    assert_eq!(edited.get_as_f32(0).unwrap(), 2.);
                    assert_eq!(edited.get_as_f32(1).unwrap(), 1.);
                }
            }
        }
    }
}
