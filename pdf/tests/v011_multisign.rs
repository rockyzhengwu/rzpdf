use pdf::{
    budget::OperationBudget,
    document::{Document, metadata::InfoTextField},
    error::PdfError,
    parser::syntax::ParseMode,
    signatures::{SignatureObjectState, SignatureOptions, SignatureVerification},
};
use sha2::{Digest, Sha256};

// Public synthetic certificate/signature only: no key, network or external
// executable is required. Generation/provenance is recorded beside the fixture.
fn hex(input: &str) -> Vec<u8> {
    let digits: Vec<_> = input.bytes().filter(|b| !b.is_ascii_whitespace()).collect();
    assert_eq!(digits.len() % 2, 0);
    digits
        .chunks_exact(2)
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect()
}
fn source() -> Vec<u8> {
    hex(include_str!("fixtures/v011/approval-seed.pdf.hex"))
}
fn cms() -> Vec<u8> {
    hex(include_str!("fixtures/v011/approval-second.cms.hex"))
}
fn open(bytes: Vec<u8>) -> Document {
    Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap()
}
fn options() -> SignatureOptions {
    SignatureOptions {
        field_name: "Second".into(),
        ..Default::default()
    }
}

#[test]
fn append_preserves_prefix_all_signatures_and_source_state() {
    let bytes = source();
    let doc = open(bytes.clone());
    let stamp = doc.stamp();
    let before = doc.verify_signatures().unwrap().remove(0);
    let prepared = doc.prepare_additional_signature(options()).unwrap();
    let (a, b) = prepared.signed_ranges();
    assert!(a.starts_with(&bytes));
    let mut digest = Sha256::new();
    digest.update(a);
    digest.update(b);
    assert_eq!(
        prepared.sha256_digest().as_slice(),
        digest.finalize().as_slice()
    );
    let output = doc.complete_signature(&prepared, &cms()).unwrap();
    assert!(output.starts_with(&bytes));
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.commit_revision(), 0);
    assert_eq!(doc.verify_signatures().unwrap().len(), 1);
    let reopened = open(output);
    assert!(reopened.signature_modifications().unwrap().iter().all(|r|
        r.authorization == pdf::signatures::ModificationAuthorization::Allowed));
    let reports = reopened.verify_signatures().unwrap();
    assert_eq!(reports.len(), 2);
    for report in &reports {
        assert_eq!(report.verification, SignatureVerification::Valid);
        assert_eq!(report.source_object_state, SignatureObjectState::Unchanged);
    }
    assert_eq!(reports[0].object_id, before.object_id);
    assert_eq!(reports[0].byte_range, before.byte_range);
    assert_eq!(reports[0].signed_revision_bytes, bytes.len() as u64);
    assert_eq!(reports[0].field, before.field);
    assert!(!reports[0].covers_current_document);
    assert!(reports[1].covers_current_document);
    assert_eq!(reports[1].field.as_ref().unwrap().name, "Second");
    let third = reopened
        .prepare_additional_signature(SignatureOptions {
            field_name: "Third".into(),
            ..Default::default()
        })
        .unwrap();
    // A valid CMS for another preparation is still the wrong PDF digest.
    assert!(reopened.complete_signature(&third, &cms()).is_err());
}

#[test]
fn failed_completion_writer_and_foreign_document_are_retryable() {
    let doc = open(source());
    let prepared = doc.prepare_additional_signature(options()).unwrap();
    for bad in [vec![], b"not CMS".to_vec(), vec![0; 65537]] {
        assert!(doc.complete_signature(&prepared, &bad).is_err());
    }
    let mut damaged = cms();
    *damaged.last_mut().unwrap() ^= 1;
    assert!(doc.complete_signature(&prepared, &damaged).is_err());
    assert!(
        open(source())
            .complete_signature(&prepared, &cms())
            .is_err()
    );
    let tiny = OperationBudget::new(1);
    assert!(
        doc.complete_signature_with_budget(&prepared, &cms(), &tiny)
            .is_err()
    );
    assert_eq!(tiny.live_bytes(), 0);
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected short-output failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(prepared.write_signed_content(&mut Broken).is_err());
    let mut content = Vec::new();
    prepared.write_signed_content(&mut content).unwrap();
    assert!(!content.is_empty());
    assert!(doc.complete_signature(&prepared, &cms()).is_ok());
    assert!(doc.complete_signature(&prepared, &cms()).is_err());
}

#[test]
fn preparation_and_output_keep_and_release_shared_reservations() {
    let doc = open(source());
    let stamp = doc.stamp();
    let tiny = OperationBudget::new(1);
    assert!(
        doc.prepare_additional_signature_with_budget(options(), &tiny)
            .is_err()
    );
    assert_eq!(tiny.live_bytes(), 0);
    let pool = OperationBudget::new(16 * 1024 * 1024);
    let prepared = doc
        .prepare_additional_signature_with_budget(options(), &pool)
        .unwrap();
    let held = pool.live_bytes();
    assert!(held > source().len());
    let output = doc
        .complete_signature_with_budget(&prepared, &cms(), &pool)
        .unwrap();
    assert!(pool.live_bytes() > held);
    drop(output);
    assert_eq!(pool.live_bytes(), held);
    drop(prepared);
    assert_eq!(pool.live_bytes(), 0);
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn undersized_cms_reservation_does_not_mutate_or_consume_other_preparations() {
    let doc = open(source());
    let stamp = doc.stamp();
    let small = doc
        .prepare_additional_signature(SignatureOptions {
            reserved_cms_bytes: 1024,
            ..options()
        })
        .unwrap();
    assert!(cms().len() > 1024);
    for _ in 0..2 {
        assert!(doc.complete_signature(&small, &cms()).is_err());
    }
    let correct = doc.prepare_additional_signature(options()).unwrap();
    assert!(doc.complete_signature(&correct, &cms()).is_ok());
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn unsigned_compatible_reader_and_tampered_sources_are_refused() {
    let unsigned = Document::new().unwrap();
    unsigned.add_page(0).unwrap();
    assert!(unsigned.prepare_additional_signature(options()).is_err());
    assert!(
        Document::from_bytes(source())
            .unwrap()
            .prepare_additional_signature(options())
            .is_err()
    );
    assert!(
        Document::from_reader(std::io::Cursor::new(source()))
            .unwrap()
            .prepare_additional_signature(options())
            .is_err()
    );
    let mut tampered = source();
    // Binary header comment remains legal PDF, but is inside the old ByteRange.
    assert_eq!(tampered[9], b'%');
    tampered[10] ^= 1;
    let doc = open(tampered);
    assert_eq!(
        doc.verify_signatures().unwrap()[0].verification,
        SignatureVerification::Invalid
    );
    assert!(doc.prepare_additional_signature(options()).is_err());
}

#[test]
fn duplicate_nested_control_and_oversized_options_are_refused_without_edits() {
    let doc = open(source());
    let stamp = doc.stamp();
    for field_name in ["", "Seed", "nested.name", "line\nbreak"] {
        assert!(
            doc.prepare_additional_signature(SignatureOptions {
                field_name: field_name.into(),
                ..Default::default()
            })
            .is_err()
        );
    }
    for reserved_cms_bytes in [0, 1023, usize::MAX] {
        assert!(
            doc.prepare_additional_signature(SignatureOptions {
                reserved_cms_bytes,
                ..options()
            })
            .is_err()
        );
    }
    assert!(
        doc.prepare_additional_signature(SignatureOptions {
            field_name: "审批签名".into(),
            ..Default::default()
        })
        .is_ok()
    );
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.commit_revision(), 0);
}

#[test]
fn rollback_and_outer_panic_invalidate_tokens_without_changing_signed_source() {
    for panic in [false, true] {
        let doc = open(source());
        let prepared = doc.prepare_additional_signature(options()).unwrap();
        let stamp = doc.stamp();
        if panic {
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _: Result<(), PdfError> = doc.edit_transaction(|_| panic!("injected"));
            }));
            assert!(result.is_err());
        } else {
            let result: Result<(), PdfError> =
                doc.edit_transaction(|_| Err(PdfError::DocumentError("injected rollback".into())));
            assert!(result.is_err());
        }
        assert!(!doc.is_stamp_current(&stamp));
        assert!(doc.complete_signature(&prepared, &cms()).is_err());
        assert_eq!(doc.commit_revision(), 0);
        assert_eq!(
            doc.verify_signatures().unwrap()[0].verification,
            SignatureVerification::Valid
        );
        let retry = doc.prepare_additional_signature(options()).unwrap();
        assert!(doc.complete_signature(&retry, &cms()).is_ok());
    }
}

#[test]
fn approval_capability_does_not_unlock_general_editing_or_saving() {
    let doc = open(source());
    let prepared = doc.prepare_additional_signature(options()).unwrap();
    assert!(
        doc.set_info_text(InfoTextField::Title, Some("forbidden"))
            .is_err()
    );
    assert!(doc.add_page(0).is_err());
    let id = doc.signature_reports().unwrap()[0].object_id;
    assert!(doc.delete_object(id).is_err());
    assert!(
        doc.update_object(id, pdf::objects::PdfObject::PdfNull)
            .is_err()
    );
    assert!(doc.save_to_bytes().is_err());
    assert!(doc.save_incremental_to_bytes().is_err());
    assert!(doc.prepare_signature(options()).is_err());
    // High-level edit rollback invalidates tokens even when the edit is denied.
    // The original signed bytes remain intact and a fresh token can retry.
    assert!(doc.complete_signature(&prepared, &cms()).is_err());
    let retry = doc.prepare_additional_signature(options()).unwrap();
    assert!(doc.complete_signature(&retry, &cms()).is_ok());
    assert!(doc.save_incremental_to_bytes().is_err());
}

fn append_revision(mut bytes: Vec<u8>, replacement: Option<u32>) -> Vec<u8> {
    use pdf::{
        io::stream_reader::StreamReader,
        parser::{parser::parse_xref, syntax::SyntaxParser},
    };
    let mut parser = SyntaxParser::with_mode(
        StreamReader::try_new(std::io::Cursor::new(bytes.as_slice())).unwrap(),
        ParseMode::Strict,
    );
    parser.read_header().unwrap();
    let table = parse_xref(&mut parser).unwrap();
    let root = table.trailer().get("Root").unwrap().as_object_id().unwrap();
    let size = table.trailer().get("Size").unwrap().as_u32().unwrap();
    let start = bytes.windows(9).rposition(|w| w == b"startxref").unwrap();
    let prev: usize = std::str::from_utf8(&bytes[start + 9..])
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let id = replacement.unwrap_or(size);
    bytes.push(b'\n');
    let address = bytes.len();
    bytes.extend_from_slice(format!("{id} 0 obj\nnull\nendobj\n").as_bytes());
    let offset = bytes.len();
    let size = size.max(id + 1);
    bytes.extend_from_slice(format!("xref\n{id} 1\n{address:010} 00000 n \ntrailer\n<< /Size {size} /Root {} {} R /Prev {prev} >>\nstartxref\n{offset}\n%%EOF\n",
        root.obj_num(), root.gen_num()).as_bytes());
    bytes
}

#[test]
fn unsigned_tail_and_replaced_old_signature_do_not_gain_approval_authority() {
    let bytes = source();
    let id = open(bytes.clone()).signature_reports().unwrap()[0].object_id;
    for replace in [None, Some(id.obj_num())] {
        let doc = open(append_revision(bytes.clone(), replace));
        let stamp = doc.stamp();
        let report = doc.verify_signatures().unwrap().remove(0);
        assert_eq!(report.verification, SignatureVerification::Valid);
        assert!(!report.covers_current_document);
        assert_eq!(
            report.source_object_state,
            if replace.is_some() {
                SignatureObjectState::Replaced
            } else {
                SignatureObjectState::Unchanged
            }
        );
        assert!(doc.prepare_additional_signature(options()).is_err());
        assert!(doc.is_stamp_current(&stamp));
        assert_eq!(doc.commit_revision(), 0);
        assert!(doc.save_incremental_to_bytes().is_err());
    }
}
