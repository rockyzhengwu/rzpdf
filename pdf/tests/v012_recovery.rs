use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        recovery::{RecoveredDocument, RecoveryLimits},
        save_options::{SaveLayout, SaveOptions},
    },
    objects::object_id::ObjectId,
    parser::{
        diagnostic::{ReadProvenance, StructureReason},
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};

fn source(extra: &[u8], trailer: &str, xref: bool) -> Vec<u8> {
    let content = b"0 0 1 rg 10 10 40 30 re f\n% 99 0 obj trailer << /Root 99 0 R >> endobj\n";
    let mut bytes = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = Vec::new();
    let objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R] /Count 1 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 160 120] /Resources << >> /Contents 4 0 R >>"
            .to_vec(),
        {
            let mut s = format!("<< /Length {} >>\nstream\n", content.len()).into_bytes();
            s.extend_from_slice(content);
            s.extend_from_slice(b"endstream");
            s
        },
    ];
    for (i, object) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        bytes.extend_from_slice(object);
        bytes.extend_from_slice(b"\nendobj\n");
    }
    bytes.extend_from_slice(extra);
    let offset = bytes.len();
    if xref {
        bytes.extend_from_slice(b"xref\n0 5\n0000000000 65535 f \n");
        for n in offsets {
            bytes.extend_from_slice(format!("{n:010} 00000 n \n").as_bytes());
        }
    }
    bytes.extend_from_slice(
        format!(
            "trailer\n<< /Size 10 /Root 1 0 R {trailer} >>\nstartxref\n{}\n%%EOF\n",
            if xref { offset } else { 0 }
        )
        .as_bytes(),
    );
    bytes
}
fn broken_start(mut bytes: Vec<u8>) -> Vec<u8> {
    let p = bytes
        .windows(10)
        .rposition(|w| w == b"startxref\n")
        .unwrap()
        + 10;
    let end = bytes[p..].iter().position(|b| *b == b'\n').unwrap() + p;
    bytes.splice(p..end, b"0".iter().copied());
    bytes
}
#[test]
fn strict_pair_bad_start_and_missing_table_recover_without_stream_false_objects() {
    let original = source(b"", "", true);
    let strict = Document::from_bytes_with_mode(original.clone(), ParseMode::Strict).unwrap();
    assert!(strict.structure_diagnostics().unwrap().is_empty());
    for bytes in [broken_start(original), source(b"", "", false)] {
        assert!(Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).is_err());
        assert!(Document::from_bytes(bytes.clone()).is_err());
        let recovered = RecoveredDocument::from_bytes(bytes).unwrap();
        assert_eq!(recovered.provenance(), ReadProvenance::RebuiltReadOnly);
        assert_eq!(recovered.try_page_count().unwrap(), 1);
        assert_eq!(
            recovered.display_list(0).unwrap().objects().len(),
            strict
                .get_page(0)
                .unwrap()
                .display_list()
                .unwrap()
                .objects()
                .len()
        );
        assert_eq!(recovered.diagnostics().len(), 1);
        assert_eq!(recovered.diagnostics()[0].object, Some(ObjectId::new(1, 0)));
    }
}
#[test]
fn recovery_refuses_ambiguous_ownership_security_and_history() {
    for extra in [
        b"1 0 obj null endobj\n".as_slice(),
        b"5 0 obj << /Type /ObjStm /N 0 >> endobj\n",
        b"5 0 obj << /Type /XRef >> endobj\n",
        b"5 0 obj << /Type 6 0 R >> endobj\n",
        b"5 0 obj << /ByteRange [0 1 2 3] >> endobj\n",
        b"%%EOF\n5 0 obj null endobj\n",
        b"junk\n",
        b"5 0 obj << /FT /Sig >> endobj\n",
    ] {
        assert!(
            RecoveredDocument::from_bytes(source(extra, "", false)).is_err(),
            "{extra:?}"
        );
    }
    for trailer in [
        "/Prev 0",
        "/XRefStm 100",
        "/Encrypt null",
        "/Root 3 0 R",
        "/Root 1 0 R /Root 1 0 R",
        "/Size 2",
    ] {
        assert!(
            RecoveredDocument::from_bytes(source(b"", trailer, false)).is_err(),
            "{trailer}"
        );
    }
    let mut trailing = source(b"", "", false);
    trailing.extend_from_slice(b"5 0 obj null endobj");
    assert!(RecoveredDocument::from_bytes(trailing).is_err());
}
#[test]
fn recovery_requires_exact_direct_stream_boundaries() {
    let bytes = source(b"", "", false);
    for marker in [b"/Length 1 ".as_slice(), b"/Length 9 0 R ", b"/Missing 71 "] {
        let start = bytes.windows(8).position(|w| w == b"/Length ").unwrap();
        let end = start + bytes[start..].iter().position(|b| *b == b'>').unwrap();
        let mut bad = bytes.clone();
        bad.splice(start..end, marker.iter().copied());
        assert!(RecoveredDocument::from_bytes(bad).is_err());
    }
}
#[test]
fn recovery_limits_release_and_retry() {
    let bytes = source(b"", "", false);
    for limits in [
        RecoveryLimits {
            max_input_bytes: bytes.len() - 1,
            ..Default::default()
        },
        RecoveryLimits {
            max_candidates: 3,
            ..Default::default()
        },
        RecoveryLimits {
            max_scan_steps: 1,
            ..Default::default()
        },
    ] {
        let budget = OperationBudget::new(1 << 20);
        assert!(
            RecoveredDocument::from_bytes_with_budget(
                bytes.clone(),
                ParseOptions::default(),
                limits,
                &budget
            )
            .is_err()
        );
        assert_eq!(budget.live_bytes(), 0);
        let recovered = RecoveredDocument::from_bytes_with_budget(
            bytes.clone(),
            ParseOptions::default(),
            RecoveryLimits::default(),
            &budget,
        )
        .unwrap();
        assert_eq!(recovered.try_page_count().unwrap(), 1);
        drop(recovered);
        assert_eq!(budget.live_bytes(), 0);
    }
    let budget = OperationBudget::new(0);
    assert!(
        RecoveredDocument::from_bytes_with_budget(
            bytes,
            ParseOptions::default(),
            RecoveryLimits::default(),
            &budget
        )
        .is_err()
    );
    assert_eq!(budget.live_bytes(), 0);
}
#[test]
fn recovery_exact_scan_and_candidate_thresholds() {
    let bytes = source(b"", "", false);
    let doc = RecoveredDocument::from_bytes(bytes.clone()).unwrap();
    let steps = doc.scan_steps();
    for (limit, success) in [(steps, true), (steps - 1, false)] {
        assert_eq!(
            RecoveredDocument::from_bytes_with_options(
                bytes.clone(),
                ParseOptions::default(),
                RecoveryLimits {
                    max_scan_steps: limit,
                    max_candidates: 4,
                    max_input_bytes: bytes.len()
                }
            )
            .is_ok(),
            success
        );
    }
}
#[test]
fn compatible_report_is_lazy_separate_and_cannot_relabel_source() {
    let mut bytes = source(b"", "", true);
    bytes.extend_from_slice(b"trailing");
    assert!(Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).is_err());
    let doc = Document::from_bytes(bytes).unwrap();
    assert_eq!(
        doc.read_provenance(),
        ReadProvenance::Parsed(ParseMode::Compatible)
    );
    assert!(
        doc.structure_diagnostics()
            .unwrap()
            .iter()
            .any(|d| d.reason == StructureReason::CompatibleEof)
    );
    doc.set_metadata_packet(Some(b"opaque".to_vec())).unwrap();
    assert!(doc.save_incremental_to_bytes().is_err());
    assert!(doc.save_to_bytes().is_ok());
    assert_eq!(
        doc.read_provenance(),
        ReadProvenance::Parsed(ParseMode::Compatible)
    );
    for layout in [
        SaveLayout::Classic,
        SaveLayout::XrefStream,
        SaveLayout::XrefStreamWithObjectStreams,
    ] {
        assert!(
            doc.save_to_bytes_with_layout(SaveOptions::default(), layout)
                .is_ok()
        );
    }
}
#[test]
fn compatible_stream_fallback_and_dictionary_duplicate_report_offsets() {
    let mut bytes = source(b"", "", true);
    let offset = bytes.windows(8).position(|w| w == b"/Length ").unwrap();
    bytes[offset..offset + 7].copy_from_slice(b"/Absent");
    let doc = Document::from_bytes(bytes).unwrap();
    assert!(
        doc.structure_diagnostics()
            .unwrap()
            .iter()
            .all(|d| d.reason != StructureReason::StreamBoundarySearch)
    );
    doc.get_page(0).unwrap().display_list().unwrap();
    assert!(
        doc.structure_diagnostics()
            .unwrap()
            .iter()
            .any(|d| d.reason == StructureReason::StreamBoundarySearch && d.offset > 0)
    );
    let mut bytes = source(b"", "", true);
    let start = bytes.windows(6).position(|w| w == b"/Count").unwrap();
    // Same-length replacement gives a duplicate Type key in Pages.
    bytes[start..start + 6].copy_from_slice(b"/Type ");
    let doc = Document::from_bytes(bytes).unwrap();
    let _ = doc.try_page_count();
    assert!(
        doc.structure_diagnostics()
            .unwrap()
            .iter()
            .any(|d| d.reason == StructureReason::DuplicateDictionaryKey)
    );
}
#[test]
fn ordinary_reader_and_created_provenance_are_immutable() {
    let new = Document::new().unwrap();
    assert_eq!(new.read_provenance(), ReadProvenance::Created);
    let strict = Document::from_reader_with_options(
        std::io::Cursor::new(source(b"", "", true)),
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )
    .unwrap();
    assert_eq!(
        strict.read_provenance(),
        ReadProvenance::Parsed(ParseMode::Strict)
    );
    assert!(strict.save_incremental_to_bytes().is_err());
}
#[test]
fn recovery_parser_limits_are_not_bypassed() {
    for limits in [
        Limits {
            max_token_bytes: 2,
            ..Default::default()
        },
        Limits {
            max_depth: 1,
            ..Default::default()
        },
        Limits {
            max_raw_stream_bytes: 1,
            ..Default::default()
        },
        Limits {
            max_objects: 3,
            ..Default::default()
        },
    ] {
        assert!(
            RecoveredDocument::from_bytes_with_options(
                source(b"", "", false),
                ParseOptions {
                    limits,
                    ..Default::default()
                },
                RecoveryLimits::default()
            )
            .is_err()
        );
    }
}

#[test]
fn compatible_xref_relaxations_are_reported_without_content_events() {
    let bytes = source(b"", "", true);
    let mut loose = bytes.clone();
    let at = loose
        .windows(18)
        .position(|w| w == b"0000000000 65535 f")
        .unwrap();
    loose.remove(at);
    assert!(Document::from_bytes_with_mode(loose.clone(), ParseMode::Strict).is_err());
    let doc = Document::from_bytes(loose).unwrap();
    assert!(
        doc.structure_diagnostics()
            .unwrap()
            .iter()
            .any(|d| d.reason == StructureReason::CompatibleXrefEntry
                && d.object == Some(ObjectId::new(0, 65535)))
    );
    assert!(
        doc.get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .diagnostics()
            .is_empty()
    );
    let original = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    let mut output = original
        .save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStream)
        .unwrap();
    let at = output
        .windows(11)
        .position(|w| w == b"/Type /XRef")
        .unwrap();
    output[at..at + 5].copy_from_slice(b"/Kind");
    assert!(Document::from_bytes_with_mode(output.clone(), ParseMode::Strict).is_err());
    let doc = Document::from_bytes(output).unwrap();
    assert!(
        doc.structure_diagnostics()
            .unwrap()
            .iter()
            .any(|d| d.reason == StructureReason::CompatibleXrefStreamType)
    );
}

#[test]
fn recovery_strings_and_early_revision_markers_do_not_confuse_scanning() {
    let extra = b"5 0 obj (999 0 obj trailer << /Root 999 0 R >> endobj) endobj\n";
    assert!(RecoveredDocument::from_bytes(source(extra, "", false)).is_ok());
    let bytes = source(b"", "", false);
    let mut early = bytes.clone();
    early.splice(15..15, b"%%EOF\n".iter().copied());
    assert!(RecoveredDocument::from_bytes(early).is_err());
    for extra in [
        b"5 0 obj << /SubFilter /adbe.pkcs7.detached >> endobj\n".as_slice(),
        b"5 0 obj << /SigFlags 3 >> endobj\n",
    ] {
        assert!(RecoveredDocument::from_bytes(source(extra, "", false)).is_err());
    }
}

#[test]
fn diagnostics_do_not_add_body_line_or_xref_token_size_requirements() {
    use pdf::{io::stream_reader::StreamReader, parser::syntax::SyntaxParser};
    let options = ParseOptions {
        limits: Limits {
            max_token_bytes: 10,
            ..Default::default()
        },
        ..Default::default()
    };
    let doc = Document::from_bytes_with_options(source(b"", "", true), options).unwrap();
    assert_eq!(doc.try_page_count().unwrap(), 1);
    assert!(doc.structure_diagnostics().unwrap().is_empty());
    for binary in [false, true] {
        let mut bytes = b"%PDF-1.7\n".to_vec();
        if binary {
            bytes.push(b'%');
        }
        bytes.extend_from_slice(&[b'x'; 100]);
        if binary {
            bytes.extend_from_slice(b"\xe2\xe3\xcf\xd3");
        }
        bytes.push(b'\n');
        let mut parser = SyntaxParser::with_options(
            StreamReader::try_new(std::io::Cursor::new(bytes)).unwrap(),
            options,
        );
        assert_eq!(parser.read_header().unwrap().version(), "1.7");
        assert_eq!(parser.position(), 0);
    }
}
