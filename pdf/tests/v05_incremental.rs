use pdf::{
    budget::OperationBudget,
    document::{
        Document, acroform::FormValue, metadata::InfoTextField,
        save_options::IncrementalSaveOptions,
    },
    error::PdfError,
    objects::{
        PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_reference::PdfReference,
        pdf_string::PdfString,
    },
    parser::syntax::ParseMode,
};
use std::io::{self, Cursor, Write};

fn open(bytes: Vec<u8>) -> Document {
    Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap()
}
fn base() -> Vec<u8> {
    let document = Document::new().unwrap();
    document.add_page(0).unwrap();
    document.save_to_bytes().unwrap()
}
fn last_xref(bytes: &[u8]) -> usize {
    let marker = bytes.windows(9).rposition(|s| s == b"startxref").unwrap();
    std::str::from_utf8(&bytes[marker + 9..])
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap()
}

#[test]
fn no_changes_and_rollback_return_exact_original_bytes_without_mutation() {
    let mut original = base();
    original.extend_from_slice(b"\r\n \t");
    let document = open(original.clone());
    let stamp = document.stamp();
    assert_eq!(document.save_incremental_to_bytes().unwrap(), original);
    assert!(document.is_stamp_current(&stamp));
    assert_eq!(document.commit_revision(), 0);
    let failed: Result<(), PdfError> = document.edit_transaction(|doc| {
        doc.set_info_text(InfoTextField::Title, Some("rollback"))?;
        Err(PdfError::DocumentError("rollback".into()))
    });
    assert!(failed.is_err());
    assert_eq!(document.save_incremental_to_bytes().unwrap(), original);
}

#[test]
fn metadata_updates_append_only_and_three_revisions_link_to_latest_xref() {
    let mut original = base();
    for title in ["first", "second", "third"] {
        let document = open(original.clone());
        document
            .set_info_text(InfoTextField::Title, Some(title))
            .unwrap();
        let stamp = document.stamp();
        let commit = document.commit_revision();
        let output = document.save_incremental_to_bytes().unwrap();
        assert!(output.starts_with(&original));
        let tail = &output[original.len()..];
        assert!(String::from_utf8_lossy(tail).contains(&format!("/Prev {}", last_xref(&original))));
        assert!(!String::from_utf8_lossy(tail).contains("/Type /Page"));
        assert!(document.is_stamp_current(&stamp));
        assert_eq!(document.commit_revision(), commit);
        assert_eq!(document.save_incremental_to_bytes().unwrap(), output);
        assert_eq!(
            open(output.clone())
                .info_text(InfoTextField::Title)
                .unwrap()
                .as_deref(),
            Some(title)
        );
        original = output;
    }
}

#[test]
fn repeated_saves_do_not_implicitly_rebase_and_trailer_only_removal_is_explicit() {
    let original = base();
    let document = open(original.clone());
    document
        .set_info_text(InfoTextField::Title, Some("first"))
        .unwrap();
    let first = document.save_incremental_to_bytes().unwrap();
    document
        .set_info_text(InfoTextField::Title, Some("second"))
        .unwrap();
    let second = document.save_incremental_to_bytes().unwrap();
    assert!(second.starts_with(&original));
    assert!(!second.starts_with(&first));
    let document = open(second.clone());
    document.remove_info_dict().unwrap();
    let output = document.save_incremental_to_bytes().unwrap();
    assert!(output.starts_with(&second));
    assert!(String::from_utf8_lossy(&output[second.len()..]).contains("/Info null"));
    assert_eq!(open(output).info_text(InfoTextField::Title).unwrap(), None);
}

#[test]
fn deletion_updates_free_chain_and_allocations_never_reuse_source_numbers() {
    let created = Document::new().unwrap();
    created.add_page(0).unwrap();
    let unused = created
        .try_insert_object(PdfObject::PdfString(PdfString::new(
            b"old bytes remain".to_vec(),
            false,
        )))
        .unwrap();
    let original = created.save_to_bytes().unwrap();
    let document = open(original.clone());
    document.delete_object(unused).unwrap();
    let output = document.save_incremental_to_bytes().unwrap();
    assert!(output.starts_with(&original));
    let reopened = open(output);
    assert!(reopened.resolve_object(unused).is_err());
    let fresh = reopened.try_insert_object(PdfObject::PdfNull).unwrap();
    assert!(fresh.obj_num() > unused.obj_num());
    reopened.delete_object(fresh).unwrap();
    let again = reopened.save_incremental_to_bytes().unwrap();
    assert_eq!(open(again).try_page_count().unwrap(), 1);
}

#[test]
fn live_references_to_deleted_objects_are_rejected_and_retry_succeeds() {
    let document = open(base());
    let target = document.try_insert_object(PdfObject::PdfNull).unwrap();
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Target".into(),
        PdfObject::PdfReference(PdfReference::from_id(target)),
    )
    .unwrap();
    let holder = document
        .try_insert_object(PdfObject::PdfDict(dict))
        .unwrap();
    document.delete_object(target).unwrap();
    assert!(matches!(
        document.save_incremental_to_bytes(),
        Err(PdfError::InvalidReference(_))
    ));
    document.update_object(holder, PdfObject::PdfNull).unwrap();
    assert_eq!(
        open(document.save_incremental_to_bytes().unwrap())
            .try_page_count()
            .unwrap(),
        1
    );
}

#[test]
fn source_preflight_cannot_be_bypassed_by_overwriting_signature_objects() {
    for key in ["Type", "FT", "ByteRange"] {
        let created = Document::new().unwrap();
        created.add_page(0).unwrap();
        let mut dict = PdfDict::default();
        dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new("Sig".into())))
            .unwrap();
        let id = created.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
        let source = created.save_to_bytes().unwrap();
        let document = open(source);
        assert!(document.update_object(id, PdfObject::PdfNull).is_err());
        assert!(
            matches!(
                document.save_incremental_to_bytes(),
                Err(PdfError::UnsupportedFeature(_))
            ),
            "{key}"
        );
    }
}

#[test]
fn linearized_source_preflight_survives_overwriting_the_parameter_dictionary() {
    let document = open(include_bytes!("fixtures/linearized/two-pages.pdf").to_vec());
    document
        .update_object(
            pdf::objects::object_id::ObjectId::new(4, 0),
            PdfObject::PdfNull,
        )
        .unwrap();
    assert!(matches!(
        document.save_incremental_to_bytes(),
        Err(PdfError::UnsupportedFeature(_))
    ));
}

#[test]
fn unsupported_source_kinds_and_tiny_budgets_fail_without_mutation() {
    assert!(
        Document::new()
            .unwrap()
            .save_incremental_to_bytes()
            .is_err()
    );
    assert!(
        Document::from_bytes(base())
            .unwrap()
            .save_incremental_to_bytes()
            .is_err()
    );
    assert!(
        Document::from_reader_with_mode(Cursor::new(base()), ParseMode::Strict)
            .unwrap()
            .save_incremental_to_bytes()
            .is_err()
    );
    let original = base();
    let document = open(original.clone());
    document
        .set_info_text(InfoTextField::Title, Some("keep"))
        .unwrap();
    let stamp = document.stamp();
    let budget = OperationBudget::new(1);
    assert!(matches!(
        document.save_incremental_with_budget(Default::default(), &budget),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(budget.live_bytes(), 0);
    assert!(document.is_stamp_current(&stamp));
    assert!(matches!(
        document.save_incremental_to_bytes_with_options(IncrementalSaveOptions {
            max_output_bytes: original.len()
        }),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(document.is_stamp_current(&stamp));
    let budget = OperationBudget::new(2 * 1024 * 1024);
    let output = document
        .save_incremental_with_budget(Default::default(), &budget)
        .unwrap();
    assert_eq!(budget.live_bytes(), output.capacity());
    assert!(output.as_slice().starts_with(&original));
    drop(output);
    assert_eq!(budget.live_bytes(), 0);
}

#[test]
fn writer_failure_leaves_document_and_source_retryable() {
    struct Fails(Vec<u8>);
    impl Write for Fails {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            if self.0.is_empty() {
                self.0.extend_from_slice(&bytes[..10]);
                Ok(10)
            } else {
                Err(io::Error::other("injected failure"))
            }
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let original = base();
    let document = open(original.clone());
    document
        .set_info_text(InfoTextField::Title, Some("retry"))
        .unwrap();
    let stamp = document.stamp();
    let mut output = Fails(vec![]);
    assert!(document.save_incremental_to_writer(&mut output).is_err());
    assert_eq!(output.0, original[..10]);
    assert!(document.is_stamp_current(&stamp));
    assert_eq!(
        open(document.save_incremental_to_bytes().unwrap())
            .info_text(InfoTextField::Title)
            .unwrap()
            .as_deref(),
        Some("retry")
    );
}

#[test]
fn v04_form_navigation_attachment_and_annotation_edits_survive_incremental_save() {
    let original = include_bytes!("fixtures/v04/reportlab.pdf").to_vec();
    let document = open(original.clone());
    document
        .set_form_value("external_text", FormValue::Text("Incremental".into()))
        .unwrap();
    document
        .set_form_value("external_check", FormValue::Checkbox(false))
        .unwrap();
    document
        .rename_flat_bookmark(0, "Updated bookmark")
        .unwrap();
    document.set_named_destination("incremental", 1).unwrap();
    document
        .add_named_link(0, [10.0, 10.0, 80.0, 30.0], "incremental")
        .unwrap();
    document
        .add_attachment("new.txt", b"payload".to_vec())
        .unwrap();
    document
        .add_square_annotation(0, [20.0, 60.0, 60.0, 100.0], [1.0, 0.0, 0.0])
        .unwrap();
    let output = document.save_incremental_to_bytes().unwrap();
    assert!(output.starts_with(&original));
    let incremental = open(output);
    let full = open(document.save_to_bytes().unwrap());
    assert_eq!(
        incremental.form_fields().unwrap(),
        full.form_fields().unwrap()
    );
    assert_eq!(
        incremental.flat_bookmarks().unwrap()[0].title,
        "Updated bookmark"
    );
    assert_eq!(
        incremental.named_destinations().unwrap(),
        full.named_destinations().unwrap()
    );
    assert_eq!(
        incremental.page_links(0).unwrap().len(),
        full.page_links(0).unwrap().len()
    );
    assert_eq!(
        incremental
            .attachment_data("new.txt")
            .unwrap()
            .unwrap()
            .as_slice(),
        b"payload"
    );
}

fn compressed_source(hybrid: bool) -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let pages = bytes.len();
    bytes.extend_from_slice(b"1 0 obj\n<< /Type /Pages /Kids [] /Count 0 >>\nendobj\n");
    let stream = bytes.len();
    let catalog = b"<< /Type /Catalog /Pages 1 0 R >>";
    let header = format!("2 0 5 {} ", catalog.len() + 1);
    let first = header.len();
    let mut data = header.into_bytes();
    data.extend_from_slice(catalog);
    data.extend_from_slice(b" << /Title (old) >>");
    bytes.extend_from_slice(
        format!(
            "3 0 obj\n<< /Type /ObjStm /N 2 /First {first} /Length {} >>\nstream\n",
            data.len()
        )
        .as_bytes(),
    );
    bytes.extend_from_slice(&data);
    bytes.extend_from_slice(b"\nendstream\nendobj\n");
    let xref = bytes.len();
    let mut entries = Vec::new();
    for (kind, offset, generation) in [
        (0u8, 0u32, 0u16),
        (1, pages as u32, 0),
        (2, 3, 0),
        (1, stream as u32, 0),
        (1, xref as u32, 0),
        (2, 3, 1),
    ] {
        entries.push(kind);
        entries.extend_from_slice(&offset.to_be_bytes());
        entries.extend_from_slice(&generation.to_be_bytes());
    }
    bytes.extend_from_slice(
        format!(
            "4 0 obj\n<< /Type /XRef /Size 6 /Root 2 0 R /Info 5 0 R /W [1 4 2] /Length {} >>\nstream\n",
            entries.len()
        )
        .as_bytes(),
    );
    bytes.extend_from_slice(&entries);
    bytes.extend_from_slice(b"\nendstream\nendobj\n");
    let latest = if hybrid {
        let table = bytes.len();
        bytes.extend_from_slice(format!("xref\n0 6\n0000000000 65535 f \n{pages:010} 00000 n \n0000000000 00000 f \n{stream:010} 00000 n \n{xref:010} 00000 n \n0000000000 00000 f \ntrailer\n<< /Size 6 /Root 2 0 R /Info 5 0 R /XRefStm {xref} >>\n").as_bytes());
        table
    } else {
        xref
    };
    bytes.extend_from_slice(format!("startxref\n{latest}\n%%EOF\n").as_bytes());
    bytes
}

#[test]
fn deleting_old_compressed_info_does_not_treat_historical_xref_roots_as_live() {
    for hybrid in [false, true] {
        let document = open(compressed_source(hybrid));
        document
            .set_info_text(InfoTextField::Title, Some("new"))
            .unwrap();
        document
            .delete_object(pdf::objects::object_id::ObjectId::new(5, 0))
            .unwrap();
        let reopened = open(document.save_incremental_to_bytes().unwrap());
        assert_eq!(
            reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
            Some("new")
        );
    }
}

#[test]
fn signing_rejects_hybrid_history_until_explicit_full_rewrite_and_reopen() {
    let document = open(compressed_source(true));
    assert!(
        matches!(document.prepare_signature(Default::default()), Err(PdfError::UnsupportedFeature(message)) if message.contains("hybrid"))
    );
    document
        .set_info_text(InfoTextField::Title, Some("classic latest revision"))
        .unwrap();
    let appended = open(document.save_incremental_to_bytes().unwrap());
    // Latest trailer is classic and omits XRefStm, but Prev remains hybrid.
    assert!(
        matches!(appended.prepare_signature(Default::default()), Err(PdfError::UnsupportedFeature(message)) if message.contains("hybrid"))
    );
    let normalized = open(appended.save_to_bytes().unwrap());
    normalized.add_page(0).unwrap();
    assert!(normalized.prepare_signature(Default::default()).is_ok());
}

#[test]
fn structural_stream_containers_cannot_be_replaced_or_deleted_incrementally() {
    for (number, delete) in [(3, false), (4, true)] {
        let document = open(compressed_source(false));
        let id = pdf::objects::object_id::ObjectId::new(number, 0);
        if delete {
            document.delete_object(id).unwrap();
        } else {
            document.update_object(id, PdfObject::PdfNull).unwrap();
        }
        assert!(matches!(
            document.save_incremental_to_bytes(),
            Err(PdfError::UnsupportedFeature(_))
        ));
    }
}

fn nonzero_generation_source(size: u32, version_override: bool) -> Vec<u8> {
    let mut bytes = b"%PDF-1.7\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let mut offsets = Vec::new();
    for (id, generation, value) in [
        (1, 3, "<< /Type /Pages /Kids [] /Count 0 >>"),
        (
            2,
            4,
            if version_override {
                "<< /Type /Catalog /Pages 1 3 R /Version /2.0 >>"
            } else {
                "<< /Type /Catalog /Pages 1 3 R >>"
            },
        ),
        (3, 7, "null"),
    ] {
        offsets.push((bytes.len(), generation));
        bytes.extend_from_slice(format!("{id} {generation} obj\n{value}\nendobj\n").as_bytes());
    }
    let xref = bytes.len();
    bytes.extend_from_slice(b"xref\n0 4\n0000000000 65535 f \n");
    for (offset, generation) in offsets {
        bytes.extend_from_slice(format!("{offset:010} {generation:05} n \n").as_bytes());
    }
    bytes.extend_from_slice(
        format!("trailer\n<< /Size {size} /Root 2 4 R >>\nstartxref\n{xref}\n%%EOF\n").as_bytes(),
    );
    bytes
}

#[test]
fn nonzero_generations_are_preserved_and_deletion_increments_generation() {
    let document = open(nonzero_generation_source(4, false));
    let id = pdf::objects::object_id::ObjectId::new(3, 7);
    document
        .update_object(
            id,
            PdfObject::PdfString(PdfString::new(b"generation seven".to_vec(), false)),
        )
        .unwrap();
    let output = document.save_incremental_to_bytes().unwrap();
    assert!(String::from_utf8_lossy(&output).contains("3 7 obj"));
    let document = open(output);
    document.delete_object(id).unwrap();
    let output = document.save_incremental_to_bytes().unwrap();
    assert!(String::from_utf8_lossy(&output).contains("00008 f"));
    assert!(open(output).resolve_object(id).is_err());
}

#[test]
fn allocation_respects_original_size_even_with_sparse_source_entries() {
    let document = open(nonzero_generation_source(1000, false));
    let id = document.try_insert_object(PdfObject::PdfNull).unwrap();
    assert_eq!(id.obj_num(), 1000);
    let reopened = open(document.save_incremental_to_bytes().unwrap());
    assert_eq!(reopened.resolve_object(id).unwrap(), PdfObject::PdfNull);
}

#[test]
fn removing_catalog_version_must_not_downgrade_retained_source_features() {
    let document = open(nonzero_generation_source(4, true));
    let id = pdf::objects::object_id::ObjectId::new(2, 4);
    let mut catalog = document.resolve_object(id).unwrap().into_dict().unwrap();
    catalog.remove("Version");
    document
        .update_object(id, PdfObject::PdfDict(catalog))
        .unwrap();
    assert!(matches!(
        document.save_incremental_to_bytes(),
        Err(PdfError::UnsupportedFeature(_))
    ));
}

#[test]
fn exact_output_limit_succeeds_and_one_less_fails_without_committing() {
    let document = open(base());
    document
        .set_info_text(InfoTextField::Title, Some("limits"))
        .unwrap();
    let expected = document.save_incremental_to_bytes().unwrap();
    let stamp = document.stamp();
    assert_eq!(
        document
            .save_incremental_to_bytes_with_options(IncrementalSaveOptions {
                max_output_bytes: expected.len()
            })
            .unwrap(),
        expected
    );
    assert!(matches!(
        document.save_incremental_to_bytes_with_options(IncrementalSaveOptions {
            max_output_bytes: expected.len() - 1
        }),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(document.is_stamp_current(&stamp));
}

#[test]
fn compressed_catalog_is_appended_as_standalone_object_for_stream_and_hybrid_inputs() {
    for hybrid in [false, true] {
        let original = compressed_source(hybrid);
        let document = open(original.clone());
        document
            .set_metadata_packet(Some(b"<test/>".to_vec()))
            .unwrap();
        let output = document.save_incremental_to_bytes().unwrap();
        assert!(output.starts_with(&original));
        let tail = String::from_utf8_lossy(&output[original.len()..]);
        assert!(tail.contains("2 0 obj"));
        assert!(!tail.contains("/XRefStm"));
        assert!(!tail.contains("/ObjStm"));
        let reopened = open(output);
        assert_eq!(reopened.metadata_packet().unwrap().unwrap(), b"<test/>");
        assert_eq!(reopened.try_page_count().unwrap(), 0);
    }
}
