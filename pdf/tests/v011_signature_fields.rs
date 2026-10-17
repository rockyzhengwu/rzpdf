use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        acroform::{FormTextStyle, FormWidgetSpec},
        save_options::PdfVersion,
    },
    parser::syntax::ParseMode,
    signatures::{SignatureAppearance, SignatureFieldOptions},
};
fn source() -> Document {
    let doc = Document::new_with_version(PdfVersion::V2_0).unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    doc.add_signature_field(
        "Approvals.First",
        &[
            FormWidgetSpec::new(0, [20., 30., 220., 80.]),
            FormWidgetSpec::new(1, [20., 30., 220., 80.]),
        ],
    )
    .unwrap();
    Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap()
}
fn options() -> SignatureFieldOptions {
    SignatureFieldOptions {
        field_name: "Approvals.First".into(),
        reserved_cms_bytes: 16384,
        appearance: Some(SignatureAppearance {
            text: "Approval only\nNot identity proof".into(),
            style: FormTextStyle::default(),
        }),
    }
}
#[test]
fn hierarchical_multi_widget_signature_preparation_is_non_mutating() {
    let doc = source();
    let stamp = doc.stamp();
    let fields = doc.form_field_tree().unwrap();
    assert_eq!(fields[1].widgets.len(), 2);
    assert_eq!(fields[1].field_type.as_deref(), Some("Sig"));
    let prepared = doc.prepare_field_signature(options()).unwrap();
    assert!(prepared.signed_ranges().0.starts_with(b"%PDF-2.0"));
    assert!(doc.complete_signature(&prepared, b"invalid CMS").is_err());
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.commit_revision(), 0);
    assert!(doc.verify_signatures().unwrap().is_empty());
}
#[test]
fn field_appearance_limits_foreign_fonts_and_budget_fail_without_changes() {
    let doc = source();
    let stamp = doc.stamp();
    let tiny = OperationBudget::new(1);
    assert!(
        doc.prepare_field_signature_with_budget(options(), &tiny)
            .is_err()
    );
    assert_eq!(tiny.live_bytes(), 0);
    for text in ["way too long ".repeat(200), "control\tcharacter".into()] {
        let mut options = options();
        options.appearance.as_mut().unwrap().text = text;
        assert!(doc.prepare_field_signature(options).is_err());
    }
    let mut unknown = options();
    unknown.field_name = "missing".into();
    assert!(doc.prepare_field_signature(unknown).is_err());
    assert!(doc.is_stamp_current(&stamp));
    let pool = OperationBudget::new(32 * 1024 * 1024);
    let prepared = doc
        .prepare_field_signature_with_budget(options(), &pool)
        .unwrap();
    assert!(pool.live_bytes() > 0);
    drop(prepared);
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn empty_zero_area_field_and_failed_creation_keep_ownership_valid() {
    let doc = Document::new_with_version(PdfVersion::V2_0).unwrap();
    doc.add_page(0).unwrap();
    let invisible = doc
        .add_signature_field("Invisible", &[FormWidgetSpec::new(0, [0.; 4])])
        .unwrap();
    assert!(
        doc.add_signature_field("Invisible", &[FormWidgetSpec::new(0, [0.; 4])])
            .is_err()
    );
    assert!(
        doc.add_signature_field("MissingPage", &[FormWidgetSpec::new(3, [0.; 4])])
            .is_err()
    );
    assert_eq!(doc.form_field_tree().unwrap().len(), 1);
    let mut dict = doc
        .resolve_object(invisible.object_id())
        .unwrap()
        .into_dict()
        .unwrap();
    dict.try_insert(
        "F".into(),
        pdf::objects::PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(132.)),
    )
    .unwrap();
    doc.update_object(
        invisible.object_id(),
        pdf::objects::PdfObject::PdfDict(dict),
    )
    .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let prepared = reopened
        .prepare_field_signature(SignatureFieldOptions {
            field_name: "Invisible".into(),
            reserved_cms_bytes: 16384,
            appearance: None,
        })
        .unwrap();
    assert!(reopened.complete_signature(&prepared, b"bad").is_err());
    let foreign = source();
    let mut options = SignatureFieldOptions {
        field_name: "Invisible".into(),
        reserved_cms_bytes: 16384,
        appearance: Some(SignatureAppearance {
            text: "x".into(),
            style: FormTextStyle::default(),
        }),
    };
    options.appearance.as_mut().unwrap().style.font =
        Some(foreign.form_field_tree().unwrap()[0].handle.clone());
    assert!(reopened.prepare_field_signature(options).is_err());
    for (flags, allowed) in [(68., true), (516., false), (1028., false)] {
        let mut dict = doc
            .resolve_object(invisible.object_id())
            .unwrap()
            .into_dict()
            .unwrap();
        dict.try_insert(
            "F".into(),
            pdf::objects::PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(flags)),
        )
        .unwrap();
        doc.update_object(
            invisible.object_id(),
            pdf::objects::PdfObject::PdfDict(dict),
        )
        .unwrap();
        let check = Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
        assert_eq!(
            check
                .prepare_field_signature(SignatureFieldOptions {
                    field_name: "Invisible".into(),
                    reserved_cms_bytes: 16384,
                    appearance: None
                })
                .is_ok(),
            allowed
        );
    }
}

#[test]
fn preserved_source_ap_matrix_and_resources_are_not_rebuilt_without_an_appearance_request() {
    use pdf::objects::{PdfObject, pdf_array::PdfArray, pdf_number::PdfNumber};
    let doc = Document::new_with_version(PdfVersion::V2_0).unwrap();
    doc.add_page(0).unwrap();
    let handle = doc
        .add_signature_field("Existing", &[FormWidgetSpec::new(0, [20., 30., 220., 80.])])
        .unwrap();
    let field = doc.resolve_object(handle.object_id()).unwrap();
    let apid = field
        .as_dict()
        .unwrap()
        .get("AP")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("N")
        .unwrap()
        .as_object_id()
        .unwrap();
    let stream = doc.resolve_object(apid).unwrap();
    let stream = stream.as_stream().unwrap();
    let mut dict = stream.dict().clone();
    let mut matrix = PdfArray::default();
    for value in [0., 1., -1., 0., 50., 0.] {
        matrix
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(value)))
            .unwrap();
    }
    dict.try_insert("Matrix".into(), PdfObject::PdfArray(matrix))
        .unwrap();
    doc.update_object(
        apid,
        PdfObject::PdfStream(pdf::objects::pdf_stream::PdfStream::new(dict, vec![])),
    )
    .unwrap();
    let source = doc.save_to_bytes().unwrap();
    let doc = Document::from_bytes_with_mode(source, ParseMode::Strict).unwrap();
    let old = doc.resolve_object(apid).unwrap();
    let prepared = doc
        .prepare_field_signature(SignatureFieldOptions {
            field_name: "Existing".into(),
            reserved_cms_bytes: 16384,
            appearance: None,
        })
        .unwrap();
    let (a, b) = prepared.signed_ranges();
    let mut output = a.to_vec();
    output.push(b'<');
    output.extend(std::iter::repeat_n(
        b'0',
        (prepared.byte_range()[2] - prepared.byte_range()[1] - 2) as usize,
    ));
    output.push(b'>');
    output.extend_from_slice(b);
    let reopened = Document::from_bytes_with_mode(output, ParseMode::Strict).unwrap();
    assert_eq!(old, reopened.resolve_object(apid).unwrap());
}

#[test]
fn unknown_reverse_field_owner_is_refused_before_signing() {
    use pdf::objects::{PdfObject, pdf_reference::PdfReference};
    let doc = Document::new_with_version(PdfVersion::V2_0).unwrap();
    doc.add_page(0).unwrap();
    let field = doc
        .add_signature_field("Shared", &[FormWidgetSpec::new(0, [0.; 4])])
        .unwrap();
    let mut root = pdf::objects::pdf_dict::PdfDict::default();
    root.try_insert(
        "UnknownFieldOwner".into(),
        PdfObject::PdfReference(PdfReference::from_id(field.object_id())),
    )
    .unwrap();
    doc.try_insert_object(PdfObject::PdfDict(root)).unwrap();
    let doc =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let stamp = doc.stamp();
    assert!(
        doc.prepare_field_signature(SignatureFieldOptions {
            field_name: "Shared".into(),
            reserved_cms_bytes: 16384,
            appearance: None
        })
        .is_err()
    );
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn existing_field_inherits_signature_type_through_its_parent() {
    use pdf::objects::{PdfObject, pdf_name::PdfName};
    let doc = Document::new_with_version(PdfVersion::V2_0).unwrap();
    doc.add_page(0).unwrap();
    let child = doc
        .add_signature_field("Group.Leaf", &[FormWidgetSpec::new(0, [0.; 4])])
        .unwrap();
    let parent = doc
        .form_field_tree()
        .unwrap()
        .into_iter()
        .find(|n| n.name == "Group")
        .unwrap()
        .handle;
    let mut p = doc
        .resolve_object(parent.object_id())
        .unwrap()
        .into_dict()
        .unwrap();
    p.try_insert("FT".into(), PdfObject::PdfName(PdfName::new("Sig".into())))
        .unwrap();
    doc.update_object(parent.object_id(), PdfObject::PdfDict(p))
        .unwrap();
    let mut c = doc
        .resolve_object(child.object_id())
        .unwrap()
        .into_dict()
        .unwrap();
    c.remove("FT");
    doc.update_object(child.object_id(), PdfObject::PdfDict(c))
        .unwrap();
    let doc =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert!(
        doc.prepare_field_signature(SignatureFieldOptions {
            field_name: "Group.Leaf".into(),
            reserved_cms_bytes: 16384,
            appearance: None
        })
        .is_ok()
    );
}

#[test]
fn appearance_character_work_is_cumulative_across_widgets() {
    let doc = Document::new_with_version(PdfVersion::V2_0).unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    doc.add_signature_field(
        "A.B",
        &[
            FormWidgetSpec::new(0, [0., 0., 200., 50.]),
            FormWidgetSpec::new(1, [0., 0., 200., 50.]),
        ],
    )
    .unwrap();
    let mut parse = pdf::parser::options::ParseOptions::default();
    parse.mode = ParseMode::Strict;
    parse.limits.max_text_chars = 10;
    let doc = Document::from_bytes_with_options(doc.save_to_bytes().unwrap(), parse).unwrap();
    let stamp = doc.stamp();
    let result = doc.prepare_field_signature(SignatureFieldOptions {
        field_name: "A.B".into(),
        reserved_cms_bytes: 16384,
        appearance: Some(SignatureAppearance {
            text: "Review".into(),
            style: FormTextStyle::default(),
        }),
    });
    assert!(
        matches!(result,Err(pdf::error::PdfError::ResourceLimit(message)) if message.contains("cumulative AP characters"))
    );
    assert!(doc.is_stamp_current(&stamp));
}
