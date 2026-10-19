use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        cross_ref_table::{CrossRefTable, ObjectState},
        metadata::InfoTextField,
        save_options::{
            IncrementalSaveLayout as Layout, IncrementalSaveOptions, PdfVersion, SaveLayout,
        },
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference,
        pdf_string::PdfString,
    },
    parser::{
        options::{Limits, ParseOptions},
        parser::parse_xref,
        syntax::{ParseMode, SyntaxParser},
    },
    security::{EncryptionOptions, SaveSecurity},
};
use std::io::{Cursor, Write};

fn options(limits: Limits) -> ParseOptions {
    ParseOptions {
        mode: ParseMode::Strict,
        limits,
    }
}
fn open(bytes: Vec<u8>) -> Document {
    Document::from_bytes_with_options(bytes, options(Default::default())).unwrap()
}
fn xref(bytes: &[u8]) -> CrossRefTable {
    let mut parser = SyntaxParser::with_mode(
        StreamReader::try_new(Cursor::new(bytes)).unwrap(),
        ParseMode::Strict,
    );
    parser.read_header().unwrap();
    parse_xref(&mut parser).unwrap()
}
fn last(bytes: &[u8]) -> usize {
    let start = bytes.windows(9).rposition(|s| s == b"startxref").unwrap();
    std::str::from_utf8(&bytes[start + 9..])
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}
fn base() -> Document {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.set_info_text(InfoTextField::Title, Some("before"))
        .unwrap();
    doc
}
fn save(doc: &Document) -> Vec<u8> {
    doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream)
        .unwrap()
}
fn string(value: &str) -> PdfObject {
    PdfObject::PdfString(PdfString::new(value.as_bytes().to_vec(), false))
}

#[test]
fn old_options_literals_and_classic_defaults_are_unchanged() {
    let source = base().save_to_bytes().unwrap();
    let doc = open(source);
    doc.set_info_text(InfoTextField::Title, Some("after"))
        .unwrap();
    let options = IncrementalSaveOptions {
        max_output_bytes: 1_000_000,
    };
    assert_eq!(
        doc.save_incremental_to_bytes_with_options(options).unwrap(),
        doc.save_incremental_to_bytes_with_layout(options, Layout::default())
            .unwrap()
    );
}

#[test]
fn sparse_tail_has_self_entry_correct_prev_and_no_object_streams() {
    let source = base().save_to_bytes().unwrap();
    let old = xref(&source);
    let doc = open(source.clone());
    doc.set_info_text(InfoTextField::Title, Some("after"))
        .unwrap();
    let stamp = doc.stamp();
    let revision = doc.commit_revision();
    let bytes = save(&doc);
    assert!(bytes.starts_with(&source));
    let table = xref(&bytes);
    assert_eq!(
        table.trailer().get("Prev").unwrap().as_u32(),
        Some(last(&source) as u32)
    );
    assert!(table.trailer().get("XRefStm").is_none());
    assert!(table.trailer().get("Filter").is_none());
    let size = table.trailer().get("Size").unwrap().as_u32().unwrap();
    assert!(size > old.trailer().get("Size").unwrap().as_u32().unwrap());
    let id = ObjectId::new(size - 1, 0);
    assert_eq!(table.lookup_id(id).unwrap().offset(), last(&bytes) as u64);
    let reopened = open(bytes.clone());
    let object = reopened.resolve_object(id).unwrap();
    let stream = object.as_stream().unwrap();
    let count: usize = stream
        .dict()
        .get("Index")
        .unwrap()
        .as_array()
        .unwrap()
        .into_iter()
        .skip(1)
        .step_by(2)
        .map(|v| v.as_u32().unwrap() as usize)
        .sum();
    assert!(count < size as usize);
    assert_eq!(stream.raw_data().len(), count * 13);
    assert!(!bytes[source.len()..].windows(7).any(|s| s == b"/ObjStm"));
    assert_eq!(doc.stamp(), stamp);
    assert_eq!(doc.commit_revision(), revision);
    assert!(doc.resolve_object(id).is_err());
    assert_eq!(save(&doc), bytes);
    assert_eq!(
        reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("after")
    );
}

#[test]
fn no_op_and_rollback_are_exact_even_for_old_versions() {
    let mut bytes = Document::new_with_version(PdfVersion::V1_4)
        .unwrap()
        .save_to_bytes()
        .unwrap();
    bytes.extend_from_slice(b"\r\n \t");
    let doc = open(bytes.clone());
    assert_eq!(save(&doc), bytes);
    let result: Result<(), PdfError> = doc.edit_transaction(|d| {
        d.set_info_text(InfoTextField::Title, Some("rollback"))?;
        Err(PdfError::DocumentError("rollback".into()))
    });
    assert!(result.is_err());
    assert_eq!(save(&doc), bytes);
    doc.set_info_text(InfoTextField::Title, Some("changed"))
        .unwrap();
    let mut host = vec![42];
    assert!(
        doc.save_incremental_to_writer_with_layout(
            &mut host,
            Default::default(),
            Layout::XrefStream
        )
        .is_err()
    );
    assert_eq!(host, [42]);
    assert!(doc.save_incremental_to_bytes().is_ok());
}

#[test]
fn three_rounds_cross_layout_and_compressed_members_use_logical_generation_zero() {
    for full in [
        SaveLayout::Classic,
        SaveLayout::XrefStream,
        SaveLayout::XrefStreamWithObjectStreams,
    ] {
        for sequence in [
            [Layout::XrefStream, Layout::Classic, Layout::XrefStream],
            [Layout::Classic, Layout::XrefStream, Layout::Classic],
        ] {
            let doc = base();
            let id = doc.try_insert_object(string("member-before")).unwrap();
            let mut source = doc
                .save_to_bytes_with_layout(Default::default(), full)
                .unwrap();
            for (round, layout) in sequence.into_iter().enumerate() {
                let doc = open(source.clone());
                doc.update_object(id, string("member-after")).unwrap();
                let title = format!("round {round}");
                doc.set_info_text(InfoTextField::Title, Some(&title))
                    .unwrap();
                let next = doc
                    .save_incremental_to_bytes_with_layout(Default::default(), layout)
                    .unwrap();
                assert!(next.starts_with(&source));
                let table = xref(&next);
                assert_eq!(
                    table.trailer().get("Prev").unwrap().as_u32(),
                    Some(last(&source) as u32)
                );
                let entry = table.lookup_id(id).unwrap();
                assert!(matches!(entry.state(), ObjectState::Normal));
                assert_eq!(entry.gennum(), 0);
                assert!(entry.offset() >= source.len() as u64);
                assert_eq!(
                    open(next.clone()).resolve_object(id).unwrap(),
                    string("member-after")
                );
                source = next;
            }
        }
    }
}

fn sparse(generation: u32, extra: &str, size: u32) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (id, body) in [
        (
            "1 0".to_owned(),
            format!("<< /Type /Catalog /Pages 2 0 R {extra} >>"),
        ),
        ("2 0".into(), "<< /Type /Pages /Kids [] /Count 0 >>".into()),
        (format!("3 {generation}"), "(unused)".into()),
    ] {
        offsets.push(out.len());
        out.extend_from_slice(format!("{id} obj\n{body}\nendobj\n").as_bytes());
    }
    let pos = out.len();
    out.extend_from_slice(format!("xref\n0 5\n0000000004 65535 f \n{:010} 00000 n \n{:010} 00000 n \n{:010} {generation:05} n \n0000000000 00009 f \ntrailer\n<< /Size {size} /Root 1 0 R >>\nstartxref\n{pos}\n%%EOF\n", offsets[0], offsets[1], offsets[2]).as_bytes());
    out
}

#[test]
fn sparse_high_water_free_links_and_generation_saturation_survive() {
    for (generation, expected) in [(7, 8), (65535, 65535)] {
        let doc = open(sparse(generation, "", 50));
        doc.delete_object(ObjectId::new(3, generation)).unwrap();
        let output = save(&doc);
        let table = xref(&output);
        assert_eq!(table.trailer().get("Size").unwrap().as_u32(), Some(51));
        assert_eq!(table.lookup(&0).unwrap().offset(), 3);
        assert_eq!(table.lookup(&3).unwrap().offset(), 4);
        assert_eq!(table.lookup(&3).unwrap().gennum(), expected);
        assert_eq!(table.lookup(&4).unwrap().offset(), 0);
        assert_eq!(table.lookup(&4).unwrap().gennum(), 9);
        assert!(table.lookup(&49).is_none()); // holes inherit, not dense records
        assert_eq!(
            open(output)
                .try_insert_object(PdfObject::PdfNull)
                .unwrap()
                .obj_num(),
            51
        );
    }
}

#[test]
fn source_and_overlay_dangling_refs_cannot_alias_fresh_structural_id() {
    let source = sparse(7, "/Private 50 0 R", 50);
    let doc = open(source);
    doc.delete_object(ObjectId::new(3, 7)).unwrap();
    assert!(matches!(
        doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream),
        Err(PdfError::InvalidReference(_))
    ));
    let doc = open(sparse(7, "", 50));
    doc.update_object(
        ObjectId::new(3, 7),
        PdfObject::PdfReference(PdfReference::from_id(ObjectId::new(50, 0))),
    )
    .unwrap();
    assert!(matches!(
        doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream),
        Err(PdfError::InvalidReference(_))
    ));
    assert!(doc.save_incremental_to_bytes().is_ok());
}

#[test]
fn output_exact_limit_budget_failure_release_and_retry() {
    let source = base().save_to_bytes().unwrap();
    let doc = open(source);
    doc.set_info_text(InfoTextField::Title, Some("changed"))
        .unwrap();
    let expected = save(&doc);
    let stamp = doc.stamp();
    assert_eq!(
        doc.save_incremental_to_bytes_with_layout(
            IncrementalSaveOptions {
                max_output_bytes: expected.len()
            },
            Layout::XrefStream
        )
        .unwrap(),
        expected
    );
    assert!(matches!(
        doc.save_incremental_to_bytes_with_layout(
            IncrementalSaveOptions {
                max_output_bytes: expected.len() - 1
            },
            Layout::XrefStream
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    let large = OperationBudget::new(4 * 1024 * 1024);
    let bytes = doc
        .save_incremental_with_layout_and_budget(Default::default(), Layout::XrefStream, &large)
        .unwrap();
    assert!(large.live_bytes() >= bytes.as_slice().len());
    drop(bytes);
    assert_eq!(large.live_bytes(), 0);
    let exact = OperationBudget::new(large.peak_bytes());
    drop(
        doc.save_incremental_with_layout_and_budget(Default::default(), Layout::XrefStream, &exact)
            .unwrap(),
    );
    let small = OperationBudget::new(large.peak_bytes() - 1);
    assert!(matches!(
        doc.save_incremental_with_layout_and_budget(Default::default(), Layout::XrefStream, &small),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(small.live_bytes(), 0);
    assert_eq!(save(&doc), expected);
    assert_eq!(doc.stamp(), stamp);
}

#[test]
fn payload_limits_are_exact_and_size_overflow_rejects() {
    let source = sparse(7, "", 50);
    // object 0, newly freed 3, existing free 4, self 50
    for field in ["raw", "decoded"] {
        for max in [52, 51] {
            let mut limits = Limits::default();
            if field == "raw" {
                limits.max_raw_stream_bytes = max;
            } else {
                limits.max_decoded_bytes = max;
            }
            let doc = Document::from_bytes_with_options(source.clone(), options(limits)).unwrap();
            doc.delete_object(ObjectId::new(3, 7)).unwrap();
            let result =
                doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream);
            assert_eq!(result.is_ok(), max == 52, "{field} {max}: {result:?}");
        }
    }
    let doc = open(sparse(7, "", u32::MAX));
    doc.delete_object(ObjectId::new(3, 7)).unwrap();
    assert!(matches!(
        doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn signed_and_non_strict_or_non_owned_sources_remain_rejected() {
    let source = base().save_to_bytes().unwrap();
    for doc in [
        Document::new().unwrap(),
        Document::from_reader(Cursor::new(source.clone())).unwrap(),
        Document::from_bytes_with_mode(source, ParseMode::Compatible).unwrap(),
    ] {
        assert!(
            doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream)
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
    );
    assert!(
        signed
            .save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream)
            .is_err()
    );
}

#[test]
fn encrypted_three_rounds_keep_encrypt_first_id_permissions_and_decrypt_once() {
    for full in [SaveLayout::Classic, SaveLayout::XrefStreamWithObjectStreams] {
        let doc = base();
        let member = doc.try_insert_object(string("private-before")).unwrap();
        let encryption = EncryptionOptions::new("user", "owner").unwrap();
        let mut source = doc
            .save_to_bytes_with_layout_and_security(
                Default::default(),
                full,
                SaveSecurity::Set(&encryption),
            )
            .unwrap();
        let original = xref(&source).trailer().clone();
        for (round, layout) in [Layout::XrefStream, Layout::Classic, Layout::XrefStream]
            .into_iter()
            .enumerate()
        {
            let password = if round == 1 { "owner" } else { "user" };
            let doc = Document::from_bytes_with_password_and_options(
                source.clone(),
                password,
                options(Default::default()),
            )
            .unwrap();
            assert_eq!(save(&doc), source);
            doc.update_object(member, string("private-after")).unwrap();
            let filename = format!("private-{round}.bin");
            doc.add_attachment(&filename, b"private-payload".to_vec())
                .unwrap();
            let report = doc.security_report().unwrap();
            let output = doc
                .save_incremental_to_bytes_with_layout(Default::default(), layout)
                .unwrap();
            assert!(output.starts_with(&source));
            assert!(!output[source.len()..].windows(7).any(|v| v == b"private"));
            let table = xref(&output);
            assert_eq!(table.trailer().get("Encrypt"), original.get("Encrypt"));
            let ids = table.trailer().get("ID").unwrap().as_array().unwrap();
            assert_eq!(
                ids.get(0),
                original.get("ID").unwrap().as_array().unwrap().get(0)
            );
            assert_ne!(
                ids.get(1),
                xref(&source)
                    .trailer()
                    .get("ID")
                    .unwrap()
                    .as_array()
                    .unwrap()
                    .get(1)
            );
            for role in ["user", "owner"] {
                let next = Document::from_bytes_with_password_and_options(
                    output.clone(),
                    role,
                    options(Default::default()),
                )
                .unwrap();
                assert_eq!(
                    next.resolve_object(member).unwrap(),
                    string("private-after")
                );
                assert_eq!(
                    next.attachment_data(&filename).unwrap().unwrap(),
                    b"private-payload"
                );
                assert_eq!(
                    next.security_report().unwrap().permissions,
                    report.permissions
                );
            }
            assert!(
                Document::from_bytes_with_password_and_options(
                    output.clone(),
                    "wrong",
                    options(Default::default())
                )
                .is_err()
            );
            source = output;
        }
    }
}

#[test]
fn structural_containers_and_live_deleted_references_are_still_rejected() {
    let source = base()
        .save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStreamWithObjectStreams)
        .unwrap();
    let table = xref(&source);
    let size = table.trailer().get("Size").unwrap().as_u32().unwrap();
    for delete in [false, true] {
        let doc = open(source.clone());
        let id = ObjectId::new(size - 1, 0);
        if delete {
            doc.delete_object(id).unwrap();
        } else {
            doc.update_object(id, PdfObject::PdfNull).unwrap();
        }
        assert!(
            doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream)
                .is_err()
        );
    }
    let doc = base();
    let id = doc.try_insert_object(string("referenced")).unwrap();
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Ref".into(),
        PdfObject::PdfReference(PdfReference::from_id(id)),
    )
    .unwrap();
    doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
    let doc = open(doc.save_to_bytes().unwrap());
    doc.delete_object(id).unwrap();
    assert!(
        doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream)
            .is_err()
    );
    doc.update_object(id, string("restored")).unwrap();
    assert!(
        doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream)
            .is_ok()
    );
}

#[test]
fn host_writer_failure_leaves_document_retryable() {
    struct Fail;
    impl Write for Fail {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("host error"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let doc = open(base().save_to_bytes().unwrap());
    doc.set_info_text(InfoTextField::Title, Some("changed"))
        .unwrap();
    let expected = save(&doc);
    let stamp = doc.stamp();
    assert!(matches!(
        doc.save_incremental_to_writer_with_layout(
            &mut Fail,
            Default::default(),
            Layout::XrefStream
        ),
        Err(PdfError::IOError(_))
    ));
    assert_eq!(doc.stamp(), stamp);
    assert_eq!(save(&doc), expected);
}

#[test]
fn file_entry_is_atomic_and_trailer_removal_stops_inheritance() {
    let doc = open(base().save_to_bytes().unwrap());
    doc.remove_info_dict().unwrap();
    let output = save(&doc);
    assert!(matches!(
        xref(&output).trailer().get("Info"),
        Some(PdfObject::PdfNull)
    ));
    assert_eq!(open(output).info_text(InfoTextField::Title).unwrap(), None);
    let directory =
        std::env::temp_dir().join(format!("rzpdf-v012-incremental-{}", std::process::id()));
    std::fs::create_dir(&directory).unwrap();
    let target = directory.join("result.pdf");
    std::fs::write(&target, b"existing target").unwrap();
    assert!(
        doc.save_incremental_to_file_with_layout(
            &target,
            IncrementalSaveOptions {
                max_output_bytes: 1
            },
            Layout::XrefStream
        )
        .is_err()
    );
    assert_eq!(std::fs::read(&target).unwrap(), b"existing target");
    doc.save_incremental_to_file_with_layout(&target, Default::default(), Layout::XrefStream)
        .unwrap();
    assert_eq!(std::fs::read(&target).unwrap(), save(&doc));
    std::fs::remove_file(&target).unwrap();
    std::fs::remove_dir(&directory).unwrap();
}

#[test]
fn explicit_catalog_version_upgrade_and_size_boundary() {
    let doc = Document::new_with_version(PdfVersion::V1_4).unwrap();
    let source = doc.save_to_bytes().unwrap();
    let table = xref(&source);
    let root = table.trailer().get("Root").unwrap().as_object_id().unwrap();
    let doc = open(source.clone());
    let mut catalog = doc.resolve_object(root).unwrap().as_dict().unwrap().clone();
    catalog
        .try_insert(
            "Version".into(),
            PdfObject::PdfName(pdf::objects::pdf_name::PdfName::new("1.5".into())),
        )
        .unwrap();
    doc.update_object(root, PdfObject::PdfDict(catalog))
        .unwrap();
    assert!(save(&doc).starts_with(&source));
    for maximum in [51, 50] {
        let doc = Document::from_bytes_with_options(
            sparse(7, "", 50),
            options(Limits {
                max_objects: maximum,
                ..Default::default()
            }),
        )
        .unwrap();
        doc.delete_object(ObjectId::new(3, 7)).unwrap();
        let result =
            doc.save_incremental_to_bytes_with_layout(Default::default(), Layout::XrefStream);
        assert_eq!(result.is_ok(), maximum == 51);
    }
}

fn compressed_hybrid() -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let pages = bytes.len();
    bytes.extend_from_slice(b"1 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n");
    let container = bytes.len();
    let catalog = b"<< /Type /Catalog /Pages 1 0 R >>";
    let info = b"<< /Title (old) >>";
    let header = format!(
        "2 0 5 {} 6 {} ",
        catalog.len() + 1,
        catalog.len() + info.len() + 2
    );
    let first = header.len();
    let mut payload = header.into_bytes();
    payload.extend_from_slice(catalog);
    payload.push(b' ');
    payload.extend_from_slice(info);
    payload.extend_from_slice(b" (unused)");
    bytes.extend_from_slice(
        format!(
            "3 0 obj\n<< /Type /ObjStm /N 3 /First {first} /Length {} >>\nstream\n",
            payload.len()
        )
        .as_bytes(),
    );
    bytes.extend_from_slice(&payload);
    bytes.extend_from_slice(b"\nendstream\nendobj\n");
    let xref_offset = bytes.len();
    let mut rows = Vec::new();
    for (kind, field, generation) in [
        (0u8, 0u32, 65535u16),
        (1, pages as u32, 0),
        (2, 3, 0),
        (1, container as u32, 0),
        (1, xref_offset as u32, 0),
        (2, 3, 1),
        (2, 3, 2),
    ] {
        rows.push(kind);
        rows.extend_from_slice(&field.to_be_bytes());
        rows.extend_from_slice(&generation.to_be_bytes());
    }
    bytes.extend_from_slice(format!("4 0 obj\n<< /Type /XRef /Size 7 /Root 2 0 R /Info 5 0 R /W [1 4 2] /Length {} >>\nstream\n", rows.len()).as_bytes());
    bytes.extend_from_slice(&rows);
    bytes.extend_from_slice(b"\nendstream\nendobj\n");
    let physical = bytes.len();
    bytes.extend_from_slice(format!("xref\n0 7\n0000000000 65535 f \n{pages:010} 00000 n \n0000000000 00000 f \n{container:010} 00000 n \n{xref_offset:010} 00000 n \n0000000000 00000 f \n0000000000 00000 f \ntrailer\n<< /Size 7 /Root 2 0 R /Info 5 0 R /XRefStm {xref_offset} >>\nstartxref\n{physical}\n%%EOF\n").as_bytes());
    bytes
}

#[test]
fn hybrid_compressed_catalog_and_member_deletions_follow_physical_prev() {
    let mut source = compressed_hybrid();
    for (round, layout) in [Layout::XrefStream, Layout::Classic, Layout::XrefStream]
        .into_iter()
        .enumerate()
    {
        let doc = open(source.clone());
        doc.set_info_text(InfoTextField::Title, Some("new"))
            .unwrap();
        if round == 0 {
            doc.delete_object(ObjectId::new(5, 0)).unwrap();
            doc.delete_object(ObjectId::new(6, 0)).unwrap();
            let mut catalog = doc
                .resolve_object(ObjectId::new(2, 0))
                .unwrap()
                .as_dict()
                .unwrap()
                .clone();
            catalog
                .try_insert("Private".into(), string("catalog changed"))
                .unwrap();
            doc.update_object(ObjectId::new(2, 0), PdfObject::PdfDict(catalog))
                .unwrap();
        }
        let output = doc
            .save_incremental_to_bytes_with_layout(Default::default(), layout)
            .unwrap();
        assert!(output.starts_with(&source));
        let table = xref(&output);
        assert_eq!(
            table.trailer().get("Prev").unwrap().as_u32(),
            Some(last(&source) as u32)
        );
        assert!(table.trailer().get("XRefStm").is_none());
        assert_eq!(table.lookup(&5).unwrap().gennum(), 1);
        assert_eq!(table.lookup(&6).unwrap().gennum(), 1);
        assert!(matches!(
            table.lookup(&2).unwrap().state(),
            ObjectState::Normal
        ));
        assert_eq!(table.lookup(&2).unwrap().gennum(), 0);
        assert_eq!(
            open(output.clone())
                .info_text(InfoTextField::Title)
                .unwrap()
                .as_deref(),
            Some("new")
        );
        source = output;
    }
}

#[test]
fn changed_r6_streams_keep_clear_metadata_and_explicit_identity_exceptions() {
    use pdf::objects::{pdf_name::PdfName, pdf_stream::PdfStream};
    let doc = base();
    let mut settings = EncryptionOptions::new("user", "owner").unwrap();
    settings.encrypt_metadata = false;
    let source = doc
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
        .unwrap();
    let doc = Document::from_bytes_with_password_and_options(
        source.clone(),
        "owner",
        options(Default::default()),
    )
    .unwrap();
    let packet = b"<x>visible clear metadata</x>";
    doc.set_metadata_packet(Some(packet.to_vec())).unwrap();
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
    let stream = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            dict,
            b"explicit-public-stream".to_vec(),
        )))
        .unwrap();
    doc.set_info_text(InfoTextField::Title, Some("private-info-text"))
        .unwrap();
    let output = save(&doc);
    let tail = &output[source.len()..];
    assert!(tail.windows(packet.len()).any(|s| s == packet));
    assert!(tail.windows(22).any(|s| s == b"explicit-public-stream"));
    assert!(!tail.windows(17).any(|s| s == b"private-info-text"));
    let next =
        Document::from_bytes_with_password_and_options(output, "user", options(Default::default()))
            .unwrap();
    assert_eq!(next.metadata_packet().unwrap().unwrap(), packet);
    assert_eq!(
        next.resolve_object(stream)
            .unwrap()
            .as_stream()
            .unwrap()
            .raw_data(),
        b"explicit-public-stream"
    );
}
