use pdf::{
    document::{
        PdfDocument,
        destination::DestinationView,
        links::{LinkTarget, PageLink},
        outlines::OutlineTarget,
        page_labels::{PageLabelRange, PageLabelStyle},
    },
    parser::syntax::ParseMode,
};

#[test]
fn all_views_preserve_parameters_and_page_identity() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let views = vec![
        DestinationView::Fit,
        DestinationView::FitB,
        DestinationView::Xyz {
            left: None,
            top: Some(700.0),
            zoom: Some(0.0),
        },
        DestinationView::Xyz {
            left: Some(-20.0),
            top: None,
            zoom: None,
        },
        DestinationView::FitH(None),
        DestinationView::FitV(Some(25.0)),
        DestinationView::FitBH(Some(800.0)),
        DestinationView::FitBV(None),
        DestinationView::FitR {
            left: 10.0,
            bottom: 20.0,
            right: 100.0,
            top: 120.0,
        },
    ];
    for (i, view) in views.iter().enumerate() {
        doc.set_named_destination_view(
            &format!("view-{i}"),
            &doc.destination(1, view.clone()).unwrap(),
        )
        .unwrap();
    }
    let captured = doc.destination(1, DestinationView::Fit).unwrap();
    doc.reorder_pages(&[1, 0]).unwrap();
    assert_eq!(doc.destination_page_index(&captured).unwrap(), 0);
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for (i, (_, dest)) in reopened
        .named_destination_views()
        .unwrap()
        .iter()
        .enumerate()
    {
        assert_eq!(dest.view, views[i]);
        assert_eq!(reopened.destination_page_index(dest).unwrap(), 0);
    }
    assert!(reopened.destination_page_index(&captured).is_err());
}

#[test]
fn invalid_views_do_not_mutate_document() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let revision = doc.commit_revision();
    for view in [
        DestinationView::FitH(Some(f32::NAN)),
        DestinationView::Xyz {
            left: None,
            top: None,
            zoom: Some(-1.0),
        },
        DestinationView::FitR {
            left: 10.0,
            bottom: 20.0,
            right: 5.0,
            top: 30.0,
        },
    ] {
        assert!(doc.destination(0, view).is_err());
    }
    assert_eq!(doc.commit_revision(), revision);
}

#[test]
fn balanced_named_tree_roundtrips_and_can_be_edited() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    for i in (0..70).rev() {
        doc.set_named_destination(&format!("目标{i:03}"), 0)
            .unwrap();
    }
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(reopened.named_destinations().unwrap().len(), 70);
    assert!(reopened.remove_named_destination("目标035").unwrap());
    reopened.set_named_destination("目标070", 0).unwrap();
    assert_eq!(reopened.named_destinations().unwrap().len(), 70);
}

#[test]
fn hierarchical_outline_crud_moves_and_closed_counts() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let target = OutlineTarget::Destination(
        doc.destination(1, DestinationView::FitH(Some(600.0)))
            .unwrap(),
    );
    let a = doc.add_outline(None, "第一章", &target).unwrap();
    let b = doc.add_outline(Some(&a), "第二节", &target).unwrap();
    let c = doc.add_outline(Some(&b), "子节", &target).unwrap();
    doc.update_outline(&b, "折叠", &target, false).unwrap();
    assert!(!doc.outline_items().unwrap()[1].open);
    let revision = doc.commit_revision();
    assert!(doc.move_outline(&a, Some(&c), 0).is_err());
    assert!(doc.move_outline(&c, None, 10).is_err());
    assert_eq!(doc.commit_revision(), revision);
    doc.move_outline(&c, None, 0).unwrap();
    let items = doc.outline_items().unwrap();
    assert_eq!(
        items.iter().map(|x| x.title.as_str()).collect::<Vec<_>>(),
        ["子节", "第一章", "折叠"]
    );
    assert!(items[0].parent.is_none());
    assert_eq!(items[2].parent.as_ref(), Some(&a));
    doc.reorder_pages(&[1, 0]).unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let items = reopened.outline_items().unwrap();
    assert_eq!(items.len(), 3);
    if let OutlineTarget::Destination(dest) = &items[0].target {
        assert_eq!(reopened.destination_page_index(dest).unwrap(), 0);
    } else {
        panic!("expected explicit destination");
    }
    reopened.remove_outline(&items[1].handle).unwrap();
    assert_eq!(reopened.outline_items().unwrap().len(), 1);
}

#[test]
fn named_rename_updates_links_and_outlines_and_collision_rolls_back() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.set_named_destination("旧", 0).unwrap();
    doc.set_named_destination("已存在", 0).unwrap();
    doc.add_named_link(0, [1.0, 2.0, 30.0, 40.0], "旧").unwrap();
    let item = doc
        .add_outline(None, "title", &OutlineTarget::Named("旧".into()))
        .unwrap();
    let revision = doc.commit_revision();
    assert!(doc.remove_named_destination("旧").is_err());
    assert!(doc.rename_named_destination("旧", "已存在").is_err());
    assert_eq!(doc.commit_revision(), revision);
    doc.rename_named_destination("旧", "新").unwrap();
    assert_eq!(
        doc.page_links(0).unwrap()[0].target,
        LinkTarget::Named("新".into())
    );
    assert_eq!(
        doc.outline_items().unwrap()[0].target,
        OutlineTarget::Named("新".into())
    );
    assert_eq!(doc.outline_items().unwrap()[0].handle, item);
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened.page_links(0).unwrap()[0].target,
        LinkTarget::Named("新".into())
    );
    assert!(reopened.remove_named_destination("新").is_err());
}

#[test]
fn link_crud_uses_stable_handles_and_rejects_foreign_page() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    doc.add_square_annotation(0, [40.0, 40.0, 60.0, 60.0], [1.0, 0.0, 0.0])
        .unwrap();
    let mut link = PageLink {
        rect: [1.0, 2.0, 20.0, 30.0],
        target: LinkTarget::Destination(doc.destination(1, DestinationView::FitV(None)).unwrap()),
    };
    let handle = doc.add_link(0, &link).unwrap();
    assert_eq!(
        doc.page_link_handles(0).unwrap().as_slice(),
        std::slice::from_ref(&handle)
    );
    link.target = LinkTarget::Uri("https://example.org/updated".into());
    doc.update_page_link(0, &handle, &link).unwrap();
    assert_eq!(
        doc.page_links(0).unwrap().as_slice(),
        std::slice::from_ref(&link)
    );
    let revision = doc.commit_revision();
    assert!(doc.remove_page_link(1, &handle).is_err());
    link.rect[0] = f32::INFINITY;
    assert!(doc.update_page_link(0, &handle, &link).is_err());
    assert_eq!(doc.commit_revision(), revision);
    doc.remove_page_link(0, &handle).unwrap();
    assert!(doc.page_links(0).unwrap().is_empty());
    assert_eq!(doc.page_square_annotations(0).unwrap().len(), 1);
}

#[test]
fn labels_support_all_styles_and_pdf_repeated_letter_numbering() {
    let doc = PdfDocument::new().unwrap();
    for i in 0..8 {
        doc.add_page(i).unwrap();
    }
    let ranges = vec![
        PageLabelRange {
            start_index: 0,
            style: Some(PageLabelStyle::RomanLower),
            prefix: "".into(),
            start_number: 4,
        },
        PageLabelRange {
            start_index: 1,
            style: Some(PageLabelStyle::RomanUpper),
            prefix: "".into(),
            start_number: 9,
        },
        PageLabelRange {
            start_index: 2,
            style: Some(PageLabelStyle::Decimal),
            prefix: "章-".into(),
            start_number: 1,
        },
        PageLabelRange {
            start_index: 4,
            style: Some(PageLabelStyle::LettersUpper),
            prefix: "".into(),
            start_number: 27,
        },
        PageLabelRange {
            start_index: 5,
            style: Some(PageLabelStyle::LettersLower),
            prefix: "".into(),
            start_number: 28,
        },
        PageLabelRange {
            start_index: 6,
            style: None,
            prefix: "封底".into(),
            start_number: 1,
        },
    ];
    doc.set_page_label_ranges(&ranges).unwrap();
    let expected = ["iv", "IX", "章-1", "章-2", "AA", "bb", "封底", "封底"];
    for (i, text) in expected.iter().enumerate() {
        assert_eq!(doc.page_label(i as u32).unwrap(), *text);
    }
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(reopened.page_label_ranges().unwrap(), ranges);
    for (i, text) in expected.iter().enumerate() {
        assert_eq!(reopened.page_label(i as u32).unwrap(), *text);
    }
    let revision = doc.commit_revision();
    let invalid = PageLabelRange {
        start_index: 1,
        style: None,
        prefix: "".into(),
        start_number: 0,
    };
    assert!(doc.set_page_label_ranges(&[invalid]).is_err());
    assert_eq!(doc.commit_revision(), revision);
    doc.set_page_label_ranges(&[]).unwrap();
    assert_eq!(doc.page_label(7).unwrap(), "8");
}

#[test]
fn explicit_labels_follow_reorder_insertion_copy_import_and_deletion() {
    let doc = PdfDocument::new().unwrap();
    for i in 0..3 {
        doc.add_page(i).unwrap();
    }
    doc.set_page_label_ranges(&[PageLabelRange {
        start_index: 0,
        style: Some(PageLabelStyle::RomanLower),
        prefix: "p-".into(),
        start_number: 1,
    }])
    .unwrap();
    doc.reorder_pages(&[2, 0, 1]).unwrap();
    assert_eq!(
        (0..3)
            .map(|i| doc.page_label(i).unwrap())
            .collect::<Vec<_>>(),
        ["p-iii", "p-i", "p-ii"]
    );
    doc.add_page(1).unwrap();
    assert_eq!(
        (0..4)
            .map(|i| doc.page_label(i).unwrap())
            .collect::<Vec<_>>(),
        ["p-iii", "2", "p-i", "p-ii"]
    );
    doc.copy_page(3, 0).unwrap();
    assert_eq!(doc.page_label(0).unwrap(), "p-ii");
    doc.delete_page(2).unwrap();
    let source = PdfDocument::new().unwrap();
    source.add_page(0).unwrap();
    doc.import_page_from(&source, 0, 1).unwrap();
    assert_eq!(
        (0..5)
            .map(|i| doc.page_label(i).unwrap())
            .collect::<Vec<_>>(),
        ["p-ii", "2", "p-iii", "p-i", "p-ii"]
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(reopened.page_label(4).unwrap(), "p-ii");
    for _ in 0..5 {
        reopened.delete_page(0).unwrap();
    }
    assert!(reopened.page_label_ranges().unwrap().is_empty());
}

#[test]
fn nested_attachment_tree_uses_shared_tree_reader_and_writer() {
    let doc = PdfDocument::new().unwrap();
    for i in 0..40 {
        doc.add_attachment(&format!("file-{i:02}"), vec![i])
            .unwrap();
    }
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(reopened.attachment_names().unwrap().len(), 40);
    assert_eq!(reopened.attachment_data("file-37").unwrap(), Some(vec![37]));
    assert!(reopened.remove_attachment("file-02").unwrap());
    reopened.add_attachment("new", b"new".to_vec()).unwrap();
    assert_eq!(reopened.attachment_names().unwrap().len(), 40);
}
