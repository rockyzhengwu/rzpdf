use pdf::{
    document::{owned::Document, resource_manager::StandardFont},
    parser::syntax::ParseMode,
};

#[test]
fn substring_edit_checks_final_character_count_before_committing() {
    use pdf::{
        error::PdfError,
        parser::options::{Limits, ParseOptions},
    };
    let doc = Document::new_with_options(ParseOptions {
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
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(0, format!("BT /{font} 12 Tf (ABC) Tj ET").into_bytes())
        .unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    // Old and inserted strings each fit the limit; their combined result does not.
    assert!(matches!(
        doc.replace_page_text_range(0, &snapshot, 2, None, 1..1, "DEF"),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(doc.revision(), revision);
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        snapshot.source_bytes()
    );
    doc.replace_page_text_range(0, &snapshot, 2, None, 1..2, "D")
        .unwrap();
    let reopened = Document::from_bytes_with_options(
        doc.save_to_bytes().unwrap(),
        ParseOptions {
            mode: ParseMode::Strict,
            limits: Limits {
                max_text_chars: 3,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let chars = reopened
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap()
        .characters;
    assert_eq!(
        chars
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>(),
        "ADC"
    );
}

#[test]
fn mixed_width_cmap_range_edit_validates_actual_character_boundaries() {
    use pdf::{
        document::resource_manager::ResourceKind,
        io::stream_reader::StreamReader,
        objects::{
            PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream,
        },
        parser::syntax::SyntaxParser,
    };
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let mut parser = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
        b"<< /Type /Font /Subtype /Type0 /BaseFont /Mixed /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /Mixed /DW 500 >>] >>".to_vec(),
    )).unwrap());
    let mut font = parser.get_object().unwrap().into_dict().unwrap();
    for (name, bytes) in [
        ("Encoding", b"2 begincodespacerange <00> <7F> <8100> <81FF> endcodespacerange 3 begincidchar <41> 1 <8101> 2 <42> 3 endcidchar".as_slice()),
        ("ToUnicode", b"3 beginbfchar <41> <0041> <8101> <0042> <42> <0043> endbfchar".as_slice()),
    ] {
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(PdfDict::default(),bytes.to_vec()))).unwrap();
        font.try_insert(name.into(),PdfObject::PdfReference(PdfReference::from_id(id))).unwrap();
    }
    let id = doc.try_insert_object(PdfObject::PdfDict(font)).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Font, id)
        .unwrap();
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf <41810142> Tj ET").into_bytes())
        .unwrap();
    let chars = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap()
        .characters;
    assert_eq!(
        chars
            .iter()
            .map(|ch| ch.decoded_code_range.clone().unwrap())
            .collect::<Vec<_>>(),
        [0..1, 1..3, 3..4]
    );
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    for range in [1..2, 2..3, 2..2] {
        assert!(
            doc.replace_page_text_range(0, &snapshot, 2, None, range, "A")
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
    }
    doc.replace_page_text_range(0, &snapshot, 2, None, 1..3, "A")
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let program = document.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(
            program.operations()[2]
                .operator
                .get_as_string(0)
                .unwrap()
                .bytes(),
            b"AAB"
        );
        let chars = document
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap()
            .characters;
        assert_eq!(
            chars
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AAC"
        );
    }
}

#[test]
fn substring_insertion_deletion_and_empty_tj_item_keep_spacing() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf [(AB) 25 () -10 (C)] TJ ET").into_bytes(),
    )
    .unwrap();
    for (item, range, text, expected) in [
        (2, 0..0, "XY", "ABXYC"),
        (0, 1..2, "", "AXYC"),
        (2, 0..2, "", "AC"),
        (0, 0..1, "", "C"),
    ] {
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        doc.replace_page_text_range(0, &snapshot, 2, Some(item), range, text)
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let mut page = reopened.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let array = program.operations()[2].operator.get_as_array(0).unwrap();
        assert_eq!(array.len(), 5);
        assert_eq!(array.get(1).unwrap().as_f32(), Some(25.));
        assert_eq!(array.get(3).unwrap().as_f32(), Some(-10.));
        let chars = page
            .extract_characters_paint_order_report()
            .unwrap()
            .characters;
        assert_eq!(
            chars
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            expected
        );
    }
}

#[test]
fn substring_edit_does_not_interpret_unrelated_unsupported_graphics() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf (ABCD) Tj ET /NoSuchImage Do").into_bytes(),
    )
    .unwrap();
    assert!(doc.get_page(0).unwrap().display_list().is_err());
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    doc.replace_page_text_range(0, &snapshot, 2, None, 1..3, "XY")
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let program = reopened.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(
        program.operations()[2]
            .operator
            .get_as_string(0)
            .unwrap()
            .bytes(),
        b"AXYD"
    );
    assert!(program.source_bytes().ends_with(b" ET /NoSuchImage Do"));
}

#[test]
fn substring_edit_preserves_suffix_tj_spacing_quotes_and_shared_page() {
    for (target, item) in [
        ("(ABCD) Tj", None),
        ("[(ABCD) 25 (E)] TJ", Some(0)),
        ("(ABCD) '", None),
        ("1 2 (ABCD) \"", None),
    ] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        doc.replace_page_contents(0, format!("BT /{font} 12 Tf {target} ET").into_bytes())
            .unwrap();
        doc.copy_page(0, 1).unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let revision = doc.revision();
        for range in [4..5, 3..2] {
            assert!(
                doc.replace_page_text_range(0, &snapshot, 2, item, range, "X")
                    .is_err()
            );
            assert_eq!(doc.revision(), revision);
        }
        doc.replace_page_text_range(0, &snapshot, 2, item, 1..3, "XY")
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let chars = document
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap()
                .characters;
            let text: String = chars
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect();
            assert_eq!(text, if item.is_some() { "AXYDE" } else { "AXYD" });
            assert_eq!(
                document
                    .get_page(1)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                snapshot.source_bytes()
            );
            let program = document.get_page(0).unwrap().content_program().unwrap();
            assert_eq!(
                program.operations()[2].operator.name(),
                snapshot.operations()[2].operator.name()
            );
            if item.is_some() {
                assert_eq!(
                    program.operations()[2]
                        .operator
                        .get_as_array(0)
                        .unwrap()
                        .get(1)
                        .unwrap()
                        .as_f32(),
                    Some(25.)
                );
            }
        }
    }
}

#[test]
fn decoded_character_ranges_ignore_literal_escapes_and_reset_for_tj_items() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!(r"BT /{font} 12 Tf (A\102\(C\)) Tj [<4445> 20 (F)] TJ ET").into_bytes(),
    )
    .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let mut page = document.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let report = page.extract_characters_paint_order_report().unwrap();
        assert_eq!(report.characters.len(), 8);
        for (character, range) in
            report
                .characters
                .iter()
                .zip([0..1, 1..2, 2..3, 3..4, 4..5, 0..1, 1..2, 0..1])
        {
            assert_eq!(character.decoded_code_range, Some(range.clone()));
            let operation =
                &program.operations()[character.source.unwrap().operation_index].operator;
            let string = if let Some(item) = character.array_item_index {
                operation
                    .get_as_array(0)
                    .unwrap()
                    .get(item)
                    .unwrap()
                    .as_string()
                    .unwrap()
            } else {
                operation.get_as_string(0).unwrap()
            };
            assert_eq!(string.bytes()[range][0] as u32, character.charcode);
        }
    }
}

#[test]
fn character_and_run_reports_require_matching_page_and_revision() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let mut page = doc.get_page(0).unwrap();
    let snapshot = page.content_program().unwrap();
    let display = page.display_list().unwrap();
    assert!(display.matches_program(&snapshot));
    let characters = page.extract_characters_paint_order_report().unwrap();
    assert_eq!(display.source_stamp(), Some(characters.source_stamp()));
    let manual = pdf::page::display_list::DisplayList::new(10., 10., Vec::new());
    assert_eq!(manual.source_stamp(), None);
    assert!(!manual.matches_program(&snapshot));
    let runs = page.extract_text_runs_paint_order_report().unwrap();
    assert!(characters.matches_program(&snapshot));
    assert!(runs.matches_program(&snapshot));
    assert_eq!(characters.source_stamp(), runs.source_stamp());
    let other_page = doc.get_page(1).unwrap().content_program().unwrap();
    let offline = pdf::page::content_program::ContentProgram::from_decoded_bytes(
        Vec::new(),
        Default::default(),
    )
    .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let foreign = reopened.get_page(0).unwrap().content_program().unwrap();
    for program in [&other_page, &offline, &foreign] {
        assert!(!display.matches_program(program));
        assert!(!characters.matches_program(program));
        assert!(!runs.matches_program(program));
    }
    doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
    let fresh = doc.get_page(0).unwrap().content_program().unwrap();
    assert!(!display.matches_program(&fresh));
    assert!(!doc.is_stamp_current(display.source_stamp().unwrap()));
    assert!(!characters.matches_program(&fresh));
    assert!(!runs.matches_program(&fresh));
    assert!(!doc.is_stamp_current(characters.source_stamp()));
    assert!(!doc.is_stamp_current(runs.source_stamp()));
    assert!(characters.matches_program(&snapshot));
    assert!(runs.matches_program(&snapshot));
}

#[test]
fn quote_text_replacement_preserves_line_and_spacing_effects() {
    for operation in ["(A) '", "1.25 % word\n2.5 (A) \""] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{font} 12 Tf 14 TL 1 0 0 1 20 80 Tm {operation} (C) Tj ET").into_bytes(),
        )
        .unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let original = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap();
        let index = original.characters[0].source.unwrap().operation_index;
        doc.replace_page_text(0, &snapshot, index, "B").unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let mut page = document.get_page(0).unwrap();
            let program = page.content_program().unwrap();
            assert_eq!(
                program.operations()[index].operator.name(),
                snapshot.operations()[index].operator.name()
            );
            let report = page.extract_characters_paint_order_report().unwrap();
            assert_eq!(report.characters[0].unicode.as_deref(), Some("B"));
            assert_eq!(report.characters[1].unicode.as_deref(), Some("C"));
            for (before, after) in original.characters.iter().zip(&report.characters) {
                assert_eq!(before.position.x(), after.position.x());
                assert_eq!(before.position.y(), after.position.y());
            }
            if operation.ends_with('"') {
                assert_eq!(
                    program.operations()[index].operator.get_as_f32(0).unwrap(),
                    1.25
                );
                assert_eq!(
                    program.operations()[index].operator.get_as_f32(1).unwrap(),
                    2.5
                );
                assert!(
                    program
                        .source_bytes()
                        .windows(b"1.25 % word\n2.5".len())
                        .any(|s| s == b"1.25 % word\n2.5")
                );
            }
        }
    }
}

#[test]
fn text_sources_identify_tj_array_entries_and_quote_operators() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf (A) Tj [(BC) 20 () -10 (D)").into_bytes(),
    )
    .unwrap();
    doc.append_page_contents(0, b"] TJ (E) ' 1 2 (F) \" ET".to_vec())
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let mut page = document.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let report = page.extract_characters_paint_order_report().unwrap();
        assert_eq!(report.characters.len(), 6);
        for (character, (index, array_item)) in report.characters.iter().zip([
            (2, None),
            (3, Some(0)),
            (3, Some(0)),
            (3, Some(4)),
            (4, None),
            (5, None),
        ]) {
            let source = character.source.unwrap();
            assert_eq!(source.operation_index, index);
            assert_eq!(
                source.operator_byte_offset,
                program.operations()[index].operator.byte_offset()
            );
            assert_eq!(character.array_item_index, array_item);
        }
        let runs = page.extract_text_runs_paint_order_report().unwrap();
        assert_eq!(runs.runs.len(), 4);
        assert!(
            runs.runs
                .iter()
                .all(|run| run.characters.iter().all(|ch| ch.source == run.source))
        );
    }
    let mut page = doc.get_page(0).unwrap();
    let snapshot = page.content_program().unwrap();
    let characters = page
        .extract_characters_paint_order_report()
        .unwrap()
        .characters;
    let chosen = &characters[3];
    doc.replace_page_text_array_item(
        0,
        &snapshot,
        chosen.source.unwrap().operation_index,
        chosen.array_item_index.unwrap(),
        "XY",
    )
    .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let report = reopened
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    let text: String = report
        .characters
        .iter()
        .filter_map(|ch| ch.unicode.as_deref())
        .collect();
    assert_eq!(text, "ABCXYEF");
}
