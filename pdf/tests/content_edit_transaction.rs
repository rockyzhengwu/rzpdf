use pdf::{
    document::{owned::Document, resource_manager::StandardFont},
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};

#[test]
fn budgeted_transaction_snapshot_rejects_and_rolls_back_public_edits() {
    use pdf::budget::OperationBudget;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
    let before = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    let tiny = OperationBudget::new(1);
    let mut called = false;
    assert!(doc
        .edit_transaction_with_budget(&tiny, |_| {
            called = true;
            Ok(())
        })
        .is_err());
    assert!(!called);
    assert_eq!(doc.revision(), revision);
    assert_eq!(tiny.live_bytes(), 0);
    let pool = OperationBudget::new(65536);
    let neighbor = pool.reserve(7).unwrap();
    let result: PdfResult<()> = doc.edit_transaction_with_budget(&pool, |editing| {
        assert!(pool.live_bytes() > 7);
        editing.replace_page_contents(0, b"Q q".to_vec())?;
        Err(PdfError::DocumentError("budgeted outer rollback".into()))
    });
    assert!(result.is_err());
    assert_eq!(pool.live_bytes(), 7);
    let restored = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(restored.source_bytes(), before.source_bytes());
    assert_eq!(restored.stream_object_id(0), before.stream_object_id(0));
    doc.edit_transaction_with_budget(&pool, |editing| {
        editing.replace_page_contents(0, b"Q q".to_vec())
    })
    .unwrap();
    assert_eq!(pool.live_bytes(), 7);
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        b"Q q"
    );
    drop(neighbor);
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn mixed_content_edits_commit_without_changing_shared_page_after_reopen() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let image = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf (ABC) Tj ET /{image} Do 0 0 10 10 re f").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 1).unwrap();
    let original = doc.get_page(0).unwrap().content_program().unwrap();
    doc.edit_transaction(|editing| {
        let snapshot = editing.get_page(0)?.content_program()?;
        editing.replace_page_text_range(0, &snapshot, 2, None, 1..2, "XY")?;
        let snapshot = editing.get_page(0)?.content_program()?;
        editing.remove_page_image_draw(0, &snapshot, 4)?;
        let snapshot = editing.get_page(0)?.content_program()?;
        editing.remove_page_path_draw(0, &snapshot, 5)?;
        Ok(())
    })
    .unwrap();
    let untouched = doc.get_page(1).unwrap().content_program().unwrap();
    assert_eq!(untouched.source_bytes(), original.source_bytes());
    assert_eq!(untouched.stream_object_id(0), original.stream_object_id(0));
    assert_ne!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .stream_object_id(0),
        original.stream_object_id(0)
    );
    assert!(doc.remove_page_path_draw(0, &original, 6).is_err());
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let mut edited = document.get_page(0).unwrap();
        assert_eq!(
            edited
                .extract_characters_paint_order_report()
                .unwrap()
                .characters
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "AXYC"
        );
        assert!(edited.extract_images_paint_order().unwrap().is_empty());
        assert_eq!(edited.display_list().unwrap().objects().len(), 1);
        // Logical removal preserves the resource, rather than deleting a shared image.
        assert_eq!(edited.image_resource_names().unwrap(), [image.clone()]);
        let mut untouched = document.get_page(1).unwrap();
        assert_eq!(
            untouched.content_program().unwrap().source_bytes(),
            original.source_bytes()
        );
        assert_eq!(
            untouched
                .extract_characters_paint_order_report()
                .unwrap()
                .characters
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "ABC"
        );
        assert_eq!(untouched.extract_images_paint_order().unwrap().len(), 1);
        assert_eq!(untouched.display_list().unwrap().objects().len(), 3);
    }
}

#[test]
fn mixed_content_edits_rollback_errors_and_panics_after_nested_commits() {
    for panic_failure in [false, true] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let image = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{font} 12 Tf (ABC) Tj ET /{image} Do 0 0 10 10 re f").into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        let original = doc.get_page(0).unwrap().content_program().unwrap();
        let stream_ids: Vec<_> = (0..original.stream_ranges().len())
            .map(|i| original.stream_object_id(i))
            .collect();
        let revision = doc.revision();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            doc.edit_transaction(|editing| -> PdfResult<()> {
                let snapshot = editing.get_page(0)?.content_program()?;
                editing.replace_page_text_range(0, &snapshot, 2, None, 1..2, "XY")?;
                let snapshot = editing.get_page(0)?.content_program()?;
                editing.remove_page_image_draw(0, &snapshot, 4)?;
                let snapshot = editing.get_page(0)?.content_program()?;
                editing.remove_page_path_draw(0, &snapshot, 5)?;
                // Saving inside the transaction sees all nested commits.
                let staged =
                    Document::from_bytes_with_mode(editing.save_to_bytes()?, ParseMode::Strict)?;
                let mut page = staged.get_page(0)?;
                assert!(page.extract_images_paint_order()?.is_empty());
                assert_eq!(page.display_list()?.objects().len(), 1);
                assert_eq!(
                    page.extract_characters_paint_order_report()?
                        .characters
                        .iter()
                        .filter_map(|ch| ch.unicode.as_deref())
                        .collect::<String>(),
                    "AXYC"
                );
                if panic_failure {
                    panic!("injected mixed edit rollback")
                }
                Err(PdfError::ObjectError("injected mixed edit rollback".into()))
            })
        }));
        if panic_failure {
            assert!(result.is_err());
        } else {
            assert!(result.unwrap().is_err());
        }
        assert!(doc.revision() > revision);
        let restored = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(restored.source_bytes(), original.source_bytes());
        assert_eq!(
            (0..restored.stream_ranges().len())
                .map(|i| restored.stream_object_id(i))
                .collect::<Vec<_>>(),
            stream_ids
        );
        assert!(!doc.is_stamp_current(original.source_stamp().unwrap()));
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            for index in [0, 1] {
                let mut page = document.get_page(index).unwrap();
                assert_eq!(
                    page.content_program().unwrap().source_bytes(),
                    original.source_bytes()
                );
                assert_eq!(page.image_resource_names().unwrap(), [image.clone()]);
                assert_eq!(page.extract_images_paint_order().unwrap().len(), 1);
                assert_eq!(page.display_list().unwrap().objects().len(), 3);
            }
        }
    }
}
