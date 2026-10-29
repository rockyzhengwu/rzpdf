use pdf::{
    document::{Document, PdfDocument},
    error::PdfError,
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_stream::PdfStream},
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};
use std::io::Cursor;

fn stream_pdf(count: usize, length: usize) -> (Vec<u8>, Vec<ObjectId>) {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let ids = (0..count)
        .map(|n| {
            doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                vec![b'A' + n as u8; length],
            )))
            .unwrap()
        })
        .collect();
    (doc.save_to_bytes().unwrap(), ids)
}
fn options(cache: usize) -> ParseOptions {
    ParseOptions {
        mode: ParseMode::Strict,
        limits: Limits {
            max_cached_stream_bytes: cache,
            max_raw_stream_bytes: 1024 * 1024,
            ..Limits::default()
        },
    }
}
#[test]
fn source_metadata_is_lazy_and_pins_survive_clear_and_enforce_residency() {
    let (bytes, ids) = stream_pdf(3, 64 * 1024);
    let doc =
        PdfDocument::from_reader_with_options(Cursor::new(bytes), options(128 * 1024)).unwrap();
    let streams: Vec<_> = ids
        .iter()
        .map(|id| doc.resolve_object(*id).unwrap())
        .collect();
    assert_eq!(doc.source_stream_statistics().resident_bytes, 0);
    assert!(streams[0].as_stream().unwrap().raw_data().is_err());
    let a = doc
        .read_stream_data(streams[0].as_stream().unwrap())
        .unwrap();
    assert_eq!(a[0], b'A');
    doc.clear_source_stream_cache();
    let stats = doc.source_stream_statistics();
    assert_eq!(stats.entries, 0);
    assert_eq!(stats.pinned_bytes, 64 * 1024);
    let b = doc
        .read_stream_data(streams[1].as_stream().unwrap())
        .unwrap();
    assert_eq!(b[0], b'B');
    let error = doc
        .read_stream_data(streams[2].as_stream().unwrap())
        .unwrap_err();
    assert!(matches!(
        error,
        PdfError::ResourceLimitExceeded {
            reason: "raw_stream_residency",
            stage: "source_read",
            limit: 131072,
            actual: 196608
        }
    ));
    assert_eq!(a[0], b'A');
    drop(a);
    let c = doc
        .read_stream_data(streams[2].as_stream().unwrap())
        .unwrap();
    assert_eq!(c[0], b'C');
    drop((b, c));
    doc.clear_source_stream_cache();
    assert_eq!(doc.source_stream_statistics().resident_bytes, 0);
    for id in ids.iter().cycle().take(15) {
        let object = doc.resolve_object(*id).unwrap();
        assert_eq!(
            doc.read_stream_data(object.as_stream().unwrap())
                .unwrap()
                .len(),
            64 * 1024
        );
    }
    assert!(doc.source_stream_statistics().evictions > 0);
    assert!(doc.source_stream_statistics().peak_resident_bytes <= 128 * 1024);
}

#[test]
fn immutable_input_views_and_full_save_do_not_admit_all_streams() {
    let (bytes, ids) = stream_pdf(7, 64 * 1024);
    let source_len = bytes.len();
    let doc = Document::from_bytes_with_options(bytes.clone(), options(64 * 1024)).unwrap();
    for id in &ids {
        let object = doc.resolve_object(*id).unwrap();
        assert_eq!(
            object.as_stream().unwrap().raw_data().unwrap().len(),
            64 * 1024
        );
    }
    assert_eq!(doc.source_stream_statistics().source_bytes, source_len);
    assert_eq!(doc.source_stream_statistics().resident_bytes, 0);
    let generic =
        PdfDocument::from_reader_with_options(Cursor::new(bytes), options(64 * 1024)).unwrap();
    let saved = generic.save_to_bytes().unwrap();
    assert!(generic.source_stream_statistics().peak_resident_bytes <= 64 * 1024);
    let reopened = Document::from_bytes_with_options(saved, options(64 * 1024)).unwrap();
    for id in ids {
        let object = reopened.resolve_object(id).unwrap();
        assert_eq!(
            reopened
                .read_stream_data(object.as_stream().unwrap())
                .unwrap()
                .len(),
            64 * 1024
        );
    }
}

#[test]
fn oversized_stream_is_operation_scoped_not_permanently_cached() {
    let (bytes, ids) = stream_pdf(1, 128 * 1024);
    let doc =
        PdfDocument::from_reader_with_options(Cursor::new(bytes), options(64 * 1024)).unwrap();
    let object = doc.resolve_object(ids[0]).unwrap();
    let small = pdf::budget::OperationBudget::new(128 * 1024 - 1);
    assert!(matches!(
        doc.read_stream_data_with_budget(object.as_stream().unwrap(), &small),
        Err(PdfError::ResourceLimitExceeded {
            reason: "raw_stream_inflight_bytes",
            stage: "source_read",
            limit: 131071,
            actual: 131072
        })
    ));
    assert_eq!(small.live_bytes(), 0);
    assert_eq!(doc.source_stream_statistics().transient_bytes, 0);
    let budget = pdf::budget::OperationBudget::new(256 * 1024);
    let data = doc
        .read_stream_data_with_budget(object.as_stream().unwrap(), &budget)
        .unwrap();
    assert_eq!(budget.live_bytes(), 128 * 1024);
    assert_eq!(doc.source_stream_statistics().transient_bytes, 128 * 1024);
    assert_eq!(doc.source_stream_statistics().resident_bytes, 0);
    drop(data);
    assert_eq!(budget.live_bytes(), 0);
    assert_eq!(doc.source_stream_statistics().transient_bytes, 0);
    assert_eq!(
        doc.source_stream_statistics().peak_transient_bytes,
        128 * 1024
    );
}

fn exact_editor_fixture() -> Vec<u8> {
    let (side, count) = (1024, 7);
    let mut objects = vec![
        Vec::new(),
        Vec::new(),
        b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
    ];
    let mut kids = Vec::new();
    for page in 0..count {
        let mut pixels = vec![0; side * side * 3];
        for y in 0..side {
            for x in 0..side {
                let i = (y * side + x) * 3;
                pixels[i] = ((x + page * 31) % 256) as u8;
                pixels[i + 1] = ((y + page * 47) % 256) as u8;
                pixels[i + 2] = (80 + page * 20) as u8;
            }
        }
        let image_id = objects.len() + 1;
        let mut image = format!("<< /Type /XObject /Subtype /Image /Width {side} /Height {side} /ColorSpace /DeviceRGB /BitsPerComponent 8 /Length {} >>\nstream\n",pixels.len()).into_bytes();
        image.extend(pixels);
        image.extend(b"\nendstream");
        objects.push(image);
        let page_id = objects.len() + 1;
        let content_id = page_id + 1;
        kids.push(format!("{page_id} 0 R"));
        objects.push(format!("<< /Type /Page /Parent 2 0 R /MediaBox [0 0 400 400] /Resources << /Font << /F1 3 0 R >> /XObject << /Im1 {image_id} 0 R >> >> /Contents {content_id} 0 R >>").into_bytes());
        let content = format!(
            "BT /F1 16 Tf 40 370 Td (Image page {} of {count}) Tj ET\nq 320 0 0 280 40 60 cm /Im1 Do Q",
            page + 1
        );
        objects.push(
            format!(
                "<< /Length {} >>\nstream\n{content}\nendstream",
                content.len()
            )
            .into_bytes(),
        );
    }
    objects[0] = b"<< /Type /Catalog /Pages 2 0 R >>".to_vec();
    objects[1] = format!(
        "<< /Type /Pages /Count {count} /Kids [{}] >>",
        kids.join(" ")
    )
    .into_bytes();
    let mut bytes = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = vec![0];
    for (i, object) in objects.into_iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend(format!("{} 0 obj\n", i + 1).bytes());
        bytes.extend(object);
        bytes.extend(b"\nendobj\n");
    }
    let xref = bytes.len();
    bytes.extend(format!("xref\n0 {}\n0000000000 65535 f \n", offsets.len()).bytes());
    for offset in &offsets[1..] {
        bytes.extend(format!("{offset:010} 00000 n \n").bytes());
    }
    bytes.extend(
        format!(
            "trailer\n<< /Size {} /Root 1 0 R >>\nstartxref\n{xref}\n%%EOF\n",
            offsets.len()
        )
        .bytes(),
    );
    use sha2::{Digest, Sha256};
    assert_eq!(bytes.len(), 22023982);
    assert_eq!(
        format!("{:x}", Sha256::digest(&bytes)),
        "03be863a438eb2b44709947a5a211a9b94d120e231c92bfaae26be521f1f2551"
    );
    bytes
}

#[test]
fn exact_editor_fixture_reads_searches_and_saves_under_unchanged_browser_budgets() {
    let options = ParseOptions {
        mode: ParseMode::Strict,
        limits: Limits {
            max_cached_stream_bytes: 16 * 1024 * 1024,
            max_decoded_bytes: 16 * 1024 * 1024,
            max_operation_bytes: 64 * 1024 * 1024,
            max_image_pixels: 4_000_000,
            ..Limits::default()
        },
    };
    let doc = PdfDocument::from_reader_with_options(Cursor::new(exact_editor_fixture()), options)
        .unwrap();
    // Search before rendering must not read or decode the seven image payloads.
    for index in 0..7 {
        let text: String = doc
            .get_page(index)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()
            .into_iter()
            .filter_map(|c| c.unicode)
            .collect();
        assert_eq!(text, format!("Image page {} of 7", index + 1));
    }
    assert!(doc.source_stream_statistics().resident_bytes < 1024);
    for index in [0, 1, 6, 0, 1, 2, 3, 4, 5, 6] {
        let list = doc.get_page(index).unwrap().display_list().unwrap();
        assert!(
            list.objects()
                .iter()
                .any(|o| matches!(o, pdf::page::page_object::PageObject::Image(_)))
        );
    }
    assert!(doc.source_stream_statistics().evictions > 0);
    assert!(doc.source_stream_statistics().peak_resident_bytes <= 16 * 1024 * 1024);
    let saved = doc.save_to_bytes().unwrap();
    let reopened = Document::from_bytes_with_options(saved, options).unwrap();
    assert_eq!(reopened.source_stream_statistics().resident_bytes, 0);
    for index in 0..7 {
        let text: String = reopened
            .get_page(index)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()
            .into_iter()
            .filter_map(|c| c.unicode)
            .collect();
        assert_eq!(text, format!("Image page {} of 7", index + 1));
    }
}

#[test]
fn encrypted_payloads_are_lazy_and_release_plaintext_while_edits_survive_clears() {
    use pdf::security::{EncryptionOptions, SaveSecurity};
    let (source, ids) = stream_pdf(7, 64 * 1024);
    let plain = Document::from_bytes_with_mode(source, ParseMode::Strict).unwrap();
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    let encrypted = plain
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
        .unwrap();
    let doc = PdfDocument::from_reader_with_password_and_options(
        Cursor::new(encrypted),
        "owner",
        options(64 * 1024),
    )
    .unwrap();
    let objects: Vec<_> = ids
        .iter()
        .map(|id| doc.resolve_object(*id).unwrap())
        .collect();
    assert_eq!(doc.source_stream_statistics().resident_bytes, 0);
    for (i, object) in objects.iter().enumerate() {
        let budget = pdf::budget::OperationBudget::new(256 * 1024);
        let data = doc
            .read_stream_data_with_budget(object.as_stream().unwrap(), &budget)
            .unwrap();
        assert!(data.iter().all(|b| *b == b'A' + i as u8));
        assert!(budget.live_bytes() >= 64 * 1024);
        assert!(doc.source_stream_statistics().transient_bytes >= 64 * 1024);
        doc.clear_source_stream_cache();
        assert_eq!(data[0], b'A' + i as u8);
        drop(data);
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(doc.source_stream_statistics().transient_bytes, 0);
    }
    doc.replace_stream_decoded_data(ids[0], b"edited".to_vec())
        .unwrap();
    doc.clear_source_stream_cache();
    let saved = doc.save_to_bytes().unwrap();
    let reopened =
        Document::from_bytes_with_password_and_options(saved, "owner", options(64 * 1024)).unwrap();
    let object = reopened.resolve_object(ids[0]).unwrap();
    assert_eq!(
        &*reopened
            .read_stream_data(object.as_stream().unwrap())
            .unwrap(),
        b"edited"
    );
    assert!(doc.source_stream_statistics().peak_resident_bytes <= 64 * 1024);
    assert!(doc.source_stream_statistics().peak_transient_bytes > 0);
}

#[test]
fn failed_source_io_releases_reservations_restores_parser_and_can_retry() {
    use std::io::{Read, Seek, SeekFrom};
    struct FlakyReader {
        cursor: Cursor<Vec<u8>>,
        fail: bool,
    }
    impl Read for FlakyReader {
        fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
            if buf.len() > 32 * 1024 && self.fail {
                self.fail = false;
                // A partial read followed by a failure must restore the saved position.
                self.cursor.set_position(self.cursor.position() + 11);
                return Err(std::io::Error::other("injected source failure"));
            }
            self.cursor.read(buf)
        }
    }
    impl Seek for FlakyReader {
        fn seek(&mut self, pos: SeekFrom) -> std::io::Result<u64> {
            self.cursor.seek(pos)
        }
    }
    let (bytes, ids) = stream_pdf(2, 64 * 1024);
    let doc = PdfDocument::from_reader_with_options(
        FlakyReader {
            cursor: Cursor::new(bytes),
            fail: true,
        },
        options(64 * 1024),
    )
    .unwrap();
    let first = doc.resolve_object(ids[0]).unwrap();
    assert!(doc.read_stream_data(first.as_stream().unwrap()).is_err());
    assert_eq!(doc.source_stream_statistics().resident_bytes, 0);
    assert_eq!(doc.source_stream_statistics().entries, 0);
    let second = doc.resolve_object(ids[1]).unwrap();
    assert_eq!(
        doc.read_stream_data(second.as_stream().unwrap()).unwrap()[0],
        b'B'
    );
    assert_eq!(
        doc.read_stream_data(first.as_stream().unwrap()).unwrap()[0],
        b'A'
    );
}
