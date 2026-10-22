use pdf::{
    document::{
        PdfDocument,
        destination::DestinationView,
        optional_content::{OptionalContentIntent, OptionalContentOptions},
        resource_manager::StandardFont,
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_reference::PdfReference, pdf_stream::PdfStream},
    page::{
        annotations::AnnotationRenderOptions, marked_text::MarkedTextOptions,
        page_object::PageObject,
    },
    parser::{
        options::ParseOptions,
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
fn set(doc: &Doc, id: ObjectId, key: &str, value: PdfObject) {
    let mut d = doc.resolve_object(id).unwrap().into_dict().unwrap();
    d.try_insert(key.into(), value).unwrap();
    doc.update_object(id, PdfObject::PdfDict(d)).unwrap();
}
// New-document constructor allocates Pages=1, Catalog=2; not used on imported PDFs.
fn root(_doc: &Doc) -> ObjectId {
    ObjectId::new(2, 0)
}
fn page(doc: &Doc) -> ObjectId {
    doc.destination(0, DestinationView::Fit)
        .unwrap()
        .page
        .object_id()
}
fn fixture() -> (Doc, ObjectId, ObjectId, String) {
    fixture_with_options(ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    })
}
fn fixture_with_options(options: ParseOptions) -> (Doc, ObjectId, ObjectId, String) {
    let doc = Doc::new_with_options(options).unwrap();
    doc.add_page(0).unwrap();
    let a = doc
        .try_insert_object(parse("<< /Type /OCG /Name (Visible) >>"))
        .unwrap();
    let b = doc
        .try_insert_object(parse("<< /Type /OCG /Name (Hidden) >>"))
        .unwrap();
    let props = format!(
        "<< /OCGs [{} 0 R {} 0 R] /D << /OFF [{} 0 R] >> /Configs [<< /Name (Swap) /BaseState /OFF /ON [{} 0 R] >> << /Name (Keep) /BaseState /Unchanged >>] >>",
        a.obj_num(),
        b.obj_num(),
        b.obj_num(),
        b.obj_num()
    );
    set(&doc, root(&doc), "OCProperties", parse(&props));
    let f = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    let mut res = doc.get_page(0).unwrap().resource().unwrap();
    res.try_insert(
        "Properties".into(),
        parse(&format!(
            "<< /A {} 0 R /B {} 0 R >>",
            a.obj_num(),
            b.obj_num()
        )),
    )
    .unwrap();
    set(&doc, page(&doc), "Resources", PdfObject::PdfDict(res));
    (doc, a, b, f)
}
fn content(doc: &Doc, s: &str) {
    doc.replace_page_contents(0, s.as_bytes().to_vec()).unwrap();
}
fn extracted(doc: &Doc, options: &OptionalContentOptions) -> String {
    let v = doc.optional_content_visibility(options).unwrap();
    doc.get_page(0)
        .unwrap()
        .extract_marked_text_with_optional_content(MarkedTextOptions::default(), &v)
        .unwrap()
        .text(&Default::default())
        .unwrap()
}
#[test]
fn defaults_alternates_and_unchanged_are_explicit_read_only() {
    let (doc, a, b, _) = fixture();
    let before = doc.stamp();
    let model = doc.optional_content().unwrap().unwrap();
    assert_eq!(model.groups[0].name, "Visible");
    assert_eq!(model.configurations[0].name.as_deref(), Some("Swap"));
    let state = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(state.group_state(&doc.object_handle(a).unwrap()).unwrap());
    assert!(!state.group_state(&doc.object_handle(b).unwrap()).unwrap());
    let alt = doc
        .optional_content_visibility(&OptionalContentOptions {
            configuration: Some(0),
            ..Default::default()
        })
        .unwrap();
    assert!(!alt.group_state(&doc.object_handle(a).unwrap()).unwrap());
    assert!(alt.group_state(&doc.object_handle(b).unwrap()).unwrap());
    let keep = doc
        .optional_content_visibility(&OptionalContentOptions {
            configuration: Some(1),
            ..Default::default()
        })
        .unwrap();
    assert!(keep.group_state(&doc.object_handle(a).unwrap()).unwrap());
    assert!(!keep.group_state(&doc.object_handle(b).unwrap()).unwrap());
    assert_eq!(before, doc.stamp());
}
#[test]
fn four_policies_empty_null_and_single_group() {
    let (doc, a, b, _) = fixture();
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    for (p, expected) in [
        ("AnyOn", true),
        ("AllOn", false),
        ("AnyOff", true),
        ("AllOff", false),
    ] {
        let member = parse(&format!(
            "<< /Type /OCMD /P /{p} /OCGs [{} 0 R {} 0 R null] >>",
            a.obj_num(),
            b.obj_num()
        ));
        assert_eq!(v.is_visible(&doc, &member).unwrap(), expected);
        assert!(
            v.is_visible(
                &doc,
                &parse(&format!("<< /Type /OCMD /P /{p} /OCGs [null] >>"))
            )
            .unwrap()
        );
    }
    assert!(
        !v.is_visible(
            &doc,
            &parse(&format!("<< /Type /OCMD /OCGs {} 0 R >>", b.obj_num()))
        )
        .unwrap()
    );
    assert!(v.is_visible(&doc, &parse("<< /Type /OCMD >>")).unwrap());
}
#[test]
fn expressions_override_policy_and_do_not_short_circuit_validation() {
    let (doc, a, b, _) = fixture();
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    let member = parse(&format!(
        "<< /Type /OCMD /P /AllOff /VE [/And {} 0 R [/Not {} 0 R]] >>",
        a.obj_num(),
        b.obj_num()
    ));
    assert!(v.is_visible(&doc, &member).unwrap());
    for expr in [
        format!("[/Or {} 0 R [/Unknown {} 0 R]]", a.obj_num(), b.obj_num()),
        format!("[/And {} 0 R [/Not]]", b.obj_num()),
        format!("[/Not {} 0 R {} 0 R]", a.obj_num(), b.obj_num()),
    ] {
        assert!(
            v.is_visible(&doc, &parse(&format!("<< /Type /OCMD /VE {expr} >>")))
                .is_err()
        );
    }
}
#[test]
fn cyclic_expression_and_ocmd_leaf_are_rejected() {
    let (doc, a, _, _) = fixture();
    let expr = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    doc.update_object(
        expr,
        parse(&format!("[/Or {} 0 R {} 0 R]", a.obj_num(), expr.obj_num())),
    )
    .unwrap();
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(
        v.is_visible(
            &doc,
            &parse(&format!("<< /Type /OCMD /VE {} 0 R >>", expr.obj_num()))
        )
        .is_err()
    );
    assert!(
        v.is_visible(
            &doc,
            &parse("<< /Type /OCMD /VE [/Not << /Type /OCMD >>] >>")
        )
        .is_err()
    );
}
#[test]
fn conflict_unknown_policy_and_unregistered_group_refuse() {
    let (doc, a, b, _) = fixture();
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(
        v.is_visible(&doc, &parse("<< /Type /OCMD /P /Future >>"))
            .is_err()
    );
    assert!(
        v.is_visible(&doc, &parse("<< /Type /OCG /Name (Direct) >>"))
            .is_err()
    );
    set(
        &doc,
        root(&doc),
        "OCProperties",
        parse(&format!(
            "<< /OCGs [{} 0 R {} 0 R] /D << /ON [{} 0 R] /OFF [{} 0 R] >> >>",
            a.obj_num(),
            b.obj_num(),
            a.obj_num(),
            a.obj_num()
        )),
    );
    assert!(
        doc.optional_content_visibility(&Default::default())
            .is_err()
    );
}
#[test]
fn usage_requires_as_and_distinguishes_view_print_and_static_output() {
    let (doc, a, b, f) = fixture();
    set(
        &doc,
        a,
        "Usage",
        parse("<< /View << /ViewState /OFF >> /Print << /PrintState /ON >> >>"),
    );
    content(&doc, &format!("/OC /A BDC BT /{f} 10 Tf (A) Tj ET EMC"));
    assert_eq!(extracted(&doc, &Default::default()), "A"); // no AS, do not guess from Usage
    set(
        &doc,
        root(&doc),
        "OCProperties",
        parse(&format!(
            "<< /OCGs [{} 0 R {} 0 R] /D << /AS [<< /Event /View /Category [/View] /OCGs [{} 0 R] >> << /Event /Print /Category [/Print] /OCGs [{} 0 R] >>] >> >>",
            a.obj_num(),
            b.obj_num(),
            a.obj_num(),
            a.obj_num()
        )),
    );
    assert_eq!(extracted(&doc, &Default::default()), "");
    assert_eq!(
        extracted(
            &doc,
            &OptionalContentOptions {
                intent: OptionalContentIntent::Print,
                ..Default::default()
            }
        ),
        "A"
    );
    assert_eq!(
        extracted(
            &doc,
            &OptionalContentOptions {
                apply_usage: false,
                ..Default::default()
            }
        ),
        "A"
    );
}
#[test]
fn usage_categories_combine_across_applications_and_unknown_refuses() {
    let (doc, a, b, _) = fixture();
    set(
        &doc,
        a,
        "Usage",
        parse("<< /View << /ViewState /ON >> /Print << /PrintState /OFF >> >>"),
    );
    set(
        &doc,
        root(&doc),
        "OCProperties",
        parse(&format!(
            "<< /OCGs [{} 0 R {} 0 R] /D << /AS [<< /Event /View /Category [/View] /OCGs [{} 0 R] >> << /Event /View /Category [/Print] /OCGs [{} 0 R] >>] >> >>",
            a.obj_num(),
            b.obj_num(),
            a.obj_num(),
            a.obj_num()
        )),
    );
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(!v.group_state(&doc.object_handle(a).unwrap()).unwrap());
    set(
        &doc,
        root(&doc),
        "OCProperties",
        parse(&format!(
            "<< /OCGs [{} 0 R] /D << /AS [<< /Event /View /Category [/Zoom] /OCGs [{} 0 R] >>] >> >>",
            a.obj_num(),
            a.obj_num()
        )),
    );
    assert!(matches!(
        doc.optional_content_visibility(&Default::default()),
        Err(PdfError::UnsupportedFeature(_))
    ));
}
#[test]
fn overrides_stale_foreign_and_rollback_snapshots() {
    let (doc, _, b, _) = fixture();
    let bh = doc.object_handle(b).unwrap();
    let v = doc
        .optional_content_visibility(&OptionalContentOptions {
            overrides: vec![(bh.clone(), true)],
            ..Default::default()
        })
        .unwrap();
    assert!(v.group_state(&bh).unwrap());
    let (other, a, _, _) = fixture();
    assert!(v.is_visible(&other, &reference(a)).is_err());
    assert!(
        doc.optional_content_visibility(&OptionalContentOptions {
            overrides: vec![(other.object_handle(a).unwrap(), true)],
            ..Default::default()
        })
        .is_err()
    );
    let result: Result<(), PdfError> = doc.edit_transaction(|doc| {
        content(doc, "0 g");
        Err(PdfError::DocumentError("rollback".into()))
    });
    assert!(result.is_err());
    assert!(v.is_visible(&doc, &reference(b)).is_err());
}
#[test]
fn hidden_text_advances_and_sources_keep_original_operator_positions() {
    let (doc, _, _, f) = fixture();
    content(
        &doc,
        &format!("BT /{f} 10 Tf 10 10 Td /OC /B BDC (AAA) Tj EMC (Z) Tj ET"),
    );
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    let list = doc
        .get_page(0)
        .unwrap()
        .display_list_with_optional_content(&v)
        .unwrap();
    let text = list
        .objects()
        .iter()
        .find_map(|o| {
            if let PageObject::Text(t) = o {
                Some(t)
            } else {
                None
            }
        })
        .unwrap();
    assert!((text.items[0].pos().x() - 28.).abs() < 0.01);
    assert_eq!(text.source().unwrap().operation_index, 6); // BT,Tf,Td,BDC,Tj,EMC,Tj
    assert_eq!(extracted(&doc, &Default::default()), "Z");
    assert_eq!(
        extracted(
            &doc,
            &OptionalContentOptions {
                configuration: Some(0),
                ..Default::default()
            }
        ),
        "AAAZ"
    );
}
#[test]
fn hidden_clip_color_and_q_restore_are_executed() {
    let (doc, _, _, _) = fixture();
    content(
        &doc,
        "q /OC /B BDC 0 0 10 10 re W n 1 0 0 rg 0 0 1 1 re f EMC 0 0 20 20 re f Q 0 0 30 30 re f",
    );
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    let list = doc
        .get_page(0)
        .unwrap()
        .display_list_with_optional_content(&v)
        .unwrap();
    let paths: Vec<_> = list
        .objects()
        .iter()
        .filter_map(|o| {
            if let PageObject::Path(p) = o {
                Some(p)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(paths.len(), 2);
    assert!(!paths[0].graphic_state().clip_path.elements.is_empty());
    assert!(paths[1].graphic_state().clip_path.elements.is_empty());
}
#[test]
fn nested_optional_content_conjoins_and_artifact_is_independent() {
    let (doc, _, _, f) = fixture();
    content(
        &doc,
        &format!(
            "BT /{f} 10 Tf /OC /B BDC /OC /A BDC (X) Tj EMC EMC /Artifact BMC (Q) Tj EMC (Z) Tj ET"
        ),
    );
    assert_eq!(extracted(&doc, &Default::default()), "Z");
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_marked_text_with_optional_content(
            MarkedTextOptions {
                include_artifacts: true,
                ..Default::default()
            },
            &v,
        )
        .unwrap();
    assert_eq!(report.text(&Default::default()).unwrap(), "QZ");
}
#[test]
fn hidden_replacement_does_not_leak_and_cross_layer_atomic_replacement_refuses() {
    let (doc, _, _, f) = fixture();
    content(
        &doc,
        &format!("/OC /B BDC /Span << /ActualText (SECRET) >> BDC BT /{f} 10 Tf (X) Tj ET EMC EMC"),
    );
    assert_eq!(extracted(&doc, &Default::default()), "");
    content(
        &doc,
        &format!("/Span << /ActualText (SECRET) >> BDC /OC /B BDC BT /{f} 10 Tf (X) Tj ET EMC EMC"),
    );
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text_with_optional_content(Default::default(), &v)
            .is_err()
    );
}
fn form(doc: &Doc, oc: ObjectId, f: &str) -> ObjectId {
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let mut d=parse(&format!("<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] /OC {} 0 R /Group << /S /Transparency /I true >> >>",oc.obj_num())).into_dict().unwrap();
    d.try_insert("Resources".into(), PdfObject::PdfDict(resources))
        .unwrap();
    doc.try_insert_object(PdfObject::PdfStream(
        PdfStream::from_decoded_data(d, format!("BT /{f} 10 Tf (F) Tj ET").into_bytes()).unwrap(),
    ))
    .unwrap()
}
#[test]
fn xobject_oc_transparency_group_and_local_resources_propagate_selection() {
    let (doc, _, b, f) = fixture();
    let id = form(&doc, b, &f);
    let mut res = doc.get_page(0).unwrap().resource().unwrap();
    res.try_insert(
        "XObject".into(),
        parse(&format!("<< /Fm {} 0 R >>", id.obj_num())),
    )
    .unwrap();
    set(&doc, page(&doc), "Resources", PdfObject::PdfDict(res));
    content(&doc, "/Fm Do");
    assert_eq!(extracted(&doc, &Default::default()), "");
    assert_eq!(
        extracted(
            &doc,
            &OptionalContentOptions {
                configuration: Some(0),
                ..Default::default()
            }
        ),
        "F"
    );
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_optional_content(&v)
            .unwrap()
            .objects()
            .is_empty()
    );
    assert!(
        !doc.get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .objects()
            .is_empty()
    ); // legacy unfiltered collection
}
#[test]
fn annotation_oc_combines_flags_and_appearance_selection() {
    let (doc, _, b, f) = fixture();
    let ap = form(&doc, b, &f);
    let annot=doc.try_insert_object(parse(&format!("<< /Type /Annot /Subtype /Square /Rect [0 0 100 100] /F 4 /OC {} 0 R /AP << /N {} 0 R >> >>",b.obj_num(),ap.obj_num()))).unwrap();
    set(
        &doc,
        page(&doc),
        "Annots",
        parse(&format!("[{} 0 R]", annot.obj_num())),
    );
    content(&doc, "");
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_annotations_and_optional_content(
                AnnotationRenderOptions::default(),
                &v
            )
            .unwrap()
            .appearance_count(),
        0
    );
    let v = doc
        .optional_content_visibility(&OptionalContentOptions {
            configuration: Some(0),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_annotations_and_optional_content(
                AnnotationRenderOptions::default(),
                &v
            )
            .unwrap()
            .appearance_count(),
        1
    );
    assert!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_annotations(Default::default())
            .is_err()
    );
}
#[test]
fn missing_ocproperties_ignores_membership_without_hiding_content() {
    let doc = Doc::new().unwrap();
    doc.add_page(0).unwrap();
    assert!(doc.optional_content().unwrap().is_none());
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(
        v.is_visible(&doc, &parse("<< /Type /OCG /Name (Unregistered) >>"))
            .unwrap()
    );
}
#[test]
fn cumulative_expression_budget_is_shared_by_clones_and_depth_is_bounded() {
    let mut options = ParseOptions::default();
    options.limits.max_content_operations = 64;
    let (doc, a, _, _) = fixture_with_options(options);
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    let v2 = v.clone();
    let leaf = reference(a);
    for _ in 0..30 {
        v.is_visible(&doc, &leaf).unwrap();
    }
    let mut exhausted = false;
    for _ in 0..64 {
        if v2.is_visible(&doc, &leaf).is_err() {
            exhausted = true;
            break;
        }
    }
    assert!(exhausted);
    let mut options = ParseOptions::default();
    options.limits.max_depth = 4;
    let (shallow, a, _, _) = fixture_with_options(options);
    let expression = format!("{}{} 0 R{}", "[/Not ".repeat(6), a.obj_num(), "]".repeat(6));
    let v = shallow
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(matches!(
        v.is_visible(
            &shallow,
            &parse(&format!("<< /Type /OCMD /VE {expression} >>"))
        ),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn strict_roundtrip_preserves_raw_configs_memberships_and_hidden_bytes() {
    let (doc, a, b, f) = fixture();
    let member = doc
        .try_insert_object(parse(&format!(
            "<< /Type /OCMD /VE [/Or {} 0 R {} 0 R] >>",
            a.obj_num(),
            b.obj_num()
        )))
        .unwrap();
    set(
        &doc,
        a,
        "PrivateMetadata",
        parse("<< /Uninterpreted (kept) >>"),
    );
    content(
        &doc,
        &format!("BT /{f} 10 Tf /OC /B BDC (SECRET) Tj EMC (Z) Tj ET"),
    );
    let before = doc.stamp();
    let bytes = doc.save_to_bytes().unwrap();
    let reopened = Doc::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(extracted(&reopened, &Default::default()), "Z");
    assert_eq!(
        extracted(
            &reopened,
            &OptionalContentOptions {
                configuration: Some(0),
                ..Default::default()
            }
        ),
        "SECRETZ"
    );
    assert_eq!(
        doc.resolve_object(member).unwrap(),
        reopened.resolve_object(member).unwrap()
    );
    assert_eq!(
        doc.resolve_object(a).unwrap(),
        reopened.resolve_object(a).unwrap()
    );
    assert_eq!(before, doc.stamp());
}

#[test]
fn lower_versions_require_explicit_upgrade_for_groups_and_ve() {
    use pdf::document::save_options::{PdfVersion, SaveOptions};
    let doc = Doc::new_with_version(PdfVersion::V1_4).unwrap();
    doc.add_page(0).unwrap();
    let group = doc
        .try_insert_object(parse("<< /Type /OCG /Name (Layer) >>"))
        .unwrap();
    set(
        &doc,
        root(&doc),
        "OCProperties",
        parse(&format!("<< /OCGs [{} 0 R] /D << >> >>", group.obj_num())),
    );
    assert!(
        doc.optional_content_visibility(&Default::default())
            .is_err()
    );
    assert!(doc.save_to_bytes().is_err());
    let bytes = doc
        .save_to_bytes_with_options(SaveOptions {
            version: Some(PdfVersion::V1_5),
            ..Default::default()
        })
        .unwrap();
    let reopened = Doc::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert!(
        reopened
            .optional_content_visibility(&Default::default())
            .is_ok()
    );
    let md = doc
        .try_insert_object(parse(&format!(
            "<< /Type /OCMD /VE [/Not {} 0 R] >>",
            group.obj_num()
        )))
        .unwrap();
    set(&doc, root(&doc), "PrivateMembership", reference(md));
    assert!(
        doc.save_to_bytes_with_options(SaveOptions {
            version: Some(PdfVersion::V1_5),
            ..Default::default()
        })
        .is_err()
    );
    assert!(
        doc.save_to_bytes_with_options(SaveOptions {
            version: Some(PdfVersion::V1_6),
            ..Default::default()
        })
        .is_ok()
    );
}

#[test]
fn protected_and_signed_reads_remain_read_only_and_transfers_refuse() {
    let (doc, _, _, f) = fixture();
    content(&doc, &format!("/OC /A BDC BT /{f} 10 Tf (A) Tj ET EMC"));
    let mut protection = pdf::security::EncryptionOptions::new("user", "owner").unwrap();
    protection.permissions.modify = false;
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
    assert_eq!(extracted(&user, &Default::default()), "A");
    assert_eq!(stamp, user.stamp());
    assert!(user.copy_page(0, 1).is_err());
    doc.try_insert_object(parse("<< /Type /Sig >>")).unwrap();
    let signed =
        Doc::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let stamp = signed.stamp();
    assert_eq!(extracted(&signed, &Default::default()), "A");
    assert!(signed.copy_page(0, 1).is_err());
    assert_eq!(stamp, signed.stamp());
}

#[test]
fn hidden_content_still_charges_execution_and_unicode_work() {
    let mut options = ParseOptions::default();
    options.limits.max_text_chars = 32;
    let (doc, _, _, f) = fixture_with_options(options);
    content(
        &doc,
        &format!("/OC /B BDC BT /{f} 10 Tf ({}) Tj ET EMC", "A".repeat(33)),
    );
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(matches!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text_with_optional_content(Default::default(), &v),
        Err(PdfError::ResourceLimit(_))
    ));
    let mut options = ParseOptions::default();
    options.limits.max_content_operations = 64;
    let (doc, _, _, _) = fixture_with_options(options);
    content(&doc, &format!("/OC /B BDC {} EMC", "0 g ".repeat(65)));
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(matches!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_optional_content(&v),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn structure_order_filters_layers_without_changing_parenttree_ownership() {
    use pdf::document::structure::StructureElementDefinition;
    let (doc, _, _, f) = fixture();
    let leaf = doc
        .add_structure_element(None, 0, &StructureElementDefinition::new("P"))
        .unwrap();
    content(
        &doc,
        &format!("/Span << /MCID 0 >> BDC BT /{f} 10 Tf /OC /B BDC (X) Tj EMC (Z) Tj ET EMC"),
    );
    set(&doc, leaf.object_id(), "Pg", reference(page(&doc)));
    set(&doc, leaf.object_id(), "K", parse("0"));
    set(&doc, page(&doc), "StructParents", parse("0"));
    let structure = doc.structure_tree().unwrap().unwrap().root.object_id();
    set(
        &doc,
        structure,
        "ParentTree",
        parse(&format!(
            "<< /Nums [0 [{} 0 R]] >>",
            leaf.object_id().obj_num()
        )),
    );
    set(&doc, structure, "ParentTreeNextKey", parse("1"));
    let before = doc.structure_associations().unwrap().unwrap().items;
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    let text = doc
        .extract_text_structure_order_with_optional_content(Default::default(), &v)
        .unwrap()
        .unwrap()
        .text(&Default::default())
        .unwrap();
    assert_eq!(text, "Z");
    assert_eq!(before, doc.structure_associations().unwrap().unwrap().items);
}

#[test]
fn unsupported_config_and_membership_extensions_preserve_source_on_refusal() {
    let (doc, a, _, _) = fixture();
    set(
        &doc,
        root(&doc),
        "OCProperties",
        parse(&format!(
            "<< /OCGs [{} 0 R] /D << /FutureRule true >> >>",
            a.obj_num()
        )),
    );
    let before = doc.save_to_bytes().unwrap();
    assert!(
        doc.optional_content_visibility(&Default::default())
            .is_err()
    );
    assert!(
        doc.optional_content()
            .unwrap()
            .unwrap()
            .default_configuration
            .dictionary
            .get("FutureRule")
            .is_some()
    );
    assert_eq!(before, doc.save_to_bytes().unwrap());
}

#[test]
fn selections_discriminate_caches_without_changing_edit_provenance() {
    let (doc, _, _, f) = fixture();
    content(
        &doc,
        &format!("BT /{f} 10 Tf /OC /B BDC (X) Tj EMC (Z) Tj ET"),
    );
    let first = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    let second = doc
        .optional_content_visibility(&OptionalContentOptions {
            configuration: Some(0),
            ..Default::default()
        })
        .unwrap();
    assert_eq!(first.selection(), first.clone().selection());
    assert_ne!(first.selection(), second.selection());
    let a = doc
        .get_page(0)
        .unwrap()
        .display_list_with_optional_content(&first)
        .unwrap();
    let b = doc
        .get_page(0)
        .unwrap()
        .display_list_with_optional_content(&second)
        .unwrap();
    assert_eq!(a.source_stamp(), b.source_stamp());
    assert_ne!(
        a.optional_content_selection(),
        b.optional_content_selection()
    );
    assert!(a.matches_program(&doc.get_page(0).unwrap().content_program().unwrap()));
}

#[test]
fn ignored_intents_and_empty_config_intent_do_not_hide_ocg_content() {
    let (doc, a, b, _) = fixture();
    set(&doc, b, "Intent", parse("/Design"));
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(!v.group_state(&doc.object_handle(b).unwrap()).unwrap());
    assert!(v.is_visible(&doc, &reference(b)).unwrap());
    assert!(matches!(
        v.is_visible(
            &doc,
            &parse(&format!(
                "<< /Type /OCMD /P /AnyOff /OCGs [{} 0 R] >>",
                b.obj_num()
            ))
        ),
        Err(PdfError::UnsupportedFeature(_))
    ));
    set(
        &doc,
        root(&doc),
        "OCProperties",
        parse(&format!(
            "<< /OCGs [{} 0 R {} 0 R] /D << /OFF [{} 0 R {} 0 R] /Intent [] >> >>",
            a.obj_num(),
            b.obj_num(),
            a.obj_num(),
            b.obj_num()
        )),
    );
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(v.is_visible(&doc, &reference(a)).unwrap());
    assert!(v.is_visible(&doc, &reference(b)).unwrap());
}

#[test]
fn image_xobject_oc_suppresses_image_but_not_following_state() {
    let (doc, _, b, _) = fixture();
    let image=PdfObject::PdfStream(PdfStream::from_decoded_data(parse(&format!("<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 /OC {} 0 R >>",b.obj_num())).into_dict().unwrap(),vec![255,0,0]).unwrap());
    let id = doc.try_insert_object(image).unwrap();
    let mut res = doc.get_page(0).unwrap().resource().unwrap();
    res.try_insert(
        "XObject".into(),
        parse(&format!("<< /Im {} 0 R >>", id.obj_num())),
    )
    .unwrap();
    set(&doc, page(&doc), "Resources", PdfObject::PdfDict(res));
    content(&doc, "q 10 0 0 10 0 0 cm /Im Do Q 0 0 20 20 re f");
    for (config, count) in [(None, 0), (Some(0), 1)] {
        let v = doc
            .optional_content_visibility(&OptionalContentOptions {
                configuration: config,
                ..Default::default()
            })
            .unwrap();
        let list = doc
            .get_page(0)
            .unwrap()
            .display_list_with_optional_content(&v)
            .unwrap();
        assert_eq!(
            list.objects()
                .iter()
                .filter(|o| matches!(o, PageObject::Image(_)))
                .count(),
            count
        );
        assert_eq!(
            list.objects()
                .iter()
                .filter(|o| matches!(o, PageObject::Path(_)))
                .count(),
            1
        );
    }
}

#[test]
fn form_properties_resolve_locally_and_empty_scope_never_falls_back() {
    let (doc, _, b, f) = fixture();
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let mut d = parse("<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>")
        .into_dict()
        .unwrap();
    d.try_insert("Resources".into(), PdfObject::PdfDict(resources))
        .unwrap();
    let body = format!("/OC /B BDC BT /{f} 10 Tf (X) Tj ET EMC").into_bytes();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(d.clone(), body.clone()).unwrap(),
        ))
        .unwrap();
    let mut res = doc.get_page(0).unwrap().resource().unwrap();
    res.try_insert(
        "XObject".into(),
        parse(&format!("<< /Fm {} 0 R >>", id.obj_num())),
    )
    .unwrap();
    set(&doc, page(&doc), "Resources", PdfObject::PdfDict(res));
    content(&doc, "/Fm Do");
    assert_eq!(extracted(&doc, &Default::default()), "");
    let v = doc
        .optional_content_visibility(&OptionalContentOptions {
            overrides: vec![(doc.object_handle(b).unwrap(), true)],
            ..Default::default()
        })
        .unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_marked_text_with_optional_content(Default::default(), &v)
            .unwrap()
            .text(&Default::default())
            .unwrap(),
        "X"
    );
    d.try_insert("Resources".into(), parse("<< >>")).unwrap();
    doc.update_object(
        id,
        PdfObject::PdfStream(PdfStream::from_decoded_data(d, body).unwrap()),
    )
    .unwrap();
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_optional_content(&v)
            .is_err()
    );
}

#[test]
fn optional_content_on_softmask_g_refuses_instead_of_rendering_wrong_mask() {
    let (doc, _, b, f) = fixture();
    let id = form(&doc, b, &f);
    let mut res = doc.get_page(0).unwrap().resource().unwrap();
    res.try_insert(
        "ExtGState".into(),
        parse(&format!(
            "<< /Mask << /SMask << /S /Alpha /G {} 0 R >> >> >>",
            id.obj_num()
        )),
    )
    .unwrap();
    set(&doc, page(&doc), "Resources", PdfObject::PdfDict(res));
    content(&doc, "/Mask gs 0 0 10 10 re f");
    let before = doc.stamp();
    let v = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(
        matches!(doc.get_page(0).unwrap().display_list_with_optional_content(&v),Err(PdfError::UnsupportedFeature(message)) if message.contains("soft-mask G"))
    );
    assert_eq!(before, doc.stamp());
}

#[test]
fn compatible_single_alternative_config_recovers_without_changing_default() {
    let (doc, a, b, _) = fixture();
    set(
        &doc,
        root(&doc),
        "OCProperties",
        parse(&format!(
            "<< /OCGs [{} 0 R {} 0 R] /D << /ON [{} 0 R] /OFF [{} 0 R] >> /Configs << /Name (Single) /BaseState /OFF >> >>",
            a.obj_num(),
            b.obj_num(),
            a.obj_num(),
            b.obj_num()
        )),
    );
    let bytes = doc.save_to_bytes().unwrap();
    for mode in [ParseMode::Strict, ParseMode::Compatible] {
        let source = Doc::from_bytes_with_mode(bytes.clone(), mode).unwrap();
        if mode == ParseMode::Strict {
            assert!(source.optional_content().is_err());
            continue;
        }
        let model = source.optional_content().unwrap().unwrap();
        assert_eq!(model.configurations.len(), 1);
        let visibility = source
            .optional_content_visibility(&Default::default())
            .unwrap();
        assert!(
            visibility
                .group_state(&source.object_handle(a).unwrap())
                .unwrap()
        );
        assert!(
            !visibility
                .group_state(&source.object_handle(b).unwrap())
                .unwrap()
        );
        assert!(
            source.structure_diagnostics().unwrap().iter().any(
                |d| d.reason == pdf::parser::diagnostic::StructureReason::OptionalContentConfig
            )
        );
    }
}

#[test]
fn compatible_source_recovers_underdeclared_layer_version_with_diagnostic() {
    let (doc, a, b, _) = fixture();
    let mut bytes = doc.save_to_bytes().unwrap();
    bytes[5..8].copy_from_slice(b"1.4");
    let strict = Doc::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).unwrap();
    assert!(strict.optional_content().is_err());
    let source = Doc::from_bytes_with_mode(bytes, ParseMode::Compatible).unwrap();
    let before = source.stamp();
    let visibility = source
        .optional_content_visibility(&Default::default())
        .unwrap();
    assert!(
        visibility
            .group_state(&source.object_handle(a).unwrap())
            .unwrap()
    );
    assert!(
        !visibility
            .group_state(&source.object_handle(b).unwrap())
            .unwrap()
    );
    assert_eq!(before, source.stamp());
    assert!(
        source
            .structure_diagnostics()
            .unwrap()
            .iter()
            .any(|d| d.reason == pdf::parser::diagnostic::StructureReason::OptionalContentVersion)
    );
    // The compatibility exception does not turn malformed properties into layers.
    set(
        &source,
        root(&source),
        "OCProperties",
        parse("<< /OCGs (invalid) /D << >> >>"),
    );
    assert!(source.optional_content().is_err());
}

#[test]
fn visible_character_extraction_uses_the_same_layer_selection_as_rendering() {
    let (doc, _, _, font) = fixture_with_options(ParseOptions {
        mode: ParseMode::Compatible,
        ..Default::default()
    });
    content(
        &doc,
        &format!(
            "/OC /A BDC BT /{font} 10 Tf (A) Tj ET EMC /OC /B BDC BT /{font} 10 Tf (B) Tj ET EMC"
        ),
    );
    let visibility = doc
        .optional_content_visibility(&Default::default())
        .unwrap();
    let selected = doc
        .get_page(0)
        .unwrap()
        .extract_characters_with_optional_content_report(&visibility)
        .unwrap();
    assert_eq!(
        selected
            .characters
            .iter()
            .filter_map(|c| c.unicode.as_deref())
            .collect::<String>(),
        "A"
    );
    let raw = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    assert_eq!(
        raw.characters
            .iter()
            .filter_map(|c| c.unicode.as_deref())
            .collect::<String>(),
        "AB"
    );
    assert!(selected.diagnostics.is_empty());
}
