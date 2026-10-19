use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        cross_ref_table::{CrossRefTable, ObjectState},
        metadata::InfoTextField,
        save_options::{PdfVersion, SaveLayout, SaveOptions, StreamCompression},
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
    security::{Authentication, EncryptionOptions, SaveSecurity},
};
use std::io::{Cursor, Write};

const LAYOUTS: [SaveLayout; 2] = [
    SaveLayout::XrefStream,
    SaveLayout::XrefStreamWithObjectStreams,
];
fn options() -> ParseOptions {
    ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    }
}
fn open(bytes: Vec<u8>, password: &str) -> Document {
    Document::from_bytes_with_password_and_options(bytes, password, options()).unwrap()
}
fn xref(bytes: &[u8]) -> CrossRefTable {
    let mut parser = SyntaxParser::with_mode(
        StreamReader::try_new(Cursor::new(bytes)).unwrap(),
        ParseMode::Strict,
    );
    parser.read_header().unwrap();
    parse_xref(&mut parser).unwrap()
}
fn created() -> Document {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.set_info_text(InfoTextField::Title, Some("R6 layout 中文"))
        .unwrap();
    doc.add_attachment("test.bin", vec![0, 1, 2, 255]).unwrap();
    doc.set_metadata_packet(Some(b"<metadata>layout clear marker</metadata>".to_vec()))
        .unwrap();
    doc.replace_page_contents(0, b"0 0 1 rg 20 20 30 40 re f".to_vec())
        .unwrap();
    doc
}
fn check_projection(doc: &Document) {
    assert_eq!(doc.try_page_count().unwrap(), 1);
    assert_eq!(
        doc.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("R6 layout 中文")
    );
    assert_eq!(
        doc.attachment_data("test.bin").unwrap(),
        Some(vec![0, 1, 2, 255])
    );
    assert_eq!(
        doc.metadata_packet().unwrap().as_deref(),
        Some(b"<metadata>layout clear marker</metadata>".as_slice())
    );
}
fn protect(doc: &Document, layout: SaveLayout, settings: &EncryptionOptions) -> Vec<u8> {
    doc.save_to_bytes_with_layout_and_security(
        Default::default(),
        layout,
        SaveSecurity::Set(settings),
    )
    .unwrap()
}

#[test]
fn new_r6_layouts_reopen_for_user_owner_and_keep_xref_and_encrypt_clear() {
    let doc = created();
    let stamp = doc.stamp();
    let commit = doc.commit_revision();
    let id = doc
        .try_insert_object(PdfObject::PdfString(
            PdfString::from_text("packed string is encrypted once").unwrap(),
        ))
        .unwrap();
    let stamp = {
        assert!(!doc.is_stamp_current(&stamp));
        doc.stamp()
    };
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    for layout in LAYOUTS {
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let bytes = doc
                .save_to_bytes_with_layout_and_security(
                    SaveOptions {
                        compression,
                        ..Default::default()
                    },
                    layout,
                    SaveSecurity::Set(&settings),
                )
                .unwrap();
            assert!(bytes.starts_with(b"%PDF-2.0"));
            assert!(matches!(
                Document::from_bytes_with_password_and_options(bytes.clone(), "wrong", options()),
                Err(PdfError::InvalidPassword)
            ));
            let table = xref(&bytes); // No password needed to decode the xref.
            let encrypt_id = table
                .trailer()
                .get("Encrypt")
                .unwrap()
                .as_object_id()
                .unwrap();
            assert!(matches!(
                table.lookup(&encrypt_id.obj_num()).unwrap().state(),
                ObjectState::Normal
            ));
            let mut parser = SyntaxParser::with_mode(
                StreamReader::try_new(Cursor::new(&bytes)).unwrap(),
                ParseMode::Strict,
            );
            parser
                .set_pos(table.lookup(&encrypt_id.obj_num()).unwrap().offset())
                .unwrap();
            let encrypt = parser.get_indirect_object().unwrap().to_obj();
            assert_eq!(
                encrypt.as_dict().unwrap().get("R").unwrap().as_u32(),
                Some(6)
            );
            for password in ["user", "owner"] {
                let reopened = open(bytes.clone(), password);
                check_projection(&reopened);
                assert_eq!(
                    reopened.resolve_object(id).unwrap(),
                    doc.resolve_object(id).unwrap()
                );
                assert_eq!(
                    reopened.security_report().unwrap().authentication,
                    if password == "user" {
                        Authentication::User
                    } else {
                        Authentication::Owner
                    }
                );
            }
            assert_eq!(
                matches!(
                    table.lookup(&id.obj_num()).unwrap().state(),
                    ObjectState::Compressed
                ),
                layout == SaveLayout::XrefStreamWithObjectStreams
            );
        }
    }
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.commit_revision(), commit + 1);
}

#[test]
fn preserve_remove_replace_follow_owner_rules_and_encrypt_identity() {
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    let source = protect(&created(), SaveLayout::Classic, &settings);
    let source_table = xref(&source);
    let old_encrypt = source_table
        .trailer()
        .get("Encrypt")
        .unwrap()
        .as_object_id()
        .unwrap();
    let replacement = EncryptionOptions::new("new-user", "new-owner").unwrap();
    for layout in LAYOUTS {
        for role in ["user", "owner"] {
            let doc = open(source.clone(), role);
            let stamp = doc.stamp();
            let kept = doc
                .save_to_bytes_with_layout(Default::default(), layout)
                .unwrap();
            assert_eq!(
                xref(&kept).trailer().get("Encrypt").unwrap().as_object_id(),
                Some(old_encrypt)
            );
            check_projection(&open(kept.clone(), "owner"));
            let after = open(kept, "user");
            assert_eq!(
                after.resolve_object(old_encrypt).unwrap(),
                doc.resolve_object(old_encrypt).unwrap()
            );
            for policy in [SaveSecurity::Remove, SaveSecurity::Set(&replacement)] {
                let mut host = b"keep".to_vec();
                let result = doc.save_to_writer_with_layout_and_security(
                    &mut host,
                    Default::default(),
                    layout,
                    policy,
                );
                if role == "user" {
                    assert!(matches!(result, Err(PdfError::PermissionDenied(_))));
                    assert_eq!(host, b"keep");
                } else {
                    result.unwrap();
                    let bytes = host[4..].to_vec();
                    let password = if matches!(policy, SaveSecurity::Remove) {
                        ""
                    } else {
                        "new-user"
                    };
                    let reopened = open(bytes.clone(), password);
                    check_projection(&reopened);
                    assert_eq!(
                        reopened.security_report().is_some(),
                        !matches!(policy, SaveSecurity::Remove)
                    );
                    if matches!(policy, SaveSecurity::Set(_)) {
                        assert!(matches!(
                            Document::from_bytes_with_password_and_options(
                                bytes,
                                "user",
                                options()
                            ),
                            Err(PdfError::InvalidPassword)
                        ));
                    }
                }
            }
            assert!(doc.is_stamp_current(&stamp));
        }
    }
}

#[test]
fn clear_metadata_and_explicit_identity_stream_remain_clear() {
    let doc = created();
    let mut dict = PdfDict::default();
    dict.try_insert(
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
    dict.try_insert("DecodeParms".into(), PdfObject::PdfDict(params))
        .unwrap();
    let payload = b"explicit-identity-stream-marker".to_vec();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, payload.clone())))
        .unwrap();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.encrypt_metadata = false;
    for layout in LAYOUTS {
        let bytes = protect(&doc, layout, &settings);
        assert!(bytes.windows(payload.len()).any(|w| w == payload));
        assert!(
            bytes
                .windows(b"layout clear marker".len())
                .any(|w| w == b"layout clear marker")
        );
        let reopened = open(bytes, "owner");
        check_projection(&reopened);
        assert!(!reopened.security_report().unwrap().encrypt_metadata);
        assert_eq!(
            reopened
                .resolve_object(id)
                .unwrap()
                .as_stream()
                .unwrap()
                .raw_data(),
            payload
        );
    }
}

#[test]
fn packed_r6_sources_support_repacking_removal_and_member_edits() {
    let doc = created();
    let id = doc
        .try_insert_object(PdfObject::PdfString(
            PdfString::from_text("before").unwrap(),
        ))
        .unwrap();
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    let source = protect(&doc, SaveLayout::XrefStreamWithObjectStreams, &settings);
    assert!(matches!(
        xref(&source).lookup(&id.obj_num()).unwrap().state(),
        ObjectState::Compressed
    ));
    for layout in LAYOUTS {
        let reopened = open(source.clone(), "owner");
        let value = PdfObject::PdfString(PdfString::from_text("after 中文").unwrap());
        reopened.update_object(id, value.clone()).unwrap();
        let stamp = reopened.stamp();
        for policy in [SaveSecurity::Preserve, SaveSecurity::Remove] {
            let bytes = reopened
                .save_to_bytes_with_layout_and_security(Default::default(), layout, policy)
                .unwrap();
            let after = open(
                bytes,
                if matches!(policy, SaveSecurity::Remove) {
                    ""
                } else {
                    "owner"
                },
            );
            assert_eq!(after.resolve_object(id).unwrap(), value);
            check_projection(&after);
        }
        assert!(reopened.is_stamp_current(&stamp));
    }
}

#[test]
fn version_rules_and_source_signature_guard_are_not_relaxed() {
    let doc = created();
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    for layout in LAYOUTS {
        assert!(matches!(
            doc.save_to_bytes_with_layout_and_security(
                SaveOptions {
                    version: Some(PdfVersion::V1_7),
                    ..Default::default()
                },
                layout,
                SaveSecurity::Set(&settings)
            ),
            Err(PdfError::UnsupportedSecurity(_))
        ));
        let bytes = protect(&doc, layout, &settings);
        let reopened = open(bytes, "owner");
        assert!(
            reopened
                .save_to_bytes_with_layout(
                    SaveOptions {
                        version: Some(PdfVersion::V1_7),
                        ..Default::default()
                    },
                    layout
                )
                .is_err()
        );
    }
    let hex: Vec<_> = include_str!("fixtures/v011/workflow-certified.pdf.hex")
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let signed = open(
        hex.chunks_exact(2)
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect(),
        "",
    );
    for layout in LAYOUTS {
        for policy in [
            SaveSecurity::Preserve,
            SaveSecurity::Remove,
            SaveSecurity::Set(&settings),
        ] {
            assert!(
                signed
                    .save_to_bytes_with_layout_and_security(Default::default(), layout, policy)
                    .is_err()
            );
        }
    }
}

#[test]
fn encryption_budget_output_and_host_failures_release_and_allow_retry() {
    let source = protect(
        &created(),
        SaveLayout::Classic,
        &EncryptionOptions::new("user", "owner").unwrap(),
    );
    for layout in LAYOUTS {
        let doc = open(source.clone(), "owner");
        let stamp = doc.stamp();
        let pool = OperationBudget::new(32 * 1024 * 1024);
        let result = doc
            .save_with_layout_and_security_and_budget(
                Default::default(),
                layout,
                SaveSecurity::Preserve,
                &pool,
            )
            .unwrap();
        assert!(pool.live_bytes() >= result.as_slice().len());
        drop(result);
        assert_eq!(pool.live_bytes(), 0);
        let tiny = OperationBudget::new(1);
        assert!(matches!(
            doc.save_with_layout_and_security_and_budget(
                Default::default(),
                layout,
                SaveSecurity::Preserve,
                &tiny
            ),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(tiny.live_bytes(), 0);
        assert!(matches!(
            doc.save_to_bytes_with_layout(
                SaveOptions {
                    max_output_bytes: 20,
                    ..Default::default()
                },
                layout
            ),
            Err(PdfError::ResourceLimit(_))
        ));
        struct Fail;
        impl Write for Fail {
            fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
                Err(std::io::Error::other("host failure"))
            }
            fn flush(&mut self) -> std::io::Result<()> {
                Ok(())
            }
        }
        assert!(
            doc.save_to_writer_with_layout_and_security(
                &mut Fail,
                Default::default(),
                layout,
                SaveSecurity::Preserve
            )
            .is_err()
        );
        check_projection(&open(
            doc.save_to_bytes_with_layout(Default::default(), layout)
                .unwrap(),
            "owner",
        ));
        assert!(doc.is_stamp_current(&stamp));
    }
}

#[test]
fn packed_layout_does_not_bypass_security_dictionary_barriers() {
    for field in ["DocMDP", "FieldMDP", "SigFlags"] {
        let doc = Document::new().unwrap();
        let mut dict = PdfDict::default();
        dict.try_insert(
            field.into(),
            PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(1.)),
        )
        .unwrap();
        doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
        let settings = EncryptionOptions::new("user", "owner").unwrap();
        assert!(matches!(
            doc.save_to_bytes_with_layout_and_security(
                Default::default(),
                SaveLayout::XrefStreamWithObjectStreams,
                SaveSecurity::Set(&settings)
            ),
            Err(PdfError::UnsupportedSecurity(_))
        ));
    }
}

#[test]
fn fresh_encrypt_id_cannot_capture_a_dangling_input_reference() {
    let doc = Document::new().unwrap();
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Dangling".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::new(5, 0)),
    )
    .unwrap();
    doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap(); // xref 4, fresh Encrypt 5
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    for layout in LAYOUTS {
        assert!(matches!(
            doc.save_to_bytes_with_layout_and_security(
                Default::default(),
                layout,
                SaveSecurity::Set(&settings)
            ),
            Err(PdfError::InvalidReference(_))
        ));
    }
}

// The seed has no strings or streams outside Encrypt and trailer IDs. Changing
// its filter selectors therefore changes no existing object's encoding. Encrypt
// is the final body object, so only startxref moves; all xref offsets stay valid.
fn identity_seed(strings_identity: bool, streams_identity: bool) -> Vec<u8> {
    let doc = Document::new().unwrap();
    let mut bytes = protect(
        &doc,
        SaveLayout::Classic,
        &EncryptionOptions::new("user", "owner").unwrap(),
    );
    let table = xref(&bytes);
    let id = table
        .trailer()
        .get("Encrypt")
        .unwrap()
        .as_object_id()
        .unwrap();
    let offset = table.lookup(&id.obj_num()).unwrap().offset() as usize;
    assert!(
        table
            .objects()
            .values()
            .filter(|v| matches!(v.state(), ObjectState::Normal))
            .all(|v| v.offset() as usize <= offset)
    );
    let footer = bytes
        .windows(b"startxref\n".len())
        .rposition(|w| w == b"startxref\n")
        .unwrap();
    let old_xref = std::str::from_utf8(&bytes[footer + b"startxref\n".len()..])
        .unwrap()
        .lines()
        .next()
        .unwrap()
        .parse::<usize>()
        .unwrap();
    let mut delta = 0;
    for (key, identity) in [("StrF", strings_identity), ("StmF", streams_identity)] {
        if !identity {
            continue;
        }
        let needle = format!("/{key} /StdCF");
        let replacement = format!("/{key} /Identity");
        let position = offset
            + bytes[offset..old_xref + delta]
                .windows(needle.len())
                .position(|w| w == needle.as_bytes())
                .unwrap();
        delta += replacement.len() - needle.len();
        bytes.splice(position..position + needle.len(), replacement.bytes());
    }
    let footer = bytes
        .windows(b"startxref\n".len())
        .rposition(|w| w == b"startxref\n")
        .unwrap();
    bytes.truncate(footer);
    write!(bytes, "startxref\n{}\n%%EOF\n", old_xref + delta).unwrap();
    bytes
}

#[test]
fn differing_default_string_and_stream_filters_do_not_change_string_protection() {
    for (strings_identity, streams_identity) in [(true, false), (false, true), (true, true)] {
        let doc = open(identity_seed(strings_identity, streams_identity), "owner");
        let marker = b"identity-policy-string-marker";
        let id = doc
            .try_insert_object(PdfObject::PdfString(PdfString::new(marker.to_vec(), false)))
            .unwrap();
        let mut nested = PdfDict::default();
        nested
            .try_insert(
                "Value".into(),
                PdfObject::PdfString(PdfString::new(marker.to_vec(), false)),
            )
            .unwrap();
        let nested_id = doc.try_insert_object(PdfObject::PdfDict(nested)).unwrap();
        let bytes = doc
            .save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStreamWithObjectStreams)
            .unwrap();
        let table = xref(&bytes);
        for id in [id, nested_id] {
            assert_eq!(
                matches!(
                    table.lookup(&id.obj_num()).unwrap().state(),
                    ObjectState::Compressed
                ),
                strings_identity == streams_identity
            );
        }
        assert_eq!(
            bytes.windows(marker.len()).any(|w| w == marker),
            strings_identity
        );
        let after = open(bytes, "user");
        assert_eq!(
            after.resolve_object(id).unwrap(),
            doc.resolve_object(id).unwrap()
        );
        assert_eq!(
            after.resolve_object(nested_id).unwrap(),
            doc.resolve_object(nested_id).unwrap()
        );
        let report = after.security_report().unwrap();
        assert_eq!(report.strings_use_aes256, !strings_identity);
        assert_eq!(report.streams_use_aes256, !streams_identity);
    }
}

#[test]
fn restricted_users_keep_permissions_and_owner_overlay_cannot_disable_preservation() {
    let doc = created();
    doc.add_text_field(0, [10., 10., 100., 30.], "name", "old")
        .unwrap();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.permissions.modify = false;
    let source = protect(&doc, SaveLayout::Classic, &settings);
    for layout in LAYOUTS {
        let user = open(source.clone(), "user");
        assert!(matches!(
            user.set_info_text(InfoTextField::Title, Some("denied")),
            Err(PdfError::PermissionDenied(_))
        ));
        user.set_form_value(
            "name",
            pdf::document::acroform::FormValue::Text("filled".into()),
        )
        .unwrap();
        let bytes = user
            .save_to_bytes_with_layout(Default::default(), layout)
            .unwrap();
        let after = open(bytes, "owner");
        assert_eq!(
            after.form_fields().unwrap()[0].value,
            pdf::document::acroform::FormValue::Text("filled".into())
        );
        assert!(!after.security_report().unwrap().permissions.modify);
        let owner = open(source.clone(), "owner");
        let id = xref(&source)
            .trailer()
            .get("Encrypt")
            .unwrap()
            .as_object_id()
            .unwrap();
        owner.update_object(id, PdfObject::PdfNull).unwrap();
        let after = open(
            owner
                .save_to_bytes_with_layout(Default::default(), layout)
                .unwrap(),
            "user",
        );
        assert!(after.security_report().is_some());
        assert!(!after.security_report().unwrap().permissions.modify);
    }
}

#[test]
fn empty_user_password_and_combined_file_entry_point_work() {
    let doc = created();
    let settings = EncryptionOptions::new("", "owner").unwrap();
    let directory =
        std::env::temp_dir().join(format!("rzpdf-v012-layout-security-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let target = directory.join("result.pdf");
    std::fs::write(&target, b"existing target").unwrap();
    for layout in LAYOUTS {
        doc.save_to_file_with_layout_and_security(
            &target,
            Default::default(),
            layout,
            SaveSecurity::Set(&settings),
        )
        .unwrap();
        check_projection(&open(std::fs::read(&target).unwrap(), ""));
        let before = std::fs::read(&target).unwrap();
        assert!(
            doc.save_to_file_with_layout_and_security(
                &target,
                SaveOptions {
                    max_output_bytes: 1,
                    ..Default::default()
                },
                layout,
                SaveSecurity::Set(&settings)
            )
            .is_err()
        );
        assert_eq!(std::fs::read(&target).unwrap(), before);
    }
    std::fs::remove_file(&target).unwrap();
    std::fs::remove_dir(&directory).unwrap();
}

#[test]
fn nonzero_encrypt_generation_keeps_matching_header_and_reference() {
    let doc = Document::new().unwrap();
    let mut bytes = protect(
        &doc,
        SaveLayout::Classic,
        &EncryptionOptions::new("user", "owner").unwrap(),
    );
    let table = xref(&bytes);
    let id = table
        .trailer()
        .get("Encrypt")
        .unwrap()
        .as_object_id()
        .unwrap();
    let offset = table.lookup(&id.obj_num()).unwrap().offset();
    for (needle, replacement) in [
        (
            format!("{} 0 obj", id.obj_num()),
            format!("{} 7 obj", id.obj_num()),
        ),
        (
            format!("/Encrypt {} 0 R", id.obj_num()),
            format!("/Encrypt {} 7 R", id.obj_num()),
        ),
        (
            format!("{offset:010} 00000 n "),
            format!("{offset:010} 00007 n "),
        ),
    ] {
        let pos = bytes
            .windows(needle.len())
            .position(|w| w == needle.as_bytes())
            .unwrap();
        assert_eq!(needle.len(), replacement.len());
        bytes.splice(pos..pos + needle.len(), replacement.bytes());
    }
    let doc = open(bytes, "owner");
    for layout in LAYOUTS {
        let output = doc
            .save_to_bytes_with_layout(Default::default(), layout)
            .unwrap();
        let table = xref(&output);
        let enc = table
            .trailer()
            .get("Encrypt")
            .unwrap()
            .as_object_id()
            .unwrap();
        assert_eq!(enc.gen_num(), 7);
        let offset = table.lookup(&enc.obj_num()).unwrap().offset() as usize;
        assert!(output[offset..].starts_with(format!("{} 7 obj", enc.obj_num()).as_bytes()));
        assert!(open(output, "user").security_report().is_some());
    }
}
