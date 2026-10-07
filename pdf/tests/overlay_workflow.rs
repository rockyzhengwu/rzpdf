use pdf::{
    document::{owned::Document, page_overlay::PageNumberFormat, resource_manager::StandardFont},
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};

#[test]
fn single_column_lines_use_anchor_tolerance_and_stable_ties() {
    use pdf::parser::options::Limits;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(0, format!(
        "BT /{name} 10 Tf 1 0 0 1 100 100 Tm (A) Tj 1 0 0 1 50 99 Tm (B) Tj 1 0 0 1 0 98 Tm (C) Tj 1 0 0 1 100 100 Tm (D) Tj ET"
    ).into_bytes()).unwrap();
    let mut report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    let bytes = report.characters.len() * std::mem::size_of::<usize>();
    let mut limits = Limits {
        max_decoded_bytes: bytes,
        ..Default::default()
    };
    assert_eq!(
        report
            .single_column_reading_order_indices(1.0, &limits)
            .unwrap(),
        [1, 0, 3, 2]
    );
    assert_eq!(
        report
            .single_column_reading_order_indices(0.0, &limits)
            .unwrap(),
        [0, 3, 1, 2]
    );
    limits.max_decoded_bytes -= 1;
    assert!(matches!(
        report.single_column_reading_order_indices(1.0, &limits),
        Err(PdfError::ResourceLimit(_))
    ));
    report.characters[0].font_size = -10.0;
    assert!(matches!(
        report.single_column_reading_order_indices(1.0, &Limits::default()),
        Err(PdfError::UnsupportedFeature(_))
    ));
}

#[test]
fn explicit_single_column_order_is_distinct_from_paint_order() {
    use pdf::parser::options::Limits;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(0, format!(
        "BT /{name} 10 Tf 1 0 0 1 48 80 Tm (C) Tj 1 0 0 1 80 100 Tm (B) Tj 1 0 0 1 48 100 Tm (A) Tj ET"
    ).into_bytes()).unwrap();
    let mut report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    let paint: String = report
        .characters
        .iter()
        .filter_map(|ch| ch.unicode.as_deref())
        .collect();
    assert_eq!(paint, "CBA");
    let order = report
        .single_column_reading_order_indices(1.0, &Limits::default())
        .unwrap();
    assert_eq!(order, [2, 1, 0]);
    assert_eq!(report.characters[0].unicode.as_deref(), Some("C"));
    assert!(
        report
            .single_column_reading_order_indices(f32::NAN, &Limits::default())
            .is_err()
    );
    assert!(
        report
            .single_column_reading_order_indices(
                1.0,
                &Limits {
                    max_text_chars: 2,
                    ..Default::default()
                }
            )
            .is_err()
    );
    report.characters[0].horizontal_scale = -1.0;
    assert!(
        report
            .single_column_reading_order_indices(1.0, &Limits::default())
            .is_err()
    );
}

#[test]
fn extraction_writing_axis_tracks_negative_and_zero_horizontal_scale() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    for (percent, factor) in [(-50, -0.5), (0, 0.0), (100, 1.0)] {
        doc.replace_page_contents(
            0,
            format!("BT /{name} 10 Tf {percent} Tz 1 0 0 1 48 100 Tm (II) Tj ET").into_bytes(),
        )
        .unwrap();
        let chars = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(chars[0].horizontal_scale, factor);
        assert_eq!(chars[0].writing_axis(), Some([factor, 0.0]));
        let delta = chars[1].position.x() - chars[0].position.x();
        if factor < 0.0 {
            assert!(delta < 0.0);
        } else if factor == 0.0 {
            assert_eq!(delta, 0.0);
        } else {
            assert!(delta > 0.0);
        }
    }
}

#[test]
fn text_extraction_report_retains_unknown_operator_warning() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("RzpdfUnknown BT /{name} 12 Tf 1 0 0 1 48 100 Tm (A) Tj ET").into_bytes(),
    )
    .unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    assert_eq!(report.characters.len(), 1);
    assert_eq!(report.characters[0].unicode.as_deref(), Some("A"));
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].operator, "RzpdfUnknown");
    assert!(report.diagnostics[0].byte_offset.is_some());
    let runs = doc
        .get_page(0)
        .unwrap()
        .extract_text_runs_paint_order_report()
        .unwrap();
    assert_eq!(runs.diagnostics, report.diagnostics);
    assert_eq!(runs.runs.len(), 1);
    assert_eq!(runs.runs[0].characters[0].unicode.as_deref(), Some("A"));
    assert_eq!(runs.runs[0].object_index, report.characters[0].object_index);
    let strict =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert!(
        strict
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .is_err()
    );
}

#[test]
fn graphics_restore_preserves_text_cursor_but_restores_text_parameters() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(0, format!(
        "BT /{name} 10 Tf 1 0 0 1 48 100 Tm (I) Tj q 50 Tz 10 Ts 5 Tc (I) Tj Q [(I)] TJ (I) Tj ET"
    ).into_bytes()).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let chars = document
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(chars.len(), 4);
        assert_eq!(chars[0].horizontal_scale, 1.0);
        assert_eq!(chars[1].horizontal_scale, 0.5);
        assert_eq!(chars[2].horizontal_scale, 1.0);
        assert_eq!(chars[1].writing_axis(), Some([0.5, 0.0]));
        let first_advance = chars[1].position.x() - chars[0].position.x();
        let scaled_advance = chars[2].position.x() - chars[1].position.x();
        let restored_advance = chars[3].position.x() - chars[2].position.x();
        assert!(first_advance > 0.0);
        assert!((scaled_advance - (first_advance + 5.0) * 0.5).abs() < 0.0001);
        assert!((restored_advance - first_advance).abs() < 0.0001);
        assert_eq!(chars[1].position.y(), chars[0].position.y() - 10.0);
        assert_eq!(chars[2].position.y(), chars[0].position.y());
        assert!(chars[2].position.x() > chars[1].position.x());
    }
}

#[test]
fn word_spacing_depends_on_single_byte_code_not_unicode() {
    use pdf::{
        document::resource_manager::ResourceKind,
        io::stream_reader::StreamReader,
        objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
        parser::syntax::SyntaxParser,
    };
    for (length, codes, expected) in [(1, "204141", [17.0, 5.0]), (2, "002000410041", [5.0, 5.0])] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let mut parser = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
            b"<< /Type /Font /Subtype /Type0 /BaseFont /SpacingTest /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /SpacingTest /DW 500 >>] >>".to_vec(),
        )).unwrap());
        let mut font = parser.get_object().unwrap().into_dict().unwrap();
        let (low, high, space, a) = if length == 1 {
            ("00", "FF", "20", "41")
        } else {
            ("0000", "FFFF", "0020", "0041")
        };
        font.try_insert("Encoding".into(), PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(), format!("1 begincodespacerange <{low}> <{high}> endcodespacerange 2 begincidchar <{space}> 1 <{a}> 2 endcidchar").into_bytes(),
        ))).unwrap();
        font.try_insert(
            "ToUnicode".into(),
            PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                format!("2 beginbfchar <{space}> <0041> <{a}> <0020> endbfchar").into_bytes(),
            )),
        )
        .unwrap();
        let id = doc.try_insert_object(PdfObject::PdfDict(font)).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Font, id)
            .unwrap();
        for operator in [format!("<{codes}> Tj"), format!("[<{codes}>] TJ")] {
            doc.replace_page_contents(
                0,
                format!("BT /{name} 10 Tf 12 Tw 1 0 0 1 48 100 Tm {operator} ET").into_bytes(),
            )
            .unwrap();
            let chars = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(chars.len(), 3);
            assert_eq!(chars[0].unicode.as_deref(), Some("A"));
            assert_eq!(chars[1].unicode.as_deref(), Some(" "));
            assert_eq!(chars[1].position.x() - chars[0].position.x(), expected[0]);
            assert_eq!(chars[2].position.x() - chars[1].position.x(), expected[1]);
        }
    }
}

#[test]
fn text_rise_affects_tj_and_tj_array_origins_after_roundtrip() {
    use pdf::page::content_builder::ContentBuilder;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let mut builder = ContentBuilder::new();
    builder
        .begin_text()
        .set_font(&name, 12.0)
        .set_text_matrix(1.0, 0.0, 0.0, 1.0, 48.0, 100.0)
        .show_text("A")
        .raw_operator("10 Ts")
        .show_text("B")
        .raw_operator("[(C)] TJ")
        .raw_operator("0 Ts")
        .show_text("D")
        .end_text();
    doc.replace_page_content_builder(0, &builder).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let chars = document
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(chars.len(), 4);
        assert_eq!(chars[1].position.y(), chars[0].position.y() - 10.0);
        assert_eq!(chars[2].position.y(), chars[1].position.y());
        assert_eq!(chars[3].position.y(), chars[0].position.y());
        for ch in &chars {
            assert_eq!(ch.position.y(), ch.matrix.f());
        }
    }
}

#[test]
fn vertical_font_extraction_uses_font_y_axis() {
    use pdf::{
        document::{page_overlay::TextOverlay, resource_manager::ResourceKind},
        io::stream_reader::StreamReader,
        objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
        parser::syntax::SyntaxParser,
    };
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let mut parser = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
        b"<< /Type /Font /Subtype /Type0 /BaseFont /VerticalTest /Encoding /Identity-V /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /VerticalTest /DW 500 /DW2 [880 -1000] >>] >>".to_vec())).unwrap());
    let mut font = parser.get_object().unwrap().into_dict().unwrap();
    let mut encoding = PdfDict::default();
    encoding
        .try_insert(
            "UseCMap".into(),
            PdfObject::PdfName(pdf::objects::pdf_name::PdfName::new("Identity-V".into())),
        )
        .unwrap();
    font.try_insert(
        "Encoding".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(
            doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(encoding, Vec::new())))
                .unwrap(),
        )),
    )
    .unwrap();
    font.try_insert(
        "ToUnicode".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(
            doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                b"2 beginbfchar <0001> <0041> <0002> <0042> endbfchar".to_vec(),
            )))
            .unwrap(),
        )),
    )
    .unwrap();
    let id = doc.try_insert_object(PdfObject::PdfDict(font)).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Font, id)
        .unwrap();
    doc.overlay_page_text(
        0,
        TextOverlay {
            font_name: &name,
            codes: &[0, 1, 0, 2],
            font_size: 12.0,
            matrix: [1.0, 0.0, 0.0, 1.0, 48.0, 200.0],
            color: [0.0; 3],
        },
    )
    .unwrap();
    let chars = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(chars.len(), 2);
    assert!(
        chars
            .iter()
            .all(|item| item.writing_mode == Some(pdf::font::WritingMode::Vertical))
    );
    assert_eq!(chars[0].writing_axis(), Some([0.0, -1.0]));
    assert_eq!(chars[0].decoded_code_range, Some(0..2));
    assert_eq!(chars[1].decoded_code_range, Some(2..4));
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    assert!(
        doc.replace_page_text_range(
            0,
            &snapshot,
            chars[0].source.unwrap().operation_index,
            None,
            1..2,
            "A",
        )
        .is_err()
    );
    assert_eq!(doc.revision(), revision);
    assert_eq!(chars[0].position.x(), chars[1].position.x());
    assert_eq!(chars[1].position.y() - chars[0].position.y(), 12.0);
    assert_eq!(chars[0].unicode.as_deref(), Some("A"));
    assert_eq!(chars[1].unicode.as_deref(), Some("B"));
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let restored = reopened
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(restored.len(), 2);
    assert_eq!(restored[0].decoded_code_range, Some(0..2));
    assert_eq!(restored[1].decoded_code_range, Some(2..4));
    for (before, after) in chars.iter().zip(&restored) {
        assert_eq!(before.charcode, after.charcode);
        assert_eq!(before.unicode, after.unicode);
        assert_eq!(before.writing_mode, after.writing_mode);
        assert_eq!(
            (before.position.x(), before.position.y()),
            (after.position.x(), after.position.y())
        );
    }
}

#[test]
fn extraction_preserves_explicit_replacement_and_missing_unicode() {
    use pdf::{
        document::{page_overlay::TextOverlay, resource_manager::ResourceKind},
        objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
    };
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let base = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let mut font = doc
        .get_page(0)
        .unwrap()
        .get_resource_font(&base)
        .unwrap()
        .unwrap()
        .into_dict()
        .unwrap();
    font.try_insert(
        "ToUnicode".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(
            doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                b"2 beginbfchar <41> <FFFD> <43> <D83DDE0000660069> endbfchar".to_vec(),
            )))
            .unwrap(),
        )),
    )
    .unwrap();
    let id = doc.try_insert_object(PdfObject::PdfDict(font)).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Font, id)
        .unwrap();
    doc.overlay_page_text(
        0,
        TextOverlay {
            font_name: &name,
            codes: b"ABC",
            font_size: 12.0,
            matrix: [1.0, 0.0, 0.0, 1.0, 48.0, 48.0],
            color: [0.0; 3],
        },
    )
    .unwrap();
    let check = |document: &Document| {
        let chars = document
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(chars.len(), 3);
        assert_eq!(chars[0].unicode.as_deref(), Some("\u{fffd}"));
        assert_eq!(chars[1].unicode, None);
        assert_eq!(chars[2].unicode.as_deref(), Some("😀fi"));
        assert_eq!(chars[2].charcode, u32::from(b'C'));
        assert!(chars[1].position.x() > chars[0].position.x());
    };
    check(&doc);
    check(
        &Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap(),
    );
}

#[test]
fn extraction_character_budget_and_empty_page() {
    use pdf::{
        page::content_builder::ContentBuilder,
        parser::options::{Limits, ParseOptions},
    };
    let doc = Document::new_with_options(ParseOptions {
        limits: Limits {
            max_text_chars: 1,
            ..Limits::default()
        },
        ..ParseOptions::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()
            .is_empty()
    );
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    for (codes, succeeds) in [(b"A".as_slice(), true), (b"AB".as_slice(), false)] {
        let mut content = ContentBuilder::new();
        content.begin_text();
        content.try_set_font(&font, 12.0).unwrap();
        content.move_text(48.0, 48.0);
        content.show_encoded_text(codes).unwrap();
        content.end_text();
        doc.replace_page_content_builder(0, &content).unwrap();
        let revision = doc.revision();
        let result = doc.get_page(0).unwrap().extract_characters_paint_order();
        if succeeds {
            assert_eq!(result.unwrap().len(), 1);
        } else {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        }
        assert_eq!(doc.revision(), revision);
    }
}

#[test]
fn extraction_snapshots_follow_edits_and_rollback() {
    use pdf::page::content_builder::ContentBuilder;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let content = |codes: &[u8]| {
        let mut builder = ContentBuilder::new();
        builder.begin_text();
        builder.try_set_font(&font, 12.0).unwrap();
        builder.move_text(48.0, 48.0);
        builder.show_encoded_text(codes).unwrap();
        builder.end_text();
        builder
    };
    doc.replace_page_content_builder(0, &content(b"A")).unwrap();
    let mut page = doc.get_page(0).unwrap();
    let first = page.extract_characters_paint_order().unwrap();
    doc.replace_page_content_builder(0, &content(b"B")).unwrap();
    assert_eq!(
        page.extract_characters_paint_order().unwrap()[0].charcode,
        u32::from(b'B')
    );
    let failed: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.replace_page_content_builder(0, &content(b"C"))?;
        assert_eq!(
            page.extract_characters_paint_order()?[0].charcode,
            u32::from(b'C')
        );
        Err(PdfError::DocumentError("extraction rollback".into()))
    });
    assert!(failed.is_err());
    assert_eq!(
        page.extract_characters_paint_order().unwrap()[0].charcode,
        u32::from(b'B')
    );
    assert_eq!(first[0].charcode, u32::from(b'A'));
}

#[test]
fn extracted_origins_follow_crop_and_page_rotation() {
    let doc = Document::new().unwrap();
    let expected = [(38.0, 252.0), (28.0, 38.0), (152.0, 28.0), (252.0, 152.0)];
    for (index, rotation) in [0, 90, 180, 270].into_iter().enumerate() {
        let index = index as u32;
        doc.add_page(index).unwrap();
        doc.set_page_crop_box(
            index,
            Some(pdf::geom::rectangle::Rectangle::new(
                10.0, 20.0, 200.0, 300.0,
            )),
        )
        .unwrap();
        doc.set_page_rotation(index, rotation).unwrap();
        doc.watermark_pages_standard(
            &[index],
            "AB",
            StandardFont::Helvetica,
            12.0,
            [1.0, 0.0, 0.0, 1.0, 48.0, 48.0],
            [0.0; 3],
        )
        .unwrap();
    }
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for (index, (x, y)) in expected.into_iter().enumerate() {
        let chars = reopened
            .get_page(index as u32)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(chars.len(), 2);
        let axes = [[1.0, 0.0], [0.0, 1.0], [-1.0, 0.0], [0.0, -1.0]];
        assert_eq!(chars[0].writing_axis(), Some(axes[index]));
        assert!(
            chars
                .iter()
                .all(|item| matches!(item.writing_mode, Some(pdf::font::WritingMode::Horizontal)))
        );
        assert_eq!((chars[0].position.x(), chars[0].position.y()), (x, y));
        assert_eq!((chars[0].matrix.e(), chars[0].matrix.f()), (x, y));
        match index {
            0 => assert!(chars[1].position.x() > x),
            1 => assert!(chars[1].position.y() > y),
            2 => assert!(chars[1].position.x() < x),
            3 => assert!(chars[1].position.y() < y),
            _ => unreachable!(),
        }
    }
}

#[test]
fn public_facade_overlay_workflow_and_outer_rollback() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let matrix = [1.0, 0.0, 0.0, 1.0, 48.0, 48.0];
    let format = PageNumberFormat {
        start: 7,
        prefix: "Page ",
        total_separator: Some(" / "),
        suffix: "",
    };
    let snapshot = doc.save_to_bytes().unwrap();
    let failed: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.watermark_pages_standard(
            &[0, 1],
            "DRAFT",
            StandardFont::Helvetica,
            24.0,
            matrix,
            [0.5; 3],
        )?;
        doc.number_pages_standard(
            &[0, 1],
            &format,
            StandardFont::Helvetica,
            12.0,
            matrix,
            [0.0; 3],
        )?;
        Err(PdfError::DocumentError("workflow rollback".into()))
    });
    assert!(failed.is_err());
    let before = Document::from_bytes_with_mode(snapshot, ParseMode::Strict).unwrap();
    for id in 1..=6 {
        let id = pdf::objects::object_id::ObjectId::new(id, 0);
        assert_eq!(
            doc.resolve_object(id).unwrap(),
            before.resolve_object(id).unwrap()
        );
    }
    doc.watermark_pages_standard(
        &[0, 1],
        "DRAFT",
        StandardFont::Helvetica,
        24.0,
        matrix,
        [0.5; 3],
    )
    .unwrap();
    doc.number_pages_standard(
        &[1, 0],
        &format,
        StandardFont::Helvetica,
        12.0,
        matrix,
        [0.0; 3],
    )
    .unwrap();
    let saved = doc.save_to_bytes().unwrap();
    // Each page reuses its existing standard font for the second overlay.
    assert_eq!(
        String::from_utf8_lossy(&saved)
            .matches("/BaseFont /Helvetica")
            .count(),
        2
    );
    let reopened = Document::from_bytes_with_mode(saved, ParseMode::Strict).unwrap();
    assert_eq!(reopened.try_page_count().unwrap(), 2);
    for index in 0..2 {
        let characters = reopened
            .get_page(index)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        let expected_text = if index == 0 {
            b"DRAFTPage 8 / 2".as_slice()
        } else {
            b"DRAFTPage 7 / 2".as_slice()
        };
        assert_eq!(
            characters
                .iter()
                .map(|item| item.charcode)
                .collect::<Vec<_>>(),
            expected_text
                .iter()
                .map(|code| *code as u32)
                .collect::<Vec<_>>()
        );
        assert!(characters.iter().all(|item| item.code_length == 1));
        assert!(
            characters
                .iter()
                .all(|item| item.font_name.as_deref() == Some("Helvetica"))
        );
        assert!(characters[..5].iter().all(|item| item.font_size == 24.0));
        assert!(characters[5..].iter().all(|item| item.font_size == 12.0));
        let runs = reopened
            .get_page(index)
            .unwrap()
            .extract_text_runs_paint_order()
            .unwrap();
        assert_eq!(runs.len(), 2);
        assert_eq!(runs[0].characters.len(), 5);
        assert_eq!(runs[1].characters.len(), 10);
        assert!(runs[0].object_index < runs[1].object_index);
        assert_eq!(
            runs.iter()
                .flat_map(|run| &run.characters)
                .map(|item| item.charcode)
                .collect::<Vec<_>>(),
            characters
                .iter()
                .map(|item| item.charcode)
                .collect::<Vec<_>>()
        );
        for first in [&characters[0], &characters[5]] {
            assert_eq!(first.position.x(), 48.0);
            assert_eq!(first.position.x(), first.matrix.e());
            assert_eq!(first.position.y(), first.matrix.f());
            assert_eq!(first.matrix.a(), 1.0);
            assert_eq!(first.matrix.d(), -1.0);
        }
        assert!(characters[1].position.x() > characters[0].position.x());
        assert_eq!(characters[1].position.y(), characters[0].position.y());
        let list = reopened.get_page(index).unwrap().display_list().unwrap();
        let labels: Vec<Vec<_>> = list
            .objects()
            .iter()
            .filter_map(|object| match object {
                pdf::page::page_object::PageObject::Text(text) => Some(
                    text.items
                        .iter()
                        .map(|item| item.charcode().code())
                        .collect(),
                ),
                _ => None,
            })
            .collect();
        let expected = if index == 0 {
            b"Page 8 / 2".as_slice()
        } else {
            b"Page 7 / 2".as_slice()
        };
        assert_eq!(
            labels,
            vec![
                b"DRAFT".iter().map(|code| *code as u32).collect::<Vec<_>>(),
                expected.iter().map(|code| *code as u32).collect::<Vec<_>>()
            ]
        );
    }
}
