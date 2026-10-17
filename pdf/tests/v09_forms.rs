use pdf::{
    document::{PdfDocument, acroform::FormValue, identity::ObjectHandle},
    io::stream_reader::StreamReader,
    objects::PdfObject,
    page::annotations::AnnotationRenderOptions,
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;

fn parse(text: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn dict(
    doc: &PdfDocument<Cursor<Vec<u8>>>,
    handle: &ObjectHandle,
) -> pdf::objects::pdf_dict::PdfDict {
    doc.resolve_handle(handle).unwrap().into_dict().unwrap()
}
fn set(doc: &PdfDocument<Cursor<Vec<u8>>>, handle: &ObjectHandle, key: &str, value: &str) {
    let mut d = dict(doc, handle);
    d.try_insert(key.into(), parse(value)).unwrap();
    doc.update_object(handle.object_id(), PdfObject::PdfDict(d))
        .unwrap();
}
fn created() -> PdfDocument<Cursor<Vec<u8>>> {
    let doc = PdfDocument::new().unwrap();
    for page in 0..2 {
        doc.add_page(page).unwrap();
        doc.replace_page_contents(page, b"q Q".to_vec()).unwrap();
    }
    doc.add_text_field(0, [20., 40., 180., 70.], "person.address.city", "Shanghai")
        .unwrap();
    doc.add_checkbox_field(0, [20., 90., 44., 114.], "person.accepted", false)
        .unwrap();
    doc
}

#[test]
fn hierarchical_crud_and_multi_widget_fill_keep_stable_field_identity() {
    let doc = created();
    let tree = doc.form_field_tree().unwrap();
    assert_eq!(
        tree.iter().map(|n| n.name.as_str()).collect::<Vec<_>>(),
        [
            "person",
            "person.address",
            "person.address.city",
            "person.accepted"
        ]
    );
    let city = tree[2].handle.clone();
    let added = doc
        .add_form_widget(&city, 1, [25., 150., 225., 190.])
        .unwrap();
    let check = tree[3].handle.clone();
    doc.add_form_widget(&check, 1, [25., 220., 49., 244.])
        .unwrap();
    let tree = doc.form_field_tree().unwrap();
    assert_eq!(tree[2].handle, city);
    assert_eq!(tree[2].widgets.len(), 2);
    assert_ne!(tree[2].widgets[0].handle, city);
    assert_eq!(tree[2].widgets[1].handle, added);
    assert!(dict(&doc, &city).get("AP").is_none());
    doc.set_form_value("person.address.city", FormValue::Text("Suzhou".into()))
        .unwrap();
    doc.set_form_value("person.accepted", FormValue::Checkbox(true))
        .unwrap();
    let stamp = doc.stamp();
    for page in 0..2 {
        let list = doc
            .get_page(page)
            .unwrap()
            .display_list_with_annotations(AnnotationRenderOptions::default())
            .unwrap();
        assert_eq!(list.appearance_count(), 2);
    }
    assert!(doc.is_stamp_current(&stamp));
    for w in &doc.form_field_tree().unwrap()[3].widgets {
        assert_eq!(dict(&doc, &w.handle).get("AS"), Some(&parse("/Yes")));
        assert!(dict(&doc, &w.handle).get("V").is_none());
    }
    doc.rename_form_field(&tree[0].handle, "customer").unwrap();
    assert_eq!(
        doc.form_field_tree().unwrap()[2].name,
        "customer.address.city"
    );
    assert_eq!(
        doc.form_fields().unwrap()[0].value,
        FormValue::Text("Suzhou".into())
    );
    doc.remove_form_widget(&added).unwrap();
    let before = doc.commit_revision();
    let remaining = doc.form_field_tree().unwrap()[2].widgets[0].handle.clone();
    assert!(doc.remove_form_widget(&remaining).is_err());
    assert_eq!(doc.commit_revision(), before);
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(reopened.form_field_tree().unwrap()[2].widgets.len(), 1);
    let address = reopened.form_field_tree().unwrap()[1].handle.clone();
    reopened.remove_form_field(&address).unwrap();
    assert_eq!(reopened.form_fields().unwrap().len(), 1);
    assert_eq!(reopened.form_field_tree().unwrap().len(), 2);
    let person = reopened.form_field_tree().unwrap()[0].handle.clone();
    reopened.remove_form_field(&person).unwrap();
    assert!(reopened.form_field_tree().unwrap().is_empty());
    assert!(
        reopened
            .get_page(0)
            .unwrap()
            .display_list_with_annotations(Default::default())
            .unwrap()
            .appearance_count()
            == 0
    );
    PdfDocument::from_bytes_with_mode(reopened.save_to_bytes().unwrap(), ParseMode::Strict)
        .unwrap();
}

#[test]
fn inherited_values_flags_and_maxlen_apply_to_split_widgets() {
    let doc = created();
    let tree = doc.form_field_tree().unwrap();
    let city = &tree[2].handle;
    doc.add_form_widget(city, 1, [20., 120., 220., 160.])
        .unwrap();
    set(&doc, &tree[1].handle, "FT", "/Tx");
    set(&doc, &tree[1].handle, "V", "(Inherited)");
    set(&doc, &tree[1].handle, "MaxLen", "8");
    let mut field = dict(&doc, city);
    field.remove("FT");
    field.remove("V");
    doc.update_object(city.object_id(), PdfObject::PdfDict(field))
        .unwrap();
    assert_eq!(
        doc.form_fields().unwrap()[0].value,
        FormValue::Text("Inherited".into())
    );
    let revision = doc.commit_revision();
    assert!(
        doc.set_form_value("person.address.city", FormValue::Text("too long!".into()))
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    assert!(
        doc.add_text_field(
            0,
            [20., 250., 220., 290.],
            "person.address.new",
            "exceeds maxlen"
        )
        .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    doc.set_form_value("person.address.city", FormValue::Text("New".into()))
        .unwrap();
    assert_eq!(
        dict(&doc, &tree[1].handle).get("V"),
        Some(&parse("(Inherited)"))
    );
    set(&doc, &tree[1].handle, "Ff", "1");
    assert_eq!(doc.form_field_tree().unwrap()[2].flags, 1);
    let revision = doc.commit_revision();
    assert!(
        doc.set_form_value("person.address.city", FormValue::Text("No".into()))
            .is_err()
    );
    assert!(doc.remove_form_field(city).is_err());
    assert_eq!(doc.commit_revision(), revision);
}

#[test]
fn optional_page_and_missing_appearance_are_inspectable_and_fill_repairs_ap() {
    let doc = created();
    let handle = doc.form_field_tree().unwrap()[2].handle.clone();
    let mut field = dict(&doc, &handle);
    field.remove("P");
    field.remove("AP");
    field.remove("V");
    doc.update_object(handle.object_id(), PdfObject::PdfDict(field))
        .unwrap();
    assert_eq!(doc.form_field_tree().unwrap()[2].widgets[0].page_index, 0);
    assert_eq!(
        doc.form_fields().unwrap()[0].value,
        FormValue::Text(String::new())
    );
    doc.set_form_value("person.address.city", FormValue::Text("Repaired".into()))
        .unwrap();
    assert!(dict(&doc, &handle).get("AP").is_some());
}

#[test]
fn collisions_invalid_names_foreign_handles_and_late_widget_failure_roll_back() {
    let doc = created();
    let tree = doc.form_field_tree().unwrap();
    let before = doc.save_to_bytes().unwrap();
    let revision = doc.commit_revision();
    assert!(doc.rename_form_field(&tree[1].handle, "accepted").is_err());
    assert!(doc.rename_form_field(&tree[0].handle, "bad.name").is_err());
    assert!(doc.add_form_group(None, "person").is_err());
    assert!(
        doc.add_text_field(0, [10., 10., 120., 40.], "person.accepted.bad", "No")
            .is_err()
    );
    assert!(
        doc.add_form_widget(&tree[2].handle, 99, [10., 10., 120., 40.])
            .is_err()
    );
    let foreign = created().form_field_tree().unwrap()[0].handle.clone();
    assert!(doc.rename_form_field(&foreign, "foreign").is_err());
    assert_eq!(doc.commit_revision(), revision);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert_eq!(doc.form_field_tree().unwrap()[2].handle, tree[2].handle);
    let group = doc.add_form_group(None, "empty").unwrap();
    doc.add_form_group(Some(&group), "child").unwrap();
    assert!(
        doc.add_form_group(Some(&tree[2].handle), "not-a-group")
            .is_err()
    );
    doc.remove_form_field(&group).unwrap();
}

#[test]
fn cycles_shared_widgets_and_wrong_parent_are_rejected_without_mutation() {
    for corrupt in 0..3 {
        let doc = created();
        let tree = doc.form_field_tree().unwrap();
        let id = tree[2].handle.object_id();
        let group = tree[1].handle.object_id();
        match corrupt {
            0 => set(
                &doc,
                &tree[1].handle,
                "Kids",
                &format!("[{} {} R]", group.obj_num(), group.gen_num()),
            ),
            1 => set(
                &doc,
                &tree[1].handle,
                "Kids",
                &format!(
                    "[{} {} R {} {} R]",
                    id.obj_num(),
                    id.gen_num(),
                    id.obj_num(),
                    id.gen_num()
                ),
            ),
            _ => set(&doc, &tree[2].handle, "Parent", "1 0 R"),
        }
        let revision = doc.commit_revision();
        assert!(doc.form_field_tree().is_err());
        assert!(
            doc.set_form_value("person.address.city", FormValue::Text("No".into()))
                .is_err()
        );
        assert_eq!(doc.commit_revision(), revision);
    }
}

#[test]
fn tree_limits_and_unknown_types_do_not_require_appearance_generation() {
    let doc = created();
    let tree = doc.form_field_tree().unwrap();
    set(&doc, &tree[2].handle, "FT", "/FutureField");
    assert_eq!(
        doc.form_field_tree().unwrap()[2].field_type.as_deref(),
        Some("FutureField")
    );
    assert!(doc.form_fields().is_err());
    // An unrelated unsupported field must not block filling a supported one.
    doc.set_form_value("person.accepted", FormValue::Checkbox(true))
        .unwrap();
    let revision = doc.commit_revision();
    assert!(doc.remove_form_field(&tree[2].handle).is_err());
    assert_eq!(doc.commit_revision(), revision);
    let bytes = doc.save_to_bytes().unwrap();
    let options = ParseOptions {
        mode: ParseMode::Strict,
        limits: Limits {
            max_depth: 2,
            ..Default::default()
        },
    };
    let reopened = PdfDocument::from_reader_with_options(Cursor::new(bytes), options).unwrap();
    assert!(reopened.form_field_tree().is_err());
}

#[test]
fn unknown_keys_r_d_actions_and_widget_locks_are_not_silently_lost() {
    let doc = created();
    let tree = doc.form_field_tree().unwrap();
    let city = &tree[2].handle;
    set(&doc, city, "Custom", "(preserved)");
    let revision = doc.commit_revision();
    assert!(
        doc.add_form_widget(city, 1, [20., 120., 220., 160.])
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    let mut d = dict(&doc, city);
    d.remove("Custom");
    let mut ap = d.get("AP").unwrap().clone().into_dict().unwrap();
    ap.try_insert("R".into(), ap.get("N").unwrap().clone())
        .unwrap();
    ap.try_insert("D".into(), ap.get("N").unwrap().clone())
        .unwrap();
    d.try_insert("AP".into(), PdfObject::PdfDict(ap.clone()))
        .unwrap();
    doc.update_object(city.object_id(), PdfObject::PdfDict(d))
        .unwrap();
    let second = doc
        .add_form_widget(city, 1, [20., 120., 220., 160.])
        .unwrap();
    let widgets = doc.form_field_tree().unwrap()[2].widgets.clone();
    doc.set_form_value("person.address.city", FormValue::Text("Filled".into()))
        .unwrap();
    let actual = dict(&doc, &widgets[0].handle)
        .get("AP")
        .unwrap()
        .clone()
        .into_dict()
        .unwrap();
    assert_eq!(actual.get("R"), ap.get("R"));
    assert_eq!(actual.get("D"), ap.get("D"));
    set(&doc, &second, "F", "128");
    let revision = doc.commit_revision();
    assert!(
        doc.set_form_value("person.address.city", FormValue::Text("No".into()))
            .is_err()
    );
    assert!(doc.remove_form_field(city).is_err());
    assert_eq!(doc.commit_revision(), revision);
    set(&doc, &second, "F", "4");
    set(
        &doc,
        &tree[1].handle,
        "AA",
        "<< /K << /S /JavaScript /JS (never executed) >> >>",
    );
    let revision = doc.commit_revision();
    assert!(
        doc.set_form_value("person.address.city", FormValue::Text("No".into()))
            .is_err()
    );
    assert!(doc.rename_form_field(city, "town").is_err());
    assert_eq!(doc.commit_revision(), revision);
}

#[test]
fn structural_form_permissions_and_fill_permission_stay_separate() {
    use pdf::document::Document;
    use pdf::security::{EncryptionOptions, SaveSecurity};
    let original = Document::from_bytes(created().save_to_bytes().unwrap()).unwrap();
    let city = original.form_field_tree().unwrap()[2].handle.clone();
    original
        .add_form_widget(&city, 1, [20., 120., 220., 160.])
        .unwrap();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.permissions.modify = false;
    settings.permissions.annotate = false;
    settings.permissions.fill_forms = true;
    let bytes = original
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
        .unwrap();
    let user =
        Document::from_bytes_with_password_and_options(bytes, "user", Default::default()).unwrap();
    let tree = user.form_field_tree().unwrap();
    let revision = user.commit_revision();
    assert!(user.add_form_group(None, "denied").is_err());
    assert!(user.rename_form_field(&tree[0].handle, "denied").is_err());
    assert!(
        user.add_form_widget(&tree[2].handle, 1, [20., 220., 220., 260.])
            .is_err()
    );
    assert!(user.remove_form_widget(&tree[2].widgets[1].handle).is_err());
    assert!(user.remove_form_field(&tree[2].handle).is_err());
    assert_eq!(user.commit_revision(), revision);
    user.set_form_value("person.address.city", FormValue::Text("Allowed".into()))
        .unwrap();
    assert_eq!(user.commit_revision(), revision + 1);
    let owner = Document::from_bytes_with_password_and_options(
        user.save_to_bytes().unwrap(),
        "owner",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        owner.form_fields().unwrap()[0].value,
        FormValue::Text("Allowed".into())
    );
    settings.permissions.modify = true;
    settings.permissions.annotate = true;
    settings.permissions.fill_forms = false;
    let bytes = owner
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
        .unwrap();
    let user =
        Document::from_bytes_with_password_and_options(bytes, "user", Default::default()).unwrap();
    let revision = user.commit_revision();
    assert!(
        user.set_form_value("person.address.city", FormValue::Text("Denied".into()))
            .is_err()
    );
    assert_eq!(user.commit_revision(), revision);
}

#[test]
fn signatures_and_invalid_page_ownership_block_edits() {
    let original = created();
    original
        .try_insert_object(parse("<< /Type /Sig >>"))
        .unwrap();
    let doc = PdfDocument::from_bytes(original.save_to_bytes().unwrap()).unwrap();
    let tree = doc.form_field_tree().unwrap();
    let revision = doc.commit_revision();
    assert!(doc.add_form_group(None, "denied").is_err());
    assert!(doc.rename_form_field(&tree[0].handle, "denied").is_err());
    assert!(doc.remove_form_field(&tree[0].handle).is_err());
    assert!(
        doc.add_form_widget(&tree[2].handle, 1, [20., 120., 220., 160.])
            .is_err()
    );
    assert!(
        doc.set_form_value("person.address.city", FormValue::Text("Denied".into()))
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    let doc = created();
    let tree = doc.form_field_tree().unwrap();
    set(&doc, &tree[2].handle, "P", "1 0 R");
    assert!(doc.form_field_tree().is_err());
}
