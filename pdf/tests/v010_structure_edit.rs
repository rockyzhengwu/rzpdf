use pdf::{
    document::{
        PdfDocument,
        destination::DestinationView,
        resource_manager::StandardFont,
        structure::{
            StructureAssociation, StructureAttributeWrite,
            StructureElementDefinition as Definition, StructureKid, StructureRemoval,
        },
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_reference::PdfReference},
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
fn parse(text: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn set(doc: &Doc, id: ObjectId, key: &str, value: PdfObject) {
    let mut dict = doc.resolve_object(id).unwrap().into_dict().unwrap();
    dict.try_insert(key.into(), value).unwrap();
    doc.update_object(id, PdfObject::PdfDict(dict)).unwrap();
}
fn created() -> (
    Doc,
    pdf::document::identity::ObjectHandle,
    pdf::document::identity::ObjectHandle,
) {
    let doc = Doc::new_with_options(ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    let f = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    doc.replace_page_contents(0, format!("BT /{f} 12 Tf (A) Tj (B) Tj ET").into_bytes())
        .unwrap();
    let group = doc
        .add_structure_element(None, 0, &Definition::new("Document"))
        .unwrap();
    let leaf = doc
        .add_structure_element(Some(&group), 0, &Definition::new("P"))
        .unwrap();
    (doc, group, leaf)
}
fn text(doc: &Doc) -> String {
    doc.extract_text_structure_order(Default::default())
        .unwrap()
        .unwrap()
        .text(&Limits::default())
        .unwrap()
}
#[test]
fn creates_explicit_heading_list_and_table_hierarchy_and_unicode_metadata() {
    let (doc, group, _) = created();
    let mut d = Definition::new("H1");
    d.title = Some("标题".into());
    d.language = Some("zh-CN".into());
    let heading = doc.add_structure_element(Some(&group), 0, &d).unwrap();
    let list = doc
        .add_structure_element(Some(&group), 1, &Definition::new("L"))
        .unwrap();
    doc.add_structure_element(Some(&list), 0, &Definition::new("LI"))
        .unwrap();
    let table = doc
        .add_structure_element(Some(&group), 2, &Definition::new("Table"))
        .unwrap();
    let row = doc
        .add_structure_element(Some(&table), 0, &Definition::new("TR"))
        .unwrap();
    doc.add_structure_element(Some(&row), 0, &Definition::new("TD"))
        .unwrap();
    let tree = doc.structure_tree().unwrap().unwrap();
    assert_eq!(tree.elements[1].handle, heading);
    assert_eq!(tree.elements[1].title.as_deref(), Some("标题"));
    let bytes = doc.save_to_bytes().unwrap();
    let reopened = Doc::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(
        reopened.structure_tree().unwrap().unwrap().elements.len(),
        8
    );
    reopened.structure_associations().unwrap();
}
#[test]
fn marking_allocates_page_mcids_and_k_order_without_changing_original_glyphs() {
    let (doc, group, first) = created();
    let second = doc
        .add_structure_element(Some(&group), 0, &Definition::new("P"))
        .unwrap();
    let mark = doc
        .mark_structure_content(&first, 0, &doc.stamp(), 2..3, 0, None)
        .unwrap();
    assert_eq!(mark.mcid, 0);
    assert!(doc.is_stamp_current(&mark.stamp));
    // BMC/EMC add two operation slots around A; B is now operation 5.
    let mark = doc
        .mark_structure_content(&second, 0, &doc.stamp(), 5..6, 0, Some("你好"))
        .unwrap();
    assert_eq!(mark.mcid, 1);
    assert_eq!(text(&doc), "你好A");
    let raw = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text(pdf::page::marked_text::MarkedTextOptions {
            use_actual_text: false,
            include_artifacts: true,
        })
        .unwrap();
    assert_eq!(raw.text(&Limits::default()).unwrap(), "AB");
    let reopened =
        Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(text(&reopened), "你好A");
    assert_ne!(mark.page.document_identity(), &reopened.identity());
}
#[test]
fn moving_content_nodes_preserves_pg_and_parenttree_and_reorders_extraction() {
    let (doc, group, first) = created();
    doc.mark_structure_content(&first, 0, &doc.stamp(), 2..3, 0, None)
        .unwrap();
    let second = doc
        .add_structure_element(Some(&group), 1, &Definition::new("P"))
        .unwrap();
    doc.mark_structure_content(&second, 0, &doc.stamp(), 5..6, 0, None)
        .unwrap();
    assert_eq!(text(&doc), "AB");
    doc.move_structure_element(&second, Some(&group), 0)
        .unwrap();
    assert_eq!(text(&doc), "BA");
    let nested = doc
        .add_structure_element(Some(&group), 2, &Definition::new("Div"))
        .unwrap();
    doc.move_structure_element(&first, Some(&nested), 0)
        .unwrap();
    assert_eq!(text(&doc), "BA");
    doc.structure_associations().unwrap();
}
#[test]
fn promotion_keeps_mixed_order_and_transfers_direct_content_owner() {
    let (doc, group, leaf) = created();
    doc.mark_structure_content(&leaf, 0, &doc.stamp(), 2..3, 0, None)
        .unwrap();
    let child = doc
        .add_structure_element(Some(&leaf), 0, &Definition::new("Span"))
        .unwrap();
    doc.mark_structure_content(&child, 0, &doc.stamp(), 5..6, 0, None)
        .unwrap();
    assert_eq!(text(&doc), "BA");
    doc.remove_structure_element(&leaf, StructureRemoval::PromoteChildren)
        .unwrap();
    assert_eq!(text(&doc), "BA");
    let tree = doc.structure_tree().unwrap().unwrap();
    assert_eq!(tree.elements.len(), 2);
    assert_eq!(tree.elements[1].parent, group);
    assert!(matches!(
        &tree.elements[0].children[1],
        StructureKid::UnvalidatedContent(_)
    ));
    let associations = doc.structure_associations().unwrap().unwrap();
    assert!(
        matches!(&associations.items[1],StructureAssociation::MarkedContent {element,mcid:0,..} if element==&group)
    );
    assert!(
        doc.update_structure_element(&leaf, &Definition::new("P"))
            .is_err()
    );
}
#[test]
fn empty_only_and_root_content_promotion_refuse_without_mutation() {
    let (doc, group, leaf) = created();
    doc.mark_structure_content(&leaf, 0, &doc.stamp(), 2..3, 0, None)
        .unwrap();
    let before = doc.save_to_bytes().unwrap();
    let commit = doc.commit_revision();
    assert!(
        doc.remove_structure_element(&leaf, StructureRemoval::EmptyOnly)
            .is_err()
    );
    doc.move_structure_element(&leaf, None, 1).unwrap();
    let moved = doc.save_to_bytes().unwrap();
    assert!(
        doc.remove_structure_element(&leaf, StructureRemoval::PromoteChildren)
            .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), moved);
    doc.move_structure_element(&leaf, Some(&group), 0).unwrap();
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert_eq!(doc.commit_revision(), commit + 2);
}
#[test]
fn empty_node_deletion_and_container_promotion_keep_surviving_handles() {
    let (doc, group, leaf) = created();
    let empty = doc
        .add_structure_element(Some(&group), 0, &Definition::new("Div"))
        .unwrap();
    doc.remove_structure_element(&empty, StructureRemoval::EmptyOnly)
        .unwrap();
    doc.remove_structure_element(&group, StructureRemoval::PromoteChildren)
        .unwrap();
    assert_eq!(
        doc.structure_tree().unwrap().unwrap().elements[0].handle,
        leaf
    );
}
#[test]
fn rolemap_and_local_attribute_updates_are_explicit_and_unicode_round_trips() {
    let (doc, _, leaf) = created();
    doc.set_structure_role_mapping("Custom", Some("P")).unwrap();
    let mut d = Definition::new("Custom");
    d.actual_text = Some("替代".into());
    d.alternate_text = Some("description".into());
    d.attributes.push(StructureAttributeWrite {
        owner: "Layout".into(),
        values: parse("<< /Placement /Block /SpaceBefore 12 >>")
            .into_dict()
            .unwrap(),
    });
    doc.update_structure_element(&leaf, &d).unwrap();
    let e = doc
        .structure_tree()
        .unwrap()
        .unwrap()
        .elements
        .pop()
        .unwrap();
    assert_eq!(e.mapped_role.as_deref(), Some("P"));
    assert_eq!(e.attribute_bindings.len(), 1);
    assert_eq!(text(&doc), "替代");
    doc.set_structure_role_mapping("Custom", None).unwrap();
    assert_eq!(
        doc.structure_tree().unwrap().unwrap().elements[1]
            .mapped_role
            .as_deref(),
        Some("Custom")
    );
    let mut d = Definition::new("P");
    d.actual_text = Some(String::new());
    doc.update_structure_element(&leaf, &d).unwrap();
    assert_eq!(text(&doc), "");
    assert!(
        doc.structure_tree().unwrap().unwrap().elements[1]
            .attribute_bindings
            .is_empty()
    );
}
#[test]
fn cycles_indices_foreign_handles_and_bad_definitions_roll_back() {
    let (doc, group, leaf) = created();
    let (other, _, foreign) = created();
    let before = doc.save_to_bytes().unwrap();
    let commit = doc.commit_revision();
    assert!(doc.move_structure_element(&group, Some(&leaf), 0).is_err());
    assert!(doc.move_structure_element(&leaf, Some(&group), 99).is_err());
    assert!(
        doc.add_structure_element(Some(&foreign), 0, &Definition::new("P"))
            .is_err()
    );
    assert!(
        other
            .update_structure_element(&leaf, &Definition::new("P"))
            .is_err()
    );
    assert!(
        doc.update_structure_element(&leaf, &Definition::new(""))
            .is_err()
    );
    assert!(doc.set_structure_role_mapping("A", Some("A")).is_err());
    doc.set_structure_role_mapping("A", Some("B")).unwrap();
    let mapped = doc.save_to_bytes().unwrap();
    assert!(doc.set_structure_role_mapping("B", Some("A")).is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), mapped);
    doc.set_structure_role_mapping("A", None).unwrap();
    // Empty RoleMap is intentionally retained, compare the unchanged model instead.
    assert_eq!(doc.structure_tree().unwrap().unwrap().elements.len(), 2);
    assert_eq!(doc.commit_revision(), commit + 2);
    assert!(!before.is_empty());
}
#[test]
fn outer_error_and_panic_restore_all_objects_and_do_not_commit_handles() {
    let (doc, group, leaf) = created();
    let before = doc.save_to_bytes().unwrap();
    let commit = doc.commit_revision();
    let result: Result<(), _> = doc.edit_transaction(|doc| {
        doc.mark_structure_content(&leaf, 0, &doc.stamp(), 2..3, 0, Some("logical"))?;
        doc.add_structure_element(Some(&group), 0, &Definition::new("H1"))?;
        Err(PdfError::DocumentError("outer".into()))
    });
    assert!(result.is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(), _> = doc.edit_transaction(|doc| {
                doc.mark_structure_content(&leaf, 0, &doc.stamp(), 2..3, 0, None)?;
                panic!("outer unwind");
            });
        }))
        .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert_eq!(doc.commit_revision(), commit);
    doc.mark_structure_content(&leaf, 0, &doc.stamp(), 2..3, 0, None)
        .unwrap();
    assert_eq!(text(&doc), "A");
}
#[test]
fn source_snapshots_and_existing_marked_selections_cannot_be_reused() {
    let (doc, _, leaf) = created();
    let stamp = doc.stamp();
    doc.mark_structure_content(&leaf, 0, &stamp, 2..3, 0, None)
        .unwrap();
    let before = doc.save_to_bytes().unwrap();
    let commit = doc.commit_revision();
    assert!(
        doc.mark_structure_content(&leaf, 0, &stamp, 2..3, 0, None)
            .is_err()
    );
    assert!(
        doc.mark_structure_content(&leaf, 0, &doc.stamp(), 3..4, 0, None)
            .is_err()
    );
    assert!(
        doc.mark_structure_content(&leaf, 0, &doc.stamp(), 2..5, 0, None)
            .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert_eq!(doc.commit_revision(), commit);
}
#[test]
fn path_text_graphics_and_stream_boundaries_reject_partial_scopes() {
    for (source, range) in [
        ("q 0 0 10 10 re f Q", 0..1),
        ("0 0 10 10 re f", 1..2),
        ("BT ET", 0..1),
        ("q Q q Q", 1..3),
    ] {
        let (doc, _, leaf) = created();
        doc.replace_page_contents(0, source.as_bytes().to_vec())
            .unwrap();
        let before = doc.save_to_bytes().unwrap();
        assert!(
            doc.mark_structure_content(&leaf, 0, &doc.stamp(), range, 0, None)
                .is_err(),
            "{source}"
        );
        assert_eq!(doc.save_to_bytes().unwrap(), before);
    }
    let (doc, _, leaf) = created();
    doc.replace_page_contents(0, b"q Q\n".to_vec()).unwrap();
    doc.append_page_contents(0, b"q Q\n".to_vec()).unwrap();
    assert!(
        doc.mark_structure_content(&leaf, 0, &doc.stamp(), 0..4, 0, None)
            .is_err()
    );
    doc.mark_structure_content(&leaf, 0, &doc.stamp(), 0..2, 0, Some(""))
        .unwrap();
}
#[test]
fn fresh_content_stream_isolates_shared_page_and_original_stream() {
    let (doc, _, leaf) = created();
    doc.add_page(1).unwrap();
    let page0 = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let page1 = doc
        .destination(1, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let d = doc.resolve_object(page0).unwrap().into_dict().unwrap();
    let shared = d.get("Contents").unwrap().clone();
    set(&doc, page1, "Contents", shared.clone());
    set(
        &doc,
        page1,
        "Resources",
        d.get("Resources").unwrap().clone(),
    );
    let original = doc.resolve_object(shared.as_object_id().unwrap()).unwrap();
    doc.mark_structure_content(&leaf, 0, &doc.stamp(), 2..3, 0, None)
        .unwrap();
    assert_eq!(
        doc.resolve_object(shared.as_object_id().unwrap()).unwrap(),
        original
    );
    assert_eq!(
        doc.resolve_object(page1)
            .unwrap()
            .as_dict()
            .unwrap()
            .get("Contents"),
        Some(&shared)
    );
    assert_eq!(
        doc.structure_associations().unwrap().unwrap().items.len(),
        1
    );
}
#[test]
fn unsupported_reverse_extensions_and_inconsistent_parenttree_fail_closed() {
    let (doc, _, leaf) = created();
    let root = doc.structure_tree().unwrap().unwrap().root.object_id();
    set(&doc, root, "IDTree", parse("<< /Names [] >>"));
    let before = doc.save_to_bytes().unwrap();
    assert!(
        doc.update_structure_element(&leaf, &Definition::new("H1"))
            .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    let (doc, _, leaf) = created();
    doc.mark_structure_content(&leaf, 0, &doc.stamp(), 2..3, 0, None)
        .unwrap();
    let root = doc.structure_tree().unwrap().unwrap().root.object_id();
    set(&doc, root, "ParentTree", parse("<< /Nums [0 [null]] >>"));
    assert!(doc.move_structure_element(&leaf, None, 1).is_err());
}
#[test]
fn direct_attribute_limits_reserved_keys_and_references_do_not_write() {
    let (doc, _, leaf) = created();
    let before = doc.save_to_bytes().unwrap();
    for values in ["<< /O /Layout >>", "<< /NS 2 0 R >>", "<< /Value 2 0 R >>"] {
        let mut d = Definition::new("P");
        d.attributes.push(StructureAttributeWrite {
            owner: "Layout".into(),
            values: parse(values).into_dict().unwrap(),
        });
        assert!(doc.update_structure_element(&leaf, &d).is_err());
        assert_eq!(doc.save_to_bytes().unwrap(), before);
    }
    let mut d = Definition::new("P");
    for _ in 0..2 {
        d.attributes.push(StructureAttributeWrite {
            owner: "Layout".into(),
            values: parse("<< /Placement /Block >>").into_dict().unwrap(),
        });
    }
    assert!(doc.update_structure_element(&leaf, &d).is_err());
    let bounded = Doc::new_with_options(ParseOptions {
        limits: Limits {
            max_token_bytes: 32,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let before = bounded.save_to_bytes().unwrap();
    assert!(
        bounded
            .add_structure_element(None, 0, &Definition::new("x".repeat(33)))
            .is_err()
    );
    assert_eq!(bounded.save_to_bytes().unwrap(), before);
}
#[test]
fn denied_modification_is_rejected_before_source_invalidation() {
    let (doc, _, _) = created();
    let mut security = pdf::security::EncryptionOptions::new("user", "owner").unwrap();
    security.permissions.modify = false;
    let bytes = doc
        .save_to_bytes_with_security(
            Default::default(),
            pdf::security::SaveSecurity::Set(&security),
        )
        .unwrap();
    let protected = Doc::from_reader_with_password_and_options(
        Cursor::new(bytes),
        "user",
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )
    .unwrap();
    let tree = protected.structure_tree().unwrap().unwrap();
    let stamp = protected.stamp();
    assert!(matches!(
        protected.update_structure_element(&tree.elements[1].handle, &Definition::new("H1")),
        Err(PdfError::PermissionDenied(_))
    ));
    assert!(protected.is_stamp_current(&stamp));
}

#[test]
fn old_versions_require_explicit_upgrade_and_signature_sources_stay_read_only() {
    use pdf::document::save_options::{PdfVersion, SaveOptions};
    let old = Doc::new_with_version(PdfVersion::V1_3).unwrap();
    let stamp = old.stamp();
    assert!(
        old.add_structure_element(None, 0, &Definition::new("Document"))
            .is_err()
    );
    assert!(old.is_stamp_current(&stamp));
    let bytes = old
        .save_to_bytes_with_options(SaveOptions {
            version: Some(PdfVersion::V1_4),
            ..Default::default()
        })
        .unwrap();
    let upgraded = Doc::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    upgraded
        .add_structure_element(None, 0, &Definition::new("Document"))
        .unwrap();
    let (doc, _, _) = created();
    doc.try_insert_object(parse("<< /Type /Sig >>")).unwrap();
    let signed =
        Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let stamp = signed.stamp();
    let leaf = signed.structure_tree().unwrap().unwrap().elements[1]
        .handle
        .clone();
    assert!(
        signed
            .update_structure_element(&leaf, &Definition::new("H1"))
            .is_err()
    );
    assert!(signed.is_stamp_current(&stamp));
}

#[test]
fn deep_structure_and_large_metadata_fail_with_atomic_rollback() {
    let doc = Doc::new_with_options(ParseOptions {
        limits: Limits {
            max_depth: 3,
            max_text_chars: 24,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let a = doc
        .add_structure_element(None, 0, &Definition::new("Document"))
        .unwrap();
    let b = doc
        .add_structure_element(Some(&a), 0, &Definition::new("Div"))
        .unwrap();
    let c = doc
        .add_structure_element(Some(&b), 0, &Definition::new("P"))
        .unwrap();
    let commit = doc.commit_revision();
    let tree = doc.structure_tree().unwrap();
    assert!(
        doc.add_structure_element(Some(&c), 0, &Definition::new("Span"))
            .is_err()
    );
    let mut d = Definition::new("P");
    d.actual_text = Some("x".repeat(25));
    assert!(doc.update_structure_element(&c, &d).is_err());
    assert_eq!(doc.commit_revision(), commit);
    assert_eq!(
        doc.structure_tree().unwrap().unwrap().elements,
        tree.unwrap().elements
    );
}

#[test]
fn parenttree_nextkey_null_holes_and_multiple_pages_are_not_reused() {
    let (doc, _, leaf) = created();
    let root = doc.structure_tree().unwrap().unwrap().root.object_id();
    set(&doc, root, "ParentTreeNextKey", parse("12"));
    let mark = doc
        .mark_structure_content(&leaf, 0, &doc.stamp(), 2..3, 0, None)
        .unwrap();
    assert_eq!(mark.mcid, 0);
    let page = mark.page.object_id();
    assert_eq!(
        doc.resolve_object(page)
            .unwrap()
            .as_dict()
            .unwrap()
            .get("StructParents")
            .unwrap()
            .as_number()
            .unwrap()
            .as_u32_checked()
            .unwrap(),
        12
    );
    let root_dict = doc.resolve_object(root).unwrap().into_dict().unwrap();
    let mut pt = root_dict
        .get("ParentTree")
        .unwrap()
        .as_dict()
        .unwrap()
        .clone();
    pt.try_insert(
        "Nums".into(),
        parse(&format!(
            "[12 [{} 0 R null null]]",
            leaf.object_id().obj_num()
        )),
    )
    .unwrap();
    set(&doc, root, "ParentTree", PdfObject::PdfDict(pt));
    let mark = doc
        .mark_structure_content(&leaf, 0, &doc.stamp(), 5..6, 1, None)
        .unwrap();
    assert_eq!(mark.mcid, 3);
    doc.add_page(1).unwrap();
    doc.replace_page_contents(1, b"q Q\n".to_vec()).unwrap();
    let mark = doc
        .mark_structure_content(&leaf, 1, &doc.stamp(), 0..2, 2, Some("page2"))
        .unwrap();
    assert_eq!(mark.mcid, 0);
    assert_eq!(
        doc.resolve_object(mark.page.object_id())
            .unwrap()
            .as_dict()
            .unwrap()
            .get("StructParents")
            .unwrap()
            .as_number()
            .unwrap()
            .as_u32_checked()
            .unwrap(),
        13
    );
    assert_eq!(text(&doc), "ABpage2");
}

#[test]
fn unicode_roles_and_name_escaping_preserve_lexical_content() {
    let (doc, _, leaf) = created();
    // Names with delimiters are encoded by the ordinary object writer, not interpolated.
    doc.set_structure_role_mapping("My role/#", Some("P"))
        .unwrap();
    let mut def = Definition::new("My role/#");
    def.title = Some("元数据".into());
    doc.update_structure_element(&leaf, &def).unwrap();
    doc.mark_structure_content(&leaf, 0, &doc.stamp(), 2..3, 0, Some("(替代)\\\n"))
        .unwrap();
    let bytes = doc.save_to_bytes().unwrap();
    let reopened = Doc::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(text(&reopened), "(替代)\\\n");
    assert_eq!(
        reopened.structure_tree().unwrap().unwrap().elements[1].role,
        "My role/#"
    );
}

#[test]
fn existing_form_mcr_and_annotation_objr_transfer_parenttree_owners_on_promotion() {
    use pdf::objects::pdf_stream::PdfStream;
    let (doc, group, leaf) = created();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let form=doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(parse("<< /Type /XObject /Subtype /Form /BBox [0 0 10 10] /Resources << >> /StructParents 4 >>").into_dict().unwrap(),b"/Span << /MCID 0 >> BDC 0 0 4 4 re f EMC".to_vec()).unwrap())).unwrap();
    let annot = doc
        .try_insert_object(parse(&format!(
            "<< /Type /Annot /Subtype /Square /Rect [0 0 10 10] /P {} 0 R /StructParent 5 >>",
            page.obj_num()
        )))
        .unwrap();
    set(
        &doc,
        page,
        "Annots",
        parse(&format!("[{} 0 R]", annot.obj_num())),
    );
    set(
        &doc,
        page,
        "Resources",
        parse(&format!("<< /XObject << /Fx {} 0 R >> >>", form.obj_num())),
    );
    doc.replace_page_contents(0, b"/Fx Do /Fx Do".to_vec())
        .unwrap();
    set(
        &doc,
        leaf.object_id(),
        "K",
        parse(&format!(
            "[<< /Type /MCR /Pg {} 0 R /Stm {} 0 R /MCID 0 >> << /Type /OBJR /Pg {} 0 R /Obj {} 0 R >>]",
            page.obj_num(),
            form.obj_num(),
            page.obj_num(),
            annot.obj_num()
        )),
    );
    let root = doc.structure_tree().unwrap().unwrap().root.object_id();
    set(
        &doc,
        root,
        "ParentTree",
        parse(&format!(
            "<< /Nums [4 [{} 0 R] 5 {} 0 R] >>",
            leaf.object_id().obj_num(),
            leaf.object_id().obj_num()
        )),
    );
    set(&doc, root, "ParentTreeNextKey", parse("6"));
    doc.structure_associations().unwrap();
    let original = doc.resolve_object(form).unwrap();
    doc.remove_structure_element(&leaf, StructureRemoval::PromoteChildren)
        .unwrap();
    assert_eq!(doc.resolve_object(form).unwrap(), original);
    for a in doc.structure_associations().unwrap().unwrap().items {
        match a {
            StructureAssociation::MarkedContent { element, .. }
            | StructureAssociation::ObjectReference { element, .. } => assert_eq!(element, group),
        }
    }
    let reopened =
        Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    reopened.structure_associations().unwrap();
}

#[test]
fn class_revision_and_namespaces_are_preserved_while_local_a_is_replaced() {
    let (doc, _, leaf) = created();
    let root = doc.structure_tree().unwrap().unwrap().root.object_id();
    set(&doc, ObjectId::new(2, 0), "Version", parse("/2.0"));
    let ns = doc
        .try_insert_object(parse(
            "<< /NS (https://example.invalid/custom) /RoleMapNS << /Custom /P >> >>",
        ))
        .unwrap();
    set(
        &doc,
        root,
        "Namespaces",
        parse(&format!("[{} 0 R]", ns.obj_num())),
    );
    set(
        &doc,
        root,
        "ClassMap",
        parse("<< /Body << /O /Layout /Placement /Block >> >>"),
    );
    set(&doc, leaf.object_id(), "NS", reference(ns));
    set(&doc, leaf.object_id(), "C", parse("/Body"));
    set(&doc, leaf.object_id(), "R", parse("2"));
    let mut d = Definition::new("Custom");
    d.language = Some("en".into());
    d.attributes.push(StructureAttributeWrite {
        owner: "Layout".into(),
        values: parse("<< /SpaceBefore 12 >>").into_dict().unwrap(),
    });
    doc.update_structure_element(&leaf, &d).unwrap();
    let node = doc
        .structure_tree()
        .unwrap()
        .unwrap()
        .elements
        .pop()
        .unwrap();
    assert_eq!(node.namespace.unwrap().object_id(), ns);
    assert_eq!(node.revision, 2);
    assert_eq!(node.classes[0].name, "Body");
    assert_eq!(node.resolved_role.name, "P");
    assert_eq!(node.attribute_bindings.len(), 2);
    assert_eq!(node.attribute_bindings[1].revision, 2);
}

#[test]
fn attribute_count_depth_and_nonfinite_values_are_bounded_before_commit() {
    let doc = Doc::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 64,
            max_depth: 6,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let leaf = doc
        .add_structure_element(None, 0, &Definition::new("P"))
        .unwrap();
    let before = doc.save_to_bytes().unwrap();
    let commit = doc.commit_revision();
    let mut d = Definition::new("P");
    for _ in 0..65 {
        d.attributes.push(StructureAttributeWrite {
            owner: "Layout".into(),
            values: Default::default(),
        });
    }
    assert!(matches!(
        doc.update_structure_element(&leaf, &d),
        Err(PdfError::ResourceLimit(_))
    ));
    d.attributes = vec![StructureAttributeWrite {
        owner: "Layout".into(),
        values: parse("<< /Value [[[[[[[1]]]]]]] >>").into_dict().unwrap(),
    }];
    assert!(matches!(
        doc.update_structure_element(&leaf, &d),
        Err(PdfError::ResourceLimit(_))
    ));
    d.attributes[0].values = Default::default();
    d.attributes[0]
        .values
        .try_insert(
            "Value".into(),
            PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(f32::NAN)),
        )
        .unwrap();
    assert!(doc.update_structure_element(&leaf, &d).is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert_eq!(doc.commit_revision(), commit);
}
