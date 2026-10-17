use pdf::{
    document::{
        PdfDocument,
        acroform::{FormFieldDefinition, FormFieldKind, FormValue, FormWidgetSpec},
        annotations::{Annotation, AnnotationKind},
        destination::DestinationView,
        links::{LinkTarget, PageLink},
        outlines::OutlineTarget,
        page_labels::{PageLabelRange, PageLabelStyle},
        page_transfer::{PageDeletionPolicy, PageTransferOptions},
    },
    objects::{PdfObject, pdf_number::PdfNumber},
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};
use std::io::Cursor;
type Doc = PdfDocument<Cursor<Vec<u8>>>;
fn fixture() -> Doc {
    let doc = PdfDocument::new().unwrap();
    for i in 0..2 {
        doc.add_page(i).unwrap();
        doc.replace_page_contents(i, b"q Q".to_vec()).unwrap();
    }
    doc.add_text_field(0, [20., 40., 180., 70.], "person.city", "Shanghai")
        .unwrap();
    let city = doc.form_field_tree().unwrap()[1].handle.clone();
    doc.add_form_widget(&city, 1, [20., 40., 180., 70.])
        .unwrap();
    doc.add_checkbox_field(0, [20., 90., 44., 114.], "person.ok", true)
        .unwrap();
    let parent = doc
        .add_annotation(
            0,
            &Annotation::new(AnnotationKind::Square, [50., 90., 130., 140.]),
        )
        .unwrap();
    let popup = doc
        .add_annotation(
            0,
            &Annotation::new(
                AnnotationKind::Popup { open: false },
                [150., 90., 250., 140.],
            ),
        )
        .unwrap();
    doc.set_annotation_popup(&parent, Some(&popup)).unwrap();
    let reply = doc
        .add_annotation(
            0,
            &Annotation::new(
                AnnotationKind::Text {
                    icon: "Note".into(),
                    open: false,
                },
                [270., 90., 290., 110.],
            ),
        )
        .unwrap();
    doc.set_annotation_reply(&reply, Some(&parent)).unwrap();
    doc.set_named_destination("start", 0).unwrap();
    doc.add_link(
        1,
        &PageLink {
            rect: [20., 150., 130., 175.],
            target: LinkTarget::Named("start".into()),
        },
    )
    .unwrap();
    let root = doc
        .add_outline(None, "Start", &OutlineTarget::Named("start".into()))
        .unwrap();
    doc.add_outline(
        Some(&root),
        "Next",
        &OutlineTarget::Destination(
            doc.destination(1, DestinationView::FitH(Some(200.)))
                .unwrap(),
        ),
    )
    .unwrap();
    doc.set_page_label_ranges(&[PageLabelRange {
        start_index: 0,
        style: Some(PageLabelStyle::RomanLower),
        prefix: "s-".into(),
        start_number: 3,
    }])
    .unwrap();
    doc
}
fn options() -> PageTransferOptions {
    PageTransferOptions {
        name_prefix: "imported".into(),
        include_navigation: true,
        ..Default::default()
    }
}
fn reopen(doc: &Doc) -> Doc {
    PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap()
}

#[test]
fn batch_import_remaps_fields_popup_replies_links_navigation_and_labels() {
    let source = fixture();
    let before = source.save_to_bytes().unwrap();
    let target = PdfDocument::new().unwrap();
    target.add_page(0).unwrap();
    target
        .import_pages_from_with_options(&source, &[1, 0], 0, &options())
        .unwrap();
    assert_eq!(source.save_to_bytes().unwrap(), before);
    let target = reopen(&target);
    assert_eq!(target.try_page_count().unwrap(), 3);
    assert_eq!(target.page_label(0).unwrap(), "s-iv");
    assert_eq!(target.page_label(1).unwrap(), "s-iii");
    assert_eq!(
        target.named_destinations().unwrap(),
        [("imported_start".into(), 1)]
    );
    assert_eq!(target.outline_items().unwrap().len(), 2);
    let fields = target.form_field_tree().unwrap();
    assert_eq!(fields[1].name, "imported_person.city");
    assert_eq!(fields[1].widgets.len(), 2);
    target
        .set_form_value("imported_person.city", FormValue::Text("Suzhou".into()))
        .unwrap();
    let records = target.page_annotations(1).unwrap();
    let square = records.iter().find(|r| r.subtype == "Square").unwrap();
    let popup = records.iter().find(|r| r.subtype == "Popup").unwrap();
    let reply = records.iter().find(|r| r.subtype == "Text").unwrap();
    assert_eq!(square.popup, popup.handle);
    assert_eq!(popup.popup_parent, square.handle);
    assert_eq!(reply.reply_to, square.handle);
    target.reorder_pages(&[2, 1, 0]).unwrap();
    assert_eq!(target.named_destinations().unwrap()[0].1, 1);
    assert_eq!(
        reopen(&target).form_fields().unwrap()[0].value,
        FormValue::Text("Suzhou".into())
    );
}

#[test]
fn copy_clones_relationships_but_not_original_field_values() {
    let doc = fixture();
    let original = doc.form_field_tree().unwrap()[1].handle.clone();
    let before = doc.save_to_bytes().unwrap();
    let rev = doc.commit_revision();
    assert!(doc.copy_page(0, 2).is_err());
    assert_eq!(doc.commit_revision(), rev);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    let copied = doc
        .copy_page_with_options(
            0,
            2,
            &PageTransferOptions {
                name_prefix: "copy".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_ne!(
        copied,
        doc.destination(0, DestinationView::Fit)
            .unwrap()
            .page
            .object_id()
    );
    assert_eq!(
        doc.page_annotations(0).unwrap().len(),
        doc.page_annotations(2).unwrap().len()
    );
    doc.set_form_value("copy_person.city", FormValue::Text("Beijing".into()))
        .unwrap();
    assert_eq!(doc.form_field_tree().unwrap()[1].handle, original);
    assert_eq!(
        doc.form_fields().unwrap()[0].value,
        FormValue::Text("Shanghai".into())
    );
    assert_eq!(doc.page_label(2).unwrap(), "s-iii");
    reopen(&doc).form_fields().unwrap();
}

#[test]
fn deletion_rejects_referenced_targets_then_retargets_and_prunes_widgets() {
    let doc = fixture();
    let before = doc.save_to_bytes().unwrap();
    let rev = doc.commit_revision();
    assert!(doc.delete_page(0).is_err());
    assert_eq!(doc.commit_revision(), rev);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    doc.delete_page_with_policy(0, PageDeletionPolicy::Retarget { page_index: 1 })
        .unwrap();
    let doc = reopen(&doc);
    assert_eq!(doc.try_page_count().unwrap(), 1);
    assert_eq!(doc.named_destinations().unwrap(), [("start".into(), 0)]);
    let tree = doc.form_field_tree().unwrap();
    assert_eq!(tree.len(), 2);
    assert_eq!(tree[1].widgets.len(), 1);
    assert_eq!(tree[1].widgets[0].page_index, 0);
    assert_eq!(doc.page_links(0).unwrap().len(), 1);
    assert_eq!(doc.page_label(0).unwrap(), "s-iv");
    doc.set_form_value("person.city", FormValue::Text("Suzhou".into()))
        .unwrap();
    reopen(&doc);
}

#[test]
fn unreferenced_page_can_be_deleted_in_document_with_other_navigation() {
    let doc = fixture();
    doc.add_page(2).unwrap();
    doc.delete_page(2).unwrap();
    assert_eq!(doc.outline_items().unwrap().len(), 2);
    assert_eq!(doc.try_page_count().unwrap(), 2);
}

#[test]
fn partial_selection_expands_named_links_and_rejects_unselected_targets() {
    let source = fixture();
    let target = PdfDocument::new().unwrap();
    let rev = target.commit_revision();
    assert!(target.import_pages_from(&source, &[1], 0).is_err());
    assert_eq!(target.commit_revision(), rev);
    target.import_pages_from(&source, &[0], 0).unwrap();
    assert!(target.named_destinations().unwrap().is_empty());
    assert_eq!(target.form_field_tree().unwrap()[1].widgets.len(), 1);
    reopen(&target);
}

#[test]
fn names_and_catalog_navigation_conflicts_rollback_all_insertions() {
    let source = fixture();
    let target = fixture();
    let rev = target.commit_revision();
    let before = target.save_to_bytes().unwrap();
    assert!(
        target
            .import_pages_from_with_options(
                &source,
                &[0, 1],
                2,
                &PageTransferOptions {
                    include_navigation: true,
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(target.commit_revision(), rev);
    assert_eq!(target.save_to_bytes().unwrap(), before);
    target
        .import_pages_from_with_options(&source, &[0, 1], 2, &options())
        .unwrap();
    let rev = target.commit_revision();
    let before = target.save_to_bytes().unwrap();
    assert!(
        target
            .import_pages_from_with_options(&source, &[0], 0, &options())
            .is_err()
    );
    assert_eq!(target.commit_revision(), rev);
    assert_eq!(target.save_to_bytes().unwrap(), before);
}

#[test]
fn partial_radio_selection_and_deletion_are_explicitly_atomic() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let widgets: Vec<_> = (0..2)
        .map(|page| FormWidgetSpec {
            page_index: page,
            rect: [20., 20., 44., 44.],
            on_state: Some(format!("State{page}")),
        })
        .collect();
    doc.add_form_field(
        "radio",
        &FormFieldDefinition::new(
            FormFieldKind::Radio {
                no_toggle_to_off: false,
                in_unison: false,
            },
            FormValue::Radio(Some("State0".into())),
        ),
        &widgets,
    )
    .unwrap();
    let before = doc.save_to_bytes().unwrap();
    let rev = doc.commit_revision();
    assert!(doc.delete_page(0).is_err());
    assert!(doc.extract_pages(&[0]).is_err());
    assert_eq!(doc.commit_revision(), rev);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    let all = doc.extract_pages(&[1, 0]).unwrap();
    assert_eq!(
        all.form_fields().unwrap()[0].value,
        FormValue::Radio(Some("State0".into()))
    );
    reopen(&all);
}

#[test]
fn graph_limits_and_outer_transactions_restore_page_tree_and_objects() {
    let source = fixture();
    let target = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_raw_stream_bytes: 16,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let rev = target.commit_revision();
    assert!(target.import_pages_from(&source, &[0, 1], 0).is_err());
    assert_eq!(target.commit_revision(), rev);
    assert_eq!(target.try_page_count().unwrap(), 0);
    let target = PdfDocument::new().unwrap();
    let before = target.save_to_bytes().unwrap();
    let result: pdf::error::PdfResult<()> = target.edit_transaction(|d| {
        d.import_pages_from_with_options(&source, &[0, 1], 0, &options())?;
        Err(pdf::error::PdfError::DocumentError("rollback".into()))
    });
    assert!(result.is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
    target
        .import_pages_from_with_options(&source, &[0, 1], 0, &options())
        .unwrap();
    reopen(&target);
}

#[test]
fn unknown_annotation_and_tagged_ownership_never_silently_disappear() {
    let doc = fixture();
    let handle = doc
        .page_annotations(0)
        .unwrap()
        .iter()
        .find(|r| r.subtype == "Square")
        .unwrap()
        .handle
        .clone()
        .unwrap();
    let mut dict = doc.resolve_handle(&handle).unwrap().into_dict().unwrap();
    dict.try_insert(
        "StructParent".into(),
        PdfObject::PdfNumber(PdfNumber::new(1.)),
    )
    .unwrap();
    doc.update_handle(&handle, PdfObject::PdfDict(dict))
        .unwrap();
    let rev = doc.commit_revision();
    assert!(doc.copy_page_with_options(0, 2, &options()).is_err());
    assert!(doc.delete_page(1).is_err());
    assert_eq!(doc.commit_revision(), rev);
}

#[test]
fn unicode_font_resources_survive_import_copy_and_reopened_refill() {
    use pdf::document::{acroform::FormTextStyle, embedded_font::TrueTypeEmbeddingOptions};
    let source = PdfDocument::new().unwrap();
    source.add_page(0).unwrap();
    let font = source
        .resources()
        .embed_truetype_font(
            0,
            include_bytes!("fixtures/v03/NotoSansSC-static-subset.ttf").to_vec(),
            "你好世界Hello PDF，！",
            TrueTypeEmbeddingOptions {
                postscript_name: "NotoSansSC-Thin".into(),
                cap_height: 733.,
                stem_v: 80.,
                serif: false,
            },
        )
        .unwrap();
    let mut definition = FormFieldDefinition::new(
        FormFieldKind::Text { multiline: false },
        FormValue::Text("你好".into()),
    );
    definition.text_style = FormTextStyle {
        font: Some(font.object_handle().clone()),
        ..Default::default()
    };
    source
        .add_form_field(
            "chinese",
            &definition,
            &[FormWidgetSpec::new(0, [20., 20., 220., 60.])],
        )
        .unwrap();
    let target = source.extract_pages(&[0]).unwrap();
    target
        .copy_page_with_options(
            0,
            1,
            &PageTransferOptions {
                name_prefix: "copy".into(),
                ..Default::default()
            },
        )
        .unwrap();
    let target = reopen(&target);
    target
        .set_form_value("chinese", FormValue::Text("世界".into()))
        .unwrap();
    assert_eq!(
        reopen(&target).form_fields().unwrap()[1].value,
        FormValue::Text("你好".into())
    );
    // A second independent font graph with the same DR resource name is not
    // silently substituted, even if it happens to embed the same font program.
    let before = target.save_to_bytes().unwrap();
    let rev = target.commit_revision();
    assert!(
        target
            .import_pages_from_with_options(
                &source,
                &[0],
                2,
                &PageTransferOptions {
                    name_prefix: "other".into(),
                    ..Default::default()
                }
            )
            .is_err()
    );
    assert_eq!(target.commit_revision(), rev);
    assert_eq!(target.save_to_bytes().unwrap(), before);
}

#[test]
fn unknown_live_page_back_references_cannot_become_dangling() {
    use pdf::objects::pdf_reference::PdfReference;
    let doc = fixture();
    let removed = doc.destination(0, DestinationView::Fit).unwrap().page;
    let survivor = doc.destination(1, DestinationView::Fit).unwrap().page;
    let mut page = doc.resolve_handle(&survivor).unwrap().into_dict().unwrap();
    page.try_insert(
        "Peer".into(),
        PdfObject::PdfReference(PdfReference::from_id(removed.object_id())),
    )
    .unwrap();
    doc.update_handle(&survivor, PdfObject::PdfDict(page))
        .unwrap();
    let before = doc.save_to_bytes().unwrap();
    let rev = doc.commit_revision();
    assert!(
        doc.delete_page_with_policy(0, PageDeletionPolicy::Retarget { page_index: 1 })
            .is_err()
    );
    assert_eq!(doc.commit_revision(), rev);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
}

#[test]
fn explicit_catalog_copy_keeps_named_targets_on_the_new_page() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.set_named_destination("start", 0).unwrap();
    doc.add_outline(None, "Start", &OutlineTarget::Named("start".into()))
        .unwrap();
    doc.copy_page_with_options(
        0,
        1,
        &PageTransferOptions {
            name_prefix: "copy".into(),
            include_navigation: true,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        doc.named_destinations().unwrap(),
        [("copy_start".into(), 1), ("start".into(), 0)]
    );
    assert_eq!(reopen(&doc).outline_items().unwrap().len(), 2);
}

#[test]
fn unknown_label_tree_extensions_block_every_automatic_remapping() {
    use pdf::objects::{object_id::ObjectId, pdf_string::PdfString};
    let doc = fixture();
    // Fixed object 2 is the catalog of this explicitly created document only.
    let id = ObjectId::new(2, 0);
    let mut root = doc.resolve_object(id).unwrap().into_dict().unwrap();
    let mut labels = root.get("PageLabels").unwrap().as_dict().unwrap().clone();
    labels
        .try_insert(
            "Custom".into(),
            PdfObject::PdfString(PdfString::new(b"keep me".to_vec(), false)),
        )
        .unwrap();
    root.try_insert("PageLabels".into(), PdfObject::PdfDict(labels))
        .unwrap();
    doc.update_object(id, PdfObject::PdfDict(root)).unwrap();
    let before = doc.save_to_bytes().unwrap();
    let rev = doc.commit_revision();
    assert!(doc.reorder_pages(&[1, 0]).is_err());
    assert!(doc.copy_page_with_options(0, 2, &options()).is_err());
    assert!(
        doc.delete_page_with_policy(0, PageDeletionPolicy::Retarget { page_index: 1 })
            .is_err()
    );
    assert!(doc.extract_pages(&[0, 1]).is_err());
    assert_eq!(doc.commit_revision(), rev);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
}

#[test]
fn assembly_annotation_permissions_and_signed_sources_are_enforced() {
    use pdf::{
        document::Document,
        objects::{pdf_dict::PdfDict, pdf_name::PdfName},
        security::{EncryptionOptions, SaveSecurity},
    };
    let source = fixture();
    let owner = Document::from_bytes(source.save_to_bytes().unwrap()).unwrap();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.permissions.modify = true;
    settings.permissions.assemble = false;
    let bytes = owner
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
        .unwrap();
    let user =
        Document::from_bytes_with_password_and_options(bytes, "user", Default::default()).unwrap();
    let rev = user.commit_revision();
    assert!(user.copy_page(0, 2).is_err());
    assert!(user.reorder_pages(&[1, 0]).is_err());
    assert!(user.delete_page(1).is_err());
    assert!(user.import_pages_from(&source, &[0, 1], 2).is_err());
    assert_eq!(user.commit_revision(), rev);
    settings.permissions.assemble = true;
    settings.permissions.annotate = false;
    let bytes = owner
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
        .unwrap();
    let user =
        Document::from_bytes_with_password_and_options(bytes, "user", Default::default()).unwrap();
    let rev = user.commit_revision();
    assert!(
        user.import_pages_from_with_options(&source, &[0, 1], 2, &options())
            .is_err()
    );
    assert!(
        user.delete_page_with_policy(0, PageDeletionPolicy::Retarget { page_index: 1 })
            .is_err()
    );
    assert_eq!(user.commit_revision(), rev);
    let mut signature = PdfDict::default();
    signature
        .try_insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new("Sig".into())),
        )
        .unwrap();
    source
        .try_insert_object(PdfObject::PdfDict(signature))
        .unwrap();
    let signed = reopen(&source);
    let rev = signed.commit_revision();
    assert!(signed.copy_page_with_options(0, 2, &options()).is_err());
    assert!(signed.reorder_pages(&[1, 0]).is_err());
    assert!(
        signed
            .delete_page_with_policy(0, PageDeletionPolicy::Retarget { page_index: 1 })
            .is_err()
    );
    assert_eq!(signed.commit_revision(), rev);
}
