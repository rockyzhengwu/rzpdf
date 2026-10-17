use pdf::{
    document::{PdfDocument, structure::StructureKid},
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference,
        pdf_string::PdfString,
    },
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;
type Doc = PdfDocument<Cursor<Vec<u8>>>;
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn update(doc: &Doc, id: ObjectId, text: &str) {
    doc.update_object(id, parse(text)).unwrap();
}
fn attach(doc: &Doc, id: ObjectId) {
    // Catalog object 2 belongs to this explicitly new-document fixture only.
    let mut root = doc
        .resolve_object(ObjectId::new(2, 0))
        .unwrap()
        .into_dict()
        .unwrap();
    root.try_insert("StructTreeRoot".into(), reference(id))
        .unwrap();
    doc.update_object(ObjectId::new(2, 0), PdfObject::PdfDict(root))
        .unwrap();
}
fn fixture() -> (Doc, ObjectId, ObjectId, ObjectId) {
    fixture_options(Default::default())
}
fn fixture_options(options: ParseOptions) -> (Doc, ObjectId, ObjectId, ObjectId) {
    let doc = PdfDocument::new_with_options(options).unwrap();
    doc.add_page(0).unwrap();
    let page = doc
        .destination(0, pdf::document::destination::DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let root = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let group = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let leaf = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    update(
        &doc,
        root,
        &format!(
            "<< /Type /StructTreeRoot /K {} 0 R /RoleMap << /Custom /P >> /ParentTree << /Nums [0 [{} 0 R]] >> >>",
            group.obj_num(),
            leaf.obj_num()
        ),
    );
    update(
        &doc,
        group,
        &format!(
            "<< /Type /StructElem /S /Document /P {} 0 R /K [{} 0 R] /T (Root) >>",
            root.obj_num(),
            leaf.obj_num()
        ),
    );
    update(
        &doc,
        leaf,
        &format!(
            "<< /Type /StructElem /S /Custom /P {} 0 R /Pg {} 0 R /K [0 << /Type /MCR /MCID 1 >> << /Type /OBJR >>] /Alt (alternate) /Lang (en-US) /Custom (keep me) >>",
            group.obj_num(),
            page.obj_num()
        ),
    );
    attach(&doc, root);
    (doc, root, group, leaf)
}
fn set(doc: &Doc, id: ObjectId, key: &str, value: PdfObject) {
    let mut d = doc.resolve_object(id).unwrap().into_dict().unwrap();
    d.try_insert(key.into(), value).unwrap();
    doc.update_object(id, PdfObject::PdfDict(d)).unwrap();
}

#[test]
fn missing_and_empty_structure_trees_are_distinct() {
    let doc = PdfDocument::new().unwrap();
    assert!(doc.structure_tree().unwrap().is_none());
    let root = doc
        .try_insert_object(parse("<< /Type /StructTreeRoot >>"))
        .unwrap();
    attach(&doc, root);
    let tree = doc.structure_tree().unwrap().unwrap();
    assert!(tree.elements.is_empty());
    assert!(tree.children.is_empty());
}

#[test]
fn ordered_read_only_hierarchy_keeps_metadata_and_unvalidated_content_explicit() {
    let (doc, root, group, leaf) = fixture();
    set(
        &doc,
        leaf,
        "ActualText",
        PdfObject::PdfString(PdfString::from_text_with_limits("你好", &Limits::default()).unwrap()),
    );
    let before = doc.save_to_bytes().unwrap();
    let stamp = doc.stamp();
    let rev = doc.revision();
    let tree = doc.structure_tree().unwrap().unwrap();
    assert_eq!(tree.root.object_id(), root);
    assert_eq!(tree.stamp, stamp);
    assert_eq!(
        tree.elements
            .iter()
            .map(|n| n.handle.object_id())
            .collect::<Vec<_>>(),
        [group, leaf]
    );
    assert_eq!(tree.elements[1].parent, tree.elements[0].handle);
    assert_eq!(tree.elements[1].role, "Custom");
    assert_eq!(tree.elements[1].mapped_role.as_deref(), Some("P"));
    assert_eq!(tree.elements[1].actual_text.as_deref(), Some("你好"));
    assert_eq!(
        tree.elements[1].alternate_text.as_deref(),
        Some("alternate")
    );
    assert_eq!(tree.elements[1].language.as_deref(), Some("en-US"));
    assert!(tree.elements[1].page.is_some());
    assert!(tree.unvalidated_parent_tree.is_some());
    assert_eq!(tree.elements[1].children.len(), 3);
    assert!(
        tree.elements[1]
            .children
            .iter()
            .all(|k| matches!(k, StructureKid::UnvalidatedContent(_)))
    );
    assert_eq!(doc.revision(), rev);
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    let reopened = Doc::from_bytes_with_mode(before, ParseMode::Strict).unwrap();
    let second = reopened.structure_tree().unwrap().unwrap();
    assert_eq!(second.elements[1].actual_text, tree.elements[1].actual_text);
    assert_ne!(second.root, tree.root);
    assert_eq!(
        reopened
            .resolve_handle(&second.elements[1].handle)
            .unwrap()
            .as_dict()
            .unwrap()
            .get("Custom"),
        Some(&parse("(keep me)"))
    );
    doc.add_page(1).unwrap();
    assert!(!doc.is_stamp_current(&tree.stamp));
}

#[test]
fn shared_children_cycles_and_wrong_parents_are_errors_not_recursion() {
    let (doc, root, group, leaf) = fixture();
    let original = doc.resolve_object(group).unwrap();
    set(
        &doc,
        group,
        "K",
        parse(&format!("[{} 0 R {} 0 R]", leaf.obj_num(), leaf.obj_num())),
    );
    assert!(doc.structure_tree().is_err());
    doc.update_object(group, original).unwrap();
    set(&doc, leaf, "K", reference(group));
    assert!(doc.structure_tree().is_err());
    set(&doc, leaf, "K", parse("0"));
    set(&doc, leaf, "P", reference(root));
    assert!(doc.structure_tree().is_err());
}

#[test]
fn cyclic_roles_are_inspectable_but_malformed_values_and_direct_ownership_reject() {
    let (doc, root, _, _) = fixture();
    let saved = doc.resolve_object(root).unwrap();
    set(
        &doc,
        root,
        "RoleMap",
        parse("<< /Custom /Other /Other /Custom >>"),
    );
    assert!(
        doc.structure_tree().unwrap().unwrap().elements[1]
            .mapped_role
            .is_none()
    );
    set(&doc, root, "RoleMap", parse("<< /Custom (not a name) >>"));
    assert!(doc.structure_tree().is_err());
    doc.update_object(root, saved).unwrap();
    for value in [
        "<< /Type /StructElem /S /P >>",
        "[null]",
        "-1",
        "[[]]",
        "<< /Type /FutureKid >>",
    ] {
        set(&doc, root, "K", parse(value));
        assert!(doc.structure_tree().is_err());
    }
}

#[test]
fn namespace_interpretation_and_invalid_pages_are_never_guessed() {
    let (doc, root, _, leaf) = fixture();
    set(&doc, leaf, "Pg", reference(root));
    assert!(doc.structure_tree().is_err());
    let (doc, root, _, _) = fixture();
    set(&doc, root, "Namespaces", parse("[]"));
    assert!(matches!(
        doc.structure_tree(),
        Err(PdfError::UnsupportedFeature(_))
    ));
    let (doc, _, _, leaf) = fixture();
    set(&doc, leaf, "NS", PdfObject::PdfDict(PdfDict::default()));
    assert!(matches!(
        doc.structure_tree(),
        Err(PdfError::UnsupportedFeature(_))
    ));
}

#[test]
fn optional_structelem_type_and_bidirectional_roles_remain_inspectable() {
    let (doc, root, _, leaf) = fixture();
    let mut node = doc.resolve_object(leaf).unwrap().into_dict().unwrap();
    node.remove("Type");
    doc.update_object(leaf, PdfObject::PdfDict(node)).unwrap();
    set(&doc, root, "RoleMap", parse("<< /Custom /P /P /Custom >>"));
    let tree = doc.structure_tree().unwrap().unwrap();
    assert_eq!(tree.elements[1].role, "Custom");
    assert!(tree.elements[1].mapped_role.is_none());
    assert_eq!(
        tree.elements[1].resolved_role.status,
        pdf::document::structure::StructureRoleStatus::Cycle
    );
    let reopened =
        Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened.structure_tree().unwrap().unwrap().elements.len(),
        2
    );
}

#[test]
fn hierarchy_roles_and_cumulative_text_have_separate_limits() {
    for limits in [
        Limits {
            max_depth: 3,
            ..Default::default()
        },
        Limits {
            max_decoded_bytes: 8,
            ..Default::default()
        },
    ] {
        let (limited, _, _, leaf) = fixture_options(ParseOptions {
            limits,
            ..Default::default()
        });
        let mut parent = leaf;
        for _ in 0..3 {
            let child = limited
                .try_insert_object(parse(&format!(
                    "<< /Type /StructElem /S /Span /P {} 0 R >>",
                    parent.obj_num()
                )))
                .unwrap();
            set(&limited, parent, "K", reference(child));
            parent = child;
        }
        assert!(matches!(
            limited.structure_tree(),
            Err(PdfError::ResourceLimit(_))
        ));
    }
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 3,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    let mut root = doc
        .resolve_object(ObjectId::new(2, 0))
        .unwrap()
        .into_dict()
        .unwrap();
    // Three child references exceed remaining slots after the root;
    // rejection happens before attempting to resolve these missing objects.
    let id = doc
        .try_insert_object(parse(
            "<< /Type /StructTreeRoot /K [99 0 R 100 0 R 101 0 R] >>",
        ))
        .unwrap();
    root.try_insert("StructTreeRoot".into(), reference(id))
        .unwrap();
    doc.update_object(ObjectId::new(2, 0), PdfObject::PdfDict(root))
        .unwrap();
    assert!(matches!(
        doc.structure_tree(),
        Err(PdfError::ResourceLimit(_))
    ));
}
