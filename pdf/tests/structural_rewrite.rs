use pdf::{
    document::{
        owned::Document,
        save_options::{SaveOptions, StreamCompression},
    },
    objects::{PdfObject, object_id::ObjectId, pdf_number::PdfNumber},
    parser::syntax::ParseMode,
};
use std::io::Write;

#[test]
fn deleted_compressed_object_is_not_resurrected_by_source_stream_or_rewrite() {
    for encoded in [false, true] {
        for preloaded in [false, true] {
            let doc =
                Document::from_bytes_with_mode(compressed_document(encoded), ParseMode::Strict)
                    .unwrap();
            let value_id = ObjectId::new(4, 0);
            if preloaded {
                assert_eq!(
                    doc.resolve_object(value_id)
                        .unwrap()
                        .as_number()
                        .unwrap()
                        .as_u64_checked()
                        .unwrap(),
                    11
                );
            }
            doc.delete_object(value_id).unwrap();
            // Trigger expansion even when deletion precedes the first source load.
            assert_eq!(doc.try_page_count().unwrap(), 1);
            assert!(doc.resolve_object(value_id).is_err());
            // Remove the custom incoming reference, so the output has no live
            // dangling reference. This is logical deletion, not secure erasure.
            let root_id = ObjectId::new(1, 0);
            let mut catalog = doc.resolve_object(root_id).unwrap().into_dict().unwrap();
            catalog.remove("Private");
            doc.update_object(root_id, PdfObject::PdfDict(catalog))
                .unwrap();
            let revision = doc.revision();
            let bytes = doc.save_to_bytes().unwrap();
            assert_eq!(doc.revision(), revision);
            assert!(doc.resolve_object(value_id).is_err());
            let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            assert!(reopened.resolve_object(value_id).is_err());
            assert_eq!(reopened.try_page_count().unwrap(), 1);
            // The old ObjStm still exists; resolving it must not populate the
            // now-free object, even though its old bytes contain that object.
            assert!(
                reopened
                    .resolve_object(ObjectId::new(5, 0))
                    .unwrap()
                    .as_stream()
                    .is_some()
            );
            assert!(reopened.resolve_object(value_id).is_err());
            let second = Document::from_bytes_with_mode(
                reopened.save_to_bytes().unwrap(),
                ParseMode::Strict,
            )
            .unwrap();
            assert!(second.resolve_object(value_id).is_err());
        }
    }
}

#[test]
fn compressed_source_first_loaded_during_failed_edit_is_restored() {
    for encoded in [false, true] {
        for panic_failure in [false, true] {
            let doc =
                Document::from_bytes_with_mode(compressed_document(encoded), ParseMode::Strict)
                    .unwrap();
            let value_id = ObjectId::new(4, 0);
            let revision = doc.revision();
            let edit = || {
                doc.edit_transaction(|doc| {
                    // Do not resolve the source object first. Loading the page
                    // later expands the shared source stream during a tentative edit.
                    doc.update_object(value_id, PdfObject::PdfNumber(PdfNumber::new(99.0)))?;
                    doc.replace_page_contents(0, b"q Q\n".to_vec())?;
                    assert_eq!(
                        doc.resolve_object(value_id)?
                            .as_number()
                            .unwrap()
                            .as_u64_checked()?,
                        99
                    );
                    assert_eq!(doc.get_page(0)?.content_program()?.operations().len(), 2);
                    // Also force all lazy source objects through the save path.
                    let reopened =
                        Document::from_bytes_with_mode(doc.save_to_bytes()?, ParseMode::Strict)?;
                    assert_eq!(
                        reopened
                            .resolve_object(value_id)?
                            .as_number()
                            .unwrap()
                            .as_u64_checked()?,
                        99
                    );
                    if panic_failure {
                        panic!("injected compressed edit unwind");
                    }
                    Err::<(), _>(pdf::error::PdfError::DocumentError(
                        "injected compressed edit failure".into(),
                    ))
                })
            };
            if panic_failure {
                assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(edit)).is_err());
            } else {
                assert!(edit().is_err());
            }
            assert!(doc.revision() > revision);
            assert_eq!(
                doc.resolve_object(value_id)
                    .unwrap()
                    .as_number()
                    .unwrap()
                    .as_u64_checked()
                    .unwrap(),
                11
            );
            assert!(
                doc.get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .operations()
                    .is_empty()
            );
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(
                reopened
                    .resolve_object(value_id)
                    .unwrap()
                    .as_number()
                    .unwrap()
                    .as_u64_checked()
                    .unwrap(),
                11
            );
            assert!(
                reopened
                    .get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .operations()
                    .is_empty()
            );
        }
    }
}

// All four live document objects are compressed. Only the object stream and
// active cross-reference stream occur as ordinary indirect source objects.
fn compressed_document(flate: bool) -> Vec<u8> {
    let objects = [
        "<< /Type /Catalog /Pages 2 0 R /Private 4 0 R >>",
        "<< /Type /Pages /Kids [3 0 R] /Count 1 >>",
        "<< /Type /Page /Parent 2 0 R /MediaBox [0 0 100 100] >>",
        "11",
    ];
    let mut header = String::new();
    let mut body = Vec::new();
    for (index, object) in objects.iter().enumerate() {
        header.push_str(&format!("{} {} ", index + 1, body.len()));
        body.extend_from_slice(object.as_bytes());
        body.push(b' ');
    }
    let first = header.len();
    let mut payload = header.into_bytes();
    payload.extend_from_slice(&body);
    if flate {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(&payload).unwrap();
        payload = encoder.finish().unwrap();
    }
    let mut pdf = b"%PDF-1.5\n%\xE2\xE3\xCF\xD3\n".to_vec();
    let object_stream_offset = pdf.len();
    pdf.extend_from_slice(
        format!(
            "5 0 obj\n<< /Type /ObjStm /N 4 /First {first} /Length {} {} >>\nstream\n",
            payload.len(),
            if flate { "/Filter /FlateDecode" } else { "" }
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(&payload);
    pdf.extend_from_slice(b"\nendstream\nendobj\n");
    let xref_offset = pdf.len();
    let mut entries = Vec::new();
    for (kind, field, generation) in [
        (0u8, 0u32, 65535u16),
        (2, 5, 0),
        (2, 5, 1),
        (2, 5, 2),
        (2, 5, 3),
        (1, object_stream_offset as u32, 0),
        (1, xref_offset as u32, 0),
    ] {
        entries.push(kind);
        entries.extend_from_slice(&field.to_be_bytes());
        entries.extend_from_slice(&generation.to_be_bytes());
    }
    pdf.extend_from_slice(
        format!(
            "6 0 obj\n<< /Type /XRef /Size 7 /Root 1 0 R /W [1 4 2] /Length {} >>\nstream\n",
            entries.len()
        )
        .as_bytes(),
    );
    pdf.extend_from_slice(&entries);
    pdf.extend_from_slice(
        format!("\nendstream\nendobj\nstartxref\n{xref_offset}\n%%EOF\n").as_bytes(),
    );
    pdf
}

#[test]
fn active_compressed_objects_are_materialized_and_edits_survive_full_rewrite() {
    for encoded in [false, true] {
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let doc =
                Document::from_bytes_with_mode(compressed_document(encoded), ParseMode::Strict)
                    .unwrap();
            let value_id = ObjectId::new(4, 0);
            assert_eq!(doc.try_page_count().unwrap(), 1);
            assert_eq!(
                doc.resolve_object(value_id)
                    .unwrap()
                    .as_number()
                    .unwrap()
                    .as_u64_checked()
                    .unwrap(),
                11
            );
            doc.update_object(value_id, PdfObject::PdfNumber(PdfNumber::new(99.0)))
                .unwrap();
            doc.replace_page_contents(0, b"q Q\n".to_vec()).unwrap();
            let revision = doc.revision();
            let bytes = doc
                .save_to_bytes_with_options(SaveOptions {
                    compression,
                    ..Default::default()
                })
                .unwrap();
            assert_eq!(doc.revision(), revision);
            // Full rewrite generates an authoritative classic xref table;
            // retained source stream entries must not override these edits.
            let startxref = bytes
                .windows(10)
                .rposition(|window| window == b"startxref\n")
                .unwrap()
                + 10;
            let end = bytes[startxref..]
                .iter()
                .position(|byte| *byte == b'\n')
                .unwrap()
                + startxref;
            let offset: usize = std::str::from_utf8(&bytes[startxref..end])
                .unwrap()
                .parse()
                .unwrap();
            assert!(bytes[offset..].starts_with(b"xref\n"));
            let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            assert_eq!(reopened.try_page_count().unwrap(), 1);
            assert_eq!(
                reopened
                    .resolve_object(value_id)
                    .unwrap()
                    .as_number()
                    .unwrap()
                    .as_u64_checked()
                    .unwrap(),
                99
            );
            assert_eq!(
                reopened
                    .get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .operations()
                    .len(),
                2
            );
            assert!(
                reopened
                    .get_page(0)
                    .unwrap()
                    .display_list()
                    .unwrap()
                    .diagnostics()
                    .is_empty()
            );
            let reopened_again = Document::from_bytes_with_mode(
                reopened.save_to_bytes().unwrap(),
                ParseMode::Strict,
            )
            .unwrap();
            assert_eq!(
                reopened_again
                    .resolve_object(value_id)
                    .unwrap()
                    .as_number()
                    .unwrap()
                    .as_u64_checked()
                    .unwrap(),
                99
            );
        }
    }
}
