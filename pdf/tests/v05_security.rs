use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        acroform::FormValue,
        metadata::InfoTextField,
        save_options::{PdfVersion, SaveOptions},
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_bool::PdfBool, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_string::PdfString,
    },
    parser::{
        options::ParseOptions,
        parser::parse_xref,
        syntax::{ParseMode, SyntaxParser},
    },
    security::{Authentication, EncryptionOptions, SaveSecurity, SecurityHandler},
};
use std::io::Cursor;

fn options() -> ParseOptions {
    ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    }
}
fn open(bytes: Vec<u8>, password: &str) -> Document {
    Document::from_bytes_with_password_and_options(bytes, password, options()).unwrap()
}
fn created() -> Document {
    let document = Document::new().unwrap();
    document.add_page(0).unwrap();
    document
        .replace_page_contents(0, b"0 0 20 20 re f\n".to_vec())
        .unwrap();
    document
        .set_info_text(InfoTextField::Title, Some("Protected 中文"))
        .unwrap();
    document
        .add_attachment("data.bin", (0u8..=33).collect())
        .unwrap();
    document
        .set_metadata_packet(Some(b"<metadata>private packet</metadata>".to_vec()))
        .unwrap();
    document
}
fn encrypt(document: &Document, settings: &EncryptionOptions) -> Vec<u8> {
    document
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(settings))
        .unwrap()
}
fn encryption_dictionary(bytes: &[u8]) -> PdfDict {
    let reader = StreamReader::try_new(Cursor::new(bytes)).unwrap();
    let mut syntax = SyntaxParser::with_mode(reader, ParseMode::Strict);
    syntax.read_header().unwrap();
    let xref = parse_xref(&mut syntax).unwrap();
    let id = xref
        .trailer()
        .get("Encrypt")
        .unwrap()
        .as_object_id()
        .unwrap();
    open(bytes.to_vec(), "owner")
        .resolve_object(id)
        .unwrap()
        .into_dict()
        .unwrap()
}

#[test]
fn granular_high_level_permissions_do_not_collapse_into_modify() {
    let document = created();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.permissions.annotate = false;
    settings.permissions.assemble = false;
    let bytes = encrypt(&document, &settings);
    let user = open(bytes.clone(), "user");
    let stamp = user.stamp();
    assert!(matches!(
        user.add_page(0),
        Err(PdfError::PermissionDenied(_))
    ));
    assert!(matches!(
        user.reorder_pages(&[0]),
        Err(PdfError::PermissionDenied(_))
    ));
    assert!(matches!(
        user.add_flat_bookmark("denied", 0),
        Err(PdfError::PermissionDenied(_))
    ));
    assert!(matches!(
        user.add_square_annotation(0, [1., 1., 10., 10.], [1., 0., 0.]),
        Err(PdfError::PermissionDenied(_))
    ));
    assert!(matches!(
        user.add_uri_link(0, [1., 1., 10., 10.], "https://example.org"),
        Err(PdfError::PermissionDenied(_))
    ));
    assert!(matches!(
        user.add_text_field(0, [1., 1., 80., 30.], "denied", "value"),
        Err(PdfError::PermissionDenied(_))
    ));
    assert!(user.is_stamp_current(&stamp));
    user.set_info_text(InfoTextField::Title, Some("allowed metadata"))
        .unwrap();
    let owner = open(bytes, "owner");
    owner.add_page(0).unwrap();
    owner
        .add_square_annotation(0, [1., 1., 10., 10.], [1., 0., 0.])
        .unwrap();
}

#[test]
fn aes256_r6_roundtrip_reads_passwords_strings_streams_attachments_and_metadata() {
    let document = created();
    let string_id = document
        .try_insert_object(PdfObject::PdfString(PdfString::new(
            b"private literal".to_vec(),
            false,
        )))
        .unwrap();
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    let stamp = document.stamp();
    let output = encrypt(&document, &settings);
    assert!(output.starts_with(b"%PDF-2.0"));
    assert_eq!(document.version(), "1.7");
    assert!(document.security_report().is_none());
    assert!(document.is_stamp_current(&stamp));
    assert!(!output.windows(15).any(|s| s == b"private literal"));
    assert!(!output.windows(14).any(|s| s == b"private packet"));
    assert!(matches!(
        Document::from_bytes_with_options(output.clone(), options()),
        Err(PdfError::PasswordRequired)
    ));
    assert!(matches!(
        Document::from_bytes_with_password_and_options(output.clone(), "wrong", options()),
        Err(PdfError::InvalidPassword)
    ));
    for (password, role) in [
        ("user", Authentication::User),
        ("owner", Authentication::Owner),
    ] {
        let reopened = open(output.clone(), password);
        assert_eq!(reopened.security_report().unwrap().authentication, role);
        assert_eq!(
            reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
            Some("Protected 中文")
        );
        assert_eq!(
            reopened
                .attachment_data("data.bin")
                .unwrap()
                .unwrap()
                .as_slice(),
            (0u8..=33).collect::<Vec<_>>()
        );
        assert_eq!(
            reopened.metadata_packet().unwrap().unwrap(),
            b"<metadata>private packet</metadata>"
        );
        assert_eq!(
            reopened
                .resolve_object(string_id)
                .unwrap()
                .as_string()
                .unwrap()
                .bytes(),
            b"private literal"
        );
        assert_eq!(
            reopened
                .get_page(0)
                .unwrap()
                .display_list()
                .unwrap()
                .objects()
                .len(),
            1
        );
        assert!(reopened.save_incremental_to_bytes().is_err());
    }
}

#[test]
fn independent_encryptions_differ_and_default_save_preserves_protection() {
    let document = created();
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    let first = encrypt(&document, &settings);
    let second = encrypt(&document, &settings);
    assert_ne!(first, second);
    let protected = open(first, "user");
    let stamp = protected.stamp();
    let output = protected.save_to_bytes().unwrap();
    assert!(protected.is_stamp_current(&stamp));
    assert!(matches!(
        Document::from_bytes(output.clone()),
        Err(PdfError::PasswordRequired)
    ));
    assert_eq!(
        open(output, "owner")
            .info_text(InfoTextField::Title)
            .unwrap()
            .as_deref(),
        Some("Protected 中文")
    );
    assert_eq!(open(second, "user").try_page_count().unwrap(), 1);
}

#[test]
fn protection_removal_and_replacement_require_owner_and_do_not_rebase() {
    let bytes = encrypt(
        &created(),
        &EncryptionOptions::new("user", "owner").unwrap(),
    );
    let user = open(bytes.clone(), "user");
    assert!(matches!(
        user.save_to_bytes_with_security(Default::default(), SaveSecurity::Remove),
        Err(PdfError::PermissionDenied(_))
    ));
    let replacement = EncryptionOptions::new("new-user", "new-owner").unwrap();
    assert!(matches!(
        user.save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&replacement)),
        Err(PdfError::PermissionDenied(_))
    ));
    let owner = open(bytes.clone(), "owner");
    let stamp = owner.stamp();
    let plain = owner
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Remove)
        .unwrap();
    let plain = Document::from_bytes_with_options(plain, options()).unwrap();
    assert!(plain.security_report().is_none());
    assert_eq!(
        plain.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("Protected 中文")
    );
    let changed = encrypt(&owner, &replacement);
    assert!(matches!(
        Document::from_bytes_with_password(changed.clone(), "user"),
        Err(PdfError::InvalidPassword)
    ));
    assert_eq!(open(changed, "new-owner").try_page_count().unwrap(), 1);
    assert!(owner.is_stamp_current(&stamp));
    assert_eq!(
        owner.security_report().unwrap().authentication,
        Authentication::Owner
    );
    assert_eq!(
        open(owner.save_to_bytes().unwrap(), "owner")
            .try_page_count()
            .unwrap(),
        1
    );
}

#[test]
fn restricted_user_can_fill_existing_forms_but_cannot_gain_general_edit_access() {
    let document = Document::new().unwrap();
    document.add_page(0).unwrap();
    document
        .add_text_field(0, [10.0, 10.0, 160.0, 40.0], "name", "old")
        .unwrap();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.permissions.modify = false;
    settings.permissions.copy = false;
    let document = open(encrypt(&document, &settings), "user");
    let commit = document.commit_revision();
    assert!(matches!(
        document.add_page(1),
        Err(PdfError::PermissionDenied(_))
    ));
    assert_eq!(document.commit_revision(), commit);
    document
        .set_form_value("name", FormValue::Text("filled".into()))
        .unwrap();
    assert_eq!(document.commit_revision(), commit + 1);
    assert!(matches!(
        document.set_info_text(InfoTextField::Title, Some("forbidden")),
        Err(PdfError::PermissionDenied(_))
    ));
    assert_eq!(document.commit_revision(), commit + 1);
    let reopened = open(document.save_to_bytes().unwrap(), "owner");
    assert_eq!(
        reopened.form_fields().unwrap()[0].value,
        FormValue::Text("filled".into())
    );
    reopened.add_page(1).unwrap();
}

#[test]
fn empty_user_password_is_supported_and_clear_metadata_is_explicit() {
    let document = created();
    let mut settings = EncryptionOptions::new("", "owner").unwrap();
    settings.encrypt_metadata = false;
    let output = encrypt(&document, &settings);
    assert!(output.windows(14).any(|s| s == b"private packet"));
    let reopened = Document::from_bytes_with_options(output, options()).unwrap();
    assert_eq!(
        reopened.security_report().unwrap().authentication,
        Authentication::User
    );
    assert!(!reopened.security_report().unwrap().encrypt_metadata);
    assert_eq!(
        reopened.metadata_packet().unwrap().unwrap(),
        b"<metadata>private packet</metadata>"
    );
}

#[test]
fn unicode_normalization_length_bounds_and_secret_debug_are_explicit() {
    let document = Document::new().unwrap();
    document.add_page(0).unwrap();
    let settings = EncryptionOptions::new("us\u{ad}er", "所有者").unwrap();
    let output = encrypt(&document, &settings);
    assert_eq!(
        open(output.clone(), "user")
            .security_report()
            .unwrap()
            .authentication,
        Authentication::User
    );
    assert_eq!(
        open(output, "所有者")
            .security_report()
            .unwrap()
            .authentication,
        Authentication::Owner
    );
    assert!(!format!("{settings:?}").contains("所有者"));
    assert!(!format!("{settings:?}").contains("us\u{ad}er"));
    assert!(EncryptionOptions::new(&"x".repeat(128), "owner").is_err());
    assert!(EncryptionOptions::new("\u{7}", "owner").is_err());
    assert!(EncryptionOptions::new("user", "").is_err());
    let boundary = EncryptionOptions::new(&"x".repeat(127), "owner").unwrap();
    assert_eq!(
        open(encrypt(&document, &boundary), &"x".repeat(127))
            .try_page_count()
            .unwrap(),
        1
    );
}

#[test]
fn malformed_handler_filters_and_permissions_are_not_reported_as_wrong_password() {
    let output = encrypt(
        &created(),
        &EncryptionOptions::new("user", "owner").unwrap(),
    );
    let dictionary = encryption_dictionary(&output);
    for key in ["O", "U", "OE", "UE", "Perms"] {
        let mut broken = dictionary.clone();
        broken
            .try_insert(
                key.into(),
                PdfObject::PdfString(PdfString::new(vec![0; 3], true)),
            )
            .unwrap();
        assert!(
            matches!(
                SecurityHandler::try_new(&broken, PdfArray::default(), Some(b"owner")),
                Err(PdfError::MalformedSecurity(_))
            ),
            "{key}"
        );
    }
    let mut unsupported = dictionary.clone();
    unsupported
        .try_insert("R".into(), PdfObject::PdfNumber(PdfNumber::new(5.0)))
        .unwrap();
    assert!(matches!(
        SecurityHandler::try_new(&unsupported, PdfArray::default(), Some(b"owner")),
        Err(PdfError::UnsupportedSecurity(_))
    ));
    let mut mismatch = dictionary.clone();
    mismatch
        .try_insert(
            "EncryptMetadata".into(),
            PdfObject::PdfBool(PdfBool::new(b"false")),
        )
        .unwrap();
    assert!(matches!(
        SecurityHandler::try_new(&mismatch, PdfArray::default(), Some(b"owner")),
        Err(PdfError::MalformedSecurity(_))
    ));
    let mut mismatch = dictionary.clone();
    mismatch
        .try_insert("P".into(), PdfObject::PdfNumber(PdfNumber::new(-12.0)))
        .unwrap();
    assert!(matches!(
        SecurityHandler::try_new(&mismatch, PdfArray::default(), Some(b"owner")),
        Err(PdfError::MalformedSecurity(_))
    ));
    let mut malformed = dictionary.clone();
    malformed
        .try_insert("P".into(), PdfObject::PdfNumber(PdfNumber::new(3.0)))
        .unwrap();
    assert!(matches!(
        SecurityHandler::try_new(&malformed, PdfArray::default(), Some(b"owner")),
        Err(PdfError::MalformedSecurity(_))
    ));
    let mut malformed = dictionary;
    malformed
        .try_insert(
            "StrF".into(),
            PdfObject::PdfName(PdfName::new("MissingCF".into())),
        )
        .unwrap();
    assert!(matches!(
        SecurityHandler::try_new(&malformed, PdfArray::default(), Some(b"owner")),
        Err(PdfError::UnsupportedSecurity(_))
    ));
}

#[test]
fn security_budget_and_output_failures_do_not_commit_and_can_be_retried() {
    let document = created();
    let stamp = document.stamp();
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    let budget = OperationBudget::new(1);
    assert!(matches!(
        document.save_with_security_and_budget(
            Default::default(),
            SaveSecurity::Set(&settings),
            &budget
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(budget.live_bytes(), 0);
    assert!(document.is_stamp_current(&stamp));
    assert!(matches!(
        document.save_to_bytes_with_security(
            SaveOptions {
                max_output_bytes: 1,
                ..Default::default()
            },
            SaveSecurity::Set(&settings)
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(document.is_stamp_current(&stamp));
    assert!(
        document
            .save_to_bytes_with_security(
                SaveOptions {
                    version: Some(PdfVersion::V1_7),
                    ..Default::default()
                },
                SaveSecurity::Set(&settings)
            )
            .is_err()
    );
    let budget = OperationBudget::new(2 * 1024 * 1024);
    let output = document
        .save_with_security_and_budget(Default::default(), SaveSecurity::Set(&settings), &budget)
        .unwrap();
    assert_eq!(budget.live_bytes(), output.capacity());
    drop(output);
    assert_eq!(budget.live_bytes(), 0);
    assert_eq!(
        open(encrypt(&document, &settings), "owner")
            .try_page_count()
            .unwrap(),
        1
    );
}
