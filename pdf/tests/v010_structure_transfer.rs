use pdf::{
    document::{
        PdfDocument,
        destination::DestinationView,
        page_transfer::{PageDeletionPolicy, PageTransferOptions},
        resource_manager::StandardFont,
        structure::{
            StructureAssociation, StructureContentDomain, StructureElementDefinition as Definition,
        },
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference,
        pdf_stream::PdfStream,
    },
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::{
    io::Cursor,
    panic::{AssertUnwindSafe, catch_unwind},
};
type Doc = PdfDocument<Cursor<Vec<u8>>>;
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn dict(doc: &Doc, id: ObjectId) -> PdfDict {
    doc.resolve_object(id).unwrap().into_dict().unwrap()
}
fn set(doc: &Doc, id: ObjectId, key: &str, value: PdfObject) {
    let mut d = dict(doc, id);
    d.try_insert(key.into(), value).unwrap();
    doc.update_object(id, PdfObject::PdfDict(d)).unwrap();
}
fn remove(doc: &Doc, id: ObjectId, key: &str) {
    let mut d = dict(doc, id);
    d.remove(key);
    doc.update_object(id, PdfObject::PdfDict(d)).unwrap();
}
fn page(doc: &Doc, index: u32) -> ObjectId {
    doc.destination(index, DestinationView::Fit)
        .unwrap()
        .page
        .object_id()
}
fn fixture() -> Doc {
    let doc = Doc::new_with_options(ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    })
    .unwrap();
    for (i, text) in ["A", "B"].into_iter().enumerate() {
        doc.add_page(i as u32).unwrap();
        let f = doc
            .resources()
            .register_standard_font(i as u32, StandardFont::Courier)
            .unwrap();
        doc.replace_page_contents(
            i as u32,
            format!("BT /{f} 12 Tf ({text}) Tj ET\n").into_bytes(),
        )
        .unwrap();
    }
    let group = doc
        .add_structure_element(None, 0, &Definition::new("Document"))
        .unwrap();
    for i in 0..2 {
        let leaf = doc
            .add_structure_element(Some(&group), i, &Definition::new("P"))
            .unwrap();
        doc.mark_structure_content(&leaf, i as u32, &doc.stamp(), 2..3, 0, None)
            .unwrap();
    }
    doc
}
fn text(doc: &Doc) -> String {
    doc.extract_text_structure_order(Default::default())
        .unwrap()
        .unwrap()
        .text(&Limits::default())
        .unwrap()
}
fn page_text(doc: &Doc, index: u32) -> String {
    doc.get_page(index)
        .unwrap()
        .extract_marked_text(Default::default())
        .unwrap()
        .text(&Limits::default())
        .unwrap()
}
fn reopen(doc: &Doc) -> Doc {
    Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap()
}
fn root(doc: &Doc) -> ObjectId {
    doc.structure_tree().unwrap().unwrap().root.object_id()
}
fn key(doc: &Doc, page: ObjectId) -> u32 {
    dict(doc, page)
        .get("StructParents")
        .unwrap()
        .as_number()
        .unwrap()
        .as_u32_checked()
        .unwrap()
}

#[test]
fn copy_projects_selected_branch_and_allocates_isolated_domain() {
    let doc = fixture();
    let original = page(&doc, 0);
    let stream = dict(&doc, original).get("Contents").cloned().unwrap();
    let raw = doc.resolve_object(stream.as_object_id().unwrap()).unwrap();
    let copied = doc.copy_page(0, 2).unwrap();
    assert_ne!(key(&doc, copied), key(&doc, original));
    assert_eq!(text(&doc), "ABA");
    assert_eq!(page_text(&doc, 2), "A");
    assert_eq!(
        doc.resolve_object(stream.as_object_id().unwrap()).unwrap(),
        raw
    );
    assert_ne!(dict(&doc, copied).get("Contents"), Some(&stream));
    assert_eq!(
        doc.structure_associations().unwrap().unwrap().items.len(),
        3
    );
    assert_eq!(text(&reopen(&doc)), "ABA");
}
#[test]
fn import_preserves_declared_order_not_selection_or_page_order_and_keeps_source_unchanged() {
    let source = fixture();
    let before = source.save_to_bytes().unwrap();
    let stamp = source.stamp();
    let target = fixture();
    let imported = target.import_pages_from(&source, &[1, 0], 1).unwrap();
    assert_eq!(imported.len(), 2);
    assert_eq!(page_text(&target, 1), "B");
    assert_eq!(page_text(&target, 2), "A");
    assert_eq!(text(&target), "ABAB");
    let mut keys = std::collections::HashSet::new();
    for i in 0..4 {
        assert!(keys.insert(key(&target, page(&target, i))));
    }
    assert_eq!(source.save_to_bytes().unwrap(), before);
    assert!(source.is_stamp_current(&stamp));
    assert_eq!(text(&reopen(&target)), "ABAB");
}
#[test]
fn import_into_empty_document_handles_numerically_equal_source_and_target_ids() {
    let source = fixture();
    let target = Doc::new().unwrap();
    target.import_pages_from(&source, &[0, 1], 0).unwrap();
    assert_eq!(text(&target), "AB");
    assert_eq!(text(&reopen(&target)), "AB");
}
#[test]
fn subset_extraction_and_merge_use_the_same_structure_projection() {
    let source = fixture();
    let subset = source.extract_pages(&[1]).unwrap();
    assert_eq!(text(&subset), "B");
    assert_eq!(subset.structure_tree().unwrap().unwrap().elements.len(), 2);
    let target = Doc::new().unwrap();
    target.merge_document(&source).unwrap();
    assert_eq!(text(&target), "AB");
}
#[test]
fn reorder_changes_physical_order_without_rewriting_declared_k_or_parenttree() {
    let doc = fixture();
    let oldroot = dict(&doc, root(&doc));
    let a = page(&doc, 0);
    let b = page(&doc, 1);
    doc.reorder_pages(&[1, 0]).unwrap();
    assert_eq!([page(&doc, 0), page(&doc, 1)], [b, a]);
    assert_eq!(text(&doc), "AB");
    assert_eq!(dict(&doc, root(&doc)), oldroot);
    assert_eq!(text(&reopen(&doc)), "AB");
}
#[test]
fn deletion_prunes_selected_content_branch_and_reverse_keys_without_retargeting_text() {
    let doc = fixture();
    let old = key(&doc, page(&doc, 0));
    doc.delete_page(0).unwrap();
    assert_eq!(text(&doc), "B");
    let tree = doc.structure_tree().unwrap().unwrap();
    assert_eq!(tree.elements.len(), 2);
    let nums = dict(&doc, root(&doc))
        .get("ParentTree")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("Nums")
        .unwrap()
        .as_array()
        .unwrap()
        .clone();
    assert_ne!(
        nums.get(0)
            .unwrap()
            .as_number()
            .unwrap()
            .as_u32_checked()
            .unwrap(),
        old
    );
    assert_eq!(text(&reopen(&doc)), "B");
    doc.delete_page(0).unwrap();
    assert!(doc.structure_tree().unwrap().unwrap().elements.is_empty());
    assert_eq!(text(&reopen(&doc)), "");
}
#[test]
fn navigation_retargeting_stays_separate_from_structure_pruning() {
    let doc = fixture();
    doc.set_named_destination("start", 0).unwrap();
    let before = doc.save_to_bytes().unwrap();
    assert!(doc.delete_page(0).is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    doc.delete_page_with_policy(0, PageDeletionPolicy::Retarget { page_index: 1 })
        .unwrap();
    assert_eq!(text(&doc), "B");
    assert_eq!(
        doc.named_destination_views().unwrap()[0].1.page.object_id(),
        page(&doc, 0)
    );
}
#[test]
fn partial_element_actualtext_and_unknown_reverse_extensions_refuse_without_mutation() {
    let doc = fixture();
    let group = doc.structure_tree().unwrap().unwrap().elements[0]
        .handle
        .clone();
    let mut d = Definition::new("Document");
    d.actual_text = Some("whole".into());
    doc.update_structure_element(&group, &d).unwrap();
    let before = doc.save_to_bytes().unwrap();
    assert!(doc.copy_page(0, 2).is_err());
    assert!(doc.delete_page(0).is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    let target = Doc::new().unwrap();
    target.import_pages_from(&doc, &[0, 1], 0).unwrap();
    assert_eq!(text(&target), "whole");
    let doc = fixture();
    set(&doc, root(&doc), "IDTree", parse("<< /Names [] >>"));
    let before = doc.save_to_bytes().unwrap();
    assert!(doc.copy_page(0, 2).is_err());
    assert!(doc.reorder_pages(&[1, 0]).is_err());
    assert!(doc.delete_page(0).is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), before);
}
#[test]
fn rolemap_and_classmap_conflicts_are_not_silently_overwritten() {
    let source = fixture();
    source
        .set_structure_role_mapping("Custom", Some("P"))
        .unwrap();
    let target = fixture();
    target
        .set_structure_role_mapping("Custom", Some("H1"))
        .unwrap();
    let before = target.save_to_bytes().unwrap();
    assert!(target.import_pages_from(&source, &[0], 2).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
    target
        .set_structure_role_mapping("Custom", Some("P"))
        .unwrap();
    target.import_pages_from(&source, &[0], 2).unwrap();
    set(
        &source,
        root(&source),
        "ClassMap",
        parse("<< /Body << /O /Layout /Placement /Block >> >>"),
    );
    set(
        &target,
        root(&target),
        "ClassMap",
        parse("<< /Body << /O /Layout /Placement /Inline >> >>"),
    );
    let before = target.save_to_bytes().unwrap();
    assert!(target.import_pages_from(&source, &[0], 3).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
}
#[test]
fn page_body_edits_keep_existing_mcids_and_reject_marker_removal_atomically() {
    let doc = fixture();
    let program = doc.get_page(0).unwrap().content_program().unwrap();
    let op = program
        .operations()
        .iter()
        .position(|o| o.operator.name() == "Tj")
        .unwrap();
    doc.replace_page_text_codes(0, &program, op, b"Z").unwrap();
    assert_eq!(text(&doc), "ZB");
    let expected = doc.get_page(0).unwrap().content_program().unwrap();
    let marker = expected
        .operations()
        .iter()
        .position(|o| o.operator.name() == "BDC")
        .unwrap();
    let before = doc.save_to_bytes().unwrap();
    let commit = doc.commit_revision();
    let replacement = pdf::page::content_program::OperationReplacement {
        index: marker,
        source: b"/Span BMC",
        operation_count: 1,
    };
    assert!(
        doc.replace_page_operation_sequences(0, &expected, &[replacement])
            .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert_eq!(doc.commit_revision(), commit);
}
#[test]
fn declared_actualtext_is_preserved_not_regenerated_from_new_glyphs() {
    let doc = fixture();
    let leaf = doc.structure_tree().unwrap().unwrap().elements[1]
        .handle
        .clone();
    let mut d = Definition::new("P");
    d.actual_text = Some("logical".into());
    doc.update_structure_element(&leaf, &d).unwrap();
    let program = doc.get_page(0).unwrap().content_program().unwrap();
    let op = program
        .operations()
        .iter()
        .position(|o| o.operator.name() == "Tj")
        .unwrap();
    doc.replace_page_text_codes(0, &program, op, b"Z").unwrap();
    assert_eq!(text(&doc), "logicalB");
    assert_eq!(page_text(&doc, 0), "Z");
}
#[test]
fn outer_error_and_unwind_restore_page_tree_structure_and_next_allocations() {
    let doc = fixture();
    let before = doc.save_to_bytes().unwrap();
    let commit = doc.commit_revision();
    let result: Result<(), _> = doc.edit_transaction(|doc| {
        doc.copy_page(0, 2)?;
        doc.delete_page(1)?;
        Err(PdfError::DocumentError("outer".into()))
    });
    assert!(result.is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(), _> = doc.edit_transaction(|doc| {
                doc.copy_page(0, 2)?;
                panic!("outer");
            });
        }))
        .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert_eq!(doc.commit_revision(), commit);
    doc.copy_page(0, 2).unwrap();
    assert_eq!(text(&doc), "ABA");
}

fn form_fixture(shared: bool) -> (Doc, ObjectId) {
    let doc = fixture();
    let a = page(&doc, 0);
    let b = page(&doc, 1);
    let leaf = doc.structure_tree().unwrap().unwrap().elements[1]
        .handle
        .object_id();
    let other = doc.structure_tree().unwrap().unwrap().elements[2]
        .handle
        .object_id();
    let form=doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(parse("<< /Type /XObject /Subtype /Form /BBox [0 0 20 20] /Resources << >> /StructParents 4 >>").into_dict().unwrap(),b"/Span << /MCID 0 /ActualText (F) >> BDC 0 0 10 10 re f EMC\n".to_vec()).unwrap())).unwrap();
    set(
        &doc,
        a,
        "Resources",
        parse(&format!("<< /XObject << /Fm {} 0 R >> >>", form.obj_num())),
    );
    remove(&doc, a, "StructParents");
    doc.replace_page_contents(0, b"/Fm Do\n".to_vec()).unwrap();
    if shared {
        let mut resources = dict(&doc, b)
            .get("Resources")
            .unwrap()
            .as_dict()
            .unwrap()
            .clone();
        resources
            .try_insert(
                "XObject".into(),
                parse(&format!("<< /Fm {} 0 R >>", form.obj_num())),
            )
            .unwrap();
        set(&doc, b, "Resources", PdfObject::PdfDict(resources));
        doc.append_page_contents(1, b" /Fm Do\n".to_vec()).unwrap();
    }
    set(
        &doc,
        leaf,
        "K",
        parse(&format!(
            "<< /Type /MCR /Pg {} 0 R /Stm {} 0 R /MCID 0 >>",
            a.obj_num(),
            form.obj_num()
        )),
    );
    set(
        &doc,
        root(&doc),
        "ParentTree",
        parse(&format!(
            "<< /Nums [1 [{} 0 R] 4 [{} 0 R]] >>",
            other.obj_num(),
            leaf.obj_num()
        )),
    );
    set(&doc, root(&doc), "ParentTreeNextKey", parse("5"));
    doc.structure_associations().unwrap();
    (doc, form)
}
#[test]
fn shared_form_copy_clones_domain_and_does_not_change_original_form_or_other_page() {
    let (doc, form) = form_fixture(true);
    let original = doc.resolve_object(form).unwrap();
    let other = dict(&doc, page(&doc, 1));
    doc.copy_page(0, 2).unwrap();
    assert_eq!(doc.resolve_object(form).unwrap(), original);
    assert_eq!(dict(&doc, page(&doc, 1)), other);
    let associations = doc.structure_associations().unwrap().unwrap();
    let domains: Vec<_> = associations
        .items
        .into_iter()
        .filter_map(|a| {
            if let StructureAssociation::MarkedContent {
                domain: StructureContentDomain::Form(h),
                ..
            } = a
            {
                Some(h.object_id())
            } else {
                None
            }
        })
        .collect();
    assert_eq!(domains.len(), 2);
    assert_ne!(domains[0], domains[1]);
    assert_eq!(page_text(&doc, 2), "F");
    reopen(&doc).structure_associations().unwrap();
}
#[test]
fn shared_form_declared_page_rebinds_to_unique_selected_or_remaining_page() {
    let (doc, _) = form_fixture(true);
    let target = Doc::new().unwrap();
    target.import_pages_from(&doc, &[1], 0).unwrap();
    assert_eq!(page_text(&target, 0), "BF");
    target.structure_associations().unwrap();
    doc.delete_page(0).unwrap();
    assert_eq!(page_text(&doc, 0), "BF");
    let items = doc.structure_associations().unwrap().unwrap().items;
    assert_eq!(items.len(), 2);
    assert!(items.iter().all(|a| match a {
        StructureAssociation::MarkedContent { page: h, .. }
        | StructureAssociation::ObjectReference { page: h, .. } => h.object_id() == page(&doc, 0),
    }));
}
#[test]
fn objc_annotation_copy_import_and_delete_rebind_owners_without_rewriting_appearance() {
    use pdf::document::annotations::{Annotation, AnnotationKind};
    let doc = fixture();
    let annot = doc
        .add_annotation(
            0,
            &Annotation::new(AnnotationKind::Square, [0., 0., 20., 20.]),
        )
        .unwrap();
    let leaf = doc.structure_tree().unwrap().unwrap().elements[1]
        .handle
        .object_id();
    let other = doc.structure_tree().unwrap().unwrap().elements[2]
        .handle
        .object_id();
    let pg = page(&doc, 0);
    set(&doc, annot.object_id(), "StructParent", parse("7"));
    set(
        &doc,
        leaf,
        "K",
        parse(&format!(
            "[<< /Type /MCR /Pg {} 0 R /MCID 0 >> << /Type /OBJR /Pg {} 0 R /Obj {} 0 R >>]",
            pg.obj_num(),
            pg.obj_num(),
            annot.object_id().obj_num()
        )),
    );
    set(
        &doc,
        root(&doc),
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [{} 0 R] 1 [{} 0 R] 7 {} 0 R] >>",
            leaf.obj_num(),
            other.obj_num(),
            leaf.obj_num()
        )),
    );
    set(&doc, root(&doc), "ParentTreeNextKey", parse("8"));
    let old = doc.resolve_object(annot.object_id()).unwrap();
    doc.copy_page(0, 2).unwrap();
    assert_eq!(doc.resolve_object(annot.object_id()).unwrap(), old);
    assert_eq!(
        doc.structure_associations().unwrap().unwrap().items.len(),
        5
    );
    let target = Doc::new().unwrap();
    target.import_pages_from(&doc, &[2], 0).unwrap();
    assert_eq!(
        target
            .structure_associations()
            .unwrap()
            .unwrap()
            .items
            .len(),
        2
    );
    doc.delete_page(0).unwrap();
    doc.structure_associations().unwrap();
    assert_eq!(text(&reopen(&doc)), "BA");
}
#[test]
fn namespaces_are_reused_for_copy_and_cross_document_uri_collisions_refuse() {
    let doc = fixture();
    set(&doc, ObjectId::new(2, 0), "Version", parse("/2.0"));
    let ns = doc
        .try_insert_object(parse(
            "<< /NS (https://example.invalid/ns) /RoleMapNS << /Custom /P >> >>",
        ))
        .unwrap();
    set(
        &doc,
        root(&doc),
        "Namespaces",
        parse(&format!("[{} 0 R]", ns.obj_num())),
    );
    let leaf = doc.structure_tree().unwrap().unwrap().elements[1]
        .handle
        .object_id();
    set(&doc, leaf, "NS", reference(ns));
    set(&doc, leaf, "S", parse("/Custom"));
    doc.copy_page(0, 2).unwrap();
    assert_eq!(doc.structure_tree().unwrap().unwrap().namespaces.len(), 1);
    let target = Doc::new_with_version(pdf::document::save_options::PdfVersion::V2_0).unwrap();
    target.import_pages_from(&doc, &[0], 0).unwrap();
    let before = target.save_to_bytes().unwrap();
    assert!(target.import_pages_from(&doc, &[0], 1).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
}
#[test]
fn permissions_and_signed_sources_keep_page_and_structure_snapshots_unchanged() {
    let doc = fixture();
    let mut protection = pdf::security::EncryptionOptions::new("user", "owner").unwrap();
    protection.permissions.assemble = false;
    let bytes = doc
        .save_to_bytes_with_security(
            Default::default(),
            pdf::security::SaveSecurity::Set(&protection),
        )
        .unwrap();
    let user =
        Doc::from_reader_with_password_and_options(Cursor::new(bytes), "user", Default::default())
            .unwrap();
    let stamp = user.stamp();
    assert!(user.copy_page(0, 2).is_err());
    assert!(user.delete_page(0).is_err());
    assert!(user.reorder_pages(&[1, 0]).is_err());
    assert!(user.is_stamp_current(&stamp));
    doc.try_insert_object(parse("<< /Type /Sig >>")).unwrap();
    let signed = reopen(&doc);
    let stamp = signed.stamp();
    assert!(signed.copy_page(0, 2).is_err());
    assert!(signed.delete_page(0).is_err());
    assert!(signed.is_stamp_current(&stamp));
}
#[test]
fn import_name_prefix_keeps_existing_field_policy_and_structure_independent() {
    let source = fixture();
    source
        .add_text_field(0, [10., 10., 90., 30.], "name", "A")
        .unwrap();
    let target = fixture();
    target
        .add_text_field(0, [10., 10., 90., 30.], "name", "B")
        .unwrap();
    let before = target.save_to_bytes().unwrap();
    assert!(target.import_pages_from(&source, &[0], 2).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
    target
        .import_pages_from_with_options(
            &source,
            &[0],
            2,
            &PageTransferOptions {
                name_prefix: "other".into(),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(text(&target), "ABA");
    target.structure_associations().unwrap();
}

#[test]
fn nested_edit_inside_page_mcid_isolated_but_structural_form_domain_edit_refuses() {
    use pdf::page::page_object::PageObject;
    let (doc, form) = form_fixture(false);
    // Remove stream ActualText for the legacy display-list source query.
    let raw = doc.resolve_object(form).unwrap();
    let mut d = raw.as_stream().unwrap().dict().clone();
    doc.update_object(
        form,
        PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                d.clone(),
                b"/Span << /MCID 0 >> BDC 0 0 10 10 re f EMC".to_vec(),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    let root_program = doc.get_page(0).unwrap().content_program().unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("path");
    };
    let source = path.nested_source().unwrap();
    let before = doc.save_to_bytes().unwrap();
    assert!(
        doc.replace_nested_path_numbers(
            0,
            &root_program,
            source,
            source.geometry_operation_indices[0],
            &[1., 1., 8., 8.]
        )
        .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    // The same Form becomes ordinary graphics owned by a page MCID.
    d.remove("StructParents");
    doc.update_object(
        form,
        PdfObject::PdfStream(PdfStream::from_decoded_data(d, b"0 0 10 10 re f".to_vec()).unwrap()),
    )
    .unwrap();
    let leaf = doc.structure_tree().unwrap().unwrap().elements[1]
        .handle
        .object_id();
    let pg = page(&doc, 0);
    let other = doc.structure_tree().unwrap().unwrap().elements[2]
        .handle
        .object_id();
    doc.replace_page_contents(0, b"/Span << /MCID 0 >> BDC /Fm Do EMC".to_vec())
        .unwrap();
    set(&doc, pg, "StructParents", parse("0"));
    set(
        &doc,
        leaf,
        "K",
        parse(&format!(
            "<< /Type /MCR /Pg {} 0 R /MCID 0 >>",
            pg.obj_num()
        )),
    );
    set(
        &doc,
        root(&doc),
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [{} 0 R] 1 [{} 0 R]] >>",
            leaf.obj_num(),
            other.obj_num()
        )),
    );
    let original = doc.resolve_object(form).unwrap();
    let root_program = doc.get_page(0).unwrap().content_program().unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("path");
    };
    let source = path.nested_source().unwrap();
    doc.replace_nested_path_numbers(
        0,
        &root_program,
        source,
        source.geometry_operation_indices[0],
        &[1., 1., 8., 8.],
    )
    .unwrap();
    assert_eq!(doc.resolve_object(form).unwrap(), original);
    doc.structure_associations().unwrap();
    reopen(&doc).structure_associations().unwrap();
}

#[test]
fn identical_numeric_references_do_not_make_cross_document_class_definitions_equal() {
    let source = fixture();
    let target = fixture();
    for (doc, value) in [(&source, 1), (&target, 2)] {
        while doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num() < 30 {}
        doc.update_object(ObjectId::new(30, 0), parse(&value.to_string()))
            .unwrap();
        set(
            doc,
            root(doc),
            "ClassMap",
            parse("<< /Body << /O /Layout /Value 30 0 R >> >>"),
        );
    }
    let before = target.save_to_bytes().unwrap();
    assert!(target.import_pages_from(&source, &[0], 2).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
}

#[test]
fn object_and_outer_operation_budgets_rollback_complete_tagged_import() {
    let source = fixture();
    let target = Doc::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 12,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let before = target.save_to_bytes().unwrap();
    let commit = target.commit_revision();
    assert!(target.import_pages_from(&source, &[0, 1], 0).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
    assert_eq!(target.commit_revision(), commit);
    let target = Doc::new().unwrap();
    let before = target.save_to_bytes().unwrap();
    let pool = pdf::budget::OperationBudget::new(1);
    assert!(
        target
            .edit_transaction_with_budget(&pool, |doc| doc.import_pages_from(&source, &[0], 0))
            .is_err()
    );
    assert_eq!(pool.live_bytes(), 0);
    assert_eq!(target.save_to_bytes().unwrap(), before);
    target.import_pages_from(&source, &[0], 0).unwrap();
    assert_eq!(text(&target), "A");
}

#[test]
fn tagged_widget_objr_uses_page_transfer_guard_without_unlocking_ordinary_field_edits() {
    let doc = fixture();
    doc.add_text_field(0, [10., 10., 90., 30.], "name", "A")
        .unwrap();
    let field = doc
        .form_field_tree()
        .unwrap()
        .into_iter()
        .find(|n| n.name == "name")
        .unwrap();
    let widget = field.widgets[0].handle.object_id();
    let tree = doc.structure_tree().unwrap().unwrap();
    let leaf = tree.elements[1].handle.object_id();
    let other = tree.elements[2].handle.object_id();
    let pg = page(&doc, 0);
    set(&doc, widget, "StructParent", parse("7"));
    set(
        &doc,
        leaf,
        "K",
        parse(&format!(
            "[<< /Type /MCR /Pg {} 0 R /MCID 0 >> << /Type /OBJR /Pg {} 0 R /Obj {} 0 R >>]",
            pg.obj_num(),
            pg.obj_num(),
            widget.obj_num()
        )),
    );
    set(
        &doc,
        root(&doc),
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [{} 0 R] 1 [{} 0 R] 7 {} 0 R] >>",
            leaf.obj_num(),
            other.obj_num(),
            leaf.obj_num()
        )),
    );
    set(&doc, root(&doc), "ParentTreeNextKey", parse("8"));
    let target = Doc::new().unwrap();
    target.import_pages_from(&doc, &[0], 0).unwrap();
    target.structure_associations().unwrap();
    target.form_fields().unwrap();
    doc.copy_page_with_options(
        0,
        2,
        &PageTransferOptions {
            name_prefix: "copy".into(),
            ..Default::default()
        },
    )
    .unwrap();
    doc.delete_page(0).unwrap();
    doc.structure_associations().unwrap();
    reopen(&doc).structure_associations().unwrap();
    assert!(
        doc.rename_form_field(
            &doc.form_field_tree()
                .unwrap()
                .iter()
                .find(|n| n.name == "copy_name")
                .unwrap()
                .handle,
            "new"
        )
        .is_err()
    );
}

#[test]
fn multiple_surviving_form_pages_refuse_arbitrary_pg_rebinding() {
    let (doc, form) = form_fixture(true);
    doc.add_page(2).unwrap();
    set(
        &doc,
        page(&doc, 2),
        "Resources",
        parse(&format!("<< /XObject << /Fm {} 0 R >> >>", form.obj_num())),
    );
    doc.replace_page_contents(2, b"/Fm Do".to_vec()).unwrap();
    doc.structure_associations().unwrap();
    let before = doc.save_to_bytes().unwrap();
    assert!(doc.delete_page(0).is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    let target = Doc::new().unwrap();
    let before = target.save_to_bytes().unwrap();
    assert!(target.import_pages_from(&doc, &[1, 2], 0).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
}

#[test]
fn optional_content_configuration_and_orphan_ocg_import_do_not_silently_drop_visibility() {
    let source = fixture();
    set(
        &source,
        ObjectId::new(2, 0),
        "OCProperties",
        parse("<< /OCGs [] /D << >> >>"),
    );
    let before = source.save_to_bytes().unwrap();
    assert!(source.copy_page(0, 2).is_err());
    assert!(source.delete_page(0).is_err());
    assert_eq!(source.save_to_bytes().unwrap(), before);
    let target = Doc::new().unwrap();
    let before = target.save_to_bytes().unwrap();
    assert!(target.import_pages_from(&source, &[0], 0).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
    let source = fixture();
    let pg = page(&source, 0);
    let mut resources = dict(&source, pg)
        .get("Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .clone();
    let ocg = source
        .try_insert_object(parse("<< /Type /OCG /Name (layer) >>"))
        .unwrap();
    resources
        .try_insert(
            "Properties".into(),
            parse(&format!("<< /Layer {} 0 R >>", ocg.obj_num())),
        )
        .unwrap();
    set(&source, pg, "Resources", PdfObject::PdfDict(resources));
    assert!(target.import_pages_from(&source, &[0], 0).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
}

#[test]
fn imported_definitions_cannot_capture_unmapped_roles_or_undefined_classes_in_either_graph() {
    for incoming_definition in [false, true] {
        for class in [false, true] {
            let source = fixture();
            let target = fixture();
            let (defined, undefined) = if incoming_definition {
                (&source, &target)
            } else {
                (&target, &source)
            };
            let leaf = undefined.structure_tree().unwrap().unwrap().elements[1]
                .handle
                .clone();
            if class {
                set(
                    defined,
                    root(defined),
                    "ClassMap",
                    parse("<< /Body << /O /Layout /Placement /Block >> >>"),
                );
                set(undefined, leaf.object_id(), "C", parse("/Body"));
            } else {
                defined
                    .set_structure_role_mapping("Unmapped", Some("P"))
                    .unwrap();
                undefined
                    .update_structure_element(&leaf, &Definition::new("Unmapped"))
                    .unwrap();
            }
            let before = target.save_to_bytes().unwrap();
            let source_before = source.save_to_bytes().unwrap();
            let commit = target.commit_revision();
            assert!(
                target.import_pages_from(&source, &[0], 2).is_err(),
                "incoming_definition={incoming_definition}, class={class}"
            );
            assert_eq!(target.save_to_bytes().unwrap(), before);
            assert_eq!(source.save_to_bytes().unwrap(), source_before);
            assert_eq!(target.commit_revision(), commit);
        }
    }
}
