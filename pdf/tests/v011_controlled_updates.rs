use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        acroform::{FormTextStyle, FormValue},
    },
    error::PdfError,
    parser::syntax::ParseMode,
    signatures::{
        DocMdpPermission, ModificationAuthorization, ModificationKind, SignatureAppearance,
        SignatureFieldOptions, SignatureVerification,
    },
};
fn hex(input: &str) -> Vec<u8> {
    let text: Vec<_> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    text.chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn bytes() -> Vec<u8> {
    hex(include_str!("fixtures/v011/workflow-certified.pdf.hex"))
}
fn cms() -> Vec<u8> {
    hex(include_str!("fixtures/v011/workflow-second.cms.hex"))
}
fn open(bytes: Vec<u8>) -> Document {
    Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap()
}
fn field(doc: &Document, name: &str) -> pdf::document::identity::ObjectHandle {
    doc.form_field_tree()
        .unwrap()
        .into_iter()
        .find(|n| n.name == name)
        .unwrap()
        .handle
}
fn options() -> SignatureFieldOptions {
    SignatureFieldOptions {
        field_name: "Sign.Second".into(),
        reserved_cms_bytes: 16384,
        appearance: Some(SignatureAppearance {
            text: "Approval only".into(),
            style: FormTextStyle::default(),
        }),
    }
}

#[test]
fn certified_existing_field_signature_preserves_prior_signature_and_reports_authorization() {
    let doc = open(bytes());
    let stamp = doc.stamp();
    assert_eq!(
        doc.signature_constraints().unwrap()[0].doc_mdp,
        Some(DocMdpPermission::FillFormsAndSign)
    );
    let old = doc.verify_signatures().unwrap().remove(0);
    let prepared = doc.prepare_field_signature(options()).unwrap();
    for bad in [vec![], b"bad CMS".to_vec(), vec![0; 65537]] {
        assert!(doc.complete_signature(&prepared, &bad).is_err());
    }
    let mut damaged = cms();
    *damaged.last_mut().unwrap() ^= 1;
    assert!(doc.complete_signature(&prepared, &damaged).is_err());
    let output = doc.complete_signature(&prepared, &cms()).unwrap();
    assert!(output.starts_with(&bytes()));
    assert!(doc.complete_signature(&prepared, &cms()).is_err());
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.commit_revision(), 0);
    let reopened = open(output);
    let reports = reopened.verify_signatures().unwrap();
    assert_eq!(reports.len(), 2);
    assert!(
        reports
            .iter()
            .all(|r| r.verification == SignatureVerification::Valid)
    );
    assert_eq!(old.byte_range, reports[0].byte_range);
    assert!(!reports[0].covers_current_document);
    assert!(reports[1].covers_current_document);
    let modifications = reopened.signature_modifications().unwrap();
    assert_eq!(modifications.len(), 1);
    assert_eq!(modifications[0].kinds, [ModificationKind::Signature]);
    assert_eq!(
        modifications[0].authorization,
        ModificationAuthorization::Allowed
    );
}

#[test]
fn text_checkbox_and_following_signature_preparation_accept_only_authorized_tail() {
    let doc = open(bytes());
    let stamp = doc.stamp();
    let update = doc
        .prepare_signed_form_fill(
            &field(&doc, "Data.Editable"),
            FormValue::Text("Allowed".into()),
            Some(FormTextStyle::default()),
        )
        .unwrap();
    let filled = doc.complete_signed_update(&update).unwrap();
    assert!(filled.starts_with(&bytes()));
    assert!(doc.complete_signed_update(&update).is_err());
    assert!(doc.is_stamp_current(&stamp));
    let doc = open(filled);
    let report = doc.verify_signatures().unwrap().remove(0);
    assert_eq!(report.verification, SignatureVerification::Valid);
    assert!(!report.covers_current_document);
    let value = doc
        .resolve_object(field(&doc, "Data.Editable").object_id())
        .unwrap();
    assert_eq!(
        value
            .as_dict()
            .unwrap()
            .get("V")
            .unwrap()
            .as_string()
            .unwrap()
            .text_content()
            .unwrap(),
        "Allowed"
    );
    let update = doc
        .prepare_signed_form_fill(&field(&doc, "Data.Check"), FormValue::Checkbox(true), None)
        .unwrap();
    let doc = open(doc.complete_signed_update(&update).unwrap());
    let value = doc
        .resolve_object(field(&doc, "Data.Check").object_id())
        .unwrap();
    assert_eq!(
        value
            .as_dict()
            .unwrap()
            .get("V")
            .unwrap()
            .as_name()
            .unwrap()
            .name(),
        "Yes"
    );
    assert_eq!(
        value
            .as_dict()
            .unwrap()
            .get("AS")
            .unwrap()
            .as_name()
            .unwrap()
            .name(),
        "Yes"
    );
    assert!(
        doc.signature_modifications()
            .unwrap()
            .iter()
            .all(|r| r.authorization == ModificationAuthorization::Allowed)
    );
    let prepared = doc.prepare_field_signature(options()).unwrap();
    // This CMS was for the original certified file, not the filled revision.
    assert!(doc.complete_signature(&prepared, &cms()).is_err());
    assert!(
        doc.prepare_signed_square(0, [20., 20., 80., 80.], "not permitted at P2")
            .is_err()
    );
    assert!(doc.save_to_bytes().is_err());
    assert!(doc.save_incremental_to_bytes().is_err());
}

#[test]
fn update_budget_foreign_identity_single_use_and_failure_retry_are_explicit() {
    let doc = open(bytes());
    let target = field(&doc, "Data.Editable");
    let stamp = doc.stamp();
    let tiny = OperationBudget::new(1);
    assert!(
        doc.prepare_signed_form_fill_with_budget(
            &target,
            FormValue::Text("Allowed".into()),
            Some(FormTextStyle::default()),
            &tiny
        )
        .is_err()
    );
    assert_eq!(tiny.live_bytes(), 0);
    let pool = OperationBudget::new(64 * 1024 * 1024);
    let update = doc
        .prepare_signed_form_fill_with_budget(
            &target,
            FormValue::Text("Allowed".into()),
            Some(FormTextStyle::default()),
            &pool,
        )
        .unwrap();
    let held = pool.live_bytes();
    assert!(held > bytes().len());
    assert!(open(bytes()).complete_signed_update(&update).is_err());
    assert!(
        doc.complete_signed_update_with_budget(&update, &tiny)
            .is_err()
    );
    assert_eq!(tiny.live_bytes(), 0);
    let output = doc
        .complete_signed_update_with_budget(&update, &pool)
        .unwrap();
    assert!(pool.live_bytes() > held);
    drop(output);
    assert_eq!(pool.live_bytes(), held);
    drop(update);
    assert_eq!(pool.live_bytes(), 0);
    assert!(doc.is_stamp_current(&stamp));
    let style = FormTextStyle {
        font_size: 10.,
        ..Default::default()
    };
    let appearance = doc
        .prepare_signed_form_fill(
            &field(&doc, "Data.Editable"),
            FormValue::Text("Original".into()),
            Some(style),
        )
        .unwrap();
    let styled = open(doc.complete_signed_update(&appearance).unwrap());
    assert_eq!(
        styled.signature_modifications().unwrap()[0].kinds,
        [ModificationKind::Appearance]
    );
}

#[test]
fn rollback_and_outer_panic_invalidate_both_workflow_tokens_without_edits() {
    for panic in [false, true] {
        let doc = open(bytes());
        let stamp = doc.stamp();
        let signature = doc.prepare_field_signature(options()).unwrap();
        let update = doc
            .prepare_signed_form_fill(
                &field(&doc, "Data.Editable"),
                FormValue::Text("Allowed".into()),
                Some(FormTextStyle::default()),
            )
            .unwrap();
        if panic {
            assert!(
                std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    let _: Result<(), PdfError> = doc.edit_transaction(|_| panic!("injected"));
                }))
                .is_err()
            );
        } else {
            let result: Result<(), PdfError> =
                doc.edit_transaction(|_| Err(PdfError::DocumentError("rollback".into())));
            assert!(result.is_err());
        }
        assert!(!doc.is_stamp_current(&stamp));
        assert_eq!(doc.commit_revision(), 0);
        assert!(doc.complete_signature(&signature, &cms()).is_err());
        assert!(doc.complete_signed_update(&update).is_err());
        let retry = doc.prepare_field_signature(options()).unwrap();
        assert!(doc.complete_signature(&retry, &cms()).is_ok());
    }
}

#[test]
fn unsupported_targets_styles_values_and_low_level_edits_are_rejected_without_mutation() {
    let doc = open(bytes());
    let stamp = doc.stamp();
    let foreign = open(bytes());
    assert!(
        doc.prepare_signed_form_fill(
            &field(&foreign, "Data.Editable"),
            FormValue::Text("Allowed".into()),
            Some(FormTextStyle::default())
        )
        .is_err()
    );
    assert!(
        doc.prepare_signed_form_fill(
            &field(&doc, "Data.Editable"),
            FormValue::Text("Allowed".into()),
            None
        )
        .is_err()
    );
    assert!(
        doc.prepare_signed_form_fill(
            &field(&doc, "Data.Editable"),
            FormValue::Text("bad\nline".into()),
            Some(FormTextStyle::default())
        )
        .is_err()
    );
    assert!(
        doc.prepare_signed_form_fill(
            &field(&doc, "Data.Check"),
            FormValue::Checkbox(true),
            Some(FormTextStyle::default())
        )
        .is_err()
    );
    let mut filled = options();
    filled.field_name = "Sign.First".into();
    assert!(doc.prepare_field_signature(filled).is_err());
    assert!(
        doc.delete_object(field(&doc, "Sign.First").object_id())
            .is_err()
    );
    assert!(doc.add_signature_field("New", &[]).is_err());
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn one_byte_below_observed_tracked_peak_fails_cleanly_then_retries() {
    let measured = open(bytes());
    let pool = OperationBudget::new(64 * 1024 * 1024);
    let prepared = measured
        .prepare_signed_form_fill_with_budget(
            &field(&measured, "Data.Editable"),
            FormValue::Text("Allowed".into()),
            Some(FormTextStyle::default()),
            &pool,
        )
        .unwrap();
    let peak = pool.peak_bytes();
    drop(prepared);
    assert_eq!(pool.live_bytes(), 0);
    let doc = open(bytes());
    let stamp = doc.stamp();
    let short = OperationBudget::new(peak - 1);
    assert!(matches!(
        doc.prepare_signed_form_fill_with_budget(
            &field(&doc, "Data.Editable"),
            FormValue::Text("Allowed".into()),
            Some(FormTextStyle::default()),
            &short
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(short.live_bytes(), 0);
    assert!(doc.is_stamp_current(&stamp));
    let retry = doc
        .prepare_signed_form_fill(
            &field(&doc, "Data.Editable"),
            FormValue::Text("Allowed".into()),
            Some(FormTextStyle::default()),
        )
        .unwrap();
    assert!(doc.complete_signed_update(&retry).is_ok());
}
