use pdf::{
    budget::OperationBudget,
    document::{PdfDocument, resource_manager::StandardFont},
    page::{content_builder::ContentBuilder, content_program::ContentProgram},
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};

#[test]
fn mixed_cid_range_compensation_preserves_suffix_and_rejects_split_codes() {
    use pdf::{
        document::resource_manager::ResourceKind,
        io::stream_reader::StreamReader,
        objects::{
            PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream,
        },
        parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;
    for vertical in [false, true] {
        for (replacement, horizontal_first, vertical_first) in [
            ("B", 0.0, 0.0),
            ("", -900.0, 1000.0),
            ("B ", 1100.0, -900.0),
        ] {
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
            // A=one byte, B=two bytes, C=one byte, space=two bytes. Tw=7
            // must NOT contribute to the two-byte space's signed advance.
            doc.replace_page_contents(0, format!(
                "q 1 .2 -.3 1 40 30 cm BT /{font} 10 Tf 2 Tc 7 Tw -75 Tz 14 TL 1 .3 .2 1 100 100 Tm [<418101>17.00<81018120>% keep\n-20.50<428101>] TJ <42> Tj T* <42> Tj ET Q"
            ).into_bytes()).unwrap();
            doc.copy_page(0, 1).unwrap();
            let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
            let operation = snapshot
                .operations()
                .iter()
                .position(|op| op.operator.name() == "TJ")
                .unwrap();
            let stamp = doc.stamp();
            for range in [(0, 2)..(4, 1), (0, 1)..(4, 2), (0, 1)..(2, 1)] {
                let error = doc
                    .replace_page_text_array_range_preserving_advance(
                        1,
                        &snapshot,
                        operation,
                        range,
                        replacement,
                    )
                    .unwrap_err();
                assert!(
                    matches!(error, pdf::error::PdfError::ObjectError(ref message) if message == "text range splits a character code")
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
            doc.replace_page_text_array_range_preserving_advance(
                1,
                &snapshot,
                operation,
                (0, 1)..(4, 1),
                replacement,
            )
            .unwrap();
            let reopened =
                PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for current in [&doc, &reopened] {
                assert_eq!(
                    current
                        .get_page(0)
                        .unwrap()
                        .content_program()
                        .unwrap()
                        .source_bytes(),
                    snapshot.source_bytes()
                );
                let changed = current.get_page(1).unwrap().content_program().unwrap();
                let array = changed.operations()[operation]
                    .operator
                    .get_as_array(0)
                    .unwrap();
                assert_eq!(array.len(), 11);
                let expected = if vertical {
                    [vertical_first, 1900.0, 700.0]
                } else {
                    [horizontal_first, -2000.0, -1000.0]
                };
                for (index, adjustment) in [(1, expected[0]), (5, expected[1]), (9, expected[2])] {
                    assert_eq!(array.get(index).unwrap().as_f32(), Some(adjustment));
                }
                assert_eq!(
                    array.get(10).unwrap().as_string().unwrap().bytes(),
                    b"\x81\x01"
                );
                assert_eq!(array.get(3).unwrap().as_f32(), Some(17.0));
                assert_eq!(array.get(7).unwrap().as_f32(), Some(-20.5));
                let baseline = current
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                let edited = current
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
                    baseline
                        .iter()
                        .chain(&edited)
                        .all(|ch| ch.writing_mode == Some(mode))
                );
                assert_eq!(
                    baseline
                        .iter()
                        .filter_map(|ch| ch.unicode.as_deref())
                        .collect::<String>(),
                    "ABB CBCC"
                );
                assert_eq!(
                    edited
                        .iter()
                        .filter_map(|ch| ch.unicode.as_deref())
                        .collect::<String>(),
                    format!("A{replacement}BCC")
                );
                let mut pairs = vec![(&baseline[0], &edited[0])];
                pairs.extend(baseline[5..].iter().zip(&edited[edited.len() - 3..]));
                assert_eq!(pairs.len(), 4);
                for (expected, actual) in pairs {
                    assert_eq!(expected.unicode, actual.unicode);
                    assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                    assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                }
                let lexical = std::str::from_utf8(changed.source_bytes()).unwrap();
                for token in ["17.00", "-20.50", "% keep\n"] {
                    assert!(lexical.contains(token));
                }
            }
        }
    }
}

#[test]
fn tj_range_compensation_keeps_partial_end_suffix_and_internal_positions() {
    for (range, retained) in [
        ((0, 1)..(4, 1), "PAFGH"),
        ((0, 1)..(0, 2), "PACDEFGH"),
        ((0, 1)..(0, 1), "PABCDEFGH"),
        ((4, 1)..(4, 2), "PABCDEGH"),
    ] {
        for replacement in ["X", "XYZ", ""] {
            for scale in [50, -75, 100] {
                let doc = PdfDocument::new().unwrap();
                doc.add_page(0).unwrap();
                let font = doc
                    .resources()
                    .register_standard_font(0, StandardFont::Courier)
                    .unwrap();
                doc.replace_page_contents(0, format!(
                    "q 1 .2 -.3 1 40 30 cm BT /{font} 10 Tf 2 Tc 3 Tw {scale} Tz 14 TL 1 .3 .2 1 100 100 Tm (P) Tj [(AB)17.00(CD)% keep\n-20.50(EF)] TJ (G) Tj T* (H) Tj ET Q"
                ).into_bytes()).unwrap();
                doc.copy_page(0, 1).unwrap();
                let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
                let operation = snapshot
                    .operations()
                    .iter()
                    .position(|op| op.operator.name() == "TJ")
                    .unwrap();
                doc.replace_page_text_array_range_preserving_advance(
                    1,
                    &snapshot,
                    operation,
                    range.clone(),
                    replacement,
                )
                .unwrap();
                let reopened = PdfDocument::from_bytes_with_mode(
                    doc.save_to_bytes().unwrap(),
                    ParseMode::Strict,
                )
                .unwrap();
                for current in [&doc, &reopened] {
                    assert_eq!(
                        current
                            .get_page(0)
                            .unwrap()
                            .content_program()
                            .unwrap()
                            .source_bytes(),
                        snapshot.source_bytes()
                    );
                    let program = current.get_page(1).unwrap().content_program().unwrap();
                    let array = program.operations()[operation]
                        .operator
                        .get_as_array(0)
                        .unwrap();
                    let original = snapshot.operations()[operation]
                        .operator
                        .get_as_array(0)
                        .unwrap();
                    let mut shift = 0;
                    for item in 0..original.len() {
                        if item < range.start.0
                            || item > range.end.0
                            || original.get(item).unwrap().as_string().is_none()
                        {
                            assert_eq!(original.get(item), array.get(item + shift));
                            continue;
                        }
                        let old = original.get(item).unwrap().as_string().unwrap().bytes();
                        let start = if item == range.start.0 {
                            range.start.1
                        } else {
                            0
                        };
                        let end = if item == range.end.0 {
                            range.end.1
                        } else {
                            old.len()
                        };
                        let inserted = if item == range.start.0 {
                            replacement.as_bytes()
                        } else {
                            b""
                        };
                        let prefix = array
                            .get(item + shift)
                            .unwrap()
                            .as_string()
                            .unwrap()
                            .bytes();
                        assert_eq!(&prefix[..start], &old[..start]);
                        assert_eq!(&prefix[start..], inserted);
                        assert_eq!(
                            array.get(item + shift + 1).unwrap().as_f32(),
                            Some((inserted.len() as f32 - (end - start) as f32) * 800.0)
                        );
                        assert_eq!(
                            array
                                .get(item + shift + 2)
                                .unwrap()
                                .as_string()
                                .unwrap()
                                .bytes(),
                            &old[end..]
                        );
                        shift += 2;
                    }
                    assert_eq!(array.len(), original.len() + shift);
                    let lexical = std::str::from_utf8(program.source_bytes()).unwrap();
                    for unchanged in ["17.00", "-20.50", "% keep\n"] {
                        assert!(lexical.contains(unchanged));
                    }
                    let baseline = current
                        .get_page(0)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    let edited = current
                        .get_page(1)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    let before: Vec<_> = baseline
                        .iter()
                        .filter(|ch| ch.unicode.as_deref().is_some_and(|s| retained.contains(s)))
                        .collect();
                    let after: Vec<_> = edited
                        .iter()
                        .filter(|ch| ch.unicode.as_deref().is_some_and(|s| retained.contains(s)))
                        .collect();
                    assert_eq!(before.len(), retained.len());
                    assert_eq!(after.len(), retained.len());
                    for (expected, actual) in before.into_iter().zip(after) {
                        assert_eq!(expected.unicode, actual.unicode);
                        assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                        assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                    }
                }
            }
        }
    }
}

#[test]
fn tj_range_compensation_low_level_budget_and_suffix_validation() {
    use pdf::page::content_program::TextArrayAdjustment;
    let options = ParseOptions::default();
    let source = b"BT [(AB)17(CD)-20(EF)] TJ ET";
    let program = ContentProgram::from_decoded_bytes(source.to_vec(), options).unwrap();
    let replacements = [
        (0, b"AX".as_slice()),
        (2, b"".as_slice()),
        (4, b"F".as_slice()),
    ];
    let adjustments = [
        TextArrayAdjustment {
            value: 0.0,
            suffix_offset: 2,
        },
        TextArrayAdjustment {
            value: -1600.0,
            suffix_offset: 0,
        },
        TextArrayAdjustment {
            value: -800.0,
            suffix_offset: 0,
        },
    ];
    let run = |pool: &OperationBudget| {
        program.with_adjusted_text_array_items_codes_with_budget(
            1,
            &replacements,
            &adjustments,
            options,
            pool,
        )
    };
    let probe = OperationBudget::new(1024 * 1024);
    let output = run(&probe).unwrap();
    assert!(
        std::str::from_utf8(output.as_ref().source_bytes())
            .unwrap()
            .contains("<4158> 0 <>17<")
    );
    let expected = output.as_ref().source_bytes().to_vec();
    let peak = probe.peak_bytes();
    drop(output);
    assert_eq!(probe.live_bytes(), 0);
    let pool = OperationBudget::new(peak + 6);
    let neighbor = pool.reserve(7).unwrap();
    assert!(run(&pool).is_err());
    assert_eq!(pool.live_bytes(), 7);
    assert_eq!(program.source_bytes(), source);
    drop(neighbor);
    let output = run(&pool).unwrap();
    let retained = pool.live_bytes();
    assert!(run(&pool).is_err());
    assert_eq!(pool.live_bytes(), retained);
    assert_eq!(output.as_ref().source_bytes(), expected);
    drop(output);
    assert_eq!(pool.live_bytes(), 0);
    let mut bad = adjustments;
    bad[0].suffix_offset = 3;
    assert!(
        program
            .with_adjusted_text_array_items_codes_with_budget(
                1,
                &replacements,
                &bad,
                options,
                &pool
            )
            .is_err()
    );
    assert!(
        program
            .with_adjusted_text_array_items_codes_with_budget(
                1,
                &replacements,
                &adjustments[..2],
                options,
                &pool
            )
            .is_err()
    );
    let mut restricted = options;
    restricted.limits.max_objects = 10; // Original five plus six inserted items.
    assert!(
        program
            .with_adjusted_text_array_items_codes_with_budget(
                1,
                &replacements,
                &adjustments,
                restricted,
                &pool
            )
            .is_err()
    );
    assert_eq!(pool.live_bytes(), 0);
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _output = run(&pool).unwrap();
        panic!("TJ range result owner unwind");
    }));
    assert_eq!(
        caught.unwrap_err().downcast_ref::<&str>().copied(),
        Some("TJ range result owner unwind")
    );
    assert_eq!(pool.live_bytes(), 0);
    drop(run(&pool).unwrap());
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn tj_range_compensation_public_errors_rollback_and_retry() {
    use pdf::error::{PdfError, PdfResult};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 10 Tf [(AB)17(CD)-20(EF)] TJ (G) Tj ET").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 1).unwrap();
    let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
    let stamp = doc.stamp();
    for (range, text) in [
        ((0, 1)..(4, 1), "中文"),
        ((1, 0)..(4, 1), "X"),
        ((4, 1)..(0, 1), "X"),
        ((0, 3)..(4, 1), "X"),
    ] {
        assert!(
            doc.replace_page_text_array_range_preserving_advance(1, &snapshot, 2, range, text)
                .is_err()
        );
        assert!(doc.is_stamp_current(&stamp));
    }
    let result: PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_page_text_array_range_preserving_advance(
            1,
            &snapshot,
            2,
            (0, 1)..(4, 1),
            "X",
        )?;
        Err(PdfError::DocumentError(
            "TJ range compensation rollback".into(),
        ))
    });
    assert!(
        matches!(result, Err(PdfError::DocumentError(ref msg)) if msg == "TJ range compensation rollback")
    );
    for page in [0, 1] {
        assert_eq!(
            doc.get_page(page)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            snapshot.source_bytes()
        );
    }
    assert!(
        doc.replace_page_text_array_range_preserving_advance(1, &snapshot, 2, (0, 1)..(4, 1), "X")
            .is_err()
    );
    let fresh = doc.get_page(1).unwrap().content_program().unwrap();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: PdfResult<()> = doc.edit_transaction(|editing| {
            editing.replace_page_text_array_range_preserving_advance(
                1,
                &fresh,
                2,
                (0, 1)..(4, 1),
                "X",
            )?;
            assert_ne!(
                editing.get_page(1)?.content_program()?.source_bytes(),
                fresh.source_bytes()
            );
            panic!("TJ range commit unwind");
        });
    }));
    assert_eq!(
        caught.unwrap_err().downcast_ref::<&str>().copied(),
        Some("TJ range commit unwind")
    );
    for page in [0, 1] {
        assert_eq!(
            doc.get_page(page)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            snapshot.source_bytes()
        );
    }
    let retry = doc.get_page(1).unwrap().content_program().unwrap();
    doc.replace_page_text_array_range_preserving_advance(1, &retry, 2, (0, 1)..(4, 1), "X")
        .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let edited = reopened
        .get_page(1)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(
        edited
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "AXFG"
    );
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

#[test]
fn tj_item_compensation_preserves_internal_neighbors_and_original_spacing() {
    for (array, item, old, next) in [
        ("[(A)17.00(C)-20.50(E)]", 0, "A", "CEFD"),
        ("[(P)17.00(A)% keep\n-20.50(C)]", 2, "A", "CFD"),
        ("[(P)17.00(A)]", 2, "A", "FD"),
        ("[(A )17.00(C)-20.50(E)]", 0, "A ", "CEFD"),
    ] {
        for replacement in ["AB", "", "B"] {
            for scale in [50, -75, 100] {
                let doc = PdfDocument::new().unwrap();
                doc.add_page(0).unwrap();
                let font = doc
                    .resources()
                    .register_standard_font(0, StandardFont::Courier)
                    .unwrap();
                doc.replace_page_contents(0, format!(
                    "q 1 .2 -.3 1 40 30 cm BT /{font} 10 Tf 2 Tc 3 Tw {scale} Tz 14 TL 1 .3 .2 1 100 100 Tm {array} TJ (F) Tj T* (D) Tj ET Q"
                ).into_bytes()).unwrap();
                doc.copy_page(0, 1).unwrap();
                let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
                let operation = snapshot
                    .operations()
                    .iter()
                    .position(|op| op.operator.name() == "TJ")
                    .unwrap();
                doc.replace_page_text_array_item_preserving_advance(
                    1,
                    &snapshot,
                    operation,
                    item,
                    replacement,
                )
                .unwrap();
                let reopened = PdfDocument::from_bytes_with_mode(
                    doc.save_to_bytes().unwrap(),
                    ParseMode::Strict,
                )
                .unwrap();
                for current in [&doc, &reopened] {
                    let source = current.get_page(0).unwrap().content_program().unwrap();
                    assert_eq!(source.source_bytes(), snapshot.source_bytes());
                    let edited = current.get_page(1).unwrap().content_program().unwrap();
                    let target = edited.operations()[operation]
                        .operator
                        .get_as_array(0)
                        .unwrap();
                    let original = snapshot.operations()[operation]
                        .operator
                        .get_as_array(0)
                        .unwrap();
                    assert_eq!(target.len(), original.len() + 1);
                    assert_eq!(
                        target.get(item).unwrap().as_string().unwrap().bytes(),
                        replacement.as_bytes()
                    );
                    // Courier at Tf=10/Tc=2: each glyph is 800 normalized
                    // units; a single-byte space gets another Tw=3 => 300.
                    let expected = (replacement.len() as f32 - old.len() as f32) * 800.0
                        - if old.ends_with(' ') { 300.0 } else { 0.0 };
                    assert_eq!(target.get(item + 1).unwrap().as_f32(), Some(expected));
                    for index in 0..original.len() {
                        if index != item {
                            assert_eq!(
                                original.get(index),
                                target.get(index + usize::from(index > item))
                            );
                        }
                    }
                    let lexical = std::str::from_utf8(edited.source_bytes()).unwrap();
                    assert!(lexical.contains("17.00"));
                    if array.contains("-20.50") {
                        assert!(lexical.contains("-20.50"));
                    }
                    if array.contains("% keep") {
                        assert!(lexical.contains("% keep\n"));
                    }
                    let baseline = current
                        .get_page(0)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    let changed = current
                        .get_page(1)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    let before: Vec<_> = baseline
                        .iter()
                        .filter(|ch| ch.unicode.as_deref().is_some_and(|s| next.contains(s)))
                        .collect();
                    let after: Vec<_> = changed
                        .iter()
                        .filter(|ch| ch.unicode.as_deref().is_some_and(|s| next.contains(s)))
                        .collect();
                    assert_eq!(before.len(), next.len());
                    assert_eq!(after.len(), next.len());
                    for (expected, actual) in before.into_iter().zip(after) {
                        assert_eq!(expected.unicode, actual.unicode);
                        assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                        assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                    }
                }
            }
        }
    }
}

#[test]
fn tj_item_compensation_budget_limits_ownership_and_separator() {
    let options = ParseOptions::default();
    let source = b"BT [(A)17(C)% keep\n-20(D)] TJ ET";
    let program = ContentProgram::from_decoded_bytes(source.to_vec(), options).unwrap();
    let probe = OperationBudget::new(1024 * 1024);
    let output = program
        .with_adjusted_text_array_item_codes_with_budget(1, 0, b"AB", 600.0, options, &probe)
        .unwrap();
    assert!(
        std::str::from_utf8(output.as_ref().source_bytes())
            .unwrap()
            .contains("<4142> 600 17(C)% keep\n-20(D)")
    );
    let expected = output.as_ref().source_bytes().to_vec();
    let peak = probe.peak_bytes();
    assert!(probe.live_bytes() > 0);
    drop(output);
    assert_eq!(probe.live_bytes(), 0);
    let pool = OperationBudget::new(peak + 6);
    let neighbor = pool.reserve(7).unwrap();
    assert!(
        program
            .with_adjusted_text_array_item_codes_with_budget(1, 0, b"AB", 600.0, options, &pool)
            .is_err()
    );
    assert_eq!(pool.live_bytes(), 7);
    assert_eq!(program.source_bytes(), source);
    drop(neighbor);
    let output = program
        .with_adjusted_text_array_item_codes_with_budget(1, 0, b"AB", 600.0, options, &pool)
        .unwrap();
    let retained = pool.live_bytes();
    assert!(
        program
            .with_adjusted_text_array_item_codes_with_budget(1, 0, b"AB", 600.0, options, &pool)
            .is_err()
    );
    assert_eq!(pool.live_bytes(), retained);
    assert_eq!(output.as_ref().source_bytes(), expected);
    drop(output);
    assert_eq!(pool.live_bytes(), 0);
    let mut restricted = options;
    restricted.limits.max_objects = 5; // Existing array has five items; insertion needs six.
    assert!(
        program
            .with_adjusted_text_array_item_codes_with_budget(1, 0, b"AB", 600.0, restricted, &pool)
            .is_err()
    );
    assert_eq!(pool.live_bytes(), 0);
    for (item, adjustment) in [(1, 600.0), (8, 600.0), (0, f32::NAN), (0, f32::INFINITY)] {
        assert!(
            program
                .with_adjusted_text_array_item_codes_with_budget(
                    1, item, b"AB", adjustment, options, &pool
                )
                .is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
    }
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _output = program
            .with_adjusted_text_array_item_codes_with_budget(1, 0, b"AB", 600.0, options, &pool)
            .unwrap();
        panic!("TJ result owner unwind");
    }));
    assert_eq!(
        caught.unwrap_err().downcast_ref::<&str>().copied(),
        Some("TJ result owner unwind")
    );
    assert_eq!(pool.live_bytes(), 0);
    drop(
        program
            .with_adjusted_text_array_item_codes_with_budget(1, 0, b"AB", 600.0, options, &pool)
            .unwrap(),
    );
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn tj_item_compensation_public_failures_and_outer_rollback() {
    use pdf::error::{PdfError, PdfResult};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 10 Tf [(A)17(C)] TJ (D) Tj ET").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 1).unwrap();
    let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
    let stamp = doc.stamp();
    for (item, text) in [(0, "中文"), (1, "AB"), (9, "AB")] {
        assert!(
            doc.replace_page_text_array_item_preserving_advance(1, &snapshot, 2, item, text)
                .is_err()
        );
        assert!(doc.is_stamp_current(&stamp));
    }
    let result: PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_page_text_array_item_preserving_advance(1, &snapshot, 2, 0, "AB")?;
        Err(PdfError::DocumentError("TJ compensation rollback".into()))
    });
    assert!(
        matches!(result, Err(PdfError::DocumentError(ref msg)) if msg == "TJ compensation rollback")
    );
    for page in [0, 1] {
        assert_eq!(
            doc.get_page(page)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            snapshot.source_bytes()
        );
    }
    assert!(
        doc.replace_page_text_array_item_preserving_advance(1, &snapshot, 2, 0, "AB")
            .is_err()
    );
    let fresh = doc.get_page(1).unwrap().content_program().unwrap();
    let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: PdfResult<()> = doc.edit_transaction(|editing| {
            editing.replace_page_text_array_item_preserving_advance(1, &fresh, 2, 0, "AB")?;
            assert_ne!(
                editing.get_page(1)?.content_program()?.source_bytes(),
                fresh.source_bytes()
            );
            panic!("TJ compensation commit unwind");
        });
    }));
    assert_eq!(
        caught.unwrap_err().downcast_ref::<&str>().copied(),
        Some("TJ compensation commit unwind")
    );
    for page in [0, 1] {
        assert_eq!(
            doc.get_page(page)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            snapshot.source_bytes()
        );
    }
    let retry = doc.get_page(1).unwrap().content_program().unwrap();
    doc.replace_page_text_array_item_preserving_advance(1, &retry, 2, 0, "AB")
        .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        snapshot.source_bytes()
    );
    let changed = reopened
        .get_page(1)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(
        changed
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "ABCD"
    );
}

#[test]
fn vertical_cid_compensation_uses_signed_w2_and_ignores_multibyte_space_tw() {
    use pdf::{
        document::resource_manager::ResourceKind,
        io::stream_reader::StreamReader,
        objects::{
            PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream,
        },
        parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;
    // Signed vertical advances at Tf=10/Tc=2: A=-8, B=-10, space=-9.
    // The two-byte space gets no Tw=7. Independent expected TJ adjustments:
    // AB-A=-10 => -1000; empty-A=8 => 800; B-(A+space)=7 => 700.
    for (old, replacement, expected_adjustment) in [
        ("0001", "AB", -1000.0),
        ("0001", "", 800.0),
        ("00010020", "B", 700.0),
    ] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let unicode = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            PdfDict::default(),
            b"1 begincodespacerange <0000> <FFFF> endcodespacerange 4 beginbfchar <0001> <0041> <0002> <0042> <0003> <0043> <0020> <0020> endbfchar".to_vec(),
        ).unwrap())).unwrap();
        let mut dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(
            b"<< /Type /Font /Subtype /Type0 /BaseFont /VerticalTest /Encoding /Identity-V /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /VerticalTest /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /DW 1000 /W [1 [600 700 800] 32 [900]] /DW2 [880 -1000] /W2 [1 [-1000 300 880 -1200 350 880 -900 400 880] 32 [-1100 450 880]] >>] >>".to_vec(),
        )).unwrap()).get_object().unwrap().into_dict().unwrap();
        dict.insert(
            "ToUnicode".into(),
            PdfObject::PdfReference(PdfReference::from_id(unicode)),
        );
        let id = doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
        let font = doc
            .resources()
            .register_existing(0, ResourceKind::Font, id)
            .unwrap();
        doc.replace_page_contents(0, format!(
            "BT /{font} 10 Tf 2 Tc 7 Tw -75 Tz 14 TL 1 .3 .2 1 100 100 Tm <{old}> Tj <0003> Tj T* <0003> Tj ET"
        ).into_bytes()).unwrap();
        doc.copy_page(0, 1).unwrap();
        let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
        let operation = snapshot
            .operations()
            .iter()
            .position(|op| op.operator.name() == "Tj")
            .unwrap();
        doc.replace_page_text_preserving_advance(1, &snapshot, operation, replacement)
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for current in [&doc, &reopened] {
            let adjusted = current.get_page(1).unwrap().content_program().unwrap();
            let array = adjusted
                .operations()
                .iter()
                .find(|op| op.operator.name() == "TJ")
                .unwrap()
                .operator
                .get_as_array(0)
                .unwrap();
            assert_eq!(array.get(1).unwrap().as_f32(), Some(expected_adjustment));
            let original = current
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let edited = current
                .get_page(1)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert!(
                original
                    .iter()
                    .chain(&edited)
                    .all(|ch| ch.writing_mode == Some(pdf::font::WritingMode::Vertical))
            );
            let before: Vec<_> = original
                .iter()
                .filter(|ch| ch.unicode.as_deref() == Some("C"))
                .collect();
            let after: Vec<_> = edited
                .iter()
                .filter(|ch| ch.unicode.as_deref() == Some("C"))
                .collect();
            assert_eq!(before.len(), 2);
            assert_eq!(after.len(), 2);
            for (expected, actual) in before.into_iter().zip(after) {
                assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
            }
            assert_eq!(
                edited
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                format!("{replacement}CC")
            );
        }
    }
}

#[test]
fn adjusted_program_peak_rejection_retained_results_and_unwind_release() {
    for source in [
        b"BT (A) Tj (C) Tj ET".as_slice(),
        b"BT (A) ' (C) Tj ET".as_slice(),
        b"BT 3 2 (A) \" (C) Tj ET".as_slice(),
    ] {
        let options = ParseOptions::default();
        let program = ContentProgram::from_decoded_bytes(source.to_vec(), options).unwrap();
        let probe = OperationBudget::new(1024 * 1024);
        let output = program
            .with_adjusted_text_codes_with_budget(1, b"AB", 800.0, options, &probe)
            .unwrap();
        let expected = output.as_ref().source_bytes().to_vec();
        let peak = probe.peak_bytes();
        assert!(probe.live_bytes() > 0);
        drop(output);
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak + 6);
        let neighbor = pool.reserve(7).unwrap();
        assert!(
            program
                .with_adjusted_text_codes_with_budget(1, b"AB", 800.0, options, &pool)
                .is_err()
        );
        assert_eq!(program.source_bytes(), source);
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        let output = program
            .with_adjusted_text_codes_with_budget(1, b"AB", 800.0, options, &pool)
            .unwrap();
        assert_eq!(output.as_ref().source_bytes(), expected);
        let retained = pool.live_bytes();
        assert!(retained > 0);
        assert!(
            program
                .with_adjusted_text_codes_with_budget(1, b"AB", 800.0, options, &pool)
                .is_err()
        );
        assert_eq!(pool.live_bytes(), retained);
        assert_eq!(output.as_ref().source_bytes(), expected);
        drop(output);
        assert_eq!(pool.live_bytes(), 0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _output = program
                .with_adjusted_text_codes_with_budget(1, b"AB", 800.0, options, &pool)
                .unwrap();
            panic!("adjusted program owner unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        let output = program
            .with_adjusted_text_codes_with_budget(1, b"AB", 800.0, options, &pool)
            .unwrap();
        drop(output);
        assert_eq!(pool.live_bytes(), 0);
    }
}

#[test]
fn adjusted_text_tokens_limits_and_owned_budget_are_atomic() {
    for adjustment in [
        600.0,
        -125.5,
        0.0,
        -0.0,
        f32::MAX,
        -f32::MAX,
        f32::MIN_POSITIVE,
        f32::from_bits(1),
    ] {
        let mut builder = ContentBuilder::new();
        builder
            .show_adjusted_encoded_text(&[0, 255], adjustment)
            .unwrap();
        let program = ContentProgram::from_decoded_bytes(
            builder.as_bytes().to_vec(),
            ParseOptions::default(),
        )
        .unwrap();
        let operator = &program.operations()[0].operator;
        assert_eq!(operator.name(), "TJ");
        let array = operator.get_as_array(0).unwrap();
        assert_eq!(array.len(), 2);
        assert_eq!(
            array.get(0).unwrap().as_string().unwrap().bytes(),
            &[0, 255]
        );
        assert_eq!(
            array.get(1).unwrap().as_number().unwrap().value(),
            adjustment
        );
        let length = builder.as_bytes().len();
        let pool = OperationBudget::new(length + 7);
        let neighbor = pool.reserve(7).unwrap();
        let output = ContentBuilder::adjusted_encoded_text_with_budget(
            &[0, 255],
            adjustment,
            Limits::default(),
            &pool,
        )
        .unwrap();
        assert_eq!(output.as_slice(), builder.as_bytes());
        assert_eq!(pool.live_bytes(), output.capacity() + 7);
        assert!(
            ContentBuilder::adjusted_encoded_text_with_budget(&[], 0.0, Limits::default(), &pool)
                .is_err()
        );
        drop(output);
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
        assert!(
            ContentBuilder::adjusted_encoded_text_with_budget(
                &[0, 255],
                adjustment,
                Limits::default(),
                &OperationBudget::new(length - 1)
            )
            .is_err()
        );
    }
    let mut builder = ContentBuilder::new();
    builder.show_encoded_text(b"keep").unwrap();
    let before = builder.as_bytes().to_vec();
    for adjustment in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
        assert!(
            builder
                .show_adjusted_encoded_text(b"A", adjustment)
                .is_err()
        );
        assert_eq!(builder.as_bytes(), before);
    }
    for limits in [
        Limits {
            max_token_bytes: 3,
            ..Limits::default()
        },
        Limits {
            max_token_bytes: 4,
            ..Limits::default()
        },
        Limits {
            max_raw_stream_bytes: before.len() + 1,
            ..Limits::default()
        },
    ] {
        assert!(
            builder
                .show_adjusted_encoded_text_with_limits(b"A", 12345.0, limits)
                .is_err()
        );
        assert_eq!(builder.as_bytes(), before);
    }
}

#[test]
fn explicit_adjustment_keeps_following_text_position_after_save() {
    // Courier widths are 600/1000. At Tf=10, Tc=2, Tw=3:
    // A advances 8, AB advances 16, empty advances 0, A-space advances 19.
    // TJ subtracts adjustment*Tf/1000 before applying horizontal scale.
    for (old, new, adjustment) in [("A", "AB", 800.0), ("A", "", -800.0), ("A ", "B", -1100.0)] {
        for scale in [50.0, -75.0, 100.0] {
            let doc = PdfDocument::new().unwrap();
            for index in 0..3 {
                doc.add_page(index).unwrap();
                let font = doc
                    .resources()
                    .register_standard_font(index, StandardFont::Courier)
                    .unwrap();
                let mut builder = ContentBuilder::new();
                let codes = StandardFont::Courier
                    .encode_text(if index == 0 { old } else { new })
                    .unwrap();
                if index == 1 {
                    builder
                        .show_adjusted_encoded_text(&codes, adjustment)
                        .unwrap();
                } else {
                    builder.show_encoded_text(&codes).unwrap();
                }
                builder.show_encoded_text(b"C").unwrap();
                let mut bytes =
                    format!("BT /{font} 10 Tf 2 Tc 3 Tw {scale} Tz 0 1 -1 0 80 90 Tm\n")
                        .into_bytes();
                bytes.extend_from_slice(builder.as_bytes());
                bytes.extend_from_slice(b"ET\n");
                doc.replace_page_contents(index, bytes).unwrap();
            }
            let reopened =
                PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for current in [&doc, &reopened] {
                let mut positions = Vec::new();
                for index in 0..3 {
                    let chars = current
                        .get_page(index)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    let following = chars.last().unwrap();
                    assert_eq!(following.unicode.as_deref(), Some("C"));
                    positions.push((following.position.x(), following.position.y()));
                }
                assert!((positions[0].0 - positions[1].0).abs() < 0.001);
                assert!((positions[0].1 - positions[1].1).abs() < 0.001);
                assert!(
                    (positions[0].0 - positions[2].0).abs()
                        + (positions[0].1 - positions[2].1).abs()
                        > 1.0
                );
            }
        }
    }
}

#[test]
fn unicode_compensation_preserves_following_text_and_quote_line_matrix() {
    for event in ["Tj", "'", "\""] {
        for (old, new) in [("A", "AB"), ("A", ""), ("A ", "B")] {
            for scale in [50.0, -75.0, 100.0] {
                let doc = PdfDocument::new().unwrap();
                doc.add_page(0).unwrap();
                let font = doc
                    .resources()
                    .register_standard_font(0, StandardFont::Courier)
                    .unwrap();
                let target = if event == "\"" {
                    format!("3 2 ({old}) \"")
                } else {
                    format!("({old}) {event}")
                };
                doc.replace_page_contents(0, format!(
                    "q 1 .2 -.3 1 40 30 cm BT /{font} 10 Tf 2 Tc 3 Tw {scale} Tz 14 TL 1 .3 .2 1 100 100 Tm (P) Tj {target} (C) Tj T* (D) Tj ET Q"
                ).into_bytes()).unwrap();
                doc.copy_page(0, 1).unwrap();
                let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
                let operation = snapshot
                    .operations()
                    .iter()
                    .position(|op| {
                        op.operator.name() == event
                            && op
                                .operator
                                .get_as_string(if event == "\"" { 2 } else { 0 })
                                .is_ok_and(|string| string.bytes() == old.as_bytes())
                    })
                    .unwrap();
                doc.replace_page_text_preserving_advance(1, &snapshot, operation, new)
                    .unwrap();
                assert_eq!(
                    doc.get_page(0)
                        .unwrap()
                        .content_program()
                        .unwrap()
                        .source_bytes(),
                    snapshot.source_bytes()
                );
                let reopened = PdfDocument::from_bytes_with_mode(
                    doc.save_to_bytes().unwrap(),
                    ParseMode::Strict,
                )
                .unwrap();
                for current in [&doc, &reopened] {
                    let original = current
                        .get_page(0)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    let edited = current
                        .get_page(1)
                        .unwrap()
                        .extract_characters_paint_order()
                        .unwrap();
                    for letter in ["C", "D"] {
                        let expected = original
                            .iter()
                            .find(|ch| ch.unicode.as_deref() == Some(letter))
                            .unwrap();
                        let actual = edited
                            .iter()
                            .find(|ch| ch.unicode.as_deref() == Some(letter))
                            .unwrap();
                        assert!(
                            (expected.position.x() - actual.position.x()).abs() < 0.002,
                            "event={event}, old={old}, new={new}, scale={scale}"
                        );
                        assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
                    }
                    assert_eq!(
                        edited
                            .iter()
                            .filter_map(|ch| ch.unicode.as_deref())
                            .collect::<String>(),
                        format!("P{new}CD")
                    );
                }
            }
        }
    }
}

#[test]
fn unicode_compensation_failure_rollback_and_retry_preserve_shared_page() {
    use pdf::error::{PdfError, PdfResult};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    doc.replace_page_contents(0, format!("BT /{font} 10 Tf (A) Tj (C) Tj ET").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
    let stamp = doc.stamp();
    assert!(
        doc.replace_page_text_preserving_advance(1, &snapshot, 2, "中文")
            .is_err()
    );
    assert!(doc.is_stamp_current(&stamp));
    let result: PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_page_text_preserving_advance(1, &snapshot, 2, "AB")?;
        Err(PdfError::DocumentError("rollback compensation".into()))
    });
    assert!(
        matches!(result, Err(PdfError::DocumentError(ref message)) if message == "rollback compensation")
    );
    assert_eq!(
        doc.get_page(1)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        snapshot.source_bytes()
    );
    assert!(
        doc.replace_page_text_preserving_advance(1, &snapshot, 2, "AB")
            .is_err()
    );
    let refreshed = doc.get_page(1).unwrap().content_program().unwrap();
    doc.replace_page_text_preserving_advance(1, &refreshed, 2, "AB")
        .unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        snapshot.source_bytes()
    );
    let zero = PdfDocument::new().unwrap();
    zero.add_page(0).unwrap();
    let font = zero
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    zero.replace_page_contents(0, format!("BT /{font} 0 Tf (A) Tj ET").into_bytes())
        .unwrap();
    let snapshot = zero.get_page(0).unwrap().content_program().unwrap();
    let revision = zero.revision();
    assert!(matches!(
        zero.replace_page_text_preserving_advance(0, &snapshot, 2, "AB"),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert_eq!(zero.revision(), revision);
    assert_eq!(
        zero.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        snapshot.source_bytes()
    );
}

#[test]
fn unicode_compensation_outer_unwind_restores_quotes_and_shared_content() {
    for event in ["Tj", "'", "\""] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        let target = if event == "\"" {
            "3 2 (A) \"".to_owned()
        } else {
            format!("(A) {event}")
        };
        doc.replace_page_contents(
            0,
            format!("BT /{font} 10 Tf 14 TL {target} (C) Tj T* (D) Tj ET").into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        let before = doc.get_page(1).unwrap().content_program().unwrap();
        let operation = before
            .operations()
            .iter()
            .position(|op| op.operator.name() == event)
            .unwrap();
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
                editing.replace_page_text_preserving_advance(1, &before, operation, "AB")?;
                let changed = editing.get_page(1)?.content_program()?;
                assert_ne!(changed.source_bytes(), before.source_bytes());
                panic!("injected compensation commit unwind");
            });
        }));
        let payload = caught.expect_err("Expected injected outer unwind");
        assert_eq!(
            payload.downcast_ref::<&str>().copied(),
            Some("injected compensation commit unwind")
        );
        for index in 0..2 {
            let restored = doc.get_page(index).unwrap().content_program().unwrap();
            assert_eq!(restored.source_bytes(), before.source_bytes());
            assert_eq!(restored.stream_object_id(0), before.stream_object_id(0));
        }
        assert!(
            doc.replace_page_text_preserving_advance(1, &before, operation, "AB")
                .is_err()
        );
        let fresh = doc.get_page(1).unwrap().content_program().unwrap();
        doc.replace_page_text_preserving_advance(1, &fresh, operation, "AB")
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for current in [&doc, &reopened] {
            let original = current
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let edited = current
                .get_page(1)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(
                original
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "ACD"
            );
            assert_eq!(
                edited
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "ABCD"
            );
            for letter in ["C", "D"] {
                let expected = original
                    .iter()
                    .find(|ch| ch.unicode.as_deref() == Some(letter))
                    .unwrap();
                let actual = edited
                    .iter()
                    .find(|ch| ch.unicode.as_deref() == Some(letter))
                    .unwrap();
                assert!((expected.position.x() - actual.position.x()).abs() < 0.002);
                assert!((expected.position.y() - actual.position.y()).abs() < 0.002);
            }
        }
    }
}
