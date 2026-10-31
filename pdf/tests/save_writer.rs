use pdf::{document::PdfDocument, error::PdfError, parser::syntax::ParseMode};
use std::io::{self, Write};

#[test]
fn serialized_xref_offsets_point_to_each_indirect_object() {
    let doc = PdfDocument::new().unwrap();
    for index in 0..3 {
        doc.add_page(index).unwrap();
    }
    let bytes = doc.save_to_bytes().unwrap();
    let marker = b"startxref\n";
    let start = bytes
        .windows(marker.len())
        .rposition(|w| w == marker)
        .unwrap()
        + marker.len();
    let end = bytes[start..].iter().position(|&b| b == b'\n').unwrap() + start;
    let offset: usize = std::str::from_utf8(&bytes[start..end])
        .unwrap()
        .parse()
        .unwrap();
    assert!(bytes[offset..].starts_with(b"xref\n"));
    let mut lines = bytes[offset..].split(|&b| b == b'\n');
    assert_eq!(lines.next().unwrap(), b"xref");
    let subsection = std::str::from_utf8(lines.next().unwrap()).unwrap();
    let mut fields = subsection.split_whitespace();
    assert_eq!(fields.next(), Some("0"));
    let count: usize = fields.next().unwrap().parse().unwrap();
    assert!(count > 3);
    for number in 0..count {
        let row = std::str::from_utf8(lines.next().unwrap()).unwrap();
        let mut fields = row.split_whitespace();
        let object_offset: usize = fields.next().unwrap().parse().unwrap();
        let generation: u16 = fields.next().unwrap().parse().unwrap();
        let state = fields.next().unwrap();
        if state == "n" {
            let header = format!("{number} {generation} obj\n");
            assert!(bytes[object_offset..].starts_with(header.as_bytes()));
        } else {
            assert_eq!(state, "f");
        }
    }
    assert_eq!(lines.next().unwrap(), b"trailer");
    assert_eq!(
        PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict)
            .unwrap()
            .try_page_count()
            .unwrap(),
        3
    );
}

#[test]
fn writer_output_is_a_strictly_readable_document() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let mut output = Vec::new();
    doc.save_to_writer(&mut output).unwrap();
    let reopened = PdfDocument::from_bytes_with_mode(output, ParseMode::Strict).unwrap();
    assert_eq!(reopened.try_page_count().unwrap(), 1);
}

struct ShortWriter {
    bytes: Vec<u8>,
    remaining: usize,
}
impl Write for ShortWriter {
    fn write(&mut self, input: &[u8]) -> io::Result<usize> {
        if self.remaining == 0 {
            return Err(io::Error::new(
                io::ErrorKind::BrokenPipe,
                "test sink closed",
            ));
        }
        let count = input.len().min(self.remaining).min(3);
        self.bytes.extend_from_slice(&input[..count]);
        self.remaining -= count;
        Ok(count)
    }
    fn flush(&mut self) -> io::Result<()> {
        panic!("save_to_writer must not flush caller's stream")
    }
}

#[test]
fn writer_handles_short_writes_and_reports_partial_io_failure() {
    let doc = PdfDocument::new().unwrap();
    let expected = doc.save_to_bytes().unwrap();
    let mut good = ShortWriter {
        bytes: Vec::new(),
        remaining: usize::MAX,
    };
    doc.save_to_writer(&mut good).unwrap();
    // Dictionary order may differ, so verify structure instead of exact byte identity.
    assert_eq!(
        PdfDocument::from_bytes_with_mode(good.bytes, ParseMode::Strict)
            .unwrap()
            .try_page_count()
            .unwrap(),
        0
    );
    let mut failed = ShortWriter {
        bytes: Vec::new(),
        remaining: 7,
    };
    assert!(matches!(
        doc.save_to_writer(&mut failed),
        Err(PdfError::IOError(_))
    ));
    assert_eq!(failed.bytes, expected[..7]);
    assert_eq!(doc.try_page_count().unwrap(), 0);
}

#[test]
fn save_options_preserve_upgrade_and_reject_downgrades_before_writing() {
    use pdf::document::save_options::{PdfVersion, SaveOptions};
    let doc = PdfDocument::new().unwrap();
    assert!(
        doc.save_to_bytes_with_options(SaveOptions::default())
            .unwrap()
            .starts_with(b"%PDF-1.7\n")
    );
    let upgraded = doc
        .save_to_bytes_with_options(SaveOptions {
            version: Some(PdfVersion::V2_0),
            ..Default::default()
        })
        .unwrap();
    let reopened = PdfDocument::from_bytes_with_mode(upgraded, ParseMode::Strict).unwrap();
    assert_eq!(reopened.version(), "2.0");
    let mut output = b"existing".to_vec();
    assert!(matches!(
        doc.save_to_writer_with_options(
            &mut output,
            SaveOptions {
                version: Some(PdfVersion::V1_4),
                ..Default::default()
            }
        ),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert_eq!(output, b"existing");
    assert_eq!(doc.version(), "1.7");
}

#[test]
fn optional_flate_compression_preserves_pixels_and_source_objects() {
    use pdf::document::save_options::{SaveOptions, StreamCompression};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let pixels = vec![42; 300];
    let name = doc
        .resources()
        .embed_rgb_image(0, 10, 10, pixels.clone())
        .unwrap();
    let source = doc.get_page(0).unwrap().get_xobject(&name).unwrap();
    assert!(source.as_stream().unwrap().dict().get("Filter").is_none());
    let output = doc
        .save_to_bytes_with_options(SaveOptions {
            compression: StreamCompression::FlateUnencoded,
            ..Default::default()
        })
        .unwrap();
    let reopened = PdfDocument::from_bytes_with_mode(output, ParseMode::Strict).unwrap();
    let compressed = reopened.get_page(0).unwrap().get_xobject(&name).unwrap();
    assert_eq!(
        compressed
            .as_stream()
            .unwrap()
            .dict()
            .get("Filter")
            .unwrap()
            .as_name()
            .unwrap()
            .bytes(),
        b"FlateDecode"
    );
    assert!(
        reopened
            .read_stream_data(compressed.as_stream().unwrap())
            .unwrap()
            .as_ref()
            .len()
            < pixels.len()
    );
    let mut decoded = Vec::new();
    std::io::Read::read_to_end(
        &mut flate2::read::ZlibDecoder::new(
            reopened
                .read_stream_data(compressed.as_stream().unwrap())
                .unwrap()
                .as_ref(),
        ),
        &mut decoded,
    )
    .unwrap();
    assert_eq!(decoded, pixels);
    assert_eq!(doc.get_page(0).unwrap().get_xobject(&name).unwrap(), source);
    // Saving again must not double-compress an already filtered stream.
    let second = reopened
        .save_to_bytes_with_options(SaveOptions {
            compression: StreamCompression::FlateUnencoded,
            ..Default::default()
        })
        .unwrap();
    let again = PdfDocument::from_bytes_with_mode(second, ParseMode::Strict).unwrap();
    assert_eq!(
        again
            .read_stream_data(
                again
                    .get_page(0)
                    .unwrap()
                    .get_xobject(&name)
                    .unwrap()
                    .as_stream()
                    .unwrap()
            )
            .unwrap()
            .as_ref(),
        reopened
            .read_stream_data(compressed.as_stream().unwrap())
            .unwrap()
            .as_ref()
    );
    assert!(again.get_page(0).unwrap().display_list().is_ok());
}

#[test]
fn flate_output_requires_compatible_version_or_explicit_upgrade() {
    use pdf::document::save_options::{PdfVersion, SaveOptions, StreamCompression};
    let mut bytes = PdfDocument::new().unwrap().save_to_bytes().unwrap();
    bytes[5..8].copy_from_slice(b"1.0");
    let doc = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    let options = SaveOptions {
        compression: StreamCompression::FlateUnencoded,
        ..Default::default()
    };
    assert!(matches!(
        doc.save_to_bytes_with_options(options),
        Err(PdfError::UnsupportedFeature(_))
    ));
    let upgraded = doc
        .save_to_bytes_with_options(SaveOptions {
            version: Some(PdfVersion::V1_2),
            ..options
        })
        .unwrap();
    assert!(upgraded.starts_with(b"%PDF-1.2\n"));
}

#[test]
fn final_output_budget_accepts_exact_size_and_rejects_before_sink_writes() {
    use pdf::document::save_options::SaveOptions;
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let size = doc.save_to_bytes().unwrap().len();
    let exact = doc
        .save_to_bytes_with_options(SaveOptions {
            max_output_bytes: size,
            ..Default::default()
        })
        .unwrap();
    assert_eq!(exact.len(), size);
    assert_eq!(
        PdfDocument::from_bytes_with_mode(exact, ParseMode::Strict)
            .unwrap()
            .try_page_count()
            .unwrap(),
        1
    );
    for limit in [0, 8, size - 1] {
        let mut output = b"existing".to_vec();
        assert!(matches!(
            doc.save_to_writer_with_options(
                &mut output,
                SaveOptions {
                    max_output_bytes: limit,
                    ..Default::default()
                }
            ),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(output, b"existing");
    }
}
