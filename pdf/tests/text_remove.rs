use pdf::{
    document::{owned::Document, resource_manager::StandardFont},
    parser::syntax::ParseMode,
};

#[test]
fn clearing_cross_stream_tj_preserves_untouched_stream_and_shared_source() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"q [(A\\".to_vec()).unwrap();
    doc.append_page_contents(0, b"(B\\)) 25 % across\n<4142> -10] T".to_vec())
        .unwrap();
    doc.append_page_contents(0, b"J Q\n".to_vec()).unwrap();
    doc.append_page_contents(0, b"% untouched\n123 RzpdfUnknown\n".to_vec())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let before = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(before.operations()[1].operator.name(), "TJ");
    doc.remove_page_text_draw(0, &before, 1).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let after = document.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(after.stream_ranges().len(), 4);
        assert_eq!(after.stream_object_id(3), before.stream_object_id(3));
        assert_eq!(after.stream_bytes(3), before.stream_bytes(3));
        assert_eq!(
            after
                .operations()
                .iter()
                .map(|op| op.operator.name())
                .collect::<Vec<_>>(),
            ["q", "TJ", "Q", "RzpdfUnknown"]
        );
        let array = after.operations()[1].operator.get_as_array(0).unwrap();
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
        assert_eq!(array.get(1).unwrap().as_f32(), Some(25.));
        assert_eq!(array.get(3).unwrap().as_f32(), Some(-10.));
        assert!(
            std::str::from_utf8(after.source_bytes())
                .unwrap()
                .contains("% across\n")
        );
        let shared = document.get_page(1).unwrap().content_program().unwrap();
        assert_eq!(shared.source_bytes(), before.source_bytes());
        for index in 0..4 {
            assert_eq!(
                shared.stream_object_id(index),
                before.stream_object_id(index)
            );
        }
    }
}

#[test]
fn clearing_empty_or_number_only_tj_preserves_adjustments() {
    for source in [b"[] TJ".as_slice(), b"[25 -10 0] TJ".as_slice()] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, source.to_vec()).unwrap();
        let before = doc.get_page(0).unwrap().content_program().unwrap();
        doc.remove_page_text_draw(0, &before, 0).unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let after = reopened.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(
            after.operations()[0].operator.get_as_array(0).unwrap(),
            before.operations()[0].operator.get_as_array(0).unwrap()
        );
    }
}

#[test]
fn clearing_large_tj_keeps_every_number_comment_and_neighbor() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let mut source = String::from("q [");
    for index in 0..300 {
        source.push_str(&format!("(A\\(B\\)) 25 % item {index}\n<4142> -10 <> "));
    }
    source.push_str("] TJ Q 123 RzpdfUnknown");
    doc.replace_page_contents(0, source.into_bytes()).unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    doc.remove_page_text_draw(0, &snapshot, 1).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let program = document.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(program.operations().len(), 4);
        assert_eq!(program.operations()[3].operator.name(), "RzpdfUnknown");
        assert_eq!(
            program.operations()[3].operator.get_as_f32(0).unwrap(),
            123.
        );
        let array = program.operations()[1].operator.get_as_array(0).unwrap();
        assert_eq!(array.len(), 1500);
        for index in 0..300 {
            for offset in [0, 2, 4] {
                assert!(
                    array
                        .get(index * 5 + offset)
                        .unwrap()
                        .as_string()
                        .unwrap()
                        .bytes()
                        .is_empty()
                );
            }
            assert_eq!(array.get(index * 5 + 1).unwrap().as_f32(), Some(25.));
            assert_eq!(array.get(index * 5 + 3).unwrap().as_f32(), Some(-10.));
            assert!(
                std::str::from_utf8(program.source_bytes())
                    .unwrap()
                    .contains(&format!("% item {index}\n"))
            );
        }
    }
}

#[test]
fn text_removal_validation_and_outer_rollback_leave_source_unchanged() {
    use pdf::error::{PdfError, PdfResult};
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    for source in [b"[(A) 25 true (B)] TJ".as_slice(), b"q Q".as_slice()] {
        doc.replace_page_contents(0, source.to_vec()).unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let revision = doc.revision();
        assert!(doc.remove_page_text_draw(0, &snapshot, 0).is_err());
        assert!(doc.remove_page_text_draw(0, &snapshot, usize::MAX).is_err());
        assert_eq!(doc.revision(), revision);
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            source
        );
    }
    // Clearing code bytes does not need font decoding or Unicode coverage.
    let source = b"[(A) 25 (B)] TJ q Q";
    doc.replace_page_contents(0, source.to_vec()).unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let result: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.remove_page_text_draw(0, &snapshot, 0)?;
        let current = doc.get_page(0)?.content_program()?;
        let array = current.operations()[0].operator.get_as_array(0)?;
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
        Err(PdfError::DocumentError("rollback deletion".into()))
    });
    assert!(result.is_err());
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        source
    );
}

#[test]
fn text_removal_preserves_quote_and_array_effects_and_shared_page() {
    for target in [
        "(AB) Tj",
        "(AB) '",
        "2 3 (AB) \"",
        "[(A) 25 % spacing\n(B) -10] TJ",
    ] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let source = format!("BT /{font} 10 Tf 12 TL 1 0 0 1 40 200 Tm {target} (C) Tj ET");
        doc.replace_page_contents(0, source.as_bytes().to_vec())
            .unwrap();
        doc.copy_page(0, 1).unwrap();
        let before = doc.get_page(0).unwrap().content_program().unwrap();
        let target_index = before
            .operations()
            .iter()
            .position(|op| matches!(op.operator.name(), "Tj" | "TJ" | "'" | "\""))
            .unwrap();
        doc.remove_page_text_draw(0, &before, target_index).unwrap();
        let edited = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(edited.operations().len(), before.operations().len());
        assert_eq!(
            edited.operations()[target_index].operator.name(),
            before.operations()[target_index].operator.name()
        );
        if target.ends_with("TJ") {
            let array = edited.operations()[target_index]
                .operator
                .get_as_array(0)
                .unwrap();
            assert_eq!(array.len(), 4);
            assert_eq!(array.get(1).unwrap().as_f32(), Some(25.));
            assert_eq!(array.get(3).unwrap().as_f32(), Some(-10.));
            assert!(
                std::str::from_utf8(edited.source_bytes())
                    .unwrap()
                    .contains("% spacing")
            );
        }
        if target.ends_with('"') {
            let op = &edited.operations()[target_index].operator;
            assert_eq!(op.get_as_f32(0).unwrap(), 2.);
            assert_eq!(op.get_as_f32(1).unwrap(), 3.);
        }
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let chars = document
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            assert_eq!(chars.len(), 1);
            assert_eq!(chars[0].unicode.as_deref(), Some("C"));
            assert_eq!(
                chars[0].position.y(),
                if target.ends_with('\'') || target.ends_with('"') {
                    604.
                } else {
                    592.
                }
            );
            assert_eq!(
                document
                    .get_page(1)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                source.as_bytes()
            );
        }
        let revision = doc.revision();
        assert!(doc.remove_page_text_draw(0, &before, target_index).is_err());
        assert_eq!(doc.revision(), revision);
    }
}
