use pdf::{
    document::{
        PdfDocument,
        destination::DestinationView,
        save_options::PdfVersion,
        structure::{
            StructureAttributeLookup, StructureAttributeOwner, StructureAttributeSource,
            StructureRoleStatus,
        },
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_reference::PdfReference, pdf_stream::PdfStream},
    parser::{
        options::ParseOptions,
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;
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
fn remove(doc: &Doc, id: ObjectId, key: &str) {
    let mut dict = doc.resolve_object(id).unwrap().into_dict().unwrap();
    dict.remove(key);
    doc.update_object(id, PdfObject::PdfDict(dict)).unwrap();
}
struct Fixture {
    doc: Doc,
    root: ObjectId,
    leaf: ObjectId,
    ns: ObjectId,
    standard: ObjectId,
    page: ObjectId,
}
fn fixture(version: PdfVersion, options: ParseOptions) -> Fixture {
    let doc = Doc::new_with_version_and_options(version, options).unwrap();
    doc.add_page(0).unwrap();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let standard = doc
        .try_insert_object(parse(
            "<< /Type /Namespace /NS (http://iso.org/pdf2/ssn) >>",
        ))
        .unwrap();
    let ns = doc.try_insert_object(parse(&format!("<< /NS (urn:example:custom) /RoleMapNS << /X [/P {} 0 R] >> /Schema << /FS /URL /F (https://invalid.example/schema) >> /Custom (keep) >>", standard.obj_num()))).unwrap();
    let root = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let leaf = doc.try_insert_object(parse(&format!("<< /S /X /P {} 0 R /Pg {} 0 R /R 1 /C [/First 0 /Second 1 /Absent] /A [<< /O /Layout /TextAlign /Center >> 1 << /O /Table /RowSpan 2 >>] >>", root.obj_num(), page.obj_num()))).unwrap();
    doc.update_object(root, parse(&format!("<< /Type /StructTreeRoot /K {} 0 R /RoleMap << /X /Wrong >> /ClassMap << /First << /O /Layout /TextAlign /Start /WritingMode /LrTb /Custom [(unknown) 3] >> /Second [<< /O /Layout /TextAlign /End >> << /O /Table /ColSpan 2 >>] >> >>", leaf.obj_num()))).unwrap();
    set(&doc, ObjectId::new(2, 0), "StructTreeRoot", reference(root));
    if version >= PdfVersion::V2_0 {
        set(
            &doc,
            root,
            "Namespaces",
            parse(&format!(
                "[{} 0 R {} 0 R]",
                ns.obj_num(),
                standard.obj_num()
            )),
        );
        set(&doc, leaf, "NS", reference(ns));
    }
    Fixture {
        doc,
        root,
        leaf,
        ns,
        standard,
        page,
    }
}
fn current() -> Fixture {
    fixture(PdfVersion::V2_0, Default::default())
}
fn value_is(lookup: StructureAttributeLookup<'_>, value: &str) {
    match lookup {
        StructureAttributeLookup::Value { value: actual, .. } => assert_eq!(actual, &parse(value)),
        other => panic!("expected value, got {other:?}"),
    }
}
fn fails(f: &Fixture, expected: &str) {
    let before = f.doc.save_to_bytes().unwrap();
    let stamp = f.doc.stamp();
    let err = f.doc.structure_tree().unwrap_err();
    assert!(err.to_string().contains(expected), "{err:?}");
    assert!(f.doc.is_stamp_current(&stamp));
    assert_eq!(before, f.doc.save_to_bytes().unwrap());
}

#[test]
fn qualified_mapping_ignores_unrelated_root_map_and_keeps_raw_extensions() {
    let f = current();
    let before = f.doc.save_to_bytes().unwrap();
    let stamp = f.doc.stamp();
    let tree = f.doc.structure_tree().unwrap().unwrap();
    let node = &tree.elements[0];
    assert_eq!(node.role, "X");
    assert!(node.mapped_role.is_none());
    assert_eq!(node.namespace.as_ref().unwrap().object_id(), f.ns);
    assert_eq!(node.resolved_role.name, "P");
    assert_eq!(node.resolved_role.namespace_uri, "http://iso.org/pdf2/ssn");
    assert_eq!(
        node.resolved_role.namespace.as_ref().unwrap().object_id(),
        f.standard
    );
    assert_eq!(node.resolved_role.status, StructureRoleStatus::Endpoint);
    assert_eq!(tree.namespaces.len(), 2);
    assert!(tree.namespaces.iter().all(|ns| ns.listed));
    assert!(
        tree.namespaces
            .iter()
            .find(|ns| ns.handle.object_id() == f.ns)
            .unwrap()
            .schema
            .is_some()
    );
    assert_eq!(
        f.doc
            .resolve_object(f.ns)
            .unwrap()
            .as_dict()
            .unwrap()
            .get("Custom"),
        Some(&parse("(keep)"))
    );
    assert!(f.doc.is_stamp_current(&stamp));
    assert_eq!(before, f.doc.save_to_bytes().unwrap());
    let reopened = Doc::from_bytes_with_mode(before, ParseMode::Strict).unwrap();
    let again = reopened.structure_tree().unwrap().unwrap();
    assert_eq!(
        again.elements[0].resolved_role.namespace_uri,
        node.resolved_role.namespace_uri
    );
    assert_ne!(again.elements[0].namespace, node.namespace);
    remove(&f.doc, f.ns, "RoleMapNS");
    let unknown = f.doc.structure_tree().unwrap().unwrap();
    assert_eq!(unknown.elements[0].resolved_role.name, "X");
    assert_eq!(
        unknown.elements[0].resolved_role.namespace_uri,
        "urn:example:custom"
    );
    assert_eq!(
        unknown.elements[0].resolved_role.status,
        StructureRoleStatus::Endpoint
    );
}

#[test]
fn named_target_is_default_namespace_and_does_not_use_global_rolemap() {
    let f = current();
    set(&f.doc, f.ns, "RoleMapNS", parse("<< /X /P >>"));
    set(
        &f.doc,
        f.root,
        "RoleMap",
        parse("<< /P /Wrong /X /Wrong >>"),
    );
    let tree = f.doc.structure_tree().unwrap().unwrap();
    assert_eq!(tree.elements[0].resolved_role.name, "P");
    assert_eq!(
        tree.elements[0].resolved_role.namespace_uri,
        "http://iso.org/pdf/ssn"
    );
    assert!(tree.elements[0].resolved_role.namespace.is_none());
}

#[test]
fn missing_element_namespace_is_not_inherited_from_parent() {
    let f = current();
    let child = f
        .doc
        .try_insert_object(parse(&format!("<< /S /X /P {} 0 R >>", f.leaf.obj_num())))
        .unwrap();
    set(&f.doc, f.leaf, "K", reference(child));
    let tree = f.doc.structure_tree().unwrap().unwrap();
    assert!(tree.elements[1].namespace.is_none());
    assert_eq!(tree.elements[1].resolved_role.name, "Wrong");
    assert_eq!(
        tree.elements[1].resolved_role.namespace_uri,
        "http://iso.org/pdf/ssn"
    );
}

#[test]
fn transitive_namespace_targets_need_not_be_listed_without_element_use() {
    let f = current();
    let middle = f
        .doc
        .try_insert_object(parse(&format!(
            "<< /NS (urn:example:middle) /RoleMapNS << /Middle [/P {} 0 R] >> >>",
            f.standard.obj_num()
        )))
        .unwrap();
    set(
        &f.doc,
        f.ns,
        "RoleMapNS",
        parse(&format!("<< /X [/Middle {} 0 R] >>", middle.obj_num())),
    );
    let tree = f.doc.structure_tree().unwrap().unwrap();
    assert_eq!(tree.elements[0].resolved_role.name, "P");
    assert!(
        !tree
            .namespaces
            .iter()
            .find(|ns| ns.handle.object_id() == middle)
            .unwrap()
            .listed
    );
    set(&f.doc, f.leaf, "NS", reference(middle));
    fails(&f, "absent from root Namespaces");
}

#[test]
fn namespace_role_cycles_and_same_namespace_reentry_are_errors() {
    let f = current();
    set(
        &f.doc,
        f.ns,
        "RoleMapNS",
        parse(&format!("<< /X [/Y {} 0 R] >>", f.ns.obj_num())),
    );
    fails(&f, "same-namespace re-entry");
    let f = current();
    set(
        &f.doc,
        f.standard,
        "RoleMapNS",
        parse(&format!("<< /P [/X {} 0 R] >>", f.ns.obj_num())),
    );
    fails(&f, "same-namespace re-entry");
    let f = current();
    set(&f.doc, f.standard, "NS", parse("(http://iso.org/pdf/ssn)"));
    set(&f.doc, f.standard, "RoleMapNS", parse("<< /P /Other >>"));
    fails(&f, "same namespace");
}

#[test]
fn invalid_namespace_shapes_references_types_and_aliases_reject() {
    for (key, value, expected) in [
        ("NS", "/Bad", "text string"),
        ("Type", "/Bad", "Namespace"),
        ("RoleMapNS", "[1]", "dictionary"),
        (
            "RoleMapNS",
            "<< /Unused [/P] >>",
            "needs name and namespace",
        ),
        ("RoleMapNS", "<< /Unused [/P << /NS (x) >>] >>", "indirect"),
        ("RoleMapNS", "<< /Unused 3 >>", "must be a name"),
        ("NS", "(http://iso.org/pdf2/ssn)", "one URI"),
    ] {
        let f = current();
        set(&f.doc, f.ns, key, parse(value));
        fails(&f, expected);
    }
    let f = current();
    set(&f.doc, f.root, "Namespaces", parse("[<< /NS (x) >>]"));
    fails(&f, "indirect");
    let f = current();
    set(
        &f.doc,
        f.ns,
        "RoleMapNS",
        parse("<< /Unused [/P 999999 0 R] >>"),
    );
    assert!(f.doc.structure_tree().is_err());
}

#[test]
fn version_gate_honors_catalog_override_without_silently_upgrading() {
    let f = fixture(PdfVersion::V1_7, Default::default());
    set(
        &f.doc,
        f.root,
        "Namespaces",
        parse(&format!("[{} 0 R]", f.ns.obj_num())),
    );
    set(&f.doc, f.leaf, "NS", reference(f.ns));
    fails(&f, "effective PDF 2.0");
    set(&f.doc, ObjectId::new(2, 0), "Version", parse("/2.0"));
    assert_eq!(f.doc.effective_version().unwrap(), PdfVersion::V2_0);
    assert_eq!(
        f.doc.structure_tree().unwrap().unwrap().elements[0]
            .resolved_role
            .name,
        "P"
    );
}

#[test]
fn direct_attributes_override_classes_per_key_not_per_owner() {
    let f = current();
    let tree = f.doc.structure_tree().unwrap().unwrap();
    let node = &tree.elements[0];
    let layout = StructureAttributeOwner::Name("Layout".into());
    let table = StructureAttributeOwner::Name("Table".into());
    value_is(node.attribute(&layout, "TextAlign"), "/Center");
    value_is(node.attribute(&layout, "WritingMode"), "/LrTb");
    value_is(node.attribute(&table, "ColSpan"), "2");
    value_is(node.attribute(&table, "RowSpan"), "2");
    value_is(node.attribute(&layout, "Custom"), "[(unknown) 3]");
    assert_eq!(node.revision, 1);
    assert_eq!(
        node.classes
            .iter()
            .map(|c| (c.name.as_str(), c.revision, c.defined))
            .collect::<Vec<_>>(),
        [
            ("First", 0, true),
            ("Second", 1, true),
            ("Absent", 0, false)
        ]
    );
    assert!(
        matches!(node.attribute(&layout, "TextAlign"), StructureAttributeLookup::Value { binding, .. } if binding.source == StructureAttributeSource::Direct && binding.revision == 1)
    );
    assert!(matches!(
        node.attribute(&table, "Unknown"),
        StructureAttributeLookup::Missing
    ));
    set(&f.doc, f.root, "ClassMap", parse("<< /Empty [] >>"));
    set(&f.doc, f.leaf, "C", parse("/Empty"));
    let empty = f.doc.structure_tree().unwrap().unwrap();
    assert!(empty.attribute_classes[0].attributes.is_empty());
    assert!(empty.elements[0].classes[0].defined);
    assert_eq!(empty.elements[0].attribute_bindings.len(), 2);
}

#[test]
fn pdf2_later_class_and_direct_entries_win_but_pdf1_duplicates_are_ambiguous() {
    for version in [PdfVersion::V1_7, PdfVersion::V2_0] {
        let f = fixture(version, Default::default());
        remove(&f.doc, f.leaf, "A");
        let tree = f.doc.structure_tree().unwrap().unwrap();
        let owner = StructureAttributeOwner::Name("Layout".into());
        let lookup = tree.elements[0].attribute(&owner, "TextAlign");
        if version == PdfVersion::V2_0 {
            value_is(lookup, "/End");
        } else {
            assert!(matches!(lookup, StructureAttributeLookup::Ambiguous(v) if v.len() == 2));
        }
        set(
            &f.doc,
            f.leaf,
            "A",
            parse("[<< /O /Layout /TextAlign /Center >> << /O /Layout /TextAlign /Justify >>]"),
        );
        let tree = f.doc.structure_tree().unwrap().unwrap();
        let lookup = tree.elements[0].attribute(&owner, "TextAlign");
        if version == PdfVersion::V2_0 {
            value_is(lookup, "/Justify");
        } else {
            assert!(
                matches!(lookup, StructureAttributeLookup::Ambiguous(v) if v.len() == 2 && v.iter().all(|b| b.source == StructureAttributeSource::Direct))
            );
        }
    }
}

#[test]
fn attribute_owner_identity_includes_namespace_uri() {
    let f = current();
    let other = f
        .doc
        .try_insert_object(parse("<< /NS (urn:example:other) >>"))
        .unwrap();
    set(
        &f.doc,
        f.leaf,
        "A",
        parse(&format!(
            "[<< /O /NSO /NS {} 0 R /Value 1 >> << /O /NSO /NS {} 0 R /Value 2 >> << /O /Layout /Value 3 >>]",
            f.ns.obj_num(),
            other.obj_num()
        )),
    );
    let tree = f.doc.structure_tree().unwrap().unwrap();
    let node = &tree.elements[0];
    value_is(
        node.attribute(
            &StructureAttributeOwner::Namespace("urn:example:custom".into()),
            "Value",
        ),
        "1",
    );
    value_is(
        node.attribute(
            &StructureAttributeOwner::Namespace("urn:example:other".into()),
            "Value",
        ),
        "2",
    );
    value_is(
        node.attribute(&StructureAttributeOwner::Name("Layout".into()), "Value"),
        "3",
    );
    assert!(
        tree.namespaces
            .iter()
            .any(|ns| ns.handle.object_id() == other && !ns.listed)
    );
}

#[test]
fn revision_pairs_keep_stale_values_and_reject_invalid_attachments() {
    let f = current();
    set(&f.doc, f.leaf, "R", parse("5"));
    set(&f.doc, f.leaf, "C", parse("[/First 2 /Second]"));
    set(
        &f.doc,
        f.leaf,
        "A",
        parse("[<< /O /Layout /TextAlign /Center >> 3 << /O /Table /RowSpan 1 >>]"),
    );
    let tree = f.doc.structure_tree().unwrap().unwrap();
    assert_eq!(tree.elements[0].revision, 5);
    assert!(
        tree.elements[0]
            .attribute_bindings
            .iter()
            .any(|b| b.source == StructureAttributeSource::Direct && b.revision == 3)
    );
    for (key, value, expected) in [
        ("A", "[1 << /O /Layout >>]", "attribute object"),
        ("A", "[<< /O /Layout >> -1]", "nonnegative integer"),
        ("A", "[<< /O /Layout >> 0.5]", "nonnegative integer"),
        ("A", "[<< /O /Layout >> 8]", "exceeds"),
        ("A", "[<< /O /Layout >> 1 1]", "attribute object"),
        ("A", "[]", "must not be empty"),
        ("A", "null", "attribute object"),
        ("C", "[1 /First]", "must be a name"),
        ("C", "[/First 6]", "exceeds"),
        ("R", "-1", "nonnegative integer"),
    ] {
        let f = current();
        set(&f.doc, f.leaf, key, parse(value));
        fails(&f, expected);
    }
}

#[test]
fn malformed_attribute_owners_and_class_maps_fail_even_when_unused() {
    for (key, value, expected) in [
        ("A", "<< /Value 1 >>", "O owner"),
        ("A", "<< /O /NSO /Value 1 >>", "requires NS"),
        ("A", "<< /O /Layout /NS 99999 0 R >>", "requires O NSO"),
    ] {
        let f = current();
        set(&f.doc, f.leaf, key, parse(value));
        fails(&f, expected);
    }
    for value in [
        "[]",
        "<< /Unused 7 >>",
        "<< /Unused [<< /O /Layout >> 1] >>",
        "<< /Unused << >> >>",
    ] {
        let f = current();
        set(&f.doc, f.root, "ClassMap", parse(value));
        assert!(f.doc.structure_tree().is_err());
    }
}

#[test]
fn attribute_streams_remain_opaque_and_unknown_values_do_not_get_dereferenced() {
    let f = current();
    let opaque = f
        .doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            parse("<< /O /PrivateOwner /Filter /UnknownOwnerFormat /Length 4 >>")
                .into_dict()
                .unwrap(),
            b"\0\xff!?".to_vec(),
        )))
        .unwrap();
    set(
        &f.doc,
        f.leaf,
        "A",
        parse(&format!(
            "[{} 0 R << /O /PrivateOwner /Key 1 >> << /O /OtherOwner /Key 99999 0 R >>]",
            opaque.obj_num()
        )),
    );
    let tree = f.doc.structure_tree().unwrap().unwrap();
    let node = &tree.elements[0];
    assert!(
        matches!(node.attribute(&StructureAttributeOwner::Name("PrivateOwner".into()), "Key"), StructureAttributeLookup::Opaque(v) if v.len() == 2)
    );
    value_is(
        node.attribute(&StructureAttributeOwner::Name("OtherOwner".into()), "Key"),
        "99999 0 R",
    );
    let again =
        Doc::from_bytes_with_mode(f.doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert!(
        again.structure_tree().unwrap().unwrap().elements[0]
            .attribute_bindings
            .iter()
            .any(|b| b.attribute.stream.is_some())
    );
    assert_eq!(
        f.doc
            .resolve_object(opaque)
            .unwrap()
            .as_stream()
            .unwrap()
            .raw_data(),
        b"\0\xff!?"
    );
}

#[test]
fn namespace_attributes_and_bindings_share_cumulative_limits() {
    let mut options = ParseOptions::default();
    options.limits.max_decoded_bytes = 140;
    let f = fixture(PdfVersion::V2_0, options);
    assert!(matches!(
        f.doc.structure_tree(),
        Err(PdfError::ResourceLimit(_))
    ));
    let mut options = ParseOptions::default();
    options.limits.max_objects = 80;
    let f = fixture(PdfVersion::V2_0, options);
    set(
        &f.doc,
        f.leaf,
        "C",
        parse(&format!("[{}]", "/First ".repeat(30))),
    );
    assert!(matches!(
        f.doc.structure_tree(),
        Err(PdfError::ResourceLimit(_))
    ));
    let mut options = ParseOptions::default();
    options.limits.max_depth = 5;
    let f = fixture(PdfVersion::V2_0, options);
    set(
        &f.doc,
        f.leaf,
        "A",
        parse("<< /O /Custom /Deep [[[[[[1]]]]]] >>"),
    );
    assert!(matches!(
        f.doc.structure_tree(),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn namespace_graph_depth_and_mapping_work_are_bounded() {
    let mut options = ParseOptions::default();
    options.limits.max_depth = 4;
    let f = fixture(PdfVersion::V2_0, options);
    let mut next = f.standard;
    for n in 0..6 {
        next = f
            .doc
            .try_insert_object(parse(&format!(
                "<< /NS (urn:example:{n}) /RoleMapNS << /P [/P {} 0 R] >> >>",
                next.obj_num()
            )))
            .unwrap();
    }
    set(
        &f.doc,
        f.ns,
        "RoleMapNS",
        parse(&format!("<< /X [/P {} 0 R] >>", next.obj_num())),
    );
    assert!(matches!(
        f.doc.structure_tree(),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn qualified_tree_integrates_with_mcid_reverse_validation_and_snapshot_invalidation() {
    let f = current();
    let contents = f
        .doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(Default::default(), b"/P << /MCID 0 >> BDC EMC".to_vec())
                .unwrap(),
        ))
        .unwrap();
    set(&f.doc, f.page, "Contents", reference(contents));
    set(&f.doc, f.page, "StructParents", parse("0"));
    set(&f.doc, f.leaf, "K", parse("0"));
    set(
        &f.doc,
        f.root,
        "ParentTree",
        parse(&format!("<< /Nums [0 [{} 0 R]] >>", f.leaf.obj_num())),
    );
    let associations = f.doc.structure_associations().unwrap().unwrap();
    assert_eq!(associations.items.len(), 1);
    let bytes = f.doc.save_to_bytes().unwrap();
    let reopened = Doc::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .structure_associations()
            .unwrap()
            .unwrap()
            .items
            .len(),
        1
    );
    set(
        &f.doc,
        f.leaf,
        "A",
        parse("<< /O /Layout /TextAlign /End >>"),
    );
    assert!(!f.doc.is_stamp_current(&associations.stamp));
}
