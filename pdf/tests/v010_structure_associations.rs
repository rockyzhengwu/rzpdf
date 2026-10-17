use pdf::{
    document::{
        PdfDocument,
        destination::DestinationView,
        structure::{StructureAssociation, StructureContentDomain},
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
fn stream(doc: &Doc, dict: &str, content: &str) -> ObjectId {
    doc.try_insert_object(PdfObject::PdfStream(
        PdfStream::from_decoded_data(
            parse(dict).into_dict().unwrap(),
            content.as_bytes().to_vec(),
        )
        .unwrap(),
    ))
    .unwrap()
}
struct Fixture {
    doc: Doc,
    page: ObjectId,
    root: ObjectId,
    leaf: ObjectId,
    form: ObjectId,
    annot: ObjectId,
}
fn fixture(options: ParseOptions) -> Fixture {
    let doc = Doc::new_with_options(options).unwrap();
    doc.add_page(0).unwrap();
    let page = doc
        .destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let root = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let leaf = doc
        .try_insert_object(parse(&format!(
            "<< /S /P /P {} 0 R /Pg {} 0 R >>",
            root.obj_num(),
            page.obj_num()
        )))
        .unwrap();
    let form = stream(
        &doc,
        "<< /Type /XObject /Subtype /Form /BBox [0 0 10 10] /StructParents 1 >>",
        "/P << /MCID 0 >> BDC EMC",
    );
    let annot = doc
        .try_insert_object(parse(&format!(
            "<< /Type /Annot /Subtype /Square /Rect [0 0 5 5] /StructParent 2 /P {} 0 R >>",
            page.obj_num()
        )))
        .unwrap();
    let contents = stream(&doc, "<< >>", "/P /Props BDC EMC /F Do /F Do");
    set(&doc, page, "Contents", reference(contents));
    set(
        &doc,
        page,
        "Resources",
        parse(&format!(
            "<< /Properties << /Props << /MCID 2 >> >> /XObject << /F {} 0 R >> >>",
            form.obj_num()
        )),
    );
    set(
        &doc,
        page,
        "Annots",
        parse(&format!("[{} 0 R]", annot.obj_num())),
    );
    set(&doc, page, "StructParents", parse("0"));
    set(
        &doc,
        leaf,
        "K",
        parse(&format!(
            "[2 << /Type /MCR /Stm {} 0 R /MCID 0 >> << /Type /OBJR /Obj {} 0 R >>]",
            form.obj_num(),
            annot.obj_num()
        )),
    );
    doc.update_object(root, parse(&format!("<< /Type /StructTreeRoot /K {} 0 R /ParentTree << /Nums [0 [null null {} 0 R] 1 [{} 0 R] 2 {} 0 R] >> /ParentTreeNextKey 3 >>", leaf.obj_num(), leaf.obj_num(), leaf.obj_num(), leaf.obj_num()))).unwrap();
    // Catalog ID is fixed only for this new-document fixture.
    set(&doc, ObjectId::new(2, 0), "StructTreeRoot", reference(root));
    Fixture {
        doc,
        page,
        root,
        leaf,
        form,
        annot,
    }
}
fn replace_contents(f: &Fixture, text: &str) {
    let id = stream(&f.doc, "<< >>", text);
    set(&f.doc, f.page, "Contents", reference(id));
}
fn fails(f: &Fixture, message: &str) {
    let stamp = f.doc.stamp();
    let before = f.doc.save_to_bytes().unwrap();
    let error = f.doc.structure_associations().unwrap_err();
    assert!(error.to_string().contains(message), "{error:?}");
    assert!(f.doc.is_stamp_current(&stamp));
    assert_eq!(f.doc.save_to_bytes().unwrap(), before);
}

#[test]
fn page_form_properties_null_slots_objr_and_read_only_roundtrip() {
    let f = fixture(Default::default());
    let stamp = f.doc.stamp();
    let before = f.doc.save_to_bytes().unwrap();
    let associations = f.doc.structure_associations().unwrap().unwrap();
    assert_eq!(associations.items.len(), 3);
    assert!(
        matches!(&associations.items[0], StructureAssociation::MarkedContent { domain: StructureContentDomain::Page(p), mcid: 2, .. } if p.object_id() == f.page)
    );
    assert!(
        matches!(&associations.items[1], StructureAssociation::MarkedContent { domain: StructureContentDomain::Form(p), mcid: 0, .. } if p.object_id() == f.form)
    );
    assert!(
        matches!(&associations.items[2], StructureAssociation::ObjectReference { object, .. } if object.object_id() == f.annot)
    );
    assert_eq!(associations.stamp, stamp);
    assert!(f.doc.is_stamp_current(&stamp));
    assert_eq!(f.doc.save_to_bytes().unwrap(), before);
    let reopened = Doc::from_bytes_with_mode(before, ParseMode::Strict).unwrap();
    let again = reopened.structure_associations().unwrap().unwrap();
    assert_eq!(again.items.len(), 3);
    assert_ne!(again.root, associations.root);
    f.doc.add_page(1).unwrap();
    assert!(!f.doc.is_stamp_current(&stamp));
}

#[test]
fn same_mcid_in_page_and_shared_form_is_not_global_identity() {
    let f = fixture(Default::default());
    replace_contents(&f, "/P << /MCID 0 >> BDC EMC /F Do");
    set(
        &f.doc,
        f.leaf,
        "K",
        parse(&format!(
            "[0 << /Type /MCR /Stm {} 0 R /MCID 0 >> << /Type /OBJR /Obj {} 0 R >>]",
            f.form.obj_num(),
            f.annot.obj_num()
        )),
    );
    set(
        &f.doc,
        f.root,
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [{} 0 R] 1 [{} 0 R] 2 {} 0 R] >>",
            f.leaf.obj_num(),
            f.leaf.obj_num(),
            f.leaf.obj_num()
        )),
    );
    assert_eq!(
        f.doc.structure_associations().unwrap().unwrap().items.len(),
        3
    );
}

#[test]
fn duplicate_mcid_and_unbalanced_marked_content_reject_without_mutation() {
    for (content, message) in [
        (
            "/P << /MCID 2 >> BDC EMC /P << /MCID 2 >> BDC EMC",
            "duplicate MCID",
        ),
        ("EMC", "unmatched EMC"),
        ("/P BMC", "unterminated"),
        ("/P << /MCID -1 >> BDC EMC", "nonnegative integer"),
        ("/P << /MCID 0.5 >> BDC EMC", "nonnegative integer"),
        ("/P BDC", "operands"),
        ("/P /Missing BDC EMC", "resource missing"),
    ] {
        let f = fixture(Default::default());
        replace_contents(&f, content);
        fails(&f, message);
    }
}

#[test]
fn content_arrays_are_one_domain_and_sequences_can_cross_streams() {
    let f = fixture(Default::default());
    let a = stream(&f.doc, "<< >>", "/P << /MCID 2 >> BDC ");
    let b = stream(&f.doc, "<< >>", "EMC /F Do");
    set(
        &f.doc,
        f.page,
        "Contents",
        parse(&format!("[{} 0 R {} 0 R]", a.obj_num(), b.obj_num())),
    );
    assert!(f.doc.structure_associations().unwrap().is_some());
    let c = stream(&f.doc, "<< >>", "/P << /MCID 2 >> BDC EMC ");
    set(
        &f.doc,
        f.page,
        "Contents",
        parse(&format!("[{} 0 R {} 0 R]", c.obj_num(), c.obj_num())),
    );
    fails(&f, "duplicate MCID");
}

#[test]
fn reverse_index_wrong_owner_null_short_missing_and_stray_keys_reject() {
    for (nums, message) in [
        (
            "0 [null null ROOT] 1 [LEAF] 2 LEAF",
            "slot/K ownership mismatch",
        ),
        ("0 [null null null] 1 [LEAF] 2 LEAF", "null slot"),
        ("0 [null] 1 [LEAF] 2 LEAF", "too short"),
        ("0 [null null LEAF] 2 LEAF", "missing from ParentTree"),
        ("0 [null null LEAF] 1 [LEAF] 2 ROOT", "object/OBJR"),
        (
            "0 [null null LEAF] 1 [LEAF] 2 LEAF 3 LEAF",
            "no reachable owner",
        ),
        (
            "0 [LEAF null LEAF] 1 [LEAF] 2 LEAF",
            "slot/K ownership mismatch",
        ),
    ] {
        let f = fixture(Default::default());
        let nums = nums
            .replace("ROOT", &format!("{} 0 R", f.root.obj_num()))
            .replace("LEAF", &format!("{} 0 R", f.leaf.obj_num()));
        set(
            &f.doc,
            f.root,
            "ParentTree",
            parse(&format!("<< /Nums [{nums}] >>")),
        );
        fails(&f, message);
    }
}

#[test]
fn mcid_forward_ownership_and_actual_membership_are_checked() {
    let f = fixture(Default::default());
    set(&f.doc, f.leaf, "K", parse("99"));
    fails(&f, "MCID missing");
    let f = fixture(Default::default());
    set(&f.doc, f.leaf, "K", parse("[2 2]"));
    fails(&f, "duplicate declared");
    let f = fixture(Default::default());
    replace_contents(
        &f,
        "/P << /MCID 2 >> BDC EMC /P << /MCID 3 >> BDC EMC /F Do",
    );
    fails(&f, "no structure K owner");
}

#[test]
fn objr_requires_page_membership_and_matching_annotation_parent() {
    let widget = fixture(Default::default());
    set(&widget.doc, widget.annot, "Subtype", parse("/Widget"));
    set(&widget.doc, widget.annot, "FT", parse("/Btn"));
    set(&widget.doc, widget.annot, "T", parse("(checkbox)"));
    set(
        &widget.doc,
        ObjectId::new(2, 0),
        "AcroForm",
        parse(&format!("<< /Fields [{} 0 R] >>", widget.annot.obj_num())),
    );
    assert_eq!(
        widget
            .doc
            .structure_associations()
            .unwrap()
            .unwrap()
            .items
            .len(),
        3
    );
    let reopened =
        Doc::from_bytes_with_mode(widget.doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .structure_associations()
            .unwrap()
            .unwrap()
            .items
            .len(),
        3
    );
    let f = fixture(Default::default());
    set(&f.doc, f.page, "Annots", parse("[]"));
    fails(&f, "not an annotation or invoked XObject");
    let f = fixture(Default::default());
    set(&f.doc, f.annot, "P", reference(f.root));
    fails(&f, "annotation P/page mismatch");
    let f = fixture(Default::default());
    set(&f.doc, f.annot, "StructParent", parse("0"));
    fails(&f, "duplicate StructParent(s) key");
    let f = fixture(Default::default());
    set(&f.doc, f.annot, "StructParents", parse("4"));
    fails(&f, "both StructParent and StructParents");
}

#[test]
fn form_must_be_invoked_and_recursive_invocation_rejects() {
    let f = fixture(Default::default());
    replace_contents(&f, "/P /Props BDC EMC");
    fails(&f, "invoked Form");
    let f = fixture(Default::default());
    let form = PdfStream::from_decoded_data(parse(&format!("<< /Subtype /Form /BBox [0 0 10 10] /StructParents 1 /Resources << /XObject << /Self {} 0 R >> >> >>", f.form.obj_num())).into_dict().unwrap(), b"/Self Do".to_vec()).unwrap();
    f.doc
        .update_object(f.form, PdfObject::PdfStream(form))
        .unwrap();
    fails(&f, "recursive Form");
}

#[test]
fn shared_form_inherited_properties_cannot_change_its_mcid_identity() {
    let f = fixture(Default::default());
    let child = PdfStream::from_decoded_data(
        parse("<< /Subtype /Form /BBox [0 0 10 10] /StructParents 1 >>")
            .into_dict()
            .unwrap(),
        b"/P /Props BDC EMC".to_vec(),
    )
    .unwrap();
    f.doc
        .update_object(f.form, PdfObject::PdfStream(child))
        .unwrap();
    set(
        &f.doc,
        f.page,
        "Resources",
        parse(&format!(
            "<< /Properties << /Props << /MCID 0 >> >> /XObject << /F {} 0 R >> >>",
            f.form.obj_num()
        )),
    );
    replace_contents(&f, "/P << /MCID 2 >> BDC EMC /F Do");
    f.doc.add_page(1).unwrap();
    let second = f
        .doc
        .destination(1, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    set(
        &f.doc,
        second,
        "Resources",
        parse(&format!(
            "<< /Properties << /Props << /MCID 1 >> >> /XObject << /F {} 0 R >> >>",
            f.form.obj_num()
        )),
    );
    set(
        &f.doc,
        second,
        "Contents",
        reference(stream(&f.doc, "<< >>", "/F Do")),
    );
    fails(&f, "resource-dependent MCIDs");
}

#[test]
fn nested_form_missing_resources_falls_back_to_page_not_parent_form() {
    let f = fixture(Default::default());
    let child = PdfStream::from_decoded_data(
        parse("<< /Subtype /Form /BBox [0 0 10 10] /StructParents 1 >>")
            .into_dict()
            .unwrap(),
        b"/P /Props BDC EMC".to_vec(),
    )
    .unwrap();
    f.doc
        .update_object(f.form, PdfObject::PdfStream(child))
        .unwrap();
    let parent = stream(
        &f.doc,
        &format!(
            "<< /Subtype /Form /BBox [0 0 10 10] /Resources << /Properties << /Props << /MCID 99 >> >> /XObject << /F {} 0 R >> >> >>",
            f.form.obj_num()
        ),
        "/F Do",
    );
    set(
        &f.doc,
        f.page,
        "Resources",
        parse(&format!(
            "<< /Properties << /Props << /MCID 0 >> >> /XObject << /Parent {} 0 R >> >>",
            parent.obj_num()
        )),
    );
    replace_contents(&f, "/P << /MCID 2 >> BDC EMC /Parent Do");
    assert!(f.doc.structure_associations().unwrap().is_some());
}

#[test]
fn stale_next_key_and_unsupported_stmown_are_explicit() {
    let f = fixture(Default::default());
    set(&f.doc, f.root, "ParentTreeNextKey", parse("2"));
    fails(&f, "must exceed");
    let f = fixture(Default::default());
    set(
        &f.doc,
        f.leaf,
        "K",
        parse(&format!(
            "<< /Type /MCR /Stm {} 0 R /StmOwn {} 0 R /MCID 0 >>",
            f.form.obj_num(),
            f.annot.obj_num()
        )),
    );
    fails(&f, "StmOwn/appearance");
}

#[test]
fn global_work_limits_include_repeated_form_invocations() {
    let mut options = ParseOptions::default();
    options.limits.max_content_operations = 9; // page 4 + two Form invocations 4
    let f = fixture(options);
    assert!(f.doc.structure_associations().unwrap().is_some());
    replace_contents(&f, "/P /Props BDC EMC /F Do /F Do /F Do");
    assert!(matches!(
        f.doc.structure_associations(),
        Err(PdfError::ResourceLimit(_))
    ));
    let mut options = ParseOptions::default();
    options.limits.max_decoded_bytes = 100;
    let f = fixture(options);
    replace_contents(&f, &format!("/P /Props BDC EMC {}", "/F Do ".repeat(8)));
    assert!(matches!(
        f.doc.structure_associations(),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn missing_tree_and_empty_root_have_no_associations() {
    let doc = Doc::new().unwrap();
    assert!(doc.structure_associations().unwrap().is_none());
    let f = fixture(Default::default());
    let doc = Doc::new().unwrap();
    let root = doc
        .try_insert_object(PdfObject::PdfDict(
            parse("<< /Type /StructTreeRoot >>").into_dict().unwrap(),
        ))
        .unwrap();
    set(&doc, ObjectId::new(2, 0), "StructTreeRoot", reference(root));
    assert!(
        doc.structure_associations()
            .unwrap()
            .unwrap()
            .items
            .is_empty()
    );
    // Keep the regular fixture's root distinct from an empty root.
    assert!(
        !f.doc
            .structure_associations()
            .unwrap()
            .unwrap()
            .items
            .is_empty()
    );
}

#[test]
fn mixed_structure_kids_preserve_depth_first_declared_order() {
    let f = fixture(Default::default());
    let child = f
        .doc
        .try_insert_object(parse(&format!(
            "<< /S /Span /P {} 0 R /Pg {} 0 R /K << /Type /MCR /Stm {} 0 R /MCID 0 >> >>",
            f.leaf.obj_num(),
            f.page.obj_num(),
            f.form.obj_num()
        )))
        .unwrap();
    set(
        &f.doc,
        f.leaf,
        "K",
        parse(&format!(
            "[2 {} 0 R << /Type /OBJR /Obj {} 0 R >>]",
            child.obj_num(),
            f.annot.obj_num()
        )),
    );
    set(
        &f.doc,
        f.root,
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [null null {} 0 R] 1 [{} 0 R] 2 {} 0 R] >>",
            f.leaf.obj_num(),
            child.obj_num(),
            f.leaf.obj_num()
        )),
    );
    let result = f.doc.structure_associations().unwrap().unwrap();
    assert!(
        matches!(&result.items[0], StructureAssociation::MarkedContent { element, mcid: 2, .. } if element.object_id() == f.leaf)
    );
    assert!(
        matches!(&result.items[1], StructureAssociation::MarkedContent { element, mcid: 0, .. } if element.object_id() == child)
    );
    assert!(
        matches!(&result.items[2], StructureAssociation::ObjectReference { element, .. } if element.object_id() == f.leaf)
    );
}

#[test]
fn objr_whole_xobject_can_have_one_reference_per_rendering_page() {
    let f = fixture(Default::default());
    let image = stream(
        &f.doc,
        "<< /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceGray /BitsPerComponent 8 /StructParent 3 >>",
        "x",
    );
    set(
        &f.doc,
        f.page,
        "Resources",
        parse(&format!(
            "<< /Properties << /Props << /MCID 2 >> >> /XObject << /F {} 0 R /Image {} 0 R >> >>",
            f.form.obj_num(),
            image.obj_num()
        )),
    );
    replace_contents(&f, "/P /Props BDC EMC /F Do /Image Do /Image Do");
    f.doc.add_page(1).unwrap();
    let second = f
        .doc
        .destination(1, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let contents = stream(&f.doc, "<< >>", "/Image Do");
    set(&f.doc, second, "Contents", reference(contents));
    set(
        &f.doc,
        second,
        "Resources",
        parse(&format!(
            "<< /XObject << /Image {} 0 R >> >>",
            image.obj_num()
        )),
    );
    set(
        &f.doc,
        f.leaf,
        "K",
        parse(&format!(
            "[2 << /Type /MCR /Stm {} 0 R /MCID 0 >> << /Type /OBJR /Obj {} 0 R >> << /Type /OBJR /Obj {} 0 R >> << /Type /OBJR /Obj {} 0 R /Pg {} 0 R >>]",
            f.form.obj_num(),
            f.annot.obj_num(),
            image.obj_num(),
            image.obj_num(),
            second.obj_num()
        )),
    );
    set(
        &f.doc,
        f.root,
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [null null {} 0 R] 1 [{} 0 R] 2 {} 0 R 3 {} 0 R] >>",
            f.leaf.obj_num(),
            f.leaf.obj_num(),
            f.leaf.obj_num(),
            f.leaf.obj_num()
        )),
    );
    set(&f.doc, f.root, "ParentTreeNextKey", parse("4"));
    assert_eq!(
        f.doc.structure_associations().unwrap().unwrap().items.len(),
        5
    );
    let mut dict = f.doc.resolve_object(second).unwrap().into_dict().unwrap();
    dict.try_insert("Contents".into(), reference(stream(&f.doc, "<< >>", "")))
        .unwrap();
    f.doc
        .update_object(second, PdfObject::PdfDict(dict))
        .unwrap();
    fails(&f, "not an annotation or invoked XObject");
    set(&f.doc, second, "Contents", reference(contents));
    set(
        &f.doc,
        f.leaf,
        "K",
        parse(&format!(
            "[2 << /Type /MCR /Stm {} 0 R /MCID 0 >> << /Type /OBJR /Obj {} 0 R >> << /Type /OBJR /Obj {} 0 R >>]",
            f.form.obj_num(),
            f.annot.obj_num(),
            image.obj_num()
        )),
    );
    fails(&f, "requires an OBJR for each rendering page");
}

#[test]
fn lexical_strings_comments_and_inline_images_are_not_structural_operators() {
    let f = fixture(Default::default());
    replace_contents(
        &f,
        "(EMC /P << /MCID 2 >> BDC) Tj % /P << /MCID 2 >> BDC\n BI /W 1 /H 1 /CS /G /BPC 8 ID x EI /P /Props BDC EMC /F Do",
    );
    assert!(f.doc.structure_associations().unwrap().is_some());
}

#[test]
fn explicit_empty_form_resources_do_not_inherit_callers_properties() {
    let f = fixture(Default::default());
    let form = PdfStream::from_decoded_data(
        parse("<< /Subtype /Form /BBox [0 0 10 10] /StructParents 1 /Resources << >> >>")
            .into_dict()
            .unwrap(),
        b"/P /Props BDC EMC".to_vec(),
    )
    .unwrap();
    f.doc
        .update_object(f.form, PdfObject::PdfStream(form))
        .unwrap();
    fails(&f, "resource category missing");
}

#[test]
fn marked_content_and_form_depth_and_cumulative_entries_are_bounded() {
    let mut options = ParseOptions::default();
    options.limits.max_depth = 4;
    let f = fixture(options);
    replace_contents(&f, "/P BMC /P BMC /P BMC /P BMC /P BMC EMC EMC EMC EMC EMC");
    assert!(matches!(
        f.doc.structure_associations(),
        Err(PdfError::ResourceLimit(_))
    ));
    let f = fixture(options);
    let mut next = f.form;
    for _ in 0..5 {
        next = stream(
            &f.doc,
            &format!(
                "<< /Subtype /Form /BBox [0 0 10 10] /Resources << /XObject << /Next {} 0 R >> >> >>",
                next.obj_num()
            ),
            "/Next Do",
        );
    }
    set(
        &f.doc,
        f.page,
        "Resources",
        parse(&format!(
            "<< /XObject << /Deep {} 0 R >> >>",
            next.obj_num()
        )),
    );
    replace_contents(&f, "/Deep Do");
    assert!(matches!(
        f.doc.structure_associations(),
        Err(PdfError::ResourceLimit(_))
    ));
    let mut options = ParseOptions::default();
    options.limits.max_objects = 16;
    let f = fixture(options);
    replace_contents(&f, &format!("/P /Props BDC EMC {}", "/F Do ".repeat(20)));
    assert!(matches!(
        f.doc.structure_associations(),
        Err(PdfError::ResourceLimit(_))
    ));
}
