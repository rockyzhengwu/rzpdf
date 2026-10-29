use pdf::{
    document::{
        PdfDocument,
        resource_manager::{ResourceKind, StandardFont},
    },
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference,
        pdf_stream::PdfStream,
    },
    page::{content_program::ContentProgram, source_provenance::NestedDrawSource},
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

type Doc = PdfDocument<Cursor<Vec<u8>>>;

fn dictionary(bytes: &[u8]) -> PdfDict {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(bytes)).unwrap())
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap()
}

fn fixture(event: &str) -> (Doc, ObjectId, ObjectId, String) {
    fixture_with_options(event, Default::default())
}

fn fixture_with_options(
    event: &str,
    options: pdf::parser::options::ParseOptions,
) -> (Doc, ObjectId, ObjectId, String) {
    let doc = Doc::new_with_options(options).unwrap();
    doc.add_page(0).unwrap();
    let courier = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    let helvetica = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let scope = doc.get_page(0).unwrap().resource().unwrap();
    let target = match event {
        "Tj" => "(AB) Tj (C) Tj",
        "'" => "(AB) ' (C) Tj",
        "\"" => "3 2 (AB) \" (C) Tj",
        "item" => "[(AB)17.00(C)] TJ",
        "range" => "[(AB)17.00(EF)% keep\n-20.50(GH)] TJ (C) Tj",
        "events" => "(AB) Tj [(EF)17.00(GH)] TJ (C) Tj",
        _ => unreachable!(),
    };
    let body = format!(
        "BT q /{helvetica} 20 Tf 9 Tc 8 Tw 25 Tz Q 1 .3 .2 1 100 100 Tm {target} T* (D) Tj ET"
    );
    let mut leaf_dict = dictionary(
        b"<< /Type /XObject /Subtype /Form /BBox [-200 -200 800 800] /Matrix [1 .1 .2 1 5 7] >>",
    );
    leaf_dict.insert("Resources".into(), PdfObject::PdfDict(scope.clone()));
    let leaf = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(leaf_dict, body.as_bytes().to_vec()).unwrap(),
        ))
        .unwrap();
    let mut outer_scope = scope;
    let mut xobjects = PdfDict::default();
    xobjects.insert(
        "Leaf".into(),
        PdfObject::PdfReference(PdfReference::from_id(leaf)),
    );
    outer_scope.insert("XObject".into(), PdfObject::PdfDict(xobjects));
    let mut outer_dict = dictionary(b"<< /Type /XObject /Subtype /Form /BBox [-200 -200 1000 1000] /Matrix [1 .2 -.1 1 15 20] >>");
    outer_dict.insert("Resources".into(), PdfObject::PdfDict(outer_scope));
    let outer = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(outer_dict,
        format!("q BT /{helvetica} 20 Tf 9 Tc 8 Tw 25 Tz ET Q q /Leaf Do Q q 1 0 0 1 250 0 cm /Leaf Do Q").into_bytes()).unwrap())).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, outer)
        .unwrap();
    doc.replace_page_contents(0, format!("BT /{courier} 10 Tf 2 Tc 3 Tw -75 Tz 14 TL ET q /{name} Do Q q 1 0 0 1 0 300 cm /{name} Do Q").into_bytes()).unwrap();
    doc.copy_page(0, 1).unwrap();
    (doc, leaf, outer, body)
}

fn selection(doc: &Doc) -> (ContentProgram, NestedDrawSource) {
    let mut page = doc.get_page(0).unwrap();
    let root = page.content_program().unwrap();
    let source = page.extract_characters_paint_order().unwrap()[0]
        .nested_source
        .as_ref()
        .unwrap()
        .clone();
    assert_eq!(source.invocations.len(), 2);
    (root, source)
}

#[test]
fn cross_invocation_span_outer_error_and_unwind_restore_publication() {
    use pdf::page::source_provenance::NestedTextCodePosition;
    for preserve in [false, true] {
        for unwind in [false, true] {
            let (doc, leaf, outer, _) = fixture("events");
            let saved = doc.save_to_bytes().unwrap();
            let leaf_before = doc.resolve_object(leaf).unwrap();
            let outer_before = doc.resolve_object(outer).unwrap();
            let edit = |doc: &Doc| -> PdfResult<()> {
                let root = doc.get_page(0)?.content_program()?;
                let chars = doc.get_page(0)?.extract_characters_paint_order()?;
                let start = NestedTextCodePosition {
                    source: chars[0].nested_source.as_ref().unwrap(),
                    array_item: None,
                    offset: 1,
                };
                let end = NestedTextCodePosition {
                    source: chars[28].nested_source.as_ref().unwrap(),
                    array_item: Some(2),
                    offset: 1,
                };
                if preserve {
                    doc.replace_nested_text_span_preserving_advance(0, &root, start..end, "X")
                } else {
                    doc.replace_nested_text_span(0, &root, start..end, "X")
                }
            };
            if unwind {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _: PdfResult<()> = doc.edit_transaction(|doc| {
                        edit(doc)?;
                        panic!("cross invocation outer unwind");
                    });
                }));
                assert_eq!(
                    caught.unwrap_err().downcast_ref::<&str>().copied(),
                    Some("cross invocation outer unwind")
                );
            } else {
                let failed: PdfResult<()> = doc.edit_transaction(|doc| {
                    edit(doc)?;
                    Err(PdfError::DocumentError(
                        "cross invocation outer error".into(),
                    ))
                });
                assert!(
                    matches!(failed, Err(PdfError::DocumentError(ref message)) if message == "cross invocation outer error")
                );
            }
            assert_eq!(doc.resolve_object(leaf).unwrap(), leaf_before);
            assert_eq!(doc.resolve_object(outer).unwrap(), outer_before);
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            // Rollback invalidates snapshots; obtain fresh endpoints for retry.
            edit(&doc).unwrap();
            Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
        }
    }
}

#[test]
fn cross_invocation_span_quota_rejection_keeps_original_endpoints() {
    use pdf::page::source_provenance::NestedTextCodePosition;
    let options = pdf::parser::options::ParseOptions {
        limits: pdf::parser::options::Limits {
            // Permit the deliberately oversized input through character-count
            // validation so rejection must exercise the live-byte pool.
            max_operation_bytes: 8 * 1024 * 1024,
            max_text_chars: 16 * 1024 * 1024,
            ..Default::default()
        },
        ..Default::default()
    };
    for preserve in [false, true] {
        let (doc, leaf, outer, _) = fixture_with_options("events", options);
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let chars = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        let start = NestedTextCodePosition {
            source: chars[0].nested_source.as_ref().unwrap(),
            array_item: None,
            offset: 1,
        };
        let end = NestedTextCodePosition {
            source: chars[28].nested_source.as_ref().unwrap(),
            array_item: Some(2),
            offset: 1,
        };
        let stamp = doc.stamp();
        let saved = doc.save_to_bytes().unwrap();
        let leaf_before = doc.resolve_object(leaf).unwrap();
        let outer_before = doc.resolve_object(outer).unwrap();
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let replace = |text: &str| {
            if preserve {
                doc.replace_nested_text_span_preserving_advance(0, &root, start..end, text)
            } else {
                doc.replace_nested_text_span(0, &root, start..end, text)
            }
        };
        assert!(matches!(replace(&"X".repeat(8 * 1024 * 1024)),
            Err(PdfError::ResourceLimit(ref message)) if message == "operation live bytes"));
        assert!(doc.is_stamp_current(&stamp));
        assert_eq!(doc.resolve_object(leaf).unwrap(), leaf_before);
        assert_eq!(doc.resolve_object(outer).unwrap(), outer_before);
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        assert_eq!(doc.save_to_bytes().unwrap(), saved);
        replace("X").unwrap();
        Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    }
}

#[test]
fn complex_four_call_span_fixed_budget_ladder() {
    use pdf::page::source_provenance::NestedTextCodePosition;
    for preserve in [false, true] {
        let mut successes = 0;
        for ceiling in [512 * 1024, 1024 * 1024, 2 * 1024 * 1024, 8 * 1024 * 1024] {
            // Each rung owns a fresh document and operation pool. This measures
            // success at fixed ceilings, not exact peak, RSS or minimum memory.
            let options = pdf::parser::options::ParseOptions {
                limits: pdf::parser::options::Limits {
                    max_operation_bytes: ceiling,
                    ..Default::default()
                },
                ..Default::default()
            };
            let (doc, leaf, outer, _) = fixture_with_options("events", options);
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let before = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let original = before
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>();
            let saved = doc.save_to_bytes().unwrap();
            let stamp = doc.stamp();
            let leaf_before = doc.resolve_object(leaf).unwrap();
            let outer_before = doc.resolve_object(outer).unwrap();
            let resources = doc.get_page(0).unwrap().resource().unwrap();
            let start = NestedTextCodePosition {
                source: before[0].nested_source.as_ref().unwrap(),
                array_item: None,
                offset: 1,
            };
            let end = NestedTextCodePosition {
                source: before[28].nested_source.as_ref().unwrap(),
                array_item: Some(2),
                offset: 1,
            };
            let result = if preserve {
                doc.replace_nested_text_span_preserving_advance(0, &root, start..end, "X")
            } else {
                doc.replace_nested_text_span(0, &root, start..end, "X")
            };
            match result {
                Err(PdfError::ResourceLimit(ref message)) if message == "operation live bytes" => {
                    println!("complex four-call preserve={preserve} ceiling={ceiling}: rejected");
                    assert!(doc.is_stamp_current(&stamp));
                    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
                    assert_eq!(doc.save_to_bytes().unwrap(), saved);
                    assert!(ceiling < 8 * 1024 * 1024, "known 8MiB retry must succeed");
                }
                Ok(()) => {
                    successes += 1;
                    println!("complex four-call preserve={preserve} ceiling={ceiling}: passed");
                    let reopened =
                        Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                            .unwrap();
                    let after = reopened
                        .get_page(0)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    assert_eq!(
                        after
                            .iter()
                            .filter_map(|ch| ch.unicode.as_deref())
                            .collect::<String>(),
                        "AXHCD"
                    );
                    assert_eq!(
                        reopened
                            .get_page(1)
                            .unwrap()
                            .extract_characters_paint_order()
                            .unwrap()
                            .iter()
                            .filter_map(|ch| ch.unicode.as_deref())
                            .collect::<String>(),
                        original
                    );
                    if preserve {
                        for (old, new) in [(0, 0), (29, 2), (30, 3), (31, 4)] {
                            assert!(
                                (before[old].position.x() - after[new].position.x()).abs() < 0.002
                            );
                            assert!(
                                (before[old].position.y() - after[new].position.y()).abs() < 0.002
                            );
                        }
                    }
                }
                Err(error) => panic!("unexpected fixed-budget error at {ceiling}: {error:?}"),
            }
            assert_eq!(doc.resolve_object(leaf).unwrap(), leaf_before);
            assert_eq!(doc.resolve_object(outer).unwrap(), outer_before);
        }
        assert!(successes > 0);
    }
}

#[test]
fn source_qualified_span_across_shared_ancestors_preserves_other_page() {
    use pdf::page::source_provenance::NestedTextCodePosition;
    for preserve in [false, true] {
        let (doc, _, _, _) = fixture("events");
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let before = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        let original: String = before
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect();
        let start = NestedTextCodePosition {
            source: before[0].nested_source.as_ref().unwrap(),
            array_item: None,
            offset: 1,
        };
        let end = NestedTextCodePosition {
            source: before[28].nested_source.as_ref().unwrap(),
            array_item: Some(2),
            offset: 1,
        };
        let stamp = doc.stamp();
        let saved = doc.save_to_bytes().unwrap();
        let invalid_end = NestedTextCodePosition { offset: 3, ..end };
        for (finish, replacement) in [(invalid_end, "X"), (end, "😀")] {
            let result = if preserve {
                doc.replace_nested_text_span_preserving_advance(
                    0,
                    &root,
                    start..finish,
                    replacement,
                )
            } else {
                doc.replace_nested_text_span(0, &root, start..finish, replacement)
            };
            assert!(result.is_err());
            assert!(doc.is_stamp_current(&stamp));
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
        }
        if preserve {
            doc.replace_nested_text_span_preserving_advance(0, &root, start..end, "X")
                .unwrap();
        } else {
            doc.replace_nested_text_span(0, &root, start..end, "X")
                .unwrap();
        }
        let reopened =
            Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
        let after = reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(
            after
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AXHCD"
        );
        assert_eq!(
            reopened
                .get_page(1)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap()
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            original
        );
        if preserve {
            for (old, new) in [(0, 0), (29, 2), (30, 3), (31, 4)] {
                assert!((before[old].position.x() - after[new].position.x()).abs() < 0.002);
                assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
            }
        }
    }
}

#[test]
fn source_qualified_nested_span_checks_both_invocations_and_end_byte_position() {
    use pdf::page::source_provenance::NestedTextCodePosition;
    for preserve in [false, true] {
        let (doc, _, _, _) = fixture("events");
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let before = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        let start_source = before[0].nested_source.as_ref().unwrap();
        let end_source = before[4].nested_source.as_ref().unwrap();
        let another_call = before[28].nested_source.as_ref().unwrap();
        assert_eq!(end_source.operation_index, another_call.operation_index);
        let start = NestedTextCodePosition {
            source: start_source,
            array_item: None,
            offset: 1,
        };
        let endpoint = |source| NestedTextCodePosition {
            source,
            array_item: Some(2),
            offset: 1,
        };
        let replace = |end| {
            if preserve {
                doc.replace_nested_text_span_preserving_advance(0, &root, start..end, "X")
            } else {
                doc.replace_nested_text_span(0, &root, start..end, "X")
            }
        };
        let stamp = doc.stamp();
        let reverse_start = endpoint(another_call);
        let reverse = if preserve {
            doc.replace_nested_text_span_preserving_advance(0, &root, reverse_start..start, "X")
        } else {
            doc.replace_nested_text_span(0, &root, reverse_start..start, "X")
        };
        assert!(reverse.is_err());
        let mut wrong_offset = end_source.clone();
        wrong_offset.operator_byte_offset = Some(0);
        assert!(
            matches!(replace(endpoint(&wrong_offset)), Err(PdfError::ObjectError(ref message)) if message == "span endpoint byte position mismatch")
        );
        assert!(doc.is_stamp_current(&stamp));
        for page in [0, 1] {
            assert_eq!(
                doc.get_page(page)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                root.source_bytes()
            );
        }
        replace(endpoint(end_source)).unwrap();
        let reopened =
            Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
        let after = reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(
            after[..5]
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AXHCD"
        );
        for (old, new) in before[8..].iter().zip(&after[5..]) {
            assert_eq!(old.unicode, new.unicode);
            assert!((old.position.x() - new.position.x()).abs() < 0.002);
            assert!((old.position.y() - new.position.y()).abs() < 0.002);
        }
        if preserve {
            for (old_index, new_index) in [(0, 0), (5, 2), (6, 3), (7, 4)] {
                assert!(
                    (before[old_index].position.x() - after[new_index].position.x()).abs() < 0.002
                );
                assert!(
                    (before[old_index].position.y() - after[new_index].position.y()).abs() < 0.002
                );
            }
        }
        assert_eq!(
            reopened
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
    }
}

#[test]
fn nested_event_range_qualifies_last_call_and_rejects_wrong_root_snapshots() {
    use pdf::page::content_program::TextCodePosition;
    for preserve in [false, true] {
        let (doc, _, _, _) = fixture("events");
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let before = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        let source = before[24].nested_source.as_ref().unwrap();
        let first = before[0].nested_source.as_ref().unwrap();
        assert_ne!(source.invocations, first.invocations);
        assert_eq!(source.operation_index, first.operation_index);
        let start = TextCodePosition {
            operation: source.operation_index,
            array_item: None,
            offset: 1,
        };
        let end = TextCodePosition {
            operation: source.operation_index + 1,
            array_item: Some(2),
            offset: 1,
        };
        let replace = |snapshot: &ContentProgram| {
            if preserve {
                doc.replace_nested_text_event_range_preserving_advance(
                    0,
                    snapshot,
                    source,
                    start..end,
                    "X",
                )
            } else {
                doc.replace_nested_text_event_range(0, snapshot, source, start..end, "X")
            }
        };
        let wrong_page = doc.get_page(1).unwrap().content_program().unwrap();
        let offline =
            ContentProgram::from_decoded_bytes(root.source_bytes().to_vec(), Default::default())
                .unwrap();
        let (foreign, _, _, _) = fixture("events");
        let foreign_root = foreign.get_page(0).unwrap().content_program().unwrap();
        let stamp = doc.stamp();
        for snapshot in [&wrong_page, &offline, &foreign_root] {
            assert!(replace(snapshot).is_err());
            assert!(doc.is_stamp_current(&stamp));
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                root.source_bytes()
            );
        }
        replace(&root).unwrap();
        let reopened =
            Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
        let after = reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(after.len(), 29);
        for (old, new) in before[..24].iter().zip(&after[..24]) {
            assert_eq!(old.unicode, new.unicode);
            assert!((old.position.x() - new.position.x()).abs() < 0.002);
            assert!((old.position.y() - new.position.y()).abs() < 0.002);
        }
        assert_eq!(
            after[24..]
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AXHCD"
        );
        if preserve {
            for (old_index, new_index) in [(24, 24), (29, 26), (30, 27), (31, 28)] {
                assert!(
                    (before[old_index].position.x() - after[new_index].position.x()).abs() < 0.002
                );
                assert!(
                    (before[old_index].position.y() - after[new_index].position.y()).abs() < 0.002
                );
            }
        }
        assert_eq!(
            reopened
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
    }
}

#[test]
fn nested_event_range_rejects_bad_anchor_and_rolls_back_error_and_unwind() {
    use pdf::page::content_program::TextCodePosition;
    for preserve in [false, true] {
        let (doc, leaf, outer, _) = fixture("events");
        let (root, source) = selection(&doc);
        let original_resources = doc.get_page(0).unwrap().resource().unwrap();
        let original_leaf = doc.resolve_object(leaf).unwrap();
        let original_outer = doc.resolve_object(outer).unwrap();
        let start = TextCodePosition {
            operation: source.operation_index,
            array_item: None,
            offset: 1,
        };
        let end = TextCodePosition {
            operation: source.operation_index + 1,
            array_item: Some(2),
            offset: 1,
        };
        let replace =
            |editing: &Doc, root: &ContentProgram, source: &NestedDrawSource, text: &str| {
                if preserve {
                    editing.replace_nested_text_event_range_preserving_advance(
                        0,
                        root,
                        source,
                        start..end,
                        text,
                    )
                } else {
                    editing.replace_nested_text_event_range(0, root, source, start..end, text)
                }
            };
        let stamp = doc.stamp();
        let mut wrong = source.clone();
        wrong.operation_index += 1;
        assert!(
            matches!(replace(&doc, &root, &wrong, "X"), Err(PdfError::ObjectError(ref message)) if message == "nested range source must anchor its start event")
        );
        assert!(replace(&doc, &root, &source, "你").is_err());
        assert!(doc.is_stamp_current(&stamp));
        let result: PdfResult<()> = doc.edit_transaction(|editing| {
            replace(editing, &root, &source, "X")?;
            assert_ne!(
                editing.get_page(0)?.content_program()?.source_bytes(),
                root.source_bytes()
            );
            Err(PdfError::DocumentError("nested event rollback".into()))
        });
        assert!(
            matches!(result, Err(PdfError::DocumentError(ref message)) if message == "nested event rollback")
        );
        assert!(replace(&doc, &root, &source, "X").is_err());
        let (fresh, fresh_source) = selection(&doc);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: PdfResult<()> = doc.edit_transaction(|editing| {
                replace(editing, &fresh, &fresh_source, "X")?;
                assert_ne!(
                    editing.get_page(0)?.content_program()?.source_bytes(),
                    root.source_bytes()
                );
                panic!("nested event unwind");
            });
        }));
        assert_eq!(
            caught.unwrap_err().downcast_ref::<&str>().copied(),
            Some("nested event unwind")
        );
        for page in [0, 1] {
            let current = doc.get_page(page).unwrap().content_program().unwrap();
            assert_eq!(current.source_bytes(), root.source_bytes());
            assert_eq!(current.stream_object_id(0), root.stream_object_id(0));
        }
        assert_eq!(
            doc.get_page(0).unwrap().resource().unwrap(),
            original_resources
        );
        assert_eq!(doc.resolve_object(leaf).unwrap(), original_leaf);
        assert_eq!(doc.resolve_object(outer).unwrap(), original_outer);
        let (retry, retry_source) = selection(&doc);
        replace(&doc, &retry, &retry_source, "X").unwrap();
        let reopened =
            Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
        assert_eq!(
            reopened
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
        let chars = reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(
            chars
                .iter()
                .take(5)
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AXHCD"
        );
    }
}

#[test]
fn nested_event_range_inherits_caller_state_and_isolates_one_invocation() {
    use pdf::page::content_program::TextCodePosition;
    for preserve in [false, true] {
        for text in ["X", "", "XX XX"] {
            let (doc, leaf, outer, _) = fixture("events");
            let (root, source) = selection(&doc);
            let old_leaf = doc.resolve_object(leaf).unwrap();
            let old_outer = doc.resolve_object(outer).unwrap();
            let start = TextCodePosition {
                operation: source.operation_index,
                array_item: None,
                offset: 1,
            };
            let end = TextCodePosition {
                operation: source.operation_index + 1,
                array_item: Some(2),
                offset: 1,
            };
            if preserve {
                doc.replace_nested_text_event_range_preserving_advance(
                    0,
                    &root,
                    &source,
                    start..end,
                    text,
                )
                .unwrap();
            } else {
                doc.replace_nested_text_event_range(0, &root, &source, start..end, text)
                    .unwrap();
            }
            assert_eq!(doc.resolve_object(leaf).unwrap(), old_leaf);
            assert_eq!(doc.resolve_object(outer).unwrap(), old_outer);
            let reopened =
                Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
            let before = reopened
                .get_page(1)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let after = reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let count = text.chars().count() + 4;
            assert_eq!(before.len(), 32);
            assert_eq!(after.len(), count + 24);
            assert_eq!(
                after[..count]
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                format!("A{text}HCD")
            );
            for (old, new) in before[8..].iter().zip(&after[count..]) {
                assert_eq!(old.unicode, new.unicode);
                assert!((old.position.x() - new.position.x()).abs() < 0.002);
                assert!((old.position.y() - new.position.y()).abs() < 0.002);
            }
            if preserve {
                for (old_index, new_index) in
                    [(0, 0), (5, count - 3), (6, count - 2), (7, count - 1)]
                {
                    let old = &before[old_index];
                    let new = &after[new_index];
                    assert_eq!(old.unicode, new.unicode);
                    assert!((old.position.x() - new.position.x()).abs() < 0.002);
                    assert!((old.position.y() - new.position.y()).abs() < 0.002);
                }
            } else {
                let old = &before[6];
                let new = &after[count - 2];
                assert!(
                    (old.position.x() - new.position.x()).abs()
                        + (old.position.y() - new.position.y()).abs()
                        > 1.0
                );
            }
            assert_eq!(
                reopened
                    .get_page(1)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                root.source_bytes()
            );
        }
    }
}

fn cid_fixture(vertical: bool, event: &str) -> (Doc, ObjectId, String) {
    let (doc, leaf, _, old_body) = fixture(event);
    doc.delete_page(1).unwrap();
    let mut font = dictionary(b"<< /Type /Font /Subtype /Type0 /BaseFont /MixedNested /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /MixedNested /CIDSystemInfo << /Registry (Test) /Ordering (Test) /Supplement 0 >> /DW 1000 /W [1 [600 700 800] 32 [900]] /DW2 [880 -1000] /W2 [1 [-1000 300 880 -1200 350 880 -900 400 880] 32 [-1100 450 880]] >>] >>");
    let encoding = format!(
        "/WMode {} def 2 begincodespacerange <00> <7F> <8100> <81FF> endcodespacerange 4 begincidchar <41> 1 <8101> 2 <42> 3 <8120> 32 endcidchar",
        usize::from(vertical)
    );
    for (key, bytes) in [
        ("Encoding", encoding.as_bytes()),
        ("ToUnicode", b"2 begincodespacerange <00> <7F> <8100> <81FF> endcodespacerange 4 beginbfchar <41> <0041> <8101> <0042> <42> <0043> <8120> <0020> endbfchar".as_slice()),
    ] {
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(PdfDict::default(), bytes.to_vec()).unwrap())).unwrap();
        font.insert(key.into(), PdfObject::PdfReference(PdfReference::from_id(id)));
    }
    let id = doc.try_insert_object(PdfObject::PdfDict(font)).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Font, id)
        .unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let root = root
        .with_replaced_operation(1, format!("/{name} 10 Tf").as_bytes(), Default::default())
        .unwrap()
        .with_replaced_operation(3, b"7 Tw", Default::default())
        .unwrap();
    doc.replace_page_contents(0, root.source_bytes().to_vec())
        .unwrap();
    let target = match event {
        "Tj" => "<418101> Tj <42> Tj T* <42> Tj",
        "'" => "<418101> ' <42> Tj T* <42> Tj",
        "\"" => "7 2 <418101> \" <42> Tj T* <42> Tj",
        "item" => "[<418101>17.00<42>] TJ T* <42> Tj",
        "range" => "[<418101>17.00<81018120>% keep\n-20.50<428101>] TJ <42> Tj T* <42> Tj",
        "events" => "<418101> Tj [<81018120>30.00] TJ <810142> Tj <42> Tj T* <42> Tj",
        _ => unreachable!(),
    };
    let split = old_body.find(" Tm ").unwrap() + 4;
    let body = format!("{}{target} ET", &old_body[..split]);
    doc.replace_stream_decoded_data(leaf, body.as_bytes().to_vec())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    (doc, leaf, body)
}

#[test]
fn nested_mixed_cid_compensation_checks_w_w2_and_partial_suffix_boundaries() {
    for vertical in [false, true] {
        for event in ["Tj", "'", "\"", "item", "range"] {
            for (text, new_h, new_v) in [
                ("B", 900.0, -1000.0),
                ("", 0.0, 0.0),
                ("B ", 2000.0, -1900.0),
            ] {
                let (doc, leaf, body) = cid_fixture(vertical, event);
                let (root, source) = selection(&doc);
                let before = doc
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                let per_call = if event == "range" { 8 } else { 4 };
                assert_eq!(before.len(), per_call * 4);
                if event == "range" {
                    let stamp = doc.stamp();
                    let resources = doc.get_page(0).unwrap().resource().unwrap();
                    for range in [(0, 2)..(4, 1), (0, 1)..(4, 2), (0, 1)..(2, 1)] {
                        let error = doc
                            .replace_nested_text_array_range_preserving_advance(
                                0, &root, &source, range, text,
                            )
                            .unwrap_err();
                        assert!(
                            matches!(error, PdfError::ObjectError(ref message) if message == "text range splits a character code")
                        );
                        assert!(doc.is_stamp_current(&stamp));
                        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
                        assert_eq!(
                            doc.read_stream_data(
                                &doc.resolve_object(leaf).unwrap().into_stream().unwrap()
                            )
                            .unwrap()
                            .as_ref(),
                            body.as_bytes()
                        );
                    }
                }
                replace(&doc, &root, &source, event, text).unwrap();
                let reopened =
                    Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                        .unwrap();
                for current in [&doc, &reopened] {
                    assert_eq!(
                        current
                            .read_stream_data(
                                &current.resolve_object(leaf).unwrap().into_stream().unwrap()
                            )
                            .unwrap()
                            .as_ref(),
                        body.as_bytes()
                    );
                    let edited = current
                        .get_page(0)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    let shared = current
                        .get_page(1)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    let mode = if vertical {
                        pdf::font::WritingMode::Vertical
                    } else {
                        pdf::font::WritingMode::Horizontal
                    };
                    assert!(
                        before
                            .iter()
                            .chain(&edited)
                            .chain(&shared)
                            .all(|ch| ch.writing_mode == Some(mode))
                    );
                    let changed_len = text.chars().count() + if event == "range" { 4 } else { 2 };
                    assert_eq!(edited.len(), changed_len + per_call * 3);
                    assert_eq!(
                        edited[..changed_len]
                            .iter()
                            .filter_map(|ch| ch.unicode.as_deref())
                            .collect::<String>(),
                        if event == "range" {
                            format!("A{text}BCC")
                        } else {
                            format!("{text}CC")
                        }
                    );
                    let retained = if event == "range" { 3 } else { 2 };
                    for (expected, actual) in before[per_call - retained..per_call]
                        .iter()
                        .zip(&edited[changed_len - retained..changed_len])
                    {
                        assert_eq!(expected.unicode, actual.unicode);
                        assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                        assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                    }
                    for (expected, actual) in before[per_call..].iter().zip(&edited[changed_len..])
                    {
                        assert_eq!(expected.unicode, actual.unicode);
                        assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                        assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                    }
                    assert_eq!(shared.len(), before.len());
                    for (expected, actual) in before.iter().zip(shared) {
                        assert_eq!(expected.unicode, actual.unicode);
                        assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                        assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                    }
                    let fresh = current.get_page(0).unwrap().content_program().unwrap();
                    let source = edited[0].nested_source.as_ref().unwrap();
                    let program = current
                        .get_page(0)
                        .unwrap()
                        .nested_content_program(&fresh, source)
                        .unwrap();
                    let array = program
                        .operations()
                        .iter()
                        .find(|op| op.operator.name() == "TJ")
                        .unwrap()
                        .operator
                        .get_as_array(0)
                        .unwrap();
                    let expected = if vertical {
                        new_v + if event == "range" { 1000.0 } else { 1800.0 }
                    } else {
                        new_h - if event == "range" { 900.0 } else { 1700.0 }
                    };
                    assert_eq!(array.get(1).unwrap().as_f32(), Some(expected));
                    if event == "range" {
                        assert_eq!(
                            array.get(5).unwrap().as_f32(),
                            Some(if vertical { 1900.0 } else { -2000.0 })
                        );
                        assert_eq!(
                            array.get(9).unwrap().as_f32(),
                            Some(if vertical { 700.0 } else { -1000.0 })
                        );
                        assert_eq!(
                            array.get(10).unwrap().as_string().unwrap().bytes(),
                            b"\x81\x01"
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn cross_invocation_mixed_cid_span_checks_boundaries_and_signed_advance() {
    use pdf::page::source_provenance::NestedTextCodePosition;
    for vertical in [false, true] {
        for preserve in [false, true] {
            for text in ["B", "", "B "] {
                let (doc, leaf, body) = cid_fixture(vertical, "events");
                let root = doc.get_page(0).unwrap().content_program().unwrap();
                let before = doc
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                assert_eq!(before.len(), 32);
                let original: String = before
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect();
                let start = NestedTextCodePosition {
                    source: before[0].nested_source.as_ref().unwrap(),
                    array_item: None,
                    offset: 1,
                };
                let end = NestedTextCodePosition {
                    source: before[28].nested_source.as_ref().unwrap(),
                    array_item: None,
                    offset: 2,
                };
                let replace = |start, end| {
                    if preserve {
                        doc.replace_nested_text_span_preserving_advance(0, &root, start..end, text)
                    } else {
                        doc.replace_nested_text_span(0, &root, start..end, text)
                    }
                };
                let stamp = doc.stamp();
                let saved = doc.save_to_bytes().unwrap();
                for (first, last) in [
                    (NestedTextCodePosition { offset: 2, ..start }, end),
                    (start, NestedTextCodePosition { offset: 1, ..end }),
                ] {
                    assert!(
                        matches!(replace(first, last), Err(PdfError::ObjectError(ref message)) if message == "text range splits a character code")
                    );
                    assert!(doc.is_stamp_current(&stamp));
                    assert_eq!(doc.save_to_bytes().unwrap(), saved);
                }
                replace(start, end).unwrap();
                let reopened =
                    Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                        .unwrap();
                assert_eq!(
                    reopened
                        .read_stream_data(
                            &reopened
                                .resolve_object(leaf)
                                .unwrap()
                                .into_stream()
                                .unwrap()
                        )
                        .unwrap()
                        .as_ref(),
                    body.as_bytes()
                );
                let after = reopened
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                assert_eq!(
                    after
                        .iter()
                        .filter_map(|ch| ch.unicode.as_deref())
                        .collect::<String>(),
                    format!("A{text}CCC")
                );
                assert_eq!(
                    reopened
                        .get_page(1)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap()
                        .iter()
                        .filter_map(|ch| ch.unicode.as_deref())
                        .collect::<String>(),
                    original
                );
                if preserve {
                    let suffix = 1 + text.chars().count();
                    for (old, new) in [(0, 0), (29, suffix), (30, suffix + 1), (31, suffix + 2)] {
                        assert!((before[old].position.x() - after[new].position.x()).abs() < 0.002);
                        assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
                    }
                }
            }
        }
    }
}

#[test]
fn nested_event_range_mixed_cid_inheritance_checks_signed_metrics_and_split_endpoints() {
    use pdf::page::content_program::TextCodePosition;
    for vertical in [false, true] {
        for preserve in [false, true] {
            for text in ["B", "", "B "] {
                let (doc, leaf, body) = cid_fixture(vertical, "events");
                let (root, source) = selection(&doc);
                let start = TextCodePosition {
                    operation: source.operation_index,
                    array_item: None,
                    offset: 1,
                };
                let end = TextCodePosition {
                    operation: source.operation_index + 2,
                    array_item: None,
                    offset: 2,
                };
                let replace = |start, end| {
                    if preserve {
                        doc.replace_nested_text_event_range_preserving_advance(
                            0,
                            &root,
                            &source,
                            start..end,
                            text,
                        )
                    } else {
                        doc.replace_nested_text_event_range(0, &root, &source, start..end, text)
                    }
                };
                let stamp = doc.stamp();
                let resources = doc.get_page(0).unwrap().resource().unwrap();
                for (start, end) in [
                    (TextCodePosition { offset: 2, ..start }, end),
                    (start, TextCodePosition { offset: 1, ..end }),
                ] {
                    let error = replace(start, end).unwrap_err();
                    assert!(
                        matches!(error, PdfError::ObjectError(ref message) if message == "text range splits a character code")
                    );
                    assert!(doc.is_stamp_current(&stamp));
                    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
                }
                replace(start, end).unwrap();
                let reopened =
                    Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                        .unwrap();
                assert_eq!(
                    reopened
                        .read_stream_data(
                            &reopened
                                .resolve_object(leaf)
                                .unwrap()
                                .into_stream()
                                .unwrap()
                        )
                        .unwrap()
                        .as_ref(),
                    body.as_bytes()
                );
                let before = reopened
                    .get_page(1)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                let after = reopened
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                let count = text.chars().count() + 4;
                assert_eq!(before.len(), 32);
                assert_eq!(after.len(), count + 24);
                assert_eq!(
                    after[..count]
                        .iter()
                        .filter_map(|ch| ch.unicode.as_deref())
                        .collect::<String>(),
                    format!("A{text}CCC")
                );
                for (old, new) in before[8..].iter().zip(&after[count..]) {
                    assert_eq!(old.unicode, new.unicode);
                    assert!((old.position.x() - new.position.x()).abs() < 0.002);
                    assert!((old.position.y() - new.position.y()).abs() < 0.002);
                }
                if preserve {
                    for (old_index, new_index) in
                        [(0, 0), (5, count - 3), (6, count - 2), (7, count - 1)]
                    {
                        assert!(
                            (before[old_index].position.x() - after[new_index].position.x()).abs()
                                < 0.002
                        );
                        assert!(
                            (before[old_index].position.y() - after[new_index].position.y()).abs()
                                < 0.002
                        );
                    }
                }
                let fresh = reopened.get_page(0).unwrap().content_program().unwrap();
                let program = reopened
                    .get_page(0)
                    .unwrap()
                    .nested_content_program(&fresh, after[0].nested_source.as_ref().unwrap())
                    .unwrap();
                let arrays: Vec<_> = program
                    .operations()
                    .iter()
                    .filter(|op| op.operator.name() == "TJ")
                    .map(|op| op.operator.get_as_array(0).unwrap())
                    .collect();
                if preserve {
                    assert_eq!(arrays.len(), 3);
                    let first = match (vertical, text) {
                        (_, "B") => 0.0,
                        (false, "") => -900.0,
                        (true, "") => 1000.0,
                        (false, "B ") => 1100.0,
                        (true, "B ") => -900.0,
                        _ => unreachable!(),
                    };
                    for (array, expected) in arrays.iter().zip([
                        first,
                        if vertical { 1900.0 } else { -2000.0 },
                        if vertical { 1000.0 } else { -900.0 },
                    ]) {
                        assert_eq!(array.get(1).unwrap().as_number().unwrap().value(), expected);
                    }
                } else {
                    assert_eq!(arrays.len(), 1);
                    assert_eq!(arrays[0].len(), 2);
                    assert_eq!(arrays[0].get(1).unwrap().as_number().unwrap().value(), 30.0);
                }
                assert_eq!(
                    reopened
                        .get_page(1)
                        .unwrap()
                        .content_program()
                        .unwrap()
                        .source_bytes(),
                    root.source_bytes()
                );
            }
        }
    }
}

fn replace(
    doc: &Doc,
    root: &ContentProgram,
    source: &NestedDrawSource,
    event: &str,
    text: &str,
) -> PdfResult<()> {
    match event {
        "item" => doc.replace_nested_text_array_item_preserving_advance(0, root, source, 0, text),
        "range" => doc.replace_nested_text_array_range_preserving_advance(
            0,
            root,
            source,
            (0, 1)..(4, 1),
            text,
        ),
        _ => doc.replace_nested_text_preserving_advance(0, root, source, text),
    }
}

#[test]
fn nested_compensation_preserves_inherited_layout_and_other_invocations() {
    for event in ["Tj", "'", "\"", "item", "range"] {
        for text in ["X", "XYZ", ""] {
            let (doc, leaf, outer, body) = fixture(event);
            let (root, source) = selection(&doc);
            let before = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let per_call = if event == "range" { 8 } else { 4 };
            assert_eq!(before.len(), per_call * 4);
            let original_outer = doc
                .read_stream_data(&doc.resolve_object(outer).unwrap().into_stream().unwrap())
                .unwrap()
                .as_ref()
                .to_vec();
            replace(&doc, &root, &source, event, text).unwrap();
            let reopened =
                Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
            for current in [&doc, &reopened] {
                assert_eq!(
                    current
                        .read_stream_data(
                            &current.resolve_object(leaf).unwrap().into_stream().unwrap()
                        )
                        .unwrap()
                        .as_ref(),
                    body.as_bytes()
                );
                assert_eq!(
                    current
                        .read_stream_data(
                            &current
                                .resolve_object(outer)
                                .unwrap()
                                .into_stream()
                                .unwrap()
                        )
                        .unwrap()
                        .as_ref(),
                    original_outer
                );
                let shared = current
                    .get_page(1)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                let edited = current
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                let changed_len = text.len() + if event == "range" { 4 } else { 2 };
                assert_eq!(edited.len(), changed_len + per_call * 3);
                let changed_text = edited[..changed_len]
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>();
                assert_eq!(
                    changed_text,
                    if event == "range" {
                        format!("A{text}HCD")
                    } else {
                        format!("{text}CD")
                    }
                );
                // Retained end suffix H (range), C within TJ (item), following
                // C event and next-line D all keep their independent positions.
                let retained = if event == "range" { 3 } else { 2 };
                for (expected, actual) in before[per_call - retained..per_call]
                    .iter()
                    .zip(&edited[changed_len - retained..changed_len])
                {
                    assert_eq!(expected.unicode, actual.unicode);
                    assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                    assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                }
                for (expected, actual) in before[per_call..].iter().zip(&edited[changed_len..]) {
                    assert_eq!(expected.unicode, actual.unicode);
                    assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                    assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                }
                assert_eq!(shared.len(), before.len());
                for (expected, actual) in before.iter().zip(shared) {
                    assert_eq!(expected.unicode, actual.unicode);
                    assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                    assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                }
                let fresh_root = current.get_page(0).unwrap().content_program().unwrap();
                let fresh_source = edited[0].nested_source.as_ref().unwrap();
                let changed_leaf = current
                    .get_page(0)
                    .unwrap()
                    .nested_content_program(&fresh_root, fresh_source)
                    .unwrap();
                let array = changed_leaf
                    .operations()
                    .iter()
                    .find(|op| op.operator.name() == "TJ")
                    .unwrap()
                    .operator
                    .get_as_array(0)
                    .unwrap();
                if event == "range" {
                    assert_eq!(
                        array.get(1).unwrap().as_f32(),
                        Some((text.len() as f32 - 1.0) * 800.0)
                    );
                    assert_eq!(array.get(5).unwrap().as_f32(), Some(-1600.0));
                    assert_eq!(array.get(9).unwrap().as_f32(), Some(-800.0));
                } else {
                    assert_eq!(
                        array.get(1).unwrap().as_f32(),
                        Some((text.len() as f32 - 2.0) * 800.0)
                    );
                }
                let lexical = std::str::from_utf8(changed_leaf.source_bytes()).unwrap();
                if matches!(event, "item" | "range") {
                    assert!(lexical.contains("17.00"));
                }
                if event == "range" {
                    assert!(lexical.contains("% keep\n-20.50"));
                }
            }
        }
    }
}

#[test]
fn nested_compensation_errors_outer_rollback_unwind_and_retry() {
    for event in ["Tj", "'", "\"", "item", "range"] {
        let (doc, leaf, outer, body) = fixture(event);
        let (root, source) = selection(&doc);
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let original_outer = doc.resolve_object(outer).unwrap();
        let stamp = doc.stamp();
        assert!(replace(&doc, &root, &source, event, "中文").is_err());
        assert!(doc.is_stamp_current(&stamp));
        let result: PdfResult<()> = doc.edit_transaction(|editing| {
            replace(editing, &root, &source, event, "X")?;
            Err(PdfError::DocumentError(
                "nested compensation rollback".into(),
            ))
        });
        assert!(
            matches!(result, Err(PdfError::DocumentError(ref message)) if message == "nested compensation rollback")
        );
        for page in [0, 1] {
            let now = doc.get_page(page).unwrap().content_program().unwrap();
            assert_eq!(now.source_bytes(), root.source_bytes());
            assert_eq!(now.stream_object_id(0), root.stream_object_id(0));
        }
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        assert_eq!(doc.resolve_object(outer).unwrap(), original_outer);
        assert_eq!(
            doc.read_stream_data(&doc.resolve_object(leaf).unwrap().into_stream().unwrap())
                .unwrap()
                .as_ref(),
            body.as_bytes()
        );
        assert!(replace(&doc, &root, &source, event, "X").is_err());
        let (fresh, source) = selection(&doc);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: PdfResult<()> = doc.edit_transaction(|editing| {
                replace(editing, &fresh, &source, event, "X")?;
                assert_ne!(
                    editing.get_page(0)?.content_program()?.source_bytes(),
                    fresh.source_bytes()
                );
                panic!("nested compensation commit unwind");
            });
        }));
        assert_eq!(
            caught.unwrap_err().downcast_ref::<&str>().copied(),
            Some("nested compensation commit unwind")
        );
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        for page in [0, 1] {
            assert_eq!(
                doc.get_page(page)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                root.source_bytes()
            );
        }
        let (fresh, source) = selection(&doc);
        replace(&doc, &fresh, &source, event, "X").unwrap();
        Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    }
}

#[test]
fn nested_compensation_public_quota_rejection_keeps_resources_and_retry() {
    let options = pdf::parser::options::ParseOptions {
        limits: pdf::parser::options::Limits {
            max_operation_bytes: 512 * 1024,
            ..Default::default()
        },
        ..Default::default()
    };
    for event in ["Tj", "'", "\"", "item", "range"] {
        let (doc, leaf, outer, body) = fixture_with_options(event, options);
        let (root, source) = selection(&doc);
        let stamp = doc.stamp();
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let original_outer = doc.resolve_object(outer).unwrap();
        let too_large = "X".repeat(512 * 1024);
        assert!(matches!(
            replace(&doc, &root, &source, event, &too_large),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(doc.is_stamp_current(&stamp));
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        assert_eq!(doc.resolve_object(outer).unwrap(), original_outer);
        assert_eq!(
            doc.read_stream_data(&doc.resolve_object(leaf).unwrap().into_stream().unwrap())
                .unwrap()
                .as_ref(),
            body.as_bytes()
        );
        for page in [0, 1] {
            assert_eq!(
                doc.get_page(page)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                root.source_bytes()
            );
        }
        replace(&doc, &root, &source, event, "X").unwrap();
        Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    }
}

#[test]
fn nested_event_range_quota_rejection_preserves_chain_and_original_source_retry() {
    use pdf::{
        page::content_program::TextCodePosition,
        parser::options::{Limits, ParseOptions},
    };
    for preserve in [false, true] {
        let (doc, leaf, outer, _) = fixture_with_options(
            "events",
            ParseOptions {
                limits: Limits {
                    max_operation_bytes: 512 * 1024,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let (root, source) = selection(&doc);
        let leaf_before = doc.resolve_object(leaf).unwrap();
        let outer_before = doc.resolve_object(outer).unwrap();
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let stamp = doc.stamp();
        let start = TextCodePosition {
            operation: source.operation_index,
            array_item: None,
            offset: 1,
        };
        let end = TextCodePosition {
            operation: source.operation_index + 1,
            array_item: Some(2),
            offset: 1,
        };
        let replace = |text: &str| {
            if preserve {
                doc.replace_nested_text_event_range_preserving_advance(
                    0,
                    &root,
                    &source,
                    start..end,
                    text,
                )
            } else {
                doc.replace_nested_text_event_range(0, &root, &source, start..end, text)
            }
        };
        let large = "X".repeat(512 * 1024);
        for _ in 0..2 {
            assert!(matches!(replace(&large), Err(PdfError::ResourceLimit(_))));
            assert!(doc.is_stamp_current(&stamp));
            assert_eq!(doc.resolve_object(leaf).unwrap(), leaf_before);
            assert_eq!(doc.resolve_object(outer).unwrap(), outer_before);
            assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
            for page in [0, 1] {
                let current = doc.get_page(page).unwrap().content_program().unwrap();
                assert_eq!(current.source_bytes(), root.source_bytes());
                assert_eq!(current.stream_object_id(0), root.stream_object_id(0));
            }
        }
        replace("X").unwrap();
        let reopened =
            Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
        assert_eq!(
            reopened
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
        let chars = reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(
            chars
                .iter()
                .take(5)
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AXHCD"
        );
    }
}
