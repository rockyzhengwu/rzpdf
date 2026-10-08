use pdf::{
    document::{PdfDocument, resource_manager::StandardFont},
    page::content_program::TextCodePosition,
    parser::syntax::ParseMode,
};

#[test]
fn ordinary_cross_event_replacement_preserves_quotes_and_tj_item_count() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf 14 TL (AB) ' 3.00 2.00 (CD) \" [(EF)20] TJ (Z) Tj ET")
            .into_bytes(),
    )
    .unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let start = TextCodePosition {
        operation: 3,
        array_item: None,
        offset: 1,
    };
    let end = TextCodePosition {
        operation: 5,
        array_item: Some(0),
        offset: 1,
    };
    doc.replace_page_text_event_range(0, &snapshot, start..end, "X")
        .unwrap();
    let edited = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(edited.operations().len(), snapshot.operations().len());
    assert_eq!(edited.operations()[3].operator.name(), "'");
    assert_eq!(edited.operations()[4].operator.name(), "\"");
    assert_eq!(
        edited.operations()[5]
            .operator
            .get_as_array(0)
            .unwrap()
            .len(),
        2
    );
    assert!(
        std::str::from_utf8(edited.source_bytes())
            .unwrap()
            .contains("3.00 2.00")
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let chars = reopened
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(
        chars
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "AXFZ"
    );
}

#[test]
fn ordinary_cross_event_replacement_moves_suffix_and_accepts_zero_font_size() {
    for size in [0, 12] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{font} {size} Tf 1 0 0 1 100 100 Tm (AB) Tj [(CD) 20 (EF)] TJ (Z) Tj ET")
                .into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
        let start = TextCodePosition {
            operation: 3,
            array_item: None,
            offset: 1,
        };
        let end = TextCodePosition {
            operation: 4,
            array_item: Some(2),
            offset: 1,
        };
        doc.replace_page_text_event_range(1, &snapshot, start..end, "X")
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let before = reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        let after = reopened
            .get_page(1)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(
            after
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AXFZ"
        );
        for letter in ["F", "Z"] {
            let old = before
                .iter()
                .find(|ch| ch.unicode.as_deref() == Some(letter))
                .unwrap();
            let new = after
                .iter()
                .find(|ch| ch.unicode.as_deref() == Some(letter))
                .unwrap();
            assert!((new.position.x() - old.position.x() + 1.8 * size as f32).abs() < 0.002);
            assert!((new.position.y() - old.position.y()).abs() < 0.002);
        }
        assert_eq!(
            reopened
                .get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            snapshot.source_bytes()
        );
    }
}

#[test]
fn cross_event_memory_rejection_keeps_resources_and_fresh_snapshot_retry() {
    use pdf::{
        error::PdfError,
        parser::options::{Limits, ParseOptions},
    };
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_operation_bytes: 512 * 1024,
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
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf (AB) Tj [(CD) 20 (EF)] TJ (Z) Tj ET").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 1).unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let stamp = doc.stamp();
    let start = TextCodePosition {
        operation: 2,
        array_item: None,
        offset: 1,
    };
    let end = TextCodePosition {
        operation: 3,
        array_item: Some(2),
        offset: 1,
    };
    let large = "X".repeat(512 * 1024);
    for _ in 0..2 {
        assert!(matches!(
            doc.replace_page_text_event_range_preserving_advance(0, &snapshot, start..end, &large),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(doc.is_stamp_current(&stamp));
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        for page in [0, 1] {
            let current = doc.get_page(page).unwrap().content_program().unwrap();
            assert_eq!(current.source_bytes(), snapshot.source_bytes());
            assert_eq!(current.stream_object_id(0), snapshot.stream_object_id(0));
        }
    }
    // Rejected edits leave the original snapshot usable for a smaller retry.
    doc.replace_page_text_event_range_preserving_advance(0, &snapshot, start..end, "X")
        .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .get_page(1)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        snapshot.source_bytes()
    );
    let chars = reopened
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(
        chars
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "AXFZ"
    );
}

#[test]
fn cross_event_mixed_cid_ranges_use_each_signed_advance_and_reject_split_codes() {
    use pdf::{
        document::resource_manager::ResourceKind,
        error::PdfError,
        io::stream_reader::StreamReader,
        objects::{
            PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream,
        },
        parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;
    for vertical in [false, true] {
        for text in ["B", "", "B "] {
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            let mut dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(
                b"<< /Type /Font /Subtype /Type0 /BaseFont /MixedAdvance /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /MixedAdvance /CIDSystemInfo << /Registry (Test) /Ordering (Test) /Supplement 0 >> /DW 1000 /W [1 [600 700 800] 32 [900]] /DW2 [880 -1000] /W2 [1 [-1000 300 880 -1200 350 880 -900 400 880] 32 [-1100 450 880]] >>] >>".to_vec()
            )).unwrap()).get_object().unwrap().into_dict().unwrap();
            let encoding = format!(
                "/WMode {} def 2 begincodespacerange <00> <7F> <8100> <81FF> endcodespacerange 4 begincidchar <41> 1 <8101> 2 <42> 3 <8120> 32 endcidchar",
                usize::from(vertical)
            );
            for (key, bytes) in [
                ("Encoding", encoding.as_bytes()),
                ("ToUnicode", b"2 begincodespacerange <00> <7F> <8100> <81FF> endcodespacerange 4 beginbfchar <41> <0041> <8101> <0042> <42> <0043> <8120> <0020> endbfchar".as_slice()),
            ] {
                let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(PdfDict::default(), bytes.to_vec()).unwrap())).unwrap();
                dict.insert(key.into(), PdfObject::PdfReference(PdfReference::from_id(id)));
            }
            let id = doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
            let font = doc
                .resources()
                .register_existing(0, ResourceKind::Font, id)
                .unwrap();
            doc.replace_page_contents(0, format!(
                "q 1 .2 -.3 1 40 30 cm BT /{font} 10 Tf 2 Tc 7 Tw -75 Tz 14 TL 1 .3 .2 1 100 100 Tm <418101> Tj [<81018120>30.00] TJ <810142> Tj <41> Tj T* <41> Tj ET Q"
            ).into_bytes()).unwrap();
            doc.copy_page(0, 1).unwrap();
            let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
            let shows: Vec<_> = snapshot
                .operations()
                .iter()
                .enumerate()
                .filter(|(_, op)| matches!(op.operator.name(), "Tj" | "TJ"))
                .map(|(i, _)| i)
                .collect();
            let start = TextCodePosition {
                operation: shows[0],
                array_item: None,
                offset: 1,
            };
            let end = TextCodePosition {
                operation: shows[2],
                array_item: None,
                offset: 2,
            };
            let stamp = doc.stamp();
            for (bad_start, bad_end) in [
                (TextCodePosition { offset: 2, ..start }, end),
                (start, TextCodePosition { offset: 1, ..end }),
            ] {
                let error = doc
                    .replace_page_text_event_range_preserving_advance(
                        1,
                        &snapshot,
                        bad_start..bad_end,
                        text,
                    )
                    .unwrap_err();
                assert!(
                    matches!(error, PdfError::ObjectError(ref message) if message == "text range splits a character code")
                );
                assert!(doc.is_stamp_current(&stamp));
                assert_eq!(
                    doc.get_page(1)
                        .unwrap()
                        .content_program()
                        .unwrap()
                        .source_bytes(),
                    snapshot.source_bytes()
                );
            }
            doc.replace_page_text_event_range_preserving_advance(1, &snapshot, start..end, text)
                .unwrap();
            let edited = doc.get_page(1).unwrap().content_program().unwrap();
            let arrays: Vec<_> = edited
                .operations()
                .iter()
                .filter(|op| op.operator.name() == "TJ")
                .map(|op| op.operator.get_as_array(0).unwrap())
                .collect();
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
            assert!(
                std::str::from_utf8(edited.source_bytes())
                    .unwrap()
                    .contains("30.00")
            );
            let reopened =
                PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            let before = reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let after = reopened
                .get_page(1)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(
                after
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                format!("A{text}CAA")
            );
            let old: Vec<_> = before
                .iter()
                .filter(|ch| matches!(ch.unicode.as_deref(), Some("A" | "C")))
                .collect();
            let new: Vec<_> = after
                .iter()
                .filter(|ch| matches!(ch.unicode.as_deref(), Some("A" | "C")))
                .collect();
            assert_eq!(old.len(), 4);
            assert_eq!(new.len(), 4);
            for (old, new) in old.into_iter().zip(new) {
                assert_eq!(old.unicode, new.unicode);
                assert!((old.position.x() - new.position.x()).abs() < 0.002);
                assert!((old.position.y() - new.position.y()).abs() < 0.002);
            }
            assert_eq!(
                reopened
                    .get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                snapshot.source_bytes()
            );
        }
    }
}

#[test]
fn cross_event_transaction_error_and_unwind_restore_streams_and_allow_retry() {
    use pdf::error::{PdfError, PdfResult};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf (AB) Tj [(CD) 20 (EF)] TJ (Z) Tj ET").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 1).unwrap();
    let initial = doc.get_page(0).unwrap().content_program().unwrap();
    let start = TextCodePosition {
        operation: 2,
        array_item: None,
        offset: 1,
    };
    let end = TextCodePosition {
        operation: 3,
        array_item: Some(2),
        offset: 1,
    };
    let result: PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_page_text_event_range_preserving_advance(0, &initial, start..end, "X")?;
        assert_ne!(
            editing.get_page(0)?.content_program()?.source_bytes(),
            initial.source_bytes()
        );
        Err(PdfError::DocumentError("cross event rollback".into()))
    });
    assert!(
        matches!(result, Err(PdfError::DocumentError(ref message)) if message == "cross event rollback")
    );
    for page in [0, 1] {
        let current = doc.get_page(page).unwrap().content_program().unwrap();
        assert_eq!(current.source_bytes(), initial.source_bytes());
        assert_eq!(current.stream_object_id(0), initial.stream_object_id(0));
    }
    assert!(
        doc.replace_page_text_event_range_preserving_advance(0, &initial, start..end, "X")
            .is_err()
    );
    let fresh = doc.get_page(0).unwrap().content_program().unwrap();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: PdfResult<()> = doc.edit_transaction(|editing| {
            editing.replace_page_text_event_range_preserving_advance(0, &fresh, start..end, "X")?;
            assert_ne!(
                editing.get_page(0)?.content_program()?.source_bytes(),
                initial.source_bytes()
            );
            panic!("cross event unwind");
        });
    }));
    assert_eq!(
        caught.unwrap_err().downcast_ref::<&str>().copied(),
        Some("cross event unwind")
    );
    for page in [0, 1] {
        let current = doc.get_page(page).unwrap().content_program().unwrap();
        assert_eq!(current.source_bytes(), initial.source_bytes());
        assert_eq!(current.stream_object_id(0), initial.stream_object_id(0));
    }
    let retry = doc.get_page(0).unwrap().content_program().unwrap();
    doc.replace_page_text_event_range_preserving_advance(0, &retry, start..end, "X")
        .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .get_page(1)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        initial.source_bytes()
    );
    let chars = reopened
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(
        chars
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "AXFZ"
    );
}

#[test]
fn cross_event_character_limit_is_aggregate_not_per_show() {
    use pdf::{
        error::PdfError,
        parser::options::{Limits, ParseOptions},
    };
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_text_chars: 3,
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
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf (AB) Tj (CD) Tj ET").into_bytes(),
    )
    .unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let start = TextCodePosition {
        operation: 2,
        array_item: None,
        offset: 1,
    };
    let end = TextCodePosition {
        operation: 3,
        array_item: None,
        offset: 1,
    };
    let stamp = doc.stamp();
    assert!(matches!(
        doc.replace_page_text_event_range_preserving_advance(0, &snapshot, start..end, "X"),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        snapshot.source_bytes()
    );
    doc.replace_page_text_event_range_preserving_advance(0, &snapshot, start..start, "X")
        .unwrap();
    let edited = doc.get_page(0).unwrap().content_program().unwrap();
    let array = edited.operations()[2].operator.get_as_array(0).unwrap();
    assert_eq!(array.get(0).unwrap().as_string().unwrap().bytes(), b"AX");
    assert_eq!(array.get(2).unwrap().as_string().unwrap().bytes(), b"B");
}

#[test]
fn cross_event_range_preserves_suffix_after_font_and_line_changes() {
    for middle in [
        "[40 (CD) -20 (EF)] TJ",
        "(CD) ' (EF) Tj",
        "3 2 (CD) \" (EF) Tj",
        "[40] TJ (CD) Tj [] TJ (EF) Tj",
    ] {
        for replacement in ["X", "", "XX XX"] {
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            let courier = doc
                .resources()
                .register_standard_font(0, StandardFont::Courier)
                .unwrap();
            let helvetica = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            doc.replace_page_contents(0, format!(
                "BT /{courier} 12 Tf 14 TL 1 Tc 2 Tw -75 Tz 1 0.2 0.1 1 100 150 Tm (AB) Tj {middle} /{helvetica} 9 Tf (GH) Tj (Z) Tj ET"
            ).into_bytes()).unwrap();
            doc.copy_page(0, 1).unwrap();
            let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
            let shows: Vec<_> = snapshot
                .operations()
                .iter()
                .enumerate()
                .filter(|(_, op)| op.operator.name() == "Tj")
                .map(|(i, _)| i)
                .collect();
            let start = TextCodePosition {
                operation: shows[0],
                array_item: None,
                offset: 1,
            };
            let end = TextCodePosition {
                operation: shows[shows.len() - 2],
                array_item: None,
                offset: 1,
            };
            doc.replace_page_text_event_range_preserving_advance(
                1,
                &snapshot,
                start..end,
                replacement,
            )
            .unwrap();
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                snapshot.source_bytes()
            );
            let bytes = doc.save_to_bytes().unwrap();
            let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            let before = reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let after = reopened
                .get_page(1)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let text: String = after
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect();
            assert_eq!(text, format!("A{replacement}HZ"));
            for letter in ["A", "H", "Z"] {
                let old = before
                    .iter()
                    .find(|ch| ch.unicode.as_deref() == Some(letter))
                    .unwrap();
                let new = after
                    .iter()
                    .find(|ch| ch.unicode.as_deref() == Some(letter))
                    .unwrap();
                assert!(
                    (old.position.x() - new.position.x()).abs() < 0.002,
                    "{middle}: {letter}"
                );
                assert!(
                    (old.position.y() - new.position.y()).abs() < 0.002,
                    "{middle}: {letter}"
                );
            }
        }
    }
}

#[test]
fn invalid_cross_event_endpoints_do_not_mutate_snapshot() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf (AB) Tj [(CD) 20 (EF)] TJ ET").into_bytes(),
    )
    .unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let start = TextCodePosition {
        operation: 2,
        array_item: None,
        offset: 1,
    };
    for end in [
        TextCodePosition {
            operation: 3,
            array_item: Some(1),
            offset: 0,
        },
        TextCodePosition {
            operation: 3,
            array_item: None,
            offset: 0,
        },
        TextCodePosition {
            operation: 3,
            array_item: Some(2),
            offset: 3,
        },
        TextCodePosition {
            operation: 1,
            array_item: None,
            offset: 0,
        },
        TextCodePosition {
            operation: 2,
            array_item: None,
            offset: 0,
        },
    ] {
        assert!(
            doc.replace_page_text_event_range_preserving_advance(0, &snapshot, start..end, "X")
                .is_err()
        );
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            snapshot.source_bytes()
        );
    }
    let end = TextCodePosition {
        operation: 3,
        array_item: Some(2),
        offset: 1,
    };
    doc.replace_page_text_event_range_preserving_advance(0, &snapshot, start..end, "X")
        .unwrap();
    let chars = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    let text: String = chars
        .iter()
        .filter_map(|ch| ch.unicode.as_deref())
        .collect();
    assert_eq!(text, "AXF");
}
