use pdf::{
    document::{
        PdfDocument,
        acroform::{
            ChoiceOption, FormFieldDefinition as Definition, FormFieldKind as Kind, FormTextStyle,
            FormValue as Value, FormWidgetSpec as Widget,
        },
        annotations::TextAlignment,
        embedded_font::TrueTypeEmbeddingOptions,
        identity::ObjectHandle,
        save_options::PdfVersion,
    },
    objects::PdfObject,
    page::annotations::AnnotationRenderOptions,
    parser::syntax::ParseMode,
};
use std::io::Cursor;
type Doc = PdfDocument<Cursor<Vec<u8>>>;

fn document() -> Doc {
    let doc = PdfDocument::new().unwrap();
    for i in 0..2 {
        doc.add_page(i).unwrap();
        doc.replace_page_contents(i, b"q Q".to_vec()).unwrap();
    }
    doc
}
fn widget(page: u32) -> Widget {
    Widget::new(page, [20., 30., 220., 130.])
}
fn button(page: u32, state: &str) -> Widget {
    Widget {
        page_index: page,
        rect: [20., 30., 44., 54.],
        on_state: Some(state.into()),
    }
}
fn dict(doc: &Doc, handle: &ObjectHandle) -> pdf::objects::pdf_dict::PdfDict {
    doc.resolve_handle(handle).unwrap().into_dict().unwrap()
}
fn selected(doc: &Doc, field: &ObjectHandle) -> Vec<String> {
    doc.form_field_tree()
        .unwrap()
        .iter()
        .find(|f| &f.handle == field)
        .unwrap()
        .widgets
        .iter()
        .map(|w| {
            dict(doc, &w.handle)
                .get("AS")
                .unwrap()
                .as_name()
                .unwrap()
                .name()
                .into()
        })
        .collect()
}
fn reopened(doc: &Doc) -> Doc {
    PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap()
}
fn cjk_style(doc: &Doc) -> FormTextStyle {
    let font = doc
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
    FormTextStyle {
        font: Some(font.object_handle().clone()),
        font_size: 16.,
        alignment: TextAlignment::Center,
    }
}

#[test]
fn custom_checkbox_states_and_unicode_exports_keep_v_as_ap_consistent() {
    let doc = document();
    let handle = doc
        .add_form_field(
            "custom",
            &Definition::new(Kind::Checkbox, Value::Checkbox(true)),
            &[button(0, "Approved/#"), button(1, "Approved/#")],
        )
        .unwrap();
    assert_eq!(selected(&doc, &handle), ["Approved/#", "Approved/#"]);
    let old_ap = dict(&doc, &doc.form_field_tree().unwrap()[0].widgets[0].handle)
        .get("AP")
        .unwrap()
        .clone();
    doc.set_form_value("custom", Value::Checkbox(false))
        .unwrap();
    assert_eq!(selected(&doc, &handle), ["Off", "Off"]);
    assert_eq!(
        dict(&doc, &doc.form_field_tree().unwrap()[0].widgets[0].handle).get("AP"),
        Some(&old_ap)
    );
    doc.set_form_value("custom", Value::Checkbox(true)).unwrap();
    let mut def = Definition::new(Kind::Checkbox, Value::ButtonState(Some("1".into())));
    def.button_exports = Some(vec!["同意".into(), "拒绝".into()]);
    let opt = doc
        .add_form_field("export", &def, &[button(0, "0"), button(1, "1")])
        .unwrap();
    assert_eq!(selected(&doc, &opt), ["Off", "1"]);
    let revision = doc.commit_revision();
    assert!(doc.set_form_value("export", Value::Checkbox(true)).is_err());
    assert_eq!(doc.commit_revision(), revision);
    doc.set_form_value("export", Value::ButtonState(Some("0".into())))
        .unwrap();
    assert_eq!(selected(&doc, &opt), ["0", "Off"]);
    let doc = reopened(&doc);
    let fields = doc.form_field_details().unwrap();
    assert_eq!(
        fields[1].button_exports.as_ref().unwrap(),
        &["同意", "拒绝"]
    );
    assert_eq!(fields[1].value, Value::ButtonState(Some("0".into())));
    for page in 0..2 {
        doc.get_page(page)
            .unwrap()
            .display_list_with_annotations(Default::default())
            .unwrap();
    }
}

#[test]
fn radio_selection_unison_duplicate_states_and_widget_removal_are_atomic() {
    let doc = document();
    let def = Definition::new(
        Kind::Radio {
            no_toggle_to_off: true,
            in_unison: false,
        },
        Value::Radio(Some("A".into())),
    );
    let handle = doc
        .add_form_field("radio", &def, &[button(0, "A"), button(1, "B")])
        .unwrap();
    assert_eq!(selected(&doc, &handle), ["A", "Off"]);
    doc.set_form_value("radio", Value::Radio(Some("B".into())))
        .unwrap();
    assert_eq!(selected(&doc, &handle), ["Off", "B"]);
    let widgets = doc.form_field_tree().unwrap()[0].widgets.clone();
    let revision = doc.commit_revision();
    assert!(doc.set_form_value("radio", Value::Radio(None)).is_err());
    assert!(doc.remove_form_widget(&widgets[1].handle).is_err());
    assert_eq!(doc.commit_revision(), revision);
    doc.set_radio_widget(&widgets[0].handle).unwrap();
    doc.remove_form_widget(&widgets[1].handle).unwrap();
    doc.add_form_widget_with_state(&handle, 1, [20., 30., 44., 54.], Some("B"))
        .unwrap();
    let mut def = Definition::new(
        Kind::Radio {
            no_toggle_to_off: false,
            in_unison: true,
        },
        Value::Radio(Some("A".into())),
    );
    let unison = doc
        .add_form_field("unison", &def, &[button(0, "A"), button(1, "A")])
        .unwrap();
    assert_eq!(selected(&doc, &unison), ["A", "A"]);
    def.kind = Kind::Radio {
        no_toggle_to_off: false,
        in_unison: false,
    };
    def.value = Value::Radio(None);
    let duplicate = doc
        .add_form_field("duplicate", &def, &[button(0, "A"), button(1, "A")])
        .unwrap();
    let revision = doc.commit_revision();
    assert!(
        doc.set_form_value("duplicate", Value::Radio(Some("A".into())))
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    let widgets = doc
        .form_field_tree()
        .unwrap()
        .iter()
        .find(|f| f.handle == duplicate)
        .unwrap()
        .widgets
        .clone();
    doc.set_radio_widget(&widgets[1].handle).unwrap();
    assert_eq!(selected(&doc, &duplicate), ["Off", "A"]);
    doc.remove_form_widget(&widgets[1].handle).unwrap();
    assert_eq!(selected(&doc, &duplicate), ["Off"]);
    reopened(&doc).form_field_details().unwrap();
}

fn choice(multiple: bool, combo: bool, editable: bool) -> Definition {
    let mut definition = Definition::new(
        Kind::Choice {
            multiple,
            combo,
            editable,
        },
        Value::Choice(vec!["B".into()]),
    );
    definition.options = vec![
        ChoiceOption {
            export: "A".into(),
            display: "Alpha".into(),
        },
        ChoiceOption {
            export: "B".into(),
            display: "Beta".into(),
        },
        ChoiceOption {
            export: "C".into(),
            display: "Gamma".into(),
        },
    ];
    definition
}

#[test]
fn choice_export_display_multiselect_and_duplicate_exports_use_sorted_indices() {
    let doc = document();
    let mut def = choice(true, false, false);
    def.value = Value::Choice(vec!["C".into(), "A".into()]);
    let handle = doc
        .add_form_field("list", &def, &[widget(0), widget(1)])
        .unwrap();
    let field = dict(&doc, &handle);
    assert_eq!(
        field
            .get("I")
            .unwrap()
            .as_array()
            .unwrap()
            .get(0)
            .unwrap()
            .as_f32(),
        Some(0.)
    );
    assert_eq!(
        doc.form_field_details().unwrap()[0].value,
        Value::ChoiceIndices(vec![0, 2])
    );
    doc.set_form_value("list", Value::Choice(vec![])).unwrap();
    assert!(matches!(
        dict(&doc, &handle).get("V"),
        Some(PdfObject::PdfNull)
    ));
    let revision = doc.commit_revision();
    assert!(
        doc.set_form_value("list", Value::Choice(vec!["not an option".into()]))
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    def.options[1].export = "A".into();
    def.value = Value::ChoiceIndices(vec![1]);
    let duplicate = doc.add_form_field("duplicate", &def, &[widget(0)]).unwrap();
    doc.set_form_value("duplicate", Value::ChoiceIndices(vec![0, 1]))
        .unwrap();
    let revision = doc.commit_revision();
    assert!(
        doc.set_form_value("duplicate", Value::Choice(vec!["A".into()]))
            .is_err()
    );
    assert!(
        doc.set_form_value("duplicate", Value::ChoiceIndices(vec![0, 0]))
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    assert_eq!(
        doc.form_field_details().unwrap()[1].value,
        Value::ChoiceIndices(vec![0, 1])
    );
    doc.add_form_widget(&duplicate, 1, [20., 250., 220., 350.])
        .unwrap();
    let field = doc
        .form_field_tree()
        .unwrap()
        .iter()
        .find(|f| f.handle == duplicate)
        .unwrap()
        .clone();
    doc.remove_form_widget(&field.widgets[1].handle).unwrap();
    let mut combo = choice(false, true, true);
    combo.value = Value::Choice(vec!["Custom".into()]);
    doc.add_form_field("combo", &combo, &[widget(0)]).unwrap();
    doc.set_form_value("combo", Value::Choice(vec!["B".into()]))
        .unwrap();
    reopened(&doc).form_field_details().unwrap();
}

#[test]
fn pushbutton_has_no_value_and_captions_and_style_update_all_widgets() {
    let doc = document();
    let mut def = Definition::new(Kind::PushButton, Value::PushButton);
    def.caption = "Submit".into();
    let handle = doc.add_form_field("push", &def, &[widget(0)]).unwrap();
    doc.add_form_widget(&handle, 1, [20., 200., 220., 240.])
        .unwrap();
    doc.set_pushbutton_caption(&handle, "Reset").unwrap();
    doc.set_form_text_style(
        &handle,
        &FormTextStyle {
            font: None,
            font_size: 14.,
            alignment: TextAlignment::Right,
        },
    )
    .unwrap();
    let field = dict(&doc, &handle);
    assert!(field.get("V").is_none() && field.get("DV").is_none());
    let revision = doc.commit_revision();
    assert!(doc.set_form_value("push", Value::PushButton).is_err());
    assert_eq!(doc.commit_revision(), revision);
    for widget in &doc.form_field_tree().unwrap()[0].widgets {
        assert_eq!(
            dict(&doc, &widget.handle)
                .get("MK")
                .unwrap()
                .as_dict()
                .unwrap()
                .get("CA")
                .unwrap()
                .as_string()
                .unwrap()
                .text_content()
                .unwrap(),
            "Reset"
        );
        assert!(dict(&doc, &widget.handle).get("AS").is_none());
    }
    reopened(&doc).form_field_details().unwrap();
}

#[test]
fn unicode_multiline_alignment_and_reopened_font_reuse_are_not_ascii_placeholders() {
    let doc = document();
    let style = cjk_style(&doc);
    let mut def = Definition::new(
        Kind::Text { multiline: true },
        Value::Text("你好\n世界".into()),
    );
    def.text_style = style;
    def.max_len = Some(5);
    doc.add_form_field("中文.text", &def, &[widget(0), widget(1)])
        .unwrap();
    let reopened = reopened(&doc);
    reopened
        .set_form_value("中文.text", Value::Text("世界\n你好".into()))
        .unwrap();
    let revision = reopened.commit_revision();
    assert!(
        reopened
            .set_form_value("中文.text", Value::Text("你好你好你好".into()))
            .is_err()
    );
    assert!(
        reopened
            .set_form_value("中文.text", Value::Text("缺字".into()))
            .is_err()
    );
    assert_eq!(reopened.commit_revision(), revision);
    for page in 0..2 {
        let mut page = reopened.get_page(page).unwrap();
        assert!(page.display_list().unwrap().objects().is_empty());
        assert_eq!(
            page.display_list_with_annotations(AnnotationRenderOptions::default())
                .unwrap()
                .appearance_count(),
            1
        );
    }
    assert_eq!(
        reopened.form_fields().unwrap()[0].value,
        Value::Text("世界\n你好".into())
    );
    let foreign = cjk_style(&document());
    let handle = reopened.form_field_tree().unwrap()[1].handle.clone();
    let revision = reopened.commit_revision();
    assert!(reopened.set_form_text_style(&handle, &foreign).is_err());
    assert_eq!(reopened.commit_revision(), revision);
}

#[test]
fn field_validation_overflow_invalid_flags_and_version_fail_without_partial_changes() {
    let doc = document();
    let revision = doc.commit_revision();
    let before = doc.save_to_bytes().unwrap();
    let mut def = Definition::new(Kind::Text { multiline: false }, Value::Text("x".repeat(80)));
    assert!(doc.add_form_field("a.b", &def, &[widget(0)]).is_err());
    def.value = Value::Text("bad\nline".into());
    assert!(doc.add_form_field("a.b", &def, &[widget(0)]).is_err());
    def.value = Value::Text("a".into());
    def.text_style.font_size = f32::NAN;
    assert!(doc.add_form_field("a.b", &def, &[widget(0)]).is_err());
    assert_eq!(doc.commit_revision(), revision);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    for (version, kind) in [
        (PdfVersion::V1_0, Kind::Checkbox),
        (
            PdfVersion::V1_3,
            Kind::Choice {
                combo: false,
                editable: false,
                multiple: true,
            },
        ),
        (
            PdfVersion::V1_4,
            Kind::Radio {
                no_toggle_to_off: false,
                in_unison: true,
            },
        ),
    ] {
        let old = PdfDocument::new_with_version(version).unwrap();
        old.add_page(0).unwrap();
        let revision = old.commit_revision();
        assert!(
            old.add_form_field(
                "new",
                &Definition::new(kind, Value::Checkbox(false)),
                &[widget(0)]
            )
            .is_err()
        );
        assert_eq!(old.commit_revision(), revision);
    }
}

#[test]
fn advanced_fill_does_not_grant_style_caption_creation_or_general_modification() {
    use pdf::{
        document::Document,
        security::{EncryptionOptions, SaveSecurity},
    };
    let doc = document();
    doc.add_form_field(
        "radio",
        &Definition::new(
            Kind::Radio {
                no_toggle_to_off: false,
                in_unison: false,
            },
            Value::Radio(Some("A".into())),
        ),
        &[button(0, "A"), button(1, "B")],
    )
    .unwrap();
    doc.add_form_field("choice", &choice(false, true, false), &[widget(0)])
        .unwrap();
    let mut push = Definition::new(Kind::PushButton, Value::PushButton);
    push.caption = "Push".into();
    doc.add_form_field("push", &push, &[widget(0)]).unwrap();
    let original = Document::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.permissions.modify = false;
    settings.permissions.annotate = false;
    settings.permissions.fill_forms = true;
    let user = Document::from_bytes_with_password(
        original
            .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
            .unwrap(),
        "user",
    )
    .unwrap();
    let fields = user.form_field_tree().unwrap();
    user.set_radio_widget(&fields[0].widgets[1].handle).unwrap();
    user.set_form_value("choice", Value::Choice(vec!["C".into()]))
        .unwrap();
    let revision = user.commit_revision();
    assert!(
        user.set_form_text_style(&fields[1].handle, &Default::default())
            .is_err()
    );
    assert!(
        user.set_pushbutton_caption(&fields[2].handle, "Denied")
            .is_err()
    );
    assert!(user.add_form_field("denied", &push, &[widget(0)]).is_err());
    assert!(user.add_page(2).is_err());
    assert_eq!(user.commit_revision(), revision);
    let owner = Document::from_bytes_with_password(user.save_to_bytes().unwrap(), "owner").unwrap();
    assert_eq!(
        owner.form_field_details().unwrap()[0].value,
        Value::Radio(Some("B".into()))
    );
    assert_eq!(
        owner.form_field_details().unwrap()[1].value,
        Value::Choice(vec!["C".into()])
    );
}

#[test]
fn text_budget_missing_glyph_overflow_and_signed_source_leave_state_unchanged() {
    use pdf::{
        error::PdfError,
        parser::options::{Limits, ParseOptions},
    };
    let doc = document();
    doc.add_text_field(0, [20., 30., 220., 100.], "text", "Before")
        .unwrap();
    let bytes = doc.save_to_bytes().unwrap();
    let limited = Doc::from_reader_with_options(
        Cursor::new(bytes),
        ParseOptions {
            limits: Limits {
                max_operation_bytes: 1,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let revision = limited.commit_revision();
    assert!(matches!(
        limited.set_form_value("text", Value::Text("After".into())),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(limited.commit_revision(), revision);
    let mut push = Definition::new(Kind::PushButton, Value::PushButton);
    push.caption = "Push".into();
    doc.add_form_field("push", &push, &[widget(0)]).unwrap();
    let mut signature = pdf::objects::pdf_dict::PdfDict::default();
    signature
        .try_insert(
            "Type".into(),
            PdfObject::PdfName(pdf::objects::pdf_name::PdfName::new("Sig".into())),
        )
        .unwrap();
    doc.try_insert_object(PdfObject::PdfDict(signature))
        .unwrap();
    let signed = Doc::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
    let fields = signed.form_field_tree().unwrap();
    let revision = signed.commit_revision();
    assert!(
        signed
            .set_form_text_style(&fields[0].handle, &Default::default())
            .is_err()
    );
    assert!(
        signed
            .set_pushbutton_caption(&fields[1].handle, "Denied")
            .is_err()
    );
    assert!(
        signed
            .set_form_value("text", Value::Text("Denied".into()))
            .is_err()
    );
    assert_eq!(signed.commit_revision(), revision);
}

#[test]
fn single_choice_duplicate_export_and_low_level_version_guards_preserve_indices() {
    let doc = document();
    let mut def = choice(false, true, false);
    def.options[1].export = "A".into();
    def.value = Value::ChoiceIndices(vec![1]);
    doc.add_form_field("single", &def, &[widget(0)]).unwrap();
    assert_eq!(
        reopened(&doc).form_field_details().unwrap()[0].value,
        Value::ChoiceIndices(vec![1])
    );
    let old = PdfDocument::new_with_version(PdfVersion::V1_4).unwrap();
    old.add_page(0).unwrap();
    let field = old
        .add_form_field(
            "radio",
            &Definition::new(
                Kind::Radio {
                    no_toggle_to_off: false,
                    in_unison: false,
                },
                Value::Radio(Some("A".into())),
            ),
            &[button(0, "A"), button(0, "B")],
        )
        .unwrap();
    let mut d = dict(&old, &field);
    d.try_insert(
        "Ff".into(),
        PdfObject::PdfNumber(
            pdf::objects::pdf_number::PdfNumber::new_from_lexeme(
                ((1u32 << 25) | (1 << 15) | 1).to_string().as_bytes(),
            )
            .unwrap(),
        ),
    )
    .unwrap();
    old.update_object(field.object_id(), PdfObject::PdfDict(d))
        .unwrap();
    assert_eq!(
        old.form_field_tree().unwrap()[0].flags,
        (1 << 25) | (1 << 15) | 1
    );
    let revision = old.commit_revision();
    assert!(old.save_to_bytes().is_err());
    assert_eq!(old.commit_revision(), revision);
}

#[test]
fn optional_off_appearance_is_materialized_without_repainting_the_on_artwork() {
    let doc = document();
    let field = doc
        .add_form_field(
            "check",
            &Definition::new(Kind::Checkbox, Value::Checkbox(true)),
            &[button(0, "Custom")],
        )
        .unwrap();
    let mut d = dict(&doc, &field);
    let mut ap = d.remove("AP").unwrap().into_dict().unwrap();
    let mut normal = ap.remove("N").unwrap().into_dict().unwrap();
    normal.remove("Off");
    let on = normal.get("Custom").unwrap().clone();
    ap.try_insert("N".into(), PdfObject::PdfDict(normal))
        .unwrap();
    d.try_insert("AP".into(), PdfObject::PdfDict(ap)).unwrap();
    doc.update_object(field.object_id(), PdfObject::PdfDict(d))
        .unwrap();
    doc.set_form_value("check", Value::Checkbox(false)).unwrap();
    let d = dict(&doc, &field);
    let states = d
        .get("AP")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("N")
        .unwrap()
        .as_dict()
        .unwrap();
    assert_eq!(states.get("Custom"), Some(&on));
    assert!(states.get("Off").is_some());
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_annotations(Default::default())
            .unwrap()
            .appearance_count(),
        1
    );
}

#[test]
fn multi_widget_appearance_character_work_is_aggregate_and_rolls_back() {
    use pdf::parser::options::{Limits, ParseOptions};
    let doc = document();
    doc.add_text_field(0, [20., 30., 220., 100.], "a", "Old")
        .unwrap();
    let field = doc.form_field_tree().unwrap()[0].handle.clone();
    doc.add_form_widget(&field, 1, [20., 30., 220., 100.])
        .unwrap();
    let limited = Doc::from_reader_with_options(
        Cursor::new(doc.save_to_bytes().unwrap()),
        ParseOptions {
            limits: Limits {
                max_text_chars: 6,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let revision = limited.commit_revision();
    assert!(
        limited
            .set_form_value("a", Value::Text("1234".into()))
            .is_err()
    );
    assert_eq!(limited.commit_revision(), revision);
    limited
        .set_form_value("a", Value::Text("123".into()))
        .unwrap();
}

#[test]
fn field_records_preserve_known_fields_beside_unsupported_profiles() {
    let doc = document();
    doc.add_text_field(0, [10., 10., 120., 30.], "known", "Hello")
        .unwrap();
    doc.add_text_field(0, [10., 40., 120., 60.], "unsupported", "Hidden")
        .unwrap();
    let field = doc
        .form_field_tree()
        .unwrap()
        .into_iter()
        .find(|f| f.name == "unsupported")
        .unwrap();
    let mut dict = doc
        .resolve_handle(&field.handle)
        .unwrap()
        .into_dict()
        .unwrap();
    dict.try_insert(
        "Ff".into(),
        PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(
            (1u32 << 13) as f32,
        )),
    )
    .unwrap();
    doc.update_object(field.handle.object_id(), PdfObject::PdfDict(dict))
        .unwrap();
    assert!(doc.form_field_details().is_err());
    let records = doc.form_field_records().unwrap();
    assert_eq!(records.len(), 2);
    assert!(
        records
            .iter()
            .find(|r| r.name == "known")
            .unwrap()
            .details
            .is_some()
    );
    let unknown = records.iter().find(|r| r.name == "unsupported").unwrap();
    assert!(unknown.details.is_none());
    assert!(unknown.unsupported_reason.is_some());
    assert_eq!(unknown.widgets.len(), 1);
}
