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
