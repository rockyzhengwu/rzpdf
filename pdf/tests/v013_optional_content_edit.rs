use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        optional_content::OptionalContentOptions,
        optional_content_edit::*,
        save_options::{
            IncrementalSaveLayout, PdfVersion, SaveLayout, SaveOptions, StreamCompression,
        },
    },
    objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_stream::PdfStream},
    page::marked_text::MarkedTextOptions,
    parser::syntax::ParseMode,
};
fn pool() -> OperationBudget {
    OperationBudget::new(64 * 1024 * 1024)
}
fn doc() -> Document {
    let d = Document::new().unwrap();
    d.add_page(0).unwrap();
    d
}
fn config(
    on: Vec<pdf::document::identity::ObjectHandle>,
    off: Vec<pdf::document::identity::ObjectHandle>,
) -> LayerConfiguration {
    LayerConfiguration {
        name: Some("配置".into()),
        base: LayerBaseState::On,
        on,
        off,
    }
}
fn text(d: &Document, options: OptionalContentOptions) -> String {
    let v = d.optional_content_visibility(&options).unwrap();
    d.get_page(0)
        .unwrap()
        .extract_marked_text_with_optional_content(MarkedTextOptions::default(), &v)
        .unwrap()
        .text(&Default::default())
        .unwrap()
}
#[test]
fn groups_configurations_memberships_and_unicode_roundtrip() {
    let d = doc();
    let b = pool();
    let a = d.create_layer("同名", LayerIntent::View, &b).unwrap();
    let c = d.create_layer("同名", LayerIntent::View, &b).unwrap();
    assert_ne!(a, c);
    d.rename_layer(&a, "新名字", &b).unwrap();
    let t = d.layer_configuration_target(None).unwrap();
    d.write_layer_configuration(Some(&t), &config(vec![a.clone()], vec![c.clone()]), &b)
        .unwrap();
    assert!(
        d.write_layer_configuration(Some(&t), &config(vec![], vec![]), &b)
            .is_err()
    );
    d.write_layer_configuration(None, &config(vec![c.clone()], vec![a.clone()]), &b)
        .unwrap();
    for p in [
        LayerPolicy::AnyOn,
        LayerPolicy::AllOn,
        LayerPolicy::AnyOff,
        LayerPolicy::AllOff,
    ] {
        let h = d
            .create_layer_membership(&[a.clone(), c.clone()], p, None, &b)
            .unwrap();
        assert!(d.resolve_handle(&h).unwrap().as_dict().is_some());
    }
    let e = LayerExpression::And(vec![
        LayerExpression::Group(a.clone()),
        LayerExpression::Not(Box::new(LayerExpression::Group(c.clone()))),
    ]);
    d.create_layer_membership(&[a.clone(), c.clone()], LayerPolicy::AnyOff, Some(&e), &b)
        .unwrap();
    for l in [
        SaveLayout::Classic,
        SaveLayout::XrefStream,
        SaveLayout::XrefStreamWithObjectStreams,
    ] {
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let bytes = d
                .save_to_bytes_with_layout(
                    SaveOptions {
                        compression,
                        ..Default::default()
                    },
                    l,
                )
                .unwrap();
            let r = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            let m = r.optional_content().unwrap().unwrap();
            assert_eq!(m.groups[0].name, "新名字");
            assert_eq!(m.configurations.len(), 1);
        }
    }
    assert_eq!(b.live_bytes(), 0);
}
#[test]
fn invalid_inputs_are_atomic_and_budget_can_retry() {
    let d = doc();
    let b = pool();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    let foreign = doc().create_layer("B", LayerIntent::View, &b).unwrap();
    let before = d.stamp();
    assert!(d.rename_layer(&foreign, "bad", &b).is_err());
    assert!(
        d.write_layer_configuration(None, &config(vec![a.clone()], vec![a.clone()]), &b)
            .is_err()
    );
    assert!(
        d.create_layer_membership(&[a.clone(), a.clone()], LayerPolicy::AnyOn, None, &b)
            .is_err()
    );
    assert!(
        d.create_layer_membership(
            std::slice::from_ref(&a),
            LayerPolicy::AnyOn,
            Some(&LayerExpression::And(vec![])),
            &b
        )
        .is_err()
    );
    assert!(d.is_stamp_current(&before));
    let small = OperationBudget::new(2 * 1024 * 1024 - 1);
    assert!(d.rename_layer(&a, "x", &small).is_err());
    assert_eq!(small.live_bytes(), 0);
    d.rename_layer(&a, "ok", &b).unwrap();
    assert_eq!(b.live_bytes(), 0);
}
#[test]
fn appended_text_is_selected_without_changing_old_bytes() {
    let d = doc();
    let b = pool();
    let a = d.create_layer("text", LayerIntent::View, &b).unwrap();
    let font = d
        .resources()
        .register_standard_font(0, pdf::document::resource_manager::StandardFont::Courier)
        .unwrap();
    d.append_layer_content(
        0,
        &a,
        format!("BT /{font} 12 Tf 10 20 Td (VISIBLE) Tj ET").as_bytes(),
        &b,
    )
    .unwrap();
    assert_eq!(text(&d, Default::default()), "VISIBLE");
    let v = OptionalContentOptions {
        overrides: vec![(a.clone(), false)],
        ..Default::default()
    };
    assert_eq!(text(&d, v), "");
    let program = d.get_page(0).unwrap().content_program().unwrap();
    let before = program.source_bytes().to_vec();
    let stamp = d.stamp();
    for bytes in [
        b"BT".as_slice(),
        b"EMC",
        b"/Span << /MCID 0 >> BDC EMC",
        b"0 0 m",
        b"/Missing Do",
    ] {
        assert!(d.append_layer_content(0, &a, bytes, &b).is_err());
        assert_eq!(
            d.get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            before
        );
    }
    assert!(!d.is_stamp_current(&stamp));
    assert_eq!(b.live_bytes(), 0);
}
#[test]
fn single_form_call_binding_keeps_other_calls_and_pages_shared() {
    let d = doc();
    d.add_page(1).unwrap();
    let b = pool();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Type".into(),
        PdfObject::PdfName(PdfName::new("XObject".into())),
    )
    .unwrap();
    dict.try_insert(
        "Subtype".into(),
        PdfObject::PdfName(PdfName::new("Form".into())),
    )
    .unwrap();
    let mut parser = pdf::parser::syntax::SyntaxParser::new(
        pdf::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
            b"[0 0 100 100]".to_vec(),
        ))
        .unwrap(),
    );
    dict.try_insert("BBox".into(), parser.get_object().unwrap())
        .unwrap();
    dict.try_insert("Resources".into(), PdfObject::PdfDict(PdfDict::default()))
        .unwrap();
    let id = d
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, b"0 0 10 10 re f".to_vec()).unwrap(),
        ))
        .unwrap();
    let h = d.object_handle(id).unwrap();
    let n = d
        .resources()
        .register_handle(0, pdf::document::resource_manager::ResourceKind::Form, &h)
        .unwrap();
    let n2 = d
        .resources()
        .register_handle(1, pdf::document::resource_manager::ResourceKind::Form, &h)
        .unwrap();
    d.replace_page_contents(0, format!("/{n} Do /{n} Do").into_bytes())
        .unwrap();
    d.replace_page_contents(1, format!("/{n2} Do").into_bytes())
        .unwrap();
    let original = d.resolve_handle(&h).unwrap();
    let p = d.get_page(0).unwrap().content_program().unwrap();
    d.bind_layer_xobject_call(0, &p, 0, &a, &b).unwrap();
    assert_eq!(d.resolve_handle(&h).unwrap(), original);
    assert_eq!(
        d.get_page(1)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        format!("/{n2} Do").as_bytes()
    );
    assert!(d.bind_layer_xobject_call(0, &p, 1, &a, &b).is_err());
    let p = d.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(
        p.operations()
            .iter()
            .filter(|o| o.operator.name() == "Do")
            .count(),
        2
    );
    assert_eq!(
        p.operations()
            .iter()
            .filter(|o| o.operator.name() == "BDC")
            .count(),
        1
    );
    assert_eq!(b.live_bytes(), 0);
}
#[test]
fn incremental_layouts_preserve_original_prefix() {
    let b = pool();
    let base = doc().save_to_bytes().unwrap();
    for layout in [
        IncrementalSaveLayout::Classic,
        IncrementalSaveLayout::XrefStream,
    ] {
        let d = Document::from_bytes_with_mode(base.clone(), ParseMode::Strict).unwrap();
        let a = d.create_layer("append", LayerIntent::View, &b).unwrap();
        d.append_layer_content(0, &a, b"0 0 20 20 re f", &b)
            .unwrap();
        let out = d
            .save_incremental_to_bytes_with_layout(Default::default(), layout)
            .unwrap();
        assert!(out.starts_with(&base));
        let r = Document::from_bytes_with_mode(out, ParseMode::Strict).unwrap();
        assert_eq!(r.optional_content().unwrap().unwrap().groups.len(), 1);
    }
}
#[test]
fn low_versions_and_default_unchanged_are_refused() {
    let d = Document::new_with_version(PdfVersion::V1_4).unwrap();
    assert!(d.create_layer("x", LayerIntent::View, &pool()).is_err());
    let d = doc();
    let b = pool();
    d.create_layer("x", LayerIntent::View, &b).unwrap();
    let t = d.layer_configuration_target(None).unwrap();
    let c = LayerConfiguration {
        name: None,
        base: LayerBaseState::Unchanged,
        on: vec![],
        off: vec![],
    };
    assert!(d.write_layer_configuration(Some(&t), &c, &b).is_err());
}

#[test]
fn r6_identity_permissions_and_append_keep_protection() {
    use pdf::security::{EncryptionOptions, SaveSecurity};
    let d = Document::new_with_version(PdfVersion::V2_0).unwrap();
    d.add_page(0).unwrap();
    let b = pool();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.permissions.modify = false;
    for layout in [
        SaveLayout::Classic,
        SaveLayout::XrefStream,
        SaveLayout::XrefStreamWithObjectStreams,
    ] {
        let source = d
            .save_to_bytes_with_layout_and_security(
                Default::default(),
                layout,
                SaveSecurity::Set(&settings),
            )
            .unwrap();
        assert!(Document::from_bytes_with_password(source.clone(), "wrong").is_err());
        let user = Document::from_bytes_with_password_and_options(
            source.clone(),
            "user",
            pdf::parser::options::ParseOptions {
                mode: ParseMode::Strict,
                ..Default::default()
            },
        )
        .unwrap();
        let stamp = user.stamp();
        assert!(user.create_layer("denied", LayerIntent::View, &b).is_err());
        assert!(user.is_stamp_current(&stamp));
        let owner = Document::from_bytes_with_password_and_options(
            source.clone(),
            "owner",
            pdf::parser::options::ParseOptions {
                mode: ParseMode::Strict,
                ..Default::default()
            },
        )
        .unwrap();
        owner
            .create_layer("allowed", LayerIntent::View, &b)
            .unwrap();
        for append in [
            IncrementalSaveLayout::Classic,
            IncrementalSaveLayout::XrefStream,
        ] {
            let bytes = owner
                .save_incremental_to_bytes_with_layout(Default::default(), append)
                .unwrap();
            assert!(bytes.starts_with(&source));
            let r = Document::from_bytes_with_password(bytes, "user").unwrap();
            assert_eq!(r.optional_content().unwrap().unwrap().groups.len(), 1);
        }
    }
}

#[test]
fn configuration_extensions_and_group_private_keys_are_preserved() {
    use pdf::objects::object_id::ObjectId;
    let d = doc();
    let b = pool();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    let mut group = d.resolve_handle(&a).unwrap().into_dict().unwrap();
    group
        .try_insert("Private".into(), PdfObject::PdfNull)
        .unwrap();
    d.update_handle(&a, PdfObject::PdfDict(group)).unwrap();
    d.rename_layer(&a, "B", &b).unwrap();
    assert!(
        d.resolve_handle(&a)
            .unwrap()
            .as_dict()
            .unwrap()
            .contains_key("Private")
    );
    let root_id = ObjectId::new(2, 0);
    let mut root = d.resolve_object(root_id).unwrap().into_dict().unwrap();
    let mut props = root
        .get("OCProperties")
        .unwrap()
        .clone()
        .into_dict()
        .unwrap();
    let mut default = props.get("D").unwrap().clone().into_dict().unwrap();
    default
        .try_insert("RBGroups".into(), PdfObject::PdfNull)
        .unwrap();
    props
        .try_insert("D".into(), PdfObject::PdfDict(default))
        .unwrap();
    props
        .try_insert("Private".into(), PdfObject::PdfNull)
        .unwrap();
    root.try_insert("OCProperties".into(), PdfObject::PdfDict(props))
        .unwrap();
    d.update_object(root_id, PdfObject::PdfDict(root)).unwrap();
    let t = d.layer_configuration_target(None).unwrap();
    let before = d.save_to_bytes().unwrap();
    assert!(
        d.write_layer_configuration(Some(&t), &config(vec![], vec![]), &b)
            .is_err()
    );
    assert_eq!(d.save_to_bytes().unwrap(), before);
    d.rename_layer(&a, "C", &b).unwrap();
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn expressions_depth_version_and_foreign_targets_refuse() {
    let d = Document::new_with_version(PdfVersion::V1_5).unwrap();
    let b = pool();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    let e = LayerExpression::Not(Box::new(LayerExpression::Group(a.clone())));
    assert!(
        d.create_layer_membership(&[a], LayerPolicy::AnyOn, Some(&e), &b)
            .is_err()
    );
    let d = doc();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    let mut e = LayerExpression::Group(a.clone());
    for _ in 0..65 {
        e = LayerExpression::Not(Box::new(e));
    }
    let stamp = d.stamp();
    assert!(
        d.create_layer_membership(&[a], LayerPolicy::AllOn, Some(&e), &b)
            .is_err()
    );
    assert!(d.is_stamp_current(&stamp));
    let foreign = doc();
    foreign.create_layer("F", LayerIntent::View, &b).unwrap();
    let t = foreign.layer_configuration_target(None).unwrap();
    assert!(
        d.write_layer_configuration(Some(&t), &config(vec![], vec![]), &b)
            .is_err()
    );
}

#[test]
fn segment_state_and_property_injection_refuse_without_partial_commit() {
    let d = doc();
    let b = pool();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    for bytes in [
        b"/Span /P BDC EMC".as_slice(),
        b"/Unknown <<>> BDC EMC",
        b"q",
        b"BT ET EMC",
        b"0 0 20 20 re W",
        b"1 Unknown",
    ] {
        let before = d.save_to_bytes().unwrap();
        assert!(d.append_layer_content(0, &a, bytes, &b).is_err());
        assert_eq!(d.save_to_bytes().unwrap(), before);
    }
    d.replace_page_contents(0, b"q".to_vec()).unwrap();
    let before = d.save_to_bytes().unwrap();
    assert!(
        d.append_layer_content(0, &a, b"0 0 20 20 re f", &b)
            .is_err()
    );
    assert_eq!(d.save_to_bytes().unwrap(), before);
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn image_call_binding_is_local_and_existing_oc_refuses() {
    let d = doc();
    d.add_page(1).unwrap();
    let b = pool();
    let a = d.create_layer("Image", LayerIntent::View, &b).unwrap();
    let n = d
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    d.replace_page_contents(
        0,
        format!("q 20 0 0 20 0 0 cm /{n} Do Q q 20 0 0 20 30 0 cm /{n} Do Q").into_bytes(),
    )
    .unwrap();
    let p = d.get_page(0).unwrap().content_program().unwrap();
    let i = p
        .operations()
        .iter()
        .position(|o| o.operator.name() == "Do")
        .unwrap();
    d.bind_layer_xobject_call(0, &p, i, &a, &b).unwrap();
    let v = d
        .optional_content_visibility(&OptionalContentOptions {
            overrides: vec![(a.clone(), false)],
            ..Default::default()
        })
        .unwrap();
    let list = d
        .get_page(0)
        .unwrap()
        .display_list_with_optional_content(&v)
        .unwrap();
    assert_eq!(
        list.objects()
            .iter()
            .filter(|o| matches!(o, pdf::page::page_object::PageObject::Image(_)))
            .count(),
        1
    );
    let image = d
        .get_page(0)
        .unwrap()
        .get_xobject(&n)
        .unwrap()
        .into_stream()
        .unwrap();
    let mut dictionary = image.dict().clone();
    dictionary
        .try_insert(
            "OC".into(),
            PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(
                a.object_id(),
            )),
        )
        .unwrap();
    let id = d
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dictionary, vec![255, 0, 0]).unwrap(),
        ))
        .unwrap();
    let h = d.object_handle(id).unwrap();
    let n = d
        .resources()
        .register_handle(1, pdf::document::resource_manager::ResourceKind::Image, &h)
        .unwrap();
    d.replace_page_contents(1, format!("/{n} Do").into_bytes())
        .unwrap();
    let p = d.get_page(1).unwrap().content_program().unwrap();
    let before = d.save_to_bytes().unwrap();
    assert!(d.bind_layer_xobject_call(1, &p, 0, &a, &b).is_err());
    assert_eq!(d.save_to_bytes().unwrap(), before);
}

#[test]
fn signed_source_cannot_gain_layer_mutation_api_authority() {
    let d = doc();
    let mut s = PdfDict::default();
    s.try_insert(
        "Type".into(),
        PdfObject::PdfName(PdfName::new("Sig".into())),
    )
    .unwrap();
    d.try_insert_object(PdfObject::PdfDict(s)).unwrap();
    let bytes = d.save_to_bytes().unwrap();
    let d = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    let before = d.stamp();
    let b = pool();
    assert!(d.create_layer("no", LayerIntent::View, &b).is_err());
    assert!(d.is_stamp_current(&before));
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn property_names_are_unique_and_unrelated_entries_survive() {
    use pdf::objects::{object_id::ObjectId, pdf_reference::PdfReference};
    let d = doc();
    let b = pool();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    let mut props = PdfDict::default();
    props
        .try_insert(
            "OCrz0".into(),
            PdfObject::PdfReference(PdfReference::from_id(a.object_id())),
        )
        .unwrap();
    props
        .try_insert("Private".into(), PdfObject::PdfNull)
        .unwrap();
    let mut resources = PdfDict::default();
    resources
        .try_insert("Properties".into(), PdfObject::PdfDict(props))
        .unwrap();
    let resource = d.try_insert_object(PdfObject::PdfDict(resources)).unwrap();
    let page = d
        .destination(0, pdf::document::destination::DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    let mut p = d.resolve_object(page).unwrap().into_dict().unwrap();
    p.remove("Resources");
    d.update_object(page, PdfObject::PdfDict(p)).unwrap();
    let mut pages = d
        .resolve_object(ObjectId::new(1, 0))
        .unwrap()
        .into_dict()
        .unwrap();
    pages
        .try_insert(
            "Resources".into(),
            PdfObject::PdfReference(PdfReference::from_id(resource)),
        )
        .unwrap();
    d.update_object(ObjectId::new(1, 0), PdfObject::PdfDict(pages))
        .unwrap();
    let original = d.resolve_object(resource).unwrap();
    d.append_layer_content(0, &a, b"0 0 10 10 re f", &b)
        .unwrap();
    assert_eq!(d.resolve_object(resource).unwrap(), original);
    let p = d.get_page(0).unwrap().resource().unwrap();
    let props = p.get("Properties").unwrap().as_dict().unwrap();
    assert!(props.contains_key("Private"));
    assert!(props.contains_key("OCrz0"));
    assert!(props.contains_key("OCrz1"));
}

#[test]
fn new_actualtext_and_artifact_nesting_obeys_selection() {
    let d = doc();
    let b = pool();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    let f = d
        .resources()
        .register_standard_font(0, pdf::document::resource_manager::StandardFont::Courier)
        .unwrap();
    d.append_layer_content(0,&a,format!("/Span << /ActualText (REPLACED) >> BDC BT /{f} 10 Tf (X) Tj ET EMC /Artifact BMC 0 0 10 10 re f EMC").as_bytes(),&b).unwrap();
    assert_eq!(text(&d, Default::default()), "REPLACED");
    assert_eq!(
        text(
            &d,
            OptionalContentOptions {
                overrides: vec![(a, false)],
                ..Default::default()
            }
        ),
        ""
    );
}

#[test]
fn exact_operation_budget_threshold_and_rollback_target_invalidation() {
    let d = doc();
    let b = pool();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    let t = d.layer_configuration_target(None).unwrap();
    let probe = pool();
    d.rename_layer(&a, "A", &probe).unwrap();
    let peak = probe.peak_bytes();
    let short = OperationBudget::new(peak - 1);
    let before = d.save_to_bytes().unwrap();
    assert!(d.rename_layer(&a, "A", &short).is_err());
    assert_eq!(short.live_bytes(), 0);
    assert_eq!(d.save_to_bytes().unwrap(), before);
    let exact = OperationBudget::new(peak);
    d.rename_layer(&a, "A", &exact).unwrap();
    assert_eq!(exact.live_bytes(), 0);
    assert!(
        d.write_layer_configuration(Some(&t), &config(vec![], vec![]), &b)
            .is_err()
    );
}

#[test]
fn fixed_group_configuration_and_expression_caps_have_exact_boundaries() {
    let d = doc();
    let b = pool();
    d.create_layer("0", LayerIntent::View, &b).unwrap();
    for i in 1..1024 {
        d.create_layer(&i.to_string(), LayerIntent::View, &b)
            .unwrap();
    }
    let before = d.stamp();
    assert!(d.create_layer("overflow", LayerIntent::View, &b).is_err());
    assert!(d.is_stamp_current(&before));
    assert_eq!(d.optional_content().unwrap().unwrap().groups.len(), 1024);
    for _ in 0..256 {
        d.write_layer_configuration(None, &config(vec![], vec![]), &b)
            .unwrap();
    }
    assert!(
        d.write_layer_configuration(None, &config(vec![], vec![]), &b)
            .is_err()
    );
    assert_eq!(
        d.optional_content().unwrap().unwrap().configurations.len(),
        256
    );
    // Avoid multiplying the large registration model for this expression check.
    let d = doc();
    let first = d.create_layer("one", LayerIntent::View, &b).unwrap();
    let good = LayerExpression::Or(vec![LayerExpression::Group(first.clone()); 4095]);
    d.create_layer_membership(
        std::slice::from_ref(&first),
        LayerPolicy::AllOff,
        Some(&good),
        &b,
    )
    .unwrap();
    let bad = LayerExpression::Or(vec![LayerExpression::Group(first.clone()); 4096]);
    assert!(
        d.create_layer_membership(&[first], LayerPolicy::AllOff, Some(&bad), &b)
            .is_err()
    );
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn absent_registry_never_authorizes_raw_ocg_or_ocmd_binding() {
    let d = doc();
    let b = pool();
    for kind in ["OCG", "OCMD"] {
        let mut dict = PdfDict::default();
        dict.try_insert("Type".into(), PdfObject::PdfName(PdfName::new(kind.into())))
            .unwrap();
        let id = d.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
        let h = d.object_handle(id).unwrap();
        let stamp = d.stamp();
        let before = d.save_to_bytes().unwrap();
        assert!(
            d.append_layer_content(0, &h, b"0 0 10 10 re f", &b)
                .is_err()
        );
        assert!(d.is_stamp_current(&stamp));
        assert_eq!(d.save_to_bytes().unwrap(), before);
    }
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn default_generation_enforces_on_and_view_without_restricting_alternatives() {
    use pdf::objects::object_id::ObjectId;
    let d = doc();
    let b = pool();
    let a = d.create_layer("A", LayerIntent::View, &b).unwrap();
    for base in [LayerBaseState::Off, LayerBaseState::Unchanged] {
        let t = d.layer_configuration_target(None).unwrap();
        let c = LayerConfiguration {
            name: None,
            base,
            on: vec![],
            off: vec![],
        };
        let stamp = d.stamp();
        assert!(d.write_layer_configuration(Some(&t), &c, &b).is_err());
        assert!(d.is_stamp_current(&stamp));
        d.write_layer_configuration(None, &c, &b).unwrap();
    }
    let root_id = ObjectId::new(2, 0);
    let mut root = d.resolve_object(root_id).unwrap().into_dict().unwrap();
    let mut props = root
        .get("OCProperties")
        .unwrap()
        .clone()
        .into_dict()
        .unwrap();
    let mut default = props.get("D").unwrap().clone().into_dict().unwrap();
    default
        .try_insert(
            "Intent".into(),
            PdfObject::PdfName(PdfName::new("Design".into())),
        )
        .unwrap();
    props
        .try_insert("D".into(), PdfObject::PdfDict(default))
        .unwrap();
    root.try_insert("OCProperties".into(), PdfObject::PdfDict(props))
        .unwrap();
    d.update_object(root_id, PdfObject::PdfDict(root)).unwrap();
    let t = d.layer_configuration_target(None).unwrap();
    let before = d.save_to_bytes().unwrap();
    assert!(
        d.write_layer_configuration(Some(&t), &config(vec![a], vec![]), &b)
            .is_err()
    );
    assert_eq!(d.save_to_bytes().unwrap(), before);
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn layer_append_separates_a_complete_final_operator_or_unterminated_comment() {
    use pdf::document::resource_manager::StandardFont;
    for suffix in ["ET", "ET% final comment"] {
        let d = doc();
        let b = pool();
        let font = d
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        d.replace_page_contents(
            0,
            format!("BT /{font} 12 Tf 20 100 Td (A) Tj {suffix}").into_bytes(),
        )
        .unwrap();
        let layer = d.create_layer("Append", LayerIntent::View, &b).unwrap();
        d.append_layer_content(
            0,
            &layer,
            format!("BT /{font} 12 Tf 20 80 Td (B) Tj ET").as_bytes(),
            &b,
        )
        .unwrap();
        assert_eq!(text(&d, Default::default()), "AB");
        let saved = d.save_to_bytes().unwrap();
        let reopened = Document::from_bytes_with_mode(saved, ParseMode::Strict).unwrap();
        assert_eq!(text(&reopened, Default::default()), "AB");
    }
}
