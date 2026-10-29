use std::io::Cursor;

use pdf::{
    document::{
        PdfDocument,
        resource_manager::{ResourceKind, StandardFont},
    },
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream},
    page::source_provenance::NestedTextCodePosition,
    parser::syntax::{ParseMode, SyntaxParser},
};

type Doc = PdfDocument<Cursor<Vec<u8>>>;

fn form_dictionary() -> PdfDict {
    SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            b"<< /Type /XObject /Subtype /Form /BBox [0 0 500 500] >>".as_slice(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap()
}

#[test]
fn page_text_endpoint_is_explicit_and_does_not_become_a_form_leaf() {
    use pdf::page::source_provenance::NestedDrawSource;
    for preserve in [false, true] {
        let doc = Doc::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{font} 12 Tf 20 50 Td (ABC) Tj ET").into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let report = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap();
        assert!(report.characters[0].nested_source.is_none());
        let plain = report.characters[0].source.as_ref().unwrap();
        let source = NestedDrawSource::from_page_text(plain);
        assert!(source.invocations.is_empty());
        assert!(source.geometry_operation_indices.is_empty());
        assert_eq!(source.operation_index, plain.operation_index);
        assert_eq!(source.operator_byte_offset, plain.operator_byte_offset);
        let saved = doc.save_to_bytes().unwrap();
        assert!(
            doc.get_page(0)
                .unwrap()
                .nested_content_program(&root, &source)
                .is_err()
        );
        assert_eq!(doc.save_to_bytes().unwrap(), saved);
        let range = NestedTextCodePosition {
            source: &source,
            array_item: None,
            offset: 1,
        }..NestedTextCodePosition {
            source: &source,
            array_item: None,
            offset: 2,
        };
        if preserve {
            doc.replace_nested_text_span_preserving_advance(0, &root, range, "X")
                .unwrap();
        } else {
            doc.replace_nested_text_span(0, &root, range, "X").unwrap();
        }
        let reopened =
            Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
        for (page, expected) in [(0, "AXC"), (1, "ABC")] {
            assert_eq!(
                reopened
                    .get_page(page)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap()
                    .iter()
                    .filter_map(|character| character.unicode.as_deref())
                    .collect::<String>(),
                expected
            );
        }
    }
}

#[test]
fn same_event_span_insertion_and_deletion_keep_other_invocations() {
    for preserve in [false, true] {
        for (start_offset, end_offset, replacement, expected, suffix) in [
            (0, 0, "X", "XABAB", Some((0, 1))),
            (1, 1, "X", "AXBAB", Some((1, 2))),
            (0, 2, "", "AB", None),
        ] {
            let doc = Doc::new().unwrap();
            doc.add_page(0).unwrap();
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Courier)
                .unwrap();
            let mut dictionary = form_dictionary();
            dictionary.insert(
                "Resources".into(),
                PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
            );
            let form = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(
                        dictionary,
                        format!("BT /{font} 12 Tf 1 0 0 1 70 200 Tm (AB) Tj ET").into_bytes(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            let original_form = doc.resolve_object(form).unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, form)
                .unwrap();
            doc.replace_page_contents(
                0,
                format!("q /{name} Do Q q 1 0 0 1 200 0 cm /{name} Do Q").into_bytes(),
            )
            .unwrap();
            doc.copy_page(0, 1).unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let before = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let source = before[0].nested_source.as_ref().unwrap();
            let start = NestedTextCodePosition {
                source,
                array_item: None,
                offset: start_offset,
            };
            let end = NestedTextCodePosition {
                source,
                array_item: None,
                offset: end_offset,
            };
            if preserve {
                doc.replace_nested_text_span_preserving_advance(0, &root, start..end, replacement)
                    .unwrap();
            } else {
                doc.replace_nested_text_span(0, &root, start..end, replacement)
                    .unwrap();
            }
            assert_eq!(doc.resolve_object(form).unwrap(), original_form);
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
                expected
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
                "ABAB"
            );
            for offset in 0..2 {
                let old = &before[2 + offset];
                let new = &after[after.len() - 2 + offset];
                assert!((old.position.x() - new.position.x()).abs() < 0.002);
                assert!((old.position.y() - new.position.y()).abs() < 0.002);
            }
            if preserve && let Some((old, new)) = suffix {
                assert!((before[old].position.x() - after[new].position.x()).abs() < 0.002);
                assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
            }
        }
    }
}

#[test]
fn same_invocation_span_includes_intervening_child_text() {
    for preserve in [false, true] {
        let operation_limit = 2 * 1024 * 1024;
        let doc = Doc::new_with_options(pdf::parser::options::ParseOptions {
            limits: pdf::parser::options::Limits {
                max_operation_bytes: operation_limit,
                // Avoid mistaking a character-quota rejection for memory use.
                max_text_chars: 2 * operation_limit,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        let scope = doc.get_page(0).unwrap().resource().unwrap();
        let mut child_dict = form_dictionary();
        child_dict.insert("Resources".into(), PdfObject::PdfDict(scope.clone()));
        let child = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    child_dict,
                    format!("BT /{font} 12 Tf 1 0 0 1 70 150 Tm (K) Tj ET").into_bytes(),
                )
                .unwrap(),
            ))
            .unwrap();
        let child_before = doc.resolve_object(child).unwrap();
        let mut parent_scope = scope;
        let mut xobjects = PdfDict::default();
        xobjects.insert(
            "Child".into(),
            PdfObject::PdfReference(PdfReference::from_id(child)),
        );
        parent_scope.insert("XObject".into(), PdfObject::PdfDict(xobjects));
        let mut parent_dict = form_dictionary();
        parent_dict.insert("Resources".into(), PdfObject::PdfDict(parent_scope));
        let parent = doc.try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(parent_dict,
                format!("BT /{font} 12 Tf 1 0 0 1 70 200 Tm (A) Tj ET q /Child Do Q BT /{font} 12 Tf 1 0 0 1 70 100 Tm (BC) Tj ET").into_bytes()).unwrap(),
        )).unwrap();
        let parent_before = doc.resolve_object(parent).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, parent)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("q /{name} Do Q q 1 0 0 1 200 0 cm /{name} Do Q").into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let before = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(
            before
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AKBCAKBC"
        );
        let start = NestedTextCodePosition {
            source: before[0].nested_source.as_ref().unwrap(),
            array_item: None,
            offset: 0,
        };
        let end = NestedTextCodePosition {
            source: before[2].nested_source.as_ref().unwrap(),
            array_item: None,
            offset: 1,
        };
        assert_eq!(start.source.invocations, end.source.invocations);
        assert_ne!(
            start.source.invocations,
            before[1].nested_source.as_ref().unwrap().invocations
        );
        let saved = doc.save_to_bytes().unwrap();
        let stamp = doc.stamp();
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        for _ in 0..2 {
            let too_large = "X".repeat(operation_limit);
            let rejected = if preserve {
                doc.replace_nested_text_span_preserving_advance(0, &root, start..end, &too_large)
            } else {
                doc.replace_nested_text_span(0, &root, start..end, &too_large)
            };
            assert!(matches!(
                rejected,
                Err(pdf::error::PdfError::ResourceLimit(ref message)) if message == "operation live bytes"
            ));
            assert!(doc.is_stamp_current(&stamp));
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
            assert_eq!(doc.resolve_object(child).unwrap(), child_before);
            assert_eq!(doc.resolve_object(parent).unwrap(), parent_before);
        }
        // Exercise rollback after the child and parent edits have been published.
        for unwind in [false, true] {
            let rollback = Doc::from_bytes_with_mode(saved.clone(), ParseMode::Strict).unwrap();
            let baseline = rollback.save_to_bytes().unwrap();
            let edit = |target: &Doc| -> pdf::error::PdfResult<()> {
                let snapshot = target.get_page(0)?.content_program()?;
                let chars = target.get_page(0)?.extract_characters_paint_order()?;
                let first = NestedTextCodePosition {
                    source: chars[0].nested_source.as_ref().unwrap(),
                    array_item: None,
                    offset: 0,
                };
                let last = NestedTextCodePosition {
                    source: chars[2].nested_source.as_ref().unwrap(),
                    array_item: None,
                    offset: 1,
                };
                if preserve {
                    target.replace_nested_text_span_preserving_advance(
                        0,
                        &snapshot,
                        first..last,
                        "X",
                    )
                } else {
                    target.replace_nested_text_span(0, &snapshot, first..last, "X")
                }
            };
            if unwind {
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _: pdf::error::PdfResult<()> = rollback.edit_transaction(|target| {
                        edit(target)?;
                        panic!("same invocation outer unwind");
                    });
                }));
                assert_eq!(
                    caught.unwrap_err().downcast_ref::<&str>().copied(),
                    Some("same invocation outer unwind")
                );
            } else {
                let rejected: pdf::error::PdfResult<()> = rollback.edit_transaction(|target| {
                    edit(target)?;
                    Err(pdf::error::PdfError::DocumentError(
                        "same invocation outer error".into(),
                    ))
                });
                assert!(
                    matches!(rejected, Err(pdf::error::PdfError::DocumentError(ref message)) if message == "same invocation outer error")
                );
            }
            assert_eq!(rollback.save_to_bytes().unwrap(), baseline);
            let actual = rollback.resolve_object(child).unwrap();
            assert_eq!(
                actual.as_stream().unwrap().dict(),
                child_before.as_stream().unwrap().dict()
            );
            assert_eq!(
                rollback
                    .read_stream_data(actual.as_stream().unwrap())
                    .unwrap()
                    .as_ref(),
                child_before.as_stream().unwrap().raw_data().unwrap()
            );
            let actual = rollback.resolve_object(parent).unwrap();
            assert_eq!(
                actual.as_stream().unwrap().dict(),
                parent_before.as_stream().unwrap().dict()
            );
            assert_eq!(
                rollback
                    .read_stream_data(actual.as_stream().unwrap())
                    .unwrap()
                    .as_ref(),
                parent_before.as_stream().unwrap().raw_data().unwrap()
            );
            // Rollback invalidates old snapshots; retry from fresh provenance.
            edit(&rollback).unwrap();
            let retried =
                Doc::from_bytes_with_mode(rollback.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(
                retried
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap()
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "XCAKBC"
            );
            assert_eq!(
                retried
                    .get_page(1)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap()
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "AKBCAKBC"
            );
        }
        // Explicit event ranges retain their single-program contract.
        let legacy = Doc::from_bytes_with_mode(saved.clone(), ParseMode::Strict).unwrap();
        let legacy_root = legacy.get_page(0).unwrap().content_program().unwrap();
        let legacy_chars = legacy
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        let legacy_source = legacy_chars[0].nested_source.as_ref().unwrap();
        let position = |operation, offset| pdf::page::content_program::TextCodePosition {
            operation,
            array_item: None,
            offset,
        };
        let legacy_range = position(legacy_source.operation_index, 0)
            ..position(
                legacy_chars[2]
                    .nested_source
                    .as_ref()
                    .unwrap()
                    .operation_index,
                1,
            );
        if preserve {
            legacy
                .replace_nested_text_event_range_preserving_advance(
                    0,
                    &legacy_root,
                    legacy_source,
                    legacy_range,
                    "X",
                )
                .unwrap();
        } else {
            legacy
                .replace_nested_text_event_range(0, &legacy_root, legacy_source, legacy_range, "X")
                .unwrap();
        }
        let legacy_reopened =
            Doc::from_bytes_with_mode(legacy.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
        assert_eq!(
            legacy_reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap()
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "XKCAKBC"
        );
        let invalid_end = NestedTextCodePosition { offset: 3, ..end };
        for (first, last, text) in [
            (end, start, "X"),
            (start, invalid_end, "X"),
            (start, end, "😀"),
        ] {
            let rejected = if preserve {
                doc.replace_nested_text_span_preserving_advance(0, &root, first..last, text)
            } else {
                doc.replace_nested_text_span(0, &root, first..last, text)
            };
            assert!(rejected.is_err());
            assert!(doc.is_stamp_current(&stamp));
            assert_eq!(doc.save_to_bytes().unwrap(), saved);
            assert_eq!(doc.resolve_object(child).unwrap(), child_before);
            assert_eq!(doc.resolve_object(parent).unwrap(), parent_before);
        }
        // The same original root and both original endpoints remain usable.
        if preserve {
            doc.replace_nested_text_span_preserving_advance(0, &root, start..end, "X")
                .unwrap();
        } else {
            doc.replace_nested_text_span(0, &root, start..end, "X")
                .unwrap();
        }
        assert_eq!(doc.resolve_object(child).unwrap(), child_before);
        assert_eq!(doc.resolve_object(parent).unwrap(), parent_before);
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
            "XCAKBC"
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
            "AKBCAKBC"
        );
        if preserve {
            assert!((before[3].position.x() - after[1].position.x()).abs() < 0.002);
            assert!((before[3].position.y() - after[1].position.y()).abs() < 0.002);
        }
        for (old, new) in [(4, 2), (5, 3), (6, 4), (7, 5)] {
            assert!((before[old].position.x() - after[new].position.x()).abs() < 0.002);
            assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
        }
    }
}
