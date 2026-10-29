use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        metadata::InfoTextField,
        save_options::{PdfVersion, SaveLayout, SaveOptions, StreamCompression},
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference},
    parser::{
        options::{Limits, ParseOptions},
        parser::parse_xref,
        syntax::{ParseMode, SyntaxParser},
    },
    security::{EncryptionOptions, SaveSecurity},
};
use std::io::{Cursor, Write};

fn created() -> Document {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.set_info_text(InfoTextField::Title, Some("xref output"))
        .unwrap();
    doc.replace_page_contents(0, b"0 0 1 rg 20 20 50 30 re f".to_vec())
        .unwrap();
    doc
}
fn open(bytes: Vec<u8>) -> Document {
    Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap()
}
fn xref(bytes: &[u8]) -> pdf::document::cross_ref_table::CrossRefTable {
    let mut parser = SyntaxParser::with_mode(
        StreamReader::try_new(Cursor::new(bytes)).unwrap(),
        ParseMode::Strict,
    );
    parser.read_header().unwrap();
    parse_xref(&mut parser).unwrap()
}
fn stream(doc: &Document) -> Vec<u8> {
    doc.save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStream)
        .unwrap()
}

#[test]
fn classic_default_and_existing_options_literals_keep_exact_bytes() {
    let doc = created();
    let stamp = doc.stamp();
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        // Keep the original exhaustive public literal source-compatible.
        let options = SaveOptions {
            version: None,
            compression,
            max_output_bytes: 1_000_000,
        };
        assert_eq!(
            doc.save_to_bytes_with_options(options).unwrap(),
            doc.save_to_bytes_with_layout(options, SaveLayout::default())
                .unwrap()
        );
    }
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn stream_has_self_entry_valid_offsets_and_preserves_document_projection() {
    let doc = created();
    let stamp = doc.stamp();
    let commit = doc.commit_revision();
    let output = stream(&doc);
    let table = xref(&output);
    let trailer = table.trailer();
    assert_eq!(
        trailer.get("Type").unwrap().as_name().unwrap().name(),
        "XRef"
    );
    assert!(trailer.get("Prev").is_none());
    assert!(trailer.get("Filter").is_none());
    let size = trailer.get("Size").unwrap().as_u32().unwrap();
    let self_id = ObjectId::new(size - 1, 0);
    assert!(doc.resolve_object(self_id).is_err()); // Output-only allocation.
    for (number, info) in table.objects() {
        if matches!(
            info.state(),
            pdf::document::cross_ref_table::ObjectState::Normal
        ) {
            let header = format!("{number} {} obj", info.gennum());
            assert!(output[info.offset() as usize..].starts_with(header.as_bytes()));
        }
    }
    let reopened = open(output.clone());
    assert_eq!(reopened.try_page_count().unwrap(), 1);
    assert_eq!(
        reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("xref output")
    );
    let object = reopened.resolve_object(self_id).unwrap();
    let stream = object.as_stream().unwrap();
    assert_eq!(stream.raw_data().unwrap().len(), size as usize * 13);
    let w: Vec<_> = stream
        .dict()
        .get("W")
        .unwrap()
        .as_array()
        .unwrap()
        .into_iter()
        .map(|v| v.as_u32().unwrap())
        .collect();
    assert_eq!(w, [1, 8, 4]);
    assert_eq!(
        stream
            .dict()
            .get("Index")
            .unwrap()
            .as_array()
            .unwrap()
            .len(),
        2
    );
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(
        stream.raw_data().unwrap()[self_id.obj_num() as usize * 13],
        1
    );
    assert_eq!(doc.commit_revision(), commit);
}

#[test]
fn low_version_requires_explicit_upgrade_without_touching_writer() {
    let doc = Document::new_with_version(PdfVersion::V1_4).unwrap();
    doc.add_page(0).unwrap();
    let stamp = doc.stamp();
    let mut target = b"keep".to_vec();
    assert!(matches!(
        doc.save_to_writer_with_layout(&mut target, Default::default(), SaveLayout::XrefStream),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert_eq!(target, b"keep");
    let output = doc
        .save_to_bytes_with_layout(
            SaveOptions {
                version: Some(PdfVersion::V1_5),
                ..Default::default()
            },
            SaveLayout::XrefStream,
        )
        .unwrap();
    assert!(output.starts_with(b"%PDF-1.5"));
    assert_eq!(open(output).try_page_count().unwrap(), 1);
    assert!(doc.is_stamp_current(&stamp));
    assert!(
        doc.save_to_bytes_with_layout(
            SaveOptions {
                version: Some(PdfVersion::V1_3),
                ..Default::default()
            },
            SaveLayout::XrefStream
        )
        .is_err()
    );
}

fn sparse_source() -> Vec<u8> {
    let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    for (id, body) in [
        ("1 0", "<< /Type /Catalog /Pages 2 0 R >>"),
        ("2 0", "<< /Type /Pages /Kids [] /Count 0 >>"),
        ("3 7", "(unused)"),
    ] {
        offsets.push(out.len());
        out.extend_from_slice(format!("{id} obj\n{body}\nendobj\n").as_bytes());
    }
    let pos = out.len();
    out.extend_from_slice(format!("xref\n0 5\n0000000004 65535 f \n{:010} 00000 n \n{:010} 00000 n \n{:010} 00007 n \n0000000000 00009 f \ntrailer\n<< /Size 16 /Root 1 0 R >>\nstartxref\n{pos}\n%%EOF\n", offsets[0], offsets[1], offsets[2]).as_bytes());
    out
}

#[test]
fn sparse_reservations_live_generations_and_deleted_free_chain_survive() {
    let doc = open(sparse_source());
    let first = stream(&doc);
    let table = xref(&first);
    assert_eq!(table.trailer().get("Size").unwrap().as_u32(), Some(17));
    assert_eq!(table.lookup(&3).unwrap().gennum(), 7);
    assert_eq!(table.lookup(&4).unwrap().gennum(), 9);
    doc.delete_object(ObjectId::new(3, 7)).unwrap();
    let output = stream(&doc);
    let table = xref(&output);
    assert_eq!(table.lookup(&3).unwrap().gennum(), 8);
    assert!(matches!(
        table.lookup(&3).unwrap().state(),
        pdf::document::cross_ref_table::ObjectState::Free
    ));
    let reopened = open(output);
    assert!(reopened.resolve_object(ObjectId::new(3, 7)).is_err());
    assert_eq!(
        reopened
            .try_insert_object(PdfObject::PdfNull)
            .unwrap()
            .obj_num(),
        17
    );
}

#[test]
fn output_limits_budget_failure_and_reservations_are_retryable() {
    let doc = created();
    let stamp = doc.stamp();
    let expected = stream(&doc);
    assert_eq!(
        doc.save_to_bytes_with_layout(
            SaveOptions {
                max_output_bytes: expected.len(),
                ..Default::default()
            },
            SaveLayout::XrefStream
        )
        .unwrap(),
        expected
    );
    assert!(matches!(
        doc.save_to_bytes_with_layout(
            SaveOptions {
                max_output_bytes: expected.len() - 1,
                ..Default::default()
            },
            SaveLayout::XrefStream
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    let tiny = OperationBudget::new(1);
    assert!(
        doc.save_with_layout_and_budget(Default::default(), SaveLayout::XrefStream, &tiny)
            .is_err()
    );
    assert_eq!(tiny.live_bytes(), 0);
    let pool = OperationBudget::new(32 * 1024 * 1024);
    let output = doc
        .save_with_layout_and_budget(Default::default(), SaveLayout::XrefStream, &pool)
        .unwrap();
    let peak = pool.peak_bytes();
    assert!(pool.live_bytes() >= output.as_slice().len());
    assert_eq!(output.as_slice(), expected);
    drop(output);
    assert_eq!(pool.live_bytes(), 0);
    let short = OperationBudget::new(peak - 1);
    assert!(matches!(
        created().save_with_layout_and_budget(Default::default(), SaveLayout::XrefStream, &short),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(short.live_bytes(), 0);
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(stream(&doc), expected);
}

#[test]
fn raw_stream_and_size_limits_are_checked_before_large_allocation() {
    for limits in [
        Limits {
            max_raw_stream_bytes: 10,
            ..Default::default()
        },
        Limits {
            max_decoded_bytes: 10,
            ..Default::default()
        },
        Limits {
            max_objects: 3,
            ..Default::default()
        },
    ] {
        let doc = Document::new_with_version_and_options(
            PdfVersion::V1_7,
            ParseOptions {
                mode: ParseMode::Strict,
                limits,
            },
        )
        .unwrap();
        let pool = OperationBudget::new(32 * 1024 * 1024);
        assert!(matches!(
            doc.save_with_layout_and_budget(Default::default(), SaveLayout::XrefStream, &pool),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), 0);
    }
}

#[test]
fn rewriting_stream_input_and_flate_body_does_not_inherit_stale_trailer_fields() {
    let source = stream(&created());
    let source_size = xref(&source)
        .trailer()
        .get("Size")
        .unwrap()
        .as_u32()
        .unwrap();
    let doc = open(source);
    let output = doc
        .save_to_bytes_with_layout(
            SaveOptions {
                compression: StreamCompression::FlateUnencoded,
                ..Default::default()
            },
            SaveLayout::XrefStream,
        )
        .unwrap();
    let table = xref(&output);
    assert!(table.trailer().get("Size").unwrap().as_u32().unwrap() > source_size);
    for key in ["Prev", "XRefStm", "Filter", "DecodeParms", "F"] {
        assert!(table.trailer().get(key).is_none());
    }
    assert_eq!(open(output).try_page_count().unwrap(), 1);
}

#[test]
fn authenticated_r6_preserves_protection_and_signed_inputs_remain_locked() {
    let encrypted = created()
        .save_to_bytes_with_security(
            Default::default(),
            SaveSecurity::Set(&EncryptionOptions::new("user", "owner").unwrap()),
        )
        .unwrap();
    let doc = Document::from_bytes_with_password_and_options(
        encrypted,
        "owner",
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )
    .unwrap();
    let protected = doc
        .save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStream)
        .unwrap();
    assert!(
        Document::from_bytes_with_password_and_options(
            protected,
            "owner",
            ParseOptions {
                mode: ParseMode::Strict,
                ..Default::default()
            }
        )
        .unwrap()
        .security_report()
        .is_some()
    );
    assert!(
        doc.save_to_bytes_with_layout(Default::default(), SaveLayout::Classic)
            .is_ok()
    );
    let hex: Vec<_> = include_str!("fixtures/v011/workflow-certified.pdf.hex")
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let bytes = hex
        .as_chunks::<2>()
        .0
        .iter()
        .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
        .collect();
    let doc = open(bytes);
    let stamp = doc.stamp();
    for layout in [SaveLayout::Classic, SaveLayout::XrefStream] {
        assert!(
            doc.save_to_bytes_with_layout(Default::default(), layout)
                .is_err()
        );
    }
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn dangling_reference_cannot_alias_output_only_xref_object() {
    let doc = Document::new().unwrap();
    let mut holder = PdfDict::default();
    holder
        .try_insert(
            "Dangling".into(),
            PdfObject::PdfReference(PdfReference::from_id(ObjectId::new(4, 0))),
        )
        .unwrap();
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfDict(holder))
            .unwrap()
            .obj_num(),
        3
    );
    let stamp = doc.stamp();
    assert!(matches!(
        doc.save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStream),
        Err(PdfError::InvalidReference(_))
    ));
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn retained_linearization_declarations_are_not_silently_relocated() {
    let doc = created();
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Linearized".into(),
        PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(1.0)),
    )
    .unwrap();
    doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
    let stamp = doc.stamp();
    let mut target = b"keep".to_vec();
    assert!(matches!(
        doc.save_to_writer_with_layout(&mut target, Default::default(), SaveLayout::XrefStream),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert_eq!(target, b"keep");
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn host_writer_failure_does_not_mutate_source_or_prevent_retry() {
    struct Short;
    impl Write for Short {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("synthetic failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let doc = created();
    let stamp = doc.stamp();
    assert!(
        doc.save_to_writer_with_layout(&mut Short, Default::default(), SaveLayout::XrefStream)
            .is_err()
    );
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(open(stream(&doc)).try_page_count().unwrap(), 1);
}
