use pdf::{
    page::content_program::ContentProgram,
    parser::options::{Limits, ParseOptions},
};

#[test]
fn unicode_text_commit_binds_effective_font_and_restores_q_state() {
    use pdf::{
        document::{
            owned::Document,
            resource_manager::{ResourceKind, StandardFont},
        },
        io::stream_reader::StreamReader,
        objects::{
            PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream,
        },
        parser::syntax::{ParseMode, SyntaxParser},
    };
    use std::io::Cursor;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let normal = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let mapping = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(PdfDict::default(), b"1 begincodespacerange <00> <FF> endcodespacerange 2 beginbfchar <01> <0041> <02> <0042> endbfchar".to_vec()).unwrap())).unwrap();
    let mut dict = SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica /Encoding /WinAnsiEncoding /FirstChar 1 /LastChar 2 /Widths [500 600] >>"
                .to_vec(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    dict.insert(
        "ToUnicode".into(),
        PdfObject::PdfReference(PdfReference::from_id(mapping)),
    );
    let font = doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
    let mapped = doc
        .resources()
        .register_existing(0, ResourceKind::Font, font)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{normal} 12 Tf q /{mapped} 12 Tf <01> Tj Q (A) Tj ET").into_bytes(),
    )
    .unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    doc.replace_page_text(0, &snapshot, 4, "AB").unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(
        snapshot.operations()[4]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        &[1, 2]
    );
    doc.replace_page_text(0, &snapshot, 6, "BB").unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let mut page = reopened.get_page(0).unwrap();
    let program = page.content_program().unwrap();
    assert_eq!(
        program.operations()[6]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        b"BB"
    );
    let report = page.extract_characters_paint_order_report().unwrap();
    assert!(report.diagnostics.is_empty());
    assert_eq!(
        report
            .characters
            .iter()
            .map(|ch| ch.unicode.as_deref().unwrap())
            .collect::<String>(),
        "ABBB"
    );
}

#[test]
fn unicode_text_commit_rejects_unprovable_font_binding_without_mutation() {
    use pdf::document::{owned::Document, resource_manager::StandardFont};
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    for source in [
        "BT (A) Tj ET".to_string(),
        format!("/{font} 12 Tf (A) Tj"),
        format!("Q BT /{font} 12 Tf (A) Tj ET"),
        format!("BT /{font} 12 Tf PrivateOp (A) Tj ET"),
        format!("BT BT /{font} 12 Tf (A) Tj ET"),
    ] {
        doc.replace_page_contents(0, source.into_bytes()).unwrap();
        let program = doc.get_page(0).unwrap().content_program().unwrap();
        let index = program
            .operations()
            .iter()
            .position(|op| op.operator.name() == "Tj")
            .unwrap();
        let revision = doc.revision();
        assert!(doc.replace_page_text(0, &program, index, "B").is_err());
        assert_eq!(doc.revision(), revision);
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            program.source_bytes()
        );
    }
}

#[test]
fn composite_font_unicode_encoding_commits_and_strictly_round_trips() {
    use pdf::{
        document::{owned::Document, resource_manager::ResourceKind},
        io::stream_reader::StreamReader,
        objects::{
            PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream,
        },
        parser::syntax::{ParseMode, SyntaxParser},
    };
    use std::io::Cursor;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let encoding = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(PdfDict::default(), b"2 begincodespacerange <00> <7F> <8000> <80FF> endcodespacerange 2 begincidchar <41> 7 <8001> 8 endcidchar".to_vec()).unwrap())).unwrap();
    let unicode = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(PdfDict::default(), b"2 begincodespacerange <00> <7F> <8000> <80FF> endcodespacerange 2 beginbfchar <41> <4E2D> <8001> <6587> endbfchar".to_vec()).unwrap())).unwrap();
    let mut dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(b"<< /Type /Font /Subtype /Type0 /BaseFont /Test /DescendantFonts [<< /Type /Font /Subtype /CIDFontType2 /BaseFont /Test /CIDSystemInfo << /Registry (Test) /Ordering (Test) /Supplement 0 >> /DW 500 >>] >>".to_vec())).unwrap()).get_object().unwrap().into_dict().unwrap();
    dict.insert(
        "Encoding".into(),
        PdfObject::PdfReference(PdfReference::from_id(encoding)),
    );
    dict.insert(
        "ToUnicode".into(),
        PdfObject::PdfReference(PdfReference::from_id(unicode)),
    );
    let font_id = doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Font, font_id)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{name} 12 Tf 1 0 0 1 48 100 Tm <41> Tj ET").into_bytes(),
    )
    .unwrap();
    let page = doc.get_page(0).unwrap();
    let snapshot = page.content_program().unwrap();
    let codes = page.encode_text_for_font(&name, "中文").unwrap();
    assert_eq!(codes, vec![0x41, 0x80, 1]);
    let revision = doc.revision();
    assert!(page.encode_text_for_font(&name, "未映射").is_err());
    assert_eq!(doc.revision(), revision);
    doc.replace_page_text(0, &snapshot, 3, "中文").unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let mut page = reopened.get_page(0).unwrap();
    assert_eq!(page.encode_text_for_font(&name, "中文").unwrap(), codes);
    let report = page.extract_characters_paint_order_report().unwrap();
    assert!(report.diagnostics.is_empty());
    assert_eq!(
        report
            .characters
            .iter()
            .map(|ch| ch.unicode.as_deref().unwrap())
            .collect::<String>(),
        "中文"
    );
    assert_eq!(
        report
            .characters
            .iter()
            .map(|ch| ch.code_length)
            .collect::<Vec<_>>(),
        vec![1, 2]
    );
}

#[test]
fn tj_item_replacement_preserves_spacing_comments_and_other_strings() {
    let source = b"%head\n[(a\\(b\\)) -12.00 % spacing\n<0041> +3 (tail)] TJ\nq Q %end";
    let original =
        ContentProgram::from_decoded_bytes(source.to_vec(), ParseOptions::default()).unwrap();
    let edited = original
        .with_replaced_text_array_item_codes(0, 2, &[0, 66], ParseOptions::default())
        .unwrap();
    assert_eq!(
        edited.source_bytes(),
        b"%head\n[(a\\(b\\)) -12.00 % spacing\n<0042> +3 (tail)] TJ\n\nq Q %end"
    );
    let before = original.operations()[0].operator.get_as_array(0).unwrap();
    let after = edited.operations()[0].operator.get_as_array(0).unwrap();
    for index in [0, 1, 3, 4] {
        assert_eq!(before.get(index), after.get(index));
    }
    assert_eq!(after.get(2).unwrap().as_string().unwrap().bytes(), &[0, 66]);
    for index in [1, 3, 5, usize::MAX] {
        assert!(
            original
                .with_replaced_text_array_item_codes(0, index, b"X", ParseOptions::default())
                .is_err()
        );
    }
    assert!(
        original
            .with_replaced_text_array_item_codes(1, 0, b"X", ParseOptions::default())
            .is_err()
    );
    let invalid =
        ContentProgram::from_decoded_bytes(b"[(A) /Bad] TJ".to_vec(), ParseOptions::default())
            .unwrap();
    assert!(
        invalid
            .with_replaced_text_array_item_codes(0, 0, b"X", ParseOptions::default())
            .is_err()
    );
    let limits = Limits {
        max_token_bytes: 3,
        ..Default::default()
    };
    assert!(
        original
            .with_replaced_text_array_item_codes(
                0,
                2,
                &[0, 66],
                ParseOptions {
                    limits,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(original.source_bytes(), source);
    assert!(edited.source_stamp().is_none());
}

#[test]
fn tj_batch_replacement_keeps_numbers_comments_and_unselected_tokens() {
    let options = ParseOptions::default();
    let original = ContentProgram::from_decoded_bytes(
        b"%head\n[(AB) -12.00 %keep\n<4344> +3 (tail)] TJ\nq Q".to_vec(),
        options,
    )
    .unwrap();
    let edited = original
        .with_replaced_text_array_items_codes(0, &[(0, b"AX"), (2, b"")], options)
        .unwrap();
    assert_eq!(
        edited.source_bytes(),
        b"%head\n[<4158> -12.00 %keep\n<> +3 (tail)] TJ\n\nq Q"
    );
    for entries in [
        vec![(2, b"X".as_slice()), (0, b"Y".as_slice())],
        vec![(0, b"X".as_slice()), (0, b"Y".as_slice())],
        vec![(1, b"X".as_slice())],
        vec![(5, b"X".as_slice())],
    ] {
        assert!(
            original
                .with_replaced_text_array_items_codes(0, &entries, options)
                .is_err()
        );
    }
    assert_eq!(
        original.operations()[0]
            .operator
            .get_as_array(0)
            .unwrap()
            .get(0)
            .unwrap()
            .as_string()
            .unwrap()
            .bytes(),
        b"AB"
    );
}

#[test]
fn tj_batch_replacement_checks_combined_serialized_output_limits() {
    let options = ParseOptions::default();
    let source = b"[(A) 25 %keep\n(B)] TJ\nq Q";
    let program = ContentProgram::from_decoded_bytes(source.to_vec(), options).unwrap();
    let codes = b"ABCDEFGHIJKLMNOPQRST";
    let edits = [(0, codes.as_slice()), (2, codes.as_slice())];
    let expected = program
        .with_replaced_text_array_items_codes(0, &edits, options)
        .unwrap();
    let exact = expected.source_bytes().len();
    let operation_exact = expected.operations()[0].lexical_range.len();
    let exact_options = ParseOptions {
        limits: Limits {
            max_decoded_bytes: exact,
            max_raw_stream_bytes: operation_exact,
            ..options.limits
        },
        ..options
    };
    assert_eq!(
        program
            .with_replaced_text_array_items_codes(0, &edits, exact_options)
            .unwrap()
            .source_bytes(),
        expected.source_bytes()
    );
    for limits in [
        Limits {
            max_decoded_bytes: exact - 1,
            ..exact_options.limits
        },
        Limits {
            max_raw_stream_bytes: operation_exact - 1,
            ..exact_options.limits
        },
    ] {
        assert!(matches!(
            program.with_replaced_text_array_items_codes(
                0,
                &edits,
                ParseOptions { limits, ..options }
            ),
            Err(pdf::error::PdfError::ResourceLimit(_))
        ));
    }
    assert_eq!(program.source_bytes(), source);
}

#[test]
fn document_tj_item_commit_strictly_round_trips_with_existing_font() {
    use pdf::{
        document::{owned::Document, resource_manager::StandardFont},
        parser::syntax::ParseMode,
    };
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf 1 0 0 1 48 100 Tm [(A) -120.00 %keep\n(B)] TJ ET").into_bytes(),
    )
    .unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    assert!(
        doc.replace_page_text_array_item_codes(0, &snapshot, 3, 1, b"C")
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    let codes = doc
        .get_page(0)
        .unwrap()
        .encode_text_for_font(&font, "CD")
        .unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .encode_text_for_font("Missing", "A")
            .is_err()
    );
    assert!(
        doc.get_page(0)
            .unwrap()
            .encode_text_for_font(&font, "中文")
            .is_err()
    );
    assert_eq!(codes, b"CD");
    doc.replace_page_text_array_item(0, &snapshot, 3, 2, "CD")
        .unwrap();
    assert!(
        doc.replace_page_text_array_item_codes(0, &snapshot, 3, 2, b"E")
            .is_err()
    );
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let mut page = reopened.get_page(0).unwrap();
    let program = page.content_program().unwrap();
    assert!(
        program
            .source_bytes()
            .windows(b"-120.00 %keep\n".len())
            .any(|part| part == b"-120.00 %keep\n")
    );
    let report = page.extract_characters_paint_order_report().unwrap();
    assert!(report.diagnostics.is_empty());
    let text: String = report
        .characters
        .iter()
        .map(|character| character.unicode.as_deref().unwrap())
        .collect();
    assert_eq!(text, "ACD");
}

#[test]
fn document_text_commit_keeps_stream_boundaries_and_rolls_back_with_outer_transaction() {
    use pdf::{document::owned::Document, error::PdfError};
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    doc.replace_page_contents(0, b"%head\n(hel".to_vec())
        .unwrap();
    doc.append_page_contents(0, Vec::new()).unwrap();
    doc.append_page_contents(0, b"lo) Tj\n%tail".to_vec())
        .unwrap();
    let original = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    assert!(doc.replace_page_text_codes(1, &original, 0, b"BC").is_err());
    assert_eq!(doc.revision(), revision);
    let result: pdf::error::PdfResult<()> = doc.edit_transaction(|inner| {
        inner.replace_page_text_codes(0, &original, 0, b"BC")?;
        Err(PdfError::ObjectError("force outer rollback".into()))
    });
    assert!(result.is_err());
    assert!(doc.revision() > revision); // Rollback invalidates snapshots to prevent ABA.
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        original.source_bytes()
    );
    assert!(doc.replace_page_text_codes(0, &original, 0, b"BC").is_err());
    let original = doc.get_page(0).unwrap().content_program().unwrap();
    doc.replace_page_text_codes(0, &original, 0, b"BC").unwrap();
    let edited = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(edited.stream_ranges().len(), 3);
    assert_eq!(edited.stream_bytes(1), Some(&b""[..]));
    assert_eq!(edited.stream_object_id(1), original.stream_object_id(1));
    assert!(edited.source_bytes().starts_with(b"%head\n"));
    assert!(edited.source_bytes().ends_with(b"\n%tail"));
    assert_eq!(
        edited.operations()[0]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        b"BC"
    );
}

#[test]
fn replacement_retains_unknown_neighbor_operands_and_checks_output_budget() {
    let program = ContentProgram::from_decoded_bytes(
        b"3 /Private UnknownOp (old) Tj [1 2] OtherUnknown %tail".to_vec(),
        ParseOptions::default(),
    )
    .unwrap();
    let replacement = b"<00010002> Tj";
    let edited = program
        .with_replaced_operation(1, replacement, ParseOptions::default())
        .unwrap();
    for index in [0, 2] {
        assert_eq!(
            edited.operations()[index].operator.name(),
            program.operations()[index].operator.name()
        );
        for operand in 0..program.operations()[index].operator.num_operands() {
            assert_eq!(
                edited.operations()[index]
                    .operator
                    .operand(operand)
                    .unwrap(),
                program.operations()[index]
                    .operator
                    .operand(operand)
                    .unwrap()
            );
        }
    }
    let limits = Limits {
        max_decoded_bytes: edited.source_bytes().len() - 1,
        ..Default::default()
    };
    assert!(
        program
            .with_replaced_operation(
                1,
                replacement,
                ParseOptions {
                    limits,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(
        program.operations()[1]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        b"old"
    );
}

#[test]
fn encoded_text_replacement_can_be_saved_and_extracted_with_existing_font() {
    use pdf::{
        document::{owned::Document, resource_manager::StandardFont},
        parser::syntax::ParseMode,
    };
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("% preserve\nBT /{name} 12 Tf 1 0 0 1 48 100 Tm (A) Tj ET\n% tail\n").into_bytes(),
    )
    .unwrap();
    let original = doc.get_page(0).unwrap().content_program().unwrap();
    let codes = doc
        .get_page(0)
        .unwrap()
        .encode_text_for_font(&name, "BC")
        .unwrap();
    let edited = original
        .with_replaced_text_codes(3, &codes, ParseOptions::default())
        .unwrap();
    assert!(edited.source_bytes().starts_with(b"% preserve\n"));
    assert!(edited.source_bytes().ends_with(b"% tail\n"));
    assert_eq!(
        edited.operations()[3]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        b"BC"
    );
    assert!(
        original
            .with_replaced_text_codes(0, b"BC", ParseOptions::default())
            .is_err()
    );
    let foreign = Document::new().unwrap();
    foreign.add_page(0).unwrap();
    assert!(
        foreign
            .replace_page_text_codes(0, &original, 3, &codes)
            .is_err()
    );
    let offline = ContentProgram::from_decoded_bytes(
        original.source_bytes().to_vec(),
        ParseOptions::default(),
    )
    .unwrap();
    assert!(doc.replace_page_text_codes(0, &offline, 3, &codes).is_err());
    let revision = doc.revision();
    assert!(
        doc.replace_page_text_codes(0, &original, 0, &codes)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    doc.replace_page_text_codes(0, &original, 3, &codes)
        .unwrap();
    assert!(doc.replace_page_text_codes(0, &original, 3, b"D").is_err());
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let report = reopened
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    assert!(report.diagnostics.is_empty());
    assert_eq!(
        report
            .characters
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "BC"
    );
}

#[test]
fn operation_replacement_preserves_comments_streams_and_unknown_neighbors() {
    let chunks = vec![
        b"% head\nq (hel".to_vec(),
        Vec::new(),
        b"lo) CustomOp\nQ % tail\n".to_vec(),
    ];
    let program = ContentProgram::from_decoded_streams(chunks, ParseOptions::default()).unwrap();
    let edited = program
        .with_replaced_operation(
            1,
            b"(new) OtherOp % replacement comment",
            ParseOptions::default(),
        )
        .unwrap();
    assert_eq!(
        edited.source_bytes(),
        b"% head\nq (new) OtherOp % replacement comment\n\nQ % tail\n"
    );
    assert_eq!(edited.stream_ranges().len(), 3);
    assert_eq!(edited.stream_bytes(1), Some(b"".as_slice()));
    assert_eq!(edited.operations()[1].operator.name(), "OtherOp");
    assert_eq!(edited.operations()[2].operator.name(), "Q");
    assert!(edited.source_stamp().is_none());
    assert_eq!(program.operations()[1].operator.name(), "CustomOp");
    assert!(
        program
            .with_replaced_operation(1, b"q Q", ParseOptions::default())
            .is_err()
    );
    assert!(
        program
            .with_replaced_operation(1, b"", ParseOptions::default())
            .is_err()
    );
    assert!(
        program
            .with_replaced_operation(99, b"q", ParseOptions::default())
            .is_err()
    );
}

#[test]
fn source_operations_identify_borrowed_resource_names_without_guessing() {
    use pdf::page::content_program::ContentResourceKind;
    let program = ContentProgram::from_decoded_bytes(
        b"/F#31 12 Tf /Im1 Do /GS1 gs /CS1 cs /Sh1 sh /Tag /P1 BDC /Tag << /MCID 1 >> DP /DeviceRGB CS /Unknown CustomOp".to_vec(),
        ParseOptions::default(),
    ).unwrap();
    let expected = [
        (ContentResourceKind::Font, "F1", 0),
        (ContentResourceKind::XObject, "Im1", 0),
        (ContentResourceKind::ExtGState, "GS1", 0),
        (ContentResourceKind::ColorSpace, "CS1", 0),
        (ContentResourceKind::Shading, "Sh1", 0),
        (ContentResourceKind::Properties, "P1", 1),
    ];
    for (operation, (kind, name, index)) in program.operations().iter().zip(expected) {
        let reference = operation.named_resource_use().unwrap().unwrap();
        assert_eq!(
            (reference.kind, reference.name, reference.operand_index),
            (kind, name, index)
        );
    }
    for operation in &program.operations()[6..] {
        assert!(operation.named_resource_use().unwrap().is_none());
    }
    let invalid =
        ContentProgram::from_decoded_bytes(b"1 Do".to_vec(), ParseOptions::default()).unwrap();
    assert!(invalid.operations()[0].named_resource_use().is_err());
}

#[test]
fn public_page_content_program_tracks_latest_edits_and_save() {
    use pdf::{document::owned::Document, parser::syntax::ParseMode};
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let empty = doc.get_page(0).unwrap().content_program().unwrap();
    assert!(empty.operations().is_empty());
    doc.replace_page_contents(0, b"% preserved\n1 CustomOp\n".to_vec())
        .unwrap();
    let before = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(
        before.source_stamp().unwrap().document_identity(),
        &doc.identity()
    );
    assert_eq!(before.source_stamp().unwrap().revision(), doc.revision());
    assert!(before.stream_object_id(0).is_some());
    assert_eq!(before.operations()[0].operator.name(), "CustomOp");
    doc.replace_page_contents(0, b"q Q\n".to_vec()).unwrap();
    let latest = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(latest.operations().len(), 2);
    assert!(latest.source_stamp().unwrap().revision() > before.source_stamp().unwrap().revision());
    assert_eq!(before.source_bytes(), b"% preserved\n1 CustomOp\n");
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let saved = reopened.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(saved.source_bytes(), latest.source_bytes());
    assert_eq!(saved.stream_ranges(), latest.stream_ranges());
    assert_ne!(
        saved.source_stamp().unwrap().document_identity(),
        latest.source_stamp().unwrap().document_identity()
    );
}

#[test]
fn multi_stream_source_preserves_boundaries_and_cross_stream_tokens() {
    let chunks = vec![
        b"q (hel".to_vec(),
        Vec::new(),
        b"lo) CustomOp Q %tail".to_vec(),
    ];
    let program =
        ContentProgram::from_decoded_streams(chunks.clone(), ParseOptions::default()).unwrap();
    assert_eq!(program.operations().len(), 3);
    assert_eq!(
        program.operations()[1]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        b"hello"
    );
    assert_eq!(program.stream_ranges(), [0..6, 6..6, 6..26]);
    for (index, bytes) in chunks.iter().enumerate() {
        assert_eq!(program.stream_bytes(index).unwrap(), bytes);
    }
    assert!(program.stream_bytes(3).is_none());
    let fragments: Vec<_> = program.operation_source_fragments(1).unwrap().collect();
    assert_eq!(fragments.len(), 2);
    assert_eq!(fragments[0].stream_index, 0);
    assert_eq!(fragments[1].stream_index, 2);
    let mut reconstructed = Vec::new();
    for fragment in fragments {
        let bytes = &program.stream_bytes(fragment.stream_index).unwrap()[fragment.stream_range];
        assert_eq!(bytes, &program.source_bytes()[fragment.combined_range]);
        reconstructed.extend_from_slice(bytes);
    }
    assert_eq!(
        reconstructed,
        program.source_bytes()[program.operations()[1].source_range.clone()]
    );
    assert!(program.operation_source_fragments(3).is_err());
    let limits = Limits {
        max_decoded_bytes: 25,
        ..Default::default()
    };
    assert!(
        ContentProgram::from_decoded_streams(
            chunks,
            ParseOptions {
                limits,
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn page_program_collects_many_tiny_streams_with_empty_boundaries() {
    use pdf::document::owned::Document;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, Vec::new()).unwrap();
    for byte in b"q (hello) CustomOp Q" {
        doc.append_page_contents(0, vec![*byte]).unwrap();
        doc.append_page_contents(0, Vec::new()).unwrap();
    }
    let program = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(program.operations().len(), 3);
    assert_eq!(
        program.operations()[1]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        b"hello"
    );
    assert_eq!(
        program.stream_ranges().len(),
        1 + 2 * b"q (hello) CustomOp Q".len()
    );
    assert_eq!(program.stream_bytes(0), Some(b"".as_slice()));
    for (index, byte) in b"q (hello) CustomOp Q".iter().enumerate() {
        assert_eq!(
            program.stream_bytes(1 + index * 2),
            Some(std::slice::from_ref(byte))
        );
        assert_eq!(program.stream_bytes(2 + index * 2), Some(b"".as_slice()));
    }
}

#[test]
fn page_program_budget_charges_source_growth_and_retains_return_owner() {
    use pdf::{budget::OperationBudget, document::owned::Document, error::PdfError};
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"q ".to_vec()).unwrap();
    doc.append_page_contents(0, b"Q".to_vec()).unwrap();
    let revision = doc.revision();
    // Include temporary source object copies as well as operation growth.
    let metadata = 2
        * (std::mem::size_of::<std::ops::Range<usize>>()
            + std::mem::size_of::<Option<pdf::objects::object_id::ObjectId>>());
    let operation_size = std::mem::size_of::<pdf::page::content_program::SourceOperation>();
    let probe = OperationBudget::new(16384);
    let probed = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&probe)
        .unwrap();
    let peak = probe.peak_bytes();
    drop(probed);
    assert_eq!(probe.live_bytes(), 0);
    let budget = OperationBudget::new(peak);
    let neighbor = budget.reserve(1).unwrap();
    assert!(matches!(
        doc.get_page(0)
            .unwrap()
            .content_program_with_budget(&budget),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(budget.live_bytes(), 1);
    assert_eq!(doc.revision(), revision);
    drop(neighbor);
    let result = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&budget)
        .unwrap();
    assert_eq!(result.program().operations().len(), 2);
    assert_eq!(result.program().stream_ranges(), [0..2, 2..3]);
    assert_eq!(budget.live_bytes(), metadata + 4 + 2 * operation_size + 2);
    assert!(
        budget
            .reserve(budget.maximum_bytes() - budget.live_bytes() + 1)
            .is_err()
    );
    drop(result);
    assert_eq!(budget.live_bytes(), 0);
    doc.replace_page_contents(0, b"q (unterminated".to_vec())
        .unwrap();
    let budget = OperationBudget::new(64);
    assert!(
        doc.get_page(0)
            .unwrap()
            .content_program_with_budget(&budget)
            .is_err()
    );
    assert_eq!(budget.live_bytes(), 0);
}

#[test]
fn empty_streams_still_charge_program_metadata() {
    use pdf::{budget::OperationBudget, document::owned::Document};
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, Vec::new()).unwrap();
    doc.append_page_contents(0, Vec::new()).unwrap();
    let metadata = 2
        * (std::mem::size_of::<std::ops::Range<usize>>()
            + std::mem::size_of::<Option<pdf::objects::object_id::ObjectId>>());
    let probe = OperationBudget::new(16384);
    let probed = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&probe)
        .unwrap();
    let peak = probe.peak_bytes();
    assert!(peak > metadata);
    drop(probed);
    assert_eq!(probe.live_bytes(), 0);
    let too_small = OperationBudget::new(peak - 1);
    assert!(
        doc.get_page(0)
            .unwrap()
            .content_program_with_budget(&too_small)
            .is_err()
    );
    assert_eq!(too_small.live_bytes(), 0);
    let budget = OperationBudget::new(peak);
    let result = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&budget)
        .unwrap();
    assert_eq!(result.program().stream_ranges(), [0..0, 0..0]);
    assert_eq!(budget.live_bytes(), metadata);
    drop(result);
    assert_eq!(budget.live_bytes(), 0);
}

#[test]
fn numeric_operand_tables_remain_charged_with_page_program() {
    use pdf::{budget::OperationBudget, document::owned::Document};
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"1 2 op".to_vec()).unwrap();
    let metadata = std::mem::size_of::<std::ops::Range<usize>>()
        + std::mem::size_of::<Option<pdf::objects::object_id::ObjectId>>();
    let object_size = std::mem::size_of::<pdf::objects::PdfObject>();
    let operation_size = std::mem::size_of::<pdf::page::content_program::SourceOperation>();
    let parsing_peak =
        6 + metadata + (3 * object_size + 1).max(2 * object_size + operation_size + 4);
    let probe = OperationBudget::new(16384);
    let probed = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&probe)
        .unwrap();
    let peak = probe.peak_bytes();
    assert!(peak >= parsing_peak);
    drop(probed);
    assert_eq!(probe.live_bytes(), 0);
    let budget = OperationBudget::new(peak);
    let neighbor = budget.reserve(1).unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .content_program_with_budget(&budget)
            .is_err()
    );
    assert_eq!(budget.live_bytes(), 1);
    drop(neighbor);
    let result = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&budget)
        .unwrap();
    assert_eq!(result.program().operations()[0].operator.num_operands(), 2);
    assert_eq!(
        budget.live_bytes(),
        6 + metadata + 2 * object_size + operation_size + 4
    );
    drop(result);
    assert_eq!(budget.live_bytes(), 0);
}

#[test]
fn numeric_lexeme_capacity_and_reference_tokens_have_distinct_lifetimes() {
    use pdf::{budget::OperationBudget, document::owned::Document};
    for (source, retained_payload) in [
        (b"+.5 0.12345678901234567890 op".as_slice(), 4 + 32),
        (b"12 34 R op".as_slice(), 0),
    ] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, source.to_vec()).unwrap();
        let budget = OperationBudget::new(4096);
        let program = doc
            .get_page(0)
            .unwrap()
            .content_program_with_budget(&budget)
            .unwrap();
        let operator = &program.program().operations()[0].operator;
        let operand_capacity = if retained_payload == 0 { 1 } else { 2 };
        assert_eq!(operator.num_operands(), operand_capacity);
        if retained_payload != 0 {
            assert_eq!(
                operator.operand(0).unwrap().as_number().unwrap().lexeme(),
                "+.5"
            );
            assert_eq!(
                operator.operand(1).unwrap().as_number().unwrap().lexeme(),
                "0.12345678901234567890"
            );
        }
        let expected = source.len()
            + std::mem::size_of::<std::ops::Range<usize>>()
            + std::mem::size_of::<Option<pdf::objects::object_id::ObjectId>>()
            + operand_capacity * std::mem::size_of::<pdf::objects::PdfObject>()
            + std::mem::size_of::<pdf::page::content_program::SourceOperation>()
            + 2
            + retained_payload;
        assert_eq!(budget.live_bytes(), expected);
        drop(program);
        assert_eq!(budget.live_bytes(), 0);
    }
}

#[test]
fn nested_array_payload_and_tables_remain_charged_with_program() {
    use pdf::{budget::OperationBudget, document::owned::Document};
    let source = b"[[(abc) /A#ff] 12 true null] op";
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, source.to_vec()).unwrap();
    let budget = OperationBudget::new(8192);
    let program = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&budget)
        .unwrap();
    assert_eq!(
        program.program().operations()[0]
            .operator
            .get_as_array(0)
            .unwrap()
            .len(),
        4
    );
    let expected = source.len()
        + std::mem::size_of::<std::ops::Range<usize>>()
        + std::mem::size_of::<Option<pdf::objects::object_id::ObjectId>>()
        + 7 * std::mem::size_of::<pdf::objects::PdfObject>()
        + std::mem::size_of::<pdf::page::content_program::SourceOperation>()
        + 2
        + 12;
    assert_eq!(budget.live_bytes(), expected);
    drop(program);
    assert_eq!(budget.live_bytes(), 0);
}

#[test]
fn dictionary_replacement_keeps_only_final_payload_charged() {
    use pdf::{budget::OperationBudget, document::owned::Document};
    let source = b"<< /A (abcdefgh) /A (x) >> op";
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, source.to_vec()).unwrap();
    let budget = OperationBudget::new(8192);
    let program = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&budget)
        .unwrap();
    let dictionary = program.program().operations()[0]
        .operator
        .operand(0)
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(
        dictionary.get("A").unwrap().as_string().unwrap().bytes(),
        b"x"
    );
    let expected = source.len()
        + std::mem::size_of::<std::ops::Range<usize>>()
        + std::mem::size_of::<Option<pdf::objects::object_id::ObjectId>>()
        + std::mem::size_of::<pdf::objects::PdfObject>()
        + std::mem::size_of::<pdf::page::content_program::SourceOperation>()
        + std::mem::size_of::<(String, pdf::objects::PdfObject)>()
        + 2
        + 2;
    assert_eq!(budget.live_bytes(), expected);
    drop(program);
    assert_eq!(budget.live_bytes(), 0);
}

#[test]
fn rebuilt_operation_program_retains_budget_and_accounts_fragment_overlap() {
    use pdf::budget::OperationBudget;
    let options = ParseOptions::default();
    let original = ContentProgram::from_decoded_bytes(b"q Q".to_vec(), options).unwrap();
    let replacement = b"(abc) Tj";
    let legacy = original
        .with_replaced_operation(0, replacement, options)
        .unwrap();
    let probe = OperationBudget::new(32768);
    let result = original
        .with_replaced_operation_with_budget(0, replacement, options, &probe)
        .unwrap();
    assert_eq!(result.program().source_bytes(), legacy.source_bytes());
    assert_eq!(result.program().stream_ranges(), legacy.stream_ranges());
    assert!(result.program().source_stamp().is_none());
    let final_bytes = result.program().source_bytes().len()
        + std::mem::size_of::<std::ops::Range<usize>>()
        + 2 * std::mem::size_of::<pdf::page::content_program::SourceOperation>()
        + std::mem::size_of::<pdf::objects::PdfObject>()
        + 4
        + 3;
    assert_eq!(probe.live_bytes(), final_bytes);
    let peak = probe.peak_bytes();
    assert!(peak > final_bytes);
    drop(result);
    assert_eq!(probe.live_bytes(), 0);
    let budget = OperationBudget::new(peak);
    let neighbor = budget.reserve(1).unwrap();
    assert!(
        original
            .with_replaced_operation_with_budget(0, replacement, options, &budget)
            .is_err()
    );
    assert_eq!(budget.live_bytes(), 1);
    assert_eq!(original.source_bytes(), b"q Q");
    drop(neighbor);
    let result = original
        .with_replaced_operation_with_budget(0, replacement, options, &budget)
        .unwrap();
    assert_eq!(budget.live_bytes(), final_bytes);
    assert_eq!(budget.peak_bytes(), peak);
    drop(result);
    assert_eq!(budget.live_bytes(), 0);
    for replacement in [b"q Q".as_slice(), b"(unterminated".as_slice()] {
        assert!(
            original
                .with_replaced_operation_with_budget(0, replacement, options, &budget)
                .is_err()
        );
        assert_eq!(budget.live_bytes(), 0);
    }
}

#[test]
fn budgeted_text_builder_and_rebuilt_program_share_live_quota() {
    use pdf::{budget::OperationBudget, page::content_builder::ContentBuilder};
    let options = ParseOptions::default();
    let original = ContentProgram::from_decoded_bytes(b"q Q".to_vec(), options).unwrap();
    let budget = OperationBudget::new(16384);
    let replacement =
        ContentBuilder::encoded_text_with_budget(b"abc", options.limits, &budget).unwrap();
    assert_eq!(budget.live_bytes(), replacement.capacity());
    let result = original
        .with_replaced_operation_with_budget(0, replacement.as_slice(), options, &budget)
        .unwrap();
    assert_eq!(
        result.program().operations()[0]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        b"abc"
    );
    assert!(budget.live_bytes() > replacement.capacity());
    drop(result);
    assert_eq!(budget.live_bytes(), replacement.capacity());
    drop(replacement);
    assert_eq!(budget.live_bytes(), 0);
}

#[test]
fn budgeted_text_replacement_preserves_tj_and_quote_operations() {
    use pdf::budget::OperationBudget;
    let options = ParseOptions::default();
    for source in [
        b"%head\n(old) Tj q".as_slice(),
        b"%head\n(old) ' q".as_slice(),
        b"%head\n2.00 3.0 (old) \" q".as_slice(),
    ] {
        let original = ContentProgram::from_decoded_bytes(source.to_vec(), options).unwrap();
        let legacy = original
            .with_replaced_text_codes(0, &[0, 255], options)
            .unwrap();
        let probe = OperationBudget::new(32768);
        let result = original
            .with_replaced_text_codes_with_budget(0, &[0, 255], options, &probe)
            .unwrap();
        assert_eq!(result.program().source_bytes(), legacy.source_bytes());
        assert_eq!(
            result.program().operations()[0].operator.name(),
            original.operations()[0].operator.name()
        );
        let string_index = if source.contains(&b'"') { 2 } else { 0 };
        assert_eq!(
            result.program().operations()[0]
                .operator
                .get_as_string(string_index)
                .unwrap()
                .bytes(),
            &[0, 255]
        );
        if string_index == 2 {
            assert_eq!(
                result.program().operations()[0]
                    .operator
                    .operand(0)
                    .unwrap()
                    .as_number()
                    .unwrap()
                    .lexeme(),
                "2.00"
            );
        }
        let final_bytes = probe.live_bytes();
        let peak = probe.peak_bytes();
        assert!(peak > final_bytes);
        drop(result);
        assert_eq!(probe.live_bytes(), 0);
        let budget = OperationBudget::new(peak);
        let neighbor = budget.reserve(1).unwrap();
        assert!(
            original
                .with_replaced_text_codes_with_budget(0, &[0, 255], options, &budget)
                .is_err()
        );
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let result = original
            .with_replaced_text_codes_with_budget(0, &[0, 255], options, &budget)
            .unwrap();
        assert_eq!(budget.live_bytes(), final_bytes);
        drop(result);
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(original.source_bytes(), source);
    }
}

#[test]
fn budgeted_tj_replacement_preserves_spacing_and_cross_stream_ranges() {
    use pdf::budget::OperationBudget;
    let options = ParseOptions::default();
    let original = ContentProgram::from_decoded_streams(
        vec![
            b"[(a".to_vec(),
            b"bc) 25.00 ".to_vec(),
            b"() -10 (z)] TJ q".to_vec(),
        ],
        options,
    )
    .unwrap();
    let replacements = [(0, &[0, 255][..]), (4, &b"xy"[..])];
    let legacy = original
        .with_replaced_text_array_items_codes(0, &replacements, options)
        .unwrap();
    let probe = OperationBudget::new(32768);
    let result = original
        .with_replaced_text_array_items_codes_with_budget(0, &replacements, options, &probe)
        .unwrap();
    assert_eq!(result.program().source_bytes(), legacy.source_bytes());
    assert_eq!(result.program().stream_ranges(), legacy.stream_ranges());
    let array = result.program().operations()[0]
        .operator
        .get_as_array(0)
        .unwrap();
    assert_eq!(
        array.get(0).unwrap().as_string().unwrap().bytes(),
        &[0, 255]
    );
    assert_eq!(array.get(1).unwrap().as_number().unwrap().lexeme(), "25.00");
    assert_eq!(array.get(2).unwrap().as_string().unwrap().bytes(), b"");
    let final_bytes = probe.live_bytes();
    let peak = probe.peak_bytes();
    drop(result);
    assert_eq!(probe.live_bytes(), 0);
    let budget = OperationBudget::new(peak);
    let neighbor = budget.reserve(1).unwrap();
    assert!(
        original
            .with_replaced_text_array_items_codes_with_budget(0, &replacements, options, &budget)
            .is_err()
    );
    assert_eq!(budget.live_bytes(), 1);
    drop(neighbor);
    let result = original
        .with_replaced_text_array_items_codes_with_budget(0, &replacements, options, &budget)
        .unwrap();
    assert_eq!(budget.live_bytes(), final_bytes);
    drop(result);
    assert_eq!(budget.live_bytes(), 0);
    for invalid in [vec![(1, &b"x"[..])], vec![(0, &b"x"[..]), (0, &b"y"[..])]] {
        assert!(
            original
                .with_replaced_text_array_items_codes_with_budget(0, &invalid, options, &budget)
                .is_err()
        );
        assert_eq!(budget.live_bytes(), 0);
    }
}

#[test]
fn partial_program_parse_failure_releases_accumulated_operand_tables() {
    use pdf::{
        budget::{BudgetedBytes, OperationBudget},
        document::owned::Document,
    };
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"1 2 op 3 4 op (unterminated".to_vec())
        .unwrap();
    let revision = doc.revision();
    let budget = OperationBudget::new(16 * 1024);
    let neighbor = BudgetedBytes::copy_from_slice(&budget, b"neighbor").unwrap();
    let retained = budget.live_bytes();
    assert!(
        doc.get_page(0)
            .unwrap()
            .content_program_with_budget(&budget)
            .is_err()
    );
    assert_eq!(budget.live_bytes(), retained);
    assert_eq!(neighbor.as_slice(), b"neighbor");
    assert_eq!(doc.revision(), revision);
    doc.replace_page_contents(0, b"1 op".to_vec()).unwrap();
    let program = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&budget)
        .unwrap();
    assert_eq!(program.program().operations().len(), 1);
    drop(program);
    assert_eq!(budget.live_bytes(), retained);
    drop(neighbor);
    assert_eq!(budget.live_bytes(), 0);
}

#[test]
fn direct_literal_and_hex_string_payloads_keep_capacity_charged() {
    use pdf::{budget::OperationBudget, document::owned::Document};
    for source in [b"(abc) Tj".as_slice(), b"<616263> Tj".as_slice()] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, source.to_vec()).unwrap();
        let budget = OperationBudget::new(4096);
        let program = doc
            .get_page(0)
            .unwrap()
            .content_program_with_budget(&budget)
            .unwrap();
        assert_eq!(
            program.program().operations()[0]
                .operator
                .get_as_string(0)
                .unwrap()
                .bytes(),
            b"abc"
        );
        let metadata = std::mem::size_of::<std::ops::Range<usize>>()
            + std::mem::size_of::<Option<pdf::objects::object_id::ObjectId>>();
        let expected = source.len()
            + metadata
            + std::mem::size_of::<pdf::objects::PdfObject>()
            + std::mem::size_of::<pdf::page::content_program::SourceOperation>()
            + 2
            + 4;
        assert_eq!(budget.live_bytes(), expected);
        drop(program);
        assert_eq!(budget.live_bytes(), 0);
    }
}

#[test]
fn direct_name_payload_keeps_binary_and_lossy_string_charged() {
    use pdf::{budget::OperationBudget, document::owned::Document};
    let source = b"/A#ff Do";
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, source.to_vec()).unwrap();
    let budget = OperationBudget::new(4096);
    let program = doc
        .get_page(0)
        .unwrap()
        .content_program_with_budget(&budget)
        .unwrap();
    let name = program.program().operations()[0]
        .operator
        .get_as_name(0)
        .unwrap();
    assert_eq!(name.bytes(), &[b'A', 255]);
    assert_eq!(name.name(), "A\u{fffd}");
    let expected = source.len()
        + std::mem::size_of::<std::ops::Range<usize>>()
        + std::mem::size_of::<Option<pdf::objects::object_id::ObjectId>>()
        + std::mem::size_of::<pdf::objects::PdfObject>()
        + std::mem::size_of::<pdf::page::content_program::SourceOperation>()
        + 2
        + 2
        + 4;
    assert_eq!(budget.live_bytes(), expected);
    drop(program);
    assert_eq!(budget.live_bytes(), 0);
}

#[test]
fn source_program_preserves_unknown_operations_and_lexical_bytes() {
    let bytes = b"% leading\nq\n/F#31 12.00 Tf (a\\(b\\)) CustomOp\nQ % tail\n".to_vec();
    let program =
        ContentProgram::from_decoded_bytes(bytes.clone(), ParseOptions::default()).unwrap();
    assert_eq!(program.source_bytes(), bytes);
    assert_eq!(
        &program.source_bytes()[program.operations()[0].lexical_range.clone()],
        b"q"
    );
    assert_eq!(program.operations().len(), 4);
    assert_eq!(
        program.operations()[1]
            .operator
            .get_as_name(0)
            .unwrap()
            .name(),
        "F1"
    );
    assert_eq!(program.operations()[2].operator.name(), "CustomOp");
    assert_eq!(
        program.operations()[2]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        b"a(b)"
    );
    for operation in program.operations() {
        let offset = operation.operator.byte_offset().unwrap() as usize;
        assert!(operation.source_range.contains(&offset));
        assert!(operation.lexical_range.contains(&offset));
        assert!(
            program.source_bytes()[operation.source_range.clone()]
                .ends_with(operation.operator.name().as_bytes())
        );
    }
    let limits = Limits {
        max_content_operations: 3,
        ..Default::default()
    };
    assert!(
        ContentProgram::from_decoded_bytes(
            bytes,
            ParseOptions {
                limits,
                ..Default::default()
            }
        )
        .is_err()
    );
    assert!(ContentProgram::from_decoded_bytes(b"1 2".to_vec(), ParseOptions::default()).is_err());
}
