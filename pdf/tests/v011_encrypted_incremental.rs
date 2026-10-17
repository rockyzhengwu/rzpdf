use pdf::{
    budget::OperationBudget,
    document::{
        Document, acroform::FormValue, metadata::InfoTextField,
        save_options::IncrementalSaveOptions,
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_stream::PdfStream,
        pdf_string::PdfString,
    },
    parser::{
        options::ParseOptions,
        parser::parse_xref,
        syntax::{ParseMode, SyntaxParser},
    },
    security::{EncryptionOptions, SaveSecurity},
};
use std::{
    io::{Cursor, Write},
    panic::{AssertUnwindSafe, catch_unwind},
};
fn options() -> ParseOptions {
    ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    }
}
fn open(bytes: Vec<u8>, password: &str) -> Document {
    Document::from_bytes_with_password_and_options(bytes, password, options()).unwrap()
}
fn trailer(bytes: &[u8]) -> PdfDict {
    let mut parser = SyntaxParser::new(StreamReader::try_new(Cursor::new(bytes)).unwrap());
    parser.read_header().unwrap();
    parse_xref(&mut parser).unwrap().trailer().clone()
}
fn created() -> Document {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"0 0 20 20 re f".to_vec())
        .unwrap();
    doc.set_info_text(InfoTextField::Title, Some("initial"))
        .unwrap();
    doc.add_text_field(0, [10., 30., 400., 50.], "value", "old")
        .unwrap();
    doc
}
fn protected(doc: &Document, clear_metadata: bool) -> Vec<u8> {
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.encrypt_metadata = !clear_metadata;
    doc.save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
        .unwrap()
}
fn contains(bytes: &[u8], text: &[u8]) -> bool {
    bytes.windows(text.len()).any(|s| s == text)
}

#[test]
fn three_rounds_preserve_ciphertext_prefix_identity_and_authentication() {
    for password in ["user", "owner"] {
        let mut source = protected(&created(), false);
        let initial = trailer(&source);
        for title in [
            "round one private 中文",
            "round two private 中文",
            "round three private 中文",
        ] {
            let doc = open(source.clone(), password);
            assert_eq!(doc.save_incremental_to_bytes().unwrap(), source);
            doc.set_info_text(InfoTextField::Title, Some(title))
                .unwrap();
            let stamp = doc.stamp();
            let revision = doc.commit_revision();
            let next = doc.save_incremental_to_bytes().unwrap();
            assert!(next.starts_with(&source));
            assert!(!contains(&next[source.len()..], b"private"));
            assert_eq!(trailer(&next).get("Encrypt"), initial.get("Encrypt"));
            let old_trailer = trailer(&source);
            let next_trailer = trailer(&next);
            let old_ids = old_trailer.get("ID").unwrap().as_array().unwrap();
            let next_ids = next_trailer.get("ID").unwrap().as_array().unwrap();
            assert_eq!(next_ids.get(0), old_ids.get(0));
            assert_ne!(next_ids.get(1), old_ids.get(1));
            assert_eq!(stamp, doc.stamp());
            assert_eq!(revision, doc.commit_revision());
            for role in ["user", "owner"] {
                assert_eq!(
                    open(next.clone(), role)
                        .info_text(InfoTextField::Title)
                        .unwrap()
                        .as_deref(),
                    Some(title)
                );
            }
            assert!(
                Document::from_bytes_with_password_and_options(next.clone(), "wrong", options())
                    .is_err()
            );
            source = next;
        }
    }
}

#[test]
fn new_strings_streams_attachments_and_form_appearances_are_encrypted_once() {
    let source = protected(&created(), false);
    let doc = open(source.clone(), "owner");
    doc.set_form_value("value", FormValue::Text("new private field".into()))
        .unwrap();
    doc.add_attachment("new.bin", b"new private payload".to_vec())
        .unwrap();
    doc.set_metadata_packet(Some(b"<x>new private metadata</x>".to_vec()))
        .unwrap();
    let string = doc
        .try_insert_object(PdfObject::PdfString(PdfString::new(
            b"new private literal".to_vec(),
            false,
        )))
        .unwrap();
    let output = doc.save_incremental_to_bytes().unwrap();
    for secret in [
        b"new private".as_slice(),
        b"private metadata",
        b"private literal",
    ] {
        assert!(!contains(&output[source.len()..], secret));
    }
    let reopened = open(output, "user");
    assert_eq!(
        reopened.form_fields().unwrap()[0].value,
        FormValue::Text("new private field".into())
    );
    assert_eq!(
        reopened.attachment_data("new.bin").unwrap().unwrap(),
        b"new private payload"
    );
    assert_eq!(
        reopened.metadata_packet().unwrap().unwrap(),
        b"<x>new private metadata</x>"
    );
    assert_eq!(
        reopened
            .resolve_object(string)
            .unwrap()
            .as_string()
            .unwrap()
            .bytes(),
        b"new private literal"
    );
    reopened
        .get_page(0)
        .unwrap()
        .display_list_with_annotations(Default::default())
        .unwrap();
}

#[test]
fn clear_metadata_remains_the_only_requested_plaintext_exception() {
    let source = protected(&created(), true);
    let doc = open(source.clone(), "user");
    let packet = b"<x>visible clear metadata</x>";
    doc.set_metadata_packet(Some(packet.to_vec())).unwrap();
    doc.set_info_text(InfoTextField::Title, Some("private-info-text"))
        .unwrap();
    let output = doc.save_incremental_to_bytes().unwrap();
    let tail = &output[source.len()..];
    assert!(contains(tail, packet));
    assert!(!contains(tail, b"private-info-text"));
    assert_eq!(
        open(output, "owner").metadata_packet().unwrap().unwrap(),
        packet
    );
}

#[test]
fn identity_crypt_streams_are_explicit_not_accidentally_reencrypted() {
    let source = protected(&created(), false);
    let doc = open(source.clone(), "owner");
    let mut d = PdfDict::default();
    d.try_insert(
        "Filter".into(),
        PdfObject::PdfName(PdfName::new("Crypt".into())),
    )
    .unwrap();
    let mut params = PdfDict::default();
    params
        .try_insert(
            "Name".into(),
            PdfObject::PdfName(PdfName::new("Identity".into())),
        )
        .unwrap();
    d.try_insert("DecodeParms".into(), PdfObject::PdfDict(params))
        .unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            d,
            b"explicit-public-stream".to_vec(),
        )))
        .unwrap();
    let output = doc.save_incremental_to_bytes().unwrap();
    assert!(contains(&output[source.len()..], b"explicit-public-stream"));
    assert_eq!(
        open(output, "owner")
            .resolve_object(id)
            .unwrap()
            .as_stream()
            .unwrap()
            .raw_data(),
        b"explicit-public-stream"
    );
}

#[test]
fn protection_parameter_mutation_and_deletion_refuse_without_save_mutation() {
    let source = protected(&created(), false);
    let id = trailer(&source)
        .get("Encrypt")
        .unwrap()
        .as_object_id()
        .unwrap();
    for delete in [false, true] {
        let doc = open(source.clone(), "owner");
        if delete {
            doc.delete_object(id).unwrap();
        } else {
            let mut d = doc.resolve_object(id).unwrap().into_dict().unwrap();
            d.try_insert("PrivateChange".into(), PdfObject::PdfNull)
                .unwrap();
            doc.update_object(id, PdfObject::PdfDict(d)).unwrap();
        }
        let stamp = doc.stamp();
        let revision = doc.commit_revision();
        assert!(matches!(
            doc.save_incremental_to_bytes(),
            Err(PdfError::UnsupportedSecurity(_))
        ));
        assert_eq!(doc.stamp(), stamp);
        assert_eq!(doc.commit_revision(), revision);
    }
}

#[test]
fn shared_budget_output_cap_and_writer_failure_allow_retry() {
    let source = protected(&created(), false);
    let doc = open(source.clone(), "owner");
    doc.set_info_text(InfoTextField::Title, Some("retry-private"))
        .unwrap();
    let stamp = doc.stamp();
    let budget = OperationBudget::new(1);
    assert!(
        doc.save_incremental_with_budget(Default::default(), &budget)
            .is_err()
    );
    assert_eq!(budget.live_bytes(), 0);
    assert!(
        doc.save_incremental_to_bytes_with_options(IncrementalSaveOptions {
            max_output_bytes: source.len()
        })
        .is_err()
    );
    struct Broken;
    impl Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(doc.save_incremental_to_writer(&mut Broken).is_err());
    assert_eq!(doc.stamp(), stamp);
    let output = doc.save_incremental_to_bytes().unwrap();
    assert_eq!(
        open(output, "owner")
            .info_text(InfoTextField::Title)
            .unwrap()
            .as_deref(),
        Some("retry-private")
    );
}

#[test]
fn rollback_and_outer_unwind_restore_exact_noop_ciphertext() {
    let source = protected(&created(), false);
    let doc = open(source.clone(), "owner");
    let failed: Result<(), PdfError> = doc.edit_transaction(|doc| {
        doc.set_info_text(InfoTextField::Title, Some("rollback"))?;
        Err(PdfError::DocumentError("injected".into()))
    });
    assert!(failed.is_err());
    assert_eq!(doc.save_incremental_to_bytes().unwrap(), source);
    assert!(
        catch_unwind(AssertUnwindSafe(|| {
            let _: Result<(), PdfError> = doc.edit_transaction(|doc| {
                doc.set_info_text(InfoTextField::Title, Some("panic"))?;
                panic!("injected")
            });
        }))
        .is_err()
    );
    assert_eq!(doc.save_incremental_to_bytes().unwrap(), source);
}

#[test]
fn user_permissions_and_encrypted_signature_boundary_remain_in_force() {
    let doc = created();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.permissions.modify = false;
    let source = doc
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
        .unwrap();
    let user = open(source.clone(), "user");
    let commit = user.commit_revision();
    assert!(
        user.set_info_text(InfoTextField::Title, Some("denied"))
            .is_err()
    );
    assert_eq!(user.commit_revision(), commit);
    assert_eq!(user.save_incremental_to_bytes().unwrap(), source);
    user.set_form_value("value", FormValue::Text("permitted fill".into()))
        .unwrap();
    let output = user.save_incremental_to_bytes().unwrap();
    assert_eq!(
        open(output, "owner").form_fields().unwrap()[0].value,
        FormValue::Text("permitted fill".into())
    );
    let owner = open(source, "owner");
    assert!(owner.prepare_signature(Default::default()).is_err());
}
