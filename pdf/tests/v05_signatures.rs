use pdf::{
    budget::OperationBudget,
    document::{Document, metadata::InfoTextField},
    parser::syntax::ParseMode,
    signatures::SignatureOptions,
};
use sha2::{Digest, Sha256};

fn created() -> Document {
    let document = Document::new().unwrap();
    document.add_page(0).unwrap();
    document
}

#[test]
fn prepare_is_immutable_non_mutating_and_hashes_exact_ranges() {
    for source in [false, true] {
        let initial = created();
        let bytes = initial.save_to_bytes().unwrap();
        let document = if source {
            Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).unwrap()
        } else {
            initial
        };
        document
            .set_info_text(InfoTextField::Title, Some("pending edit"))
            .unwrap();
        let stamp = document.stamp();
        let prepared = document.prepare_signature(Default::default()).unwrap();
        let (first, second) = prepared.signed_ranges();
        let range = prepared.byte_range();
        assert_eq!(range[0], 0);
        assert_eq!(range[1], first.len() as u64);
        assert_eq!(range[3], second.len() as u64);
        assert_eq!(range[2] - range[1], (16384 * 2 + 2) as u64);
        if source {
            assert!(first.starts_with(&bytes));
        }
        let mut hash = Sha256::new();
        hash.update(first);
        hash.update(second);
        assert_eq!(
            prepared.sha256_digest().as_slice(),
            hash.finalize().as_slice()
        );
        assert!(document.is_stamp_current(&stamp));
        assert!(document.signature_reports().unwrap().is_empty());
        let mut output = Vec::new();
        prepared.write_signed_content(&mut output).unwrap();
        assert_eq!(output, [first, second].concat());
    }
}

#[test]
fn unsupported_inputs_and_reservations_fail_without_edits() {
    let empty = Document::new().unwrap();
    assert!(empty.prepare_signature(Default::default()).is_err());
    let document = created();
    let stamp = document.stamp();
    for options in [
        SignatureOptions {
            field_name: "a.b".into(),
            ..Default::default()
        },
        SignatureOptions {
            reserved_cms_bytes: usize::MAX,
            ..Default::default()
        },
        SignatureOptions {
            reserved_cms_bytes: 16,
            ..Default::default()
        },
    ] {
        assert!(document.prepare_signature(options).is_err());
    }
    assert!(
        document
            .prepare_signature_with_budget(Default::default(), &OperationBudget::new(1))
            .is_err()
    );
    let source = document.save_to_bytes().unwrap();
    assert!(
        Document::from_bytes(source.clone())
            .unwrap()
            .prepare_signature(Default::default())
            .is_err()
    );
    assert!(
        Document::from_reader(std::io::Cursor::new(source))
            .unwrap()
            .prepare_signature(Default::default())
            .is_err()
    );
    assert!(document.is_stamp_current(&stamp));
}

#[test]
fn malformed_oversize_foreign_stale_cms_and_failed_output_are_retry_safe() {
    let document = created();
    let prepared = document.prepare_signature(Default::default()).unwrap();
    for cms in [vec![], b"not CMS".to_vec(), vec![0; 65537]] {
        assert!(document.complete_signature(&prepared, &cms).is_err());
    }
    assert!(created().complete_signature(&prepared, b"not CMS").is_err());
    struct Broken;
    impl std::io::Write for Broken {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("injected"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    assert!(prepared.write_signed_content(&mut Broken).is_err());
    let mut output = Vec::new();
    prepared.write_signed_content(&mut output).unwrap();
    assert!(!output.is_empty());
    document
        .set_info_text(InfoTextField::Title, Some("invalidates preparation"))
        .unwrap();
    assert!(document.complete_signature(&prepared, b"not CMS").is_err());
}

#[test]
fn signature_bearing_source_is_locked_before_overlay_can_hide_it() {
    use pdf::objects::{
        PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber,
    };
    for key in ["Type", "FT", "ByteRange", "DocMDP", "FieldMDP", "SigFlags"] {
        let original = created();
        let mut dict = PdfDict::default();
        dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new("Sig".into())))
            .unwrap();
        let id = original
            .try_insert_object(PdfObject::PdfDict(dict))
            .unwrap();
        let source = original.save_to_bytes().unwrap();
        let document = Document::from_bytes_with_mode(source, ParseMode::Strict).unwrap();
        let stamp = document.stamp();
        assert!(
            document.update_object(id, PdfObject::PdfNull).is_err(),
            "{key}"
        );
        assert!(document.delete_object(id).is_err(), "{key}");
        assert!(document.save_to_bytes().is_err(), "{key}");
        assert!(
            document.prepare_signature(Default::default()).is_err(),
            "{key}"
        );
        assert!(document.is_stamp_current(&stamp));
    }
    let original = created();
    let mut dict = PdfDict::default();
    let mut range = PdfArray::default();
    for value in [0., 1., 2., 999999.] {
        range
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(value)))
            .unwrap();
    }
    dict.try_insert("ByteRange".into(), PdfObject::PdfArray(range))
        .unwrap();
    original
        .try_insert_object(PdfObject::PdfDict(dict))
        .unwrap();
    let document =
        Document::from_bytes_with_mode(original.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    assert!(document.signature_reports().is_err());
}

#[test]
fn structural_coverage_is_not_a_crypto_verdict_and_hole_must_bind_to_contents() {
    use pdf::signatures::SignatureVerification;
    let document = created();
    document
        .add_text_field(0, [10., 10., 80., 30.], "existing", "unchanged")
        .unwrap();
    assert!(
        document
            .prepare_signature(SignatureOptions {
                field_name: "existing".into(),
                ..Default::default()
            })
            .is_err()
    );
    let prepared = document.prepare_signature(Default::default()).unwrap();
    let (first, second) = prepared.signed_ranges();
    let range = prepared.byte_range();
    // A placeholder with invalid zero CMS can be structurally covered, NEVER
    // cryptographically marked valid. Public report must keep this distinction.
    let mut bytes = first.to_vec();
    bytes.push(b'<');
    bytes.extend(std::iter::repeat_n(
        b'0',
        (range[2] - range[1] - 2) as usize,
    ));
    bytes.push(b'>');
    bytes.extend_from_slice(second);
    let reopened = Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).unwrap();
    let reports = reopened.signature_reports().unwrap();
    assert_eq!(reports.len(), 1);
    assert!(reports[0].covers_current_document);
    assert_eq!(reports[0].verification, SignatureVerification::NotPerformed);
    assert!(reopened.prepare_signature(Default::default()).is_err());
    // Change a fixed-width ByteRange number without changing xref offsets.
    let old = format!(
        "[{:020} {:020} {:020} {:020}]",
        range[0], range[1], range[2], range[3]
    );
    let changed = format!(
        "[{:020} {:020} {:020} {:020}]",
        range[0],
        range[1] - 1,
        range[2],
        range[3]
    );
    let index = bytes
        .windows(old.len())
        .position(|w| w == old.as_bytes())
        .unwrap();
    bytes[index..index + old.len()].copy_from_slice(changed.as_bytes());
    let altered = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert!(altered.signature_reports().is_err());
}
