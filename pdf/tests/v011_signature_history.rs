use pdf::{
    budget::OperationBudget,
    document::{Document, metadata::InfoTextField},
    io::stream_reader::StreamReader,
    objects::PdfObject,
    parser::{
        parser::parse_xref,
        syntax::{ParseMode, SyntaxParser},
    },
    signatures::{SignatureObjectState, SignatureVerification},
};
use std::io::Cursor;

fn open(bytes: Vec<u8>) -> Document {
    Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap()
}
fn placeholder() -> Vec<u8> {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let prepared = doc.prepare_signature(Default::default()).unwrap();
    let (a, b) = prepared.signed_ranges();
    let mut result = a.to_vec();
    result.push(b'<');
    result.extend(std::iter::repeat_n(
        b'0',
        (prepared.byte_range()[2] - prepared.byte_range()[1] - 2) as usize,
    ));
    result.push(b'>');
    result.extend_from_slice(b);
    result
}
fn xref(bytes: &[u8]) -> pdf::document::cross_ref_table::CrossRefTable {
    let mut parser = SyntaxParser::with_mode(
        StreamReader::try_new(Cursor::new(bytes)).unwrap(),
        ParseMode::Strict,
    );
    parser.read_header().unwrap();
    parse_xref(&mut parser).unwrap()
}
fn append(mut bytes: Vec<u8>, replacements: &[(u32, Option<&str>)]) -> Vec<u8> {
    let table = xref(&bytes);
    let root = table.trailer().get("Root").unwrap().as_object_id().unwrap();
    let size = table.trailer().get("Size").unwrap().as_u32().unwrap();
    let start = bytes.windows(9).rposition(|w| w == b"startxref").unwrap();
    let prev = std::str::from_utf8(&bytes[start + 9..])
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap();
    let prev: usize = prev.parse().unwrap();
    let mut entries = Vec::new();
    for (number, value) in replacements {
        if let Some(value) = value {
            bytes.push(b'\n');
            entries.push((*number, bytes.len(), 0, 'n'));
            bytes.extend_from_slice(format!("{number} 0 obj\n{value}\nendobj\n").as_bytes());
        } else {
            entries.push((*number, 0, 1, 'f'));
        }
    }
    let offset = bytes.len();
    bytes.extend_from_slice(b"xref\n");
    for (number, address, generation, kind) in entries {
        bytes.extend_from_slice(
            format!("{number} 1\n{address:010} {generation:05} {kind} \n").as_bytes(),
        );
    }
    bytes.extend_from_slice(
        format!(
            "trailer\n<< /Size {size} /Root {} {} R /Prev {prev} >>\nstartxref\n{offset}\n%%EOF\n",
            root.obj_num(),
            root.gen_num()
        )
        .as_bytes(),
    );
    bytes
}

#[test]
fn structural_report_and_crypto_result_are_distinct_and_budget_is_released() {
    let bytes = placeholder();
    let doc = open(bytes.clone());
    let stamp = doc.stamp();
    let report = doc.signature_reports().unwrap().remove(0);
    assert_eq!(report.signed_revision, 1);
    assert_eq!(report.field.unwrap().name, "Signature1");
    assert_eq!(report.source_object_state, SignatureObjectState::Unchanged);
    assert_eq!(report.verification, SignatureVerification::NotPerformed);
    assert!(report.covers_current_document);
    assert_eq!(
        doc.verify_signatures().unwrap()[0].verification,
        SignatureVerification::Malformed
    );
    let small = OperationBudget::new(1);
    assert!(doc.verify_signatures_with_budget(&small).is_err());
    assert_eq!(small.live_bytes(), 0);
    let pool = OperationBudget::new(8 * 1024 * 1024);
    assert_eq!(doc.verify_signatures_with_budget(&pool).unwrap().len(), 1);
    assert_eq!(pool.live_bytes(), 0);
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.commit_revision(), 0);
}

#[test]
fn history_reports_replaced_and_freed_signatures_and_source_stays_locked() {
    let bytes = placeholder();
    let original = open(bytes.clone());
    let report = original.signature_reports().unwrap().remove(0);
    for value in [Some("null"), None] {
        let appended = append(bytes.clone(), &[(report.object_id.obj_num(), value)]);
        assert!(appended.starts_with(&bytes));
        let doc = open(appended);
        let historical = doc.signature_reports().unwrap().remove(0);
        assert!(!historical.covers_current_document);
        assert_eq!(historical.signed_revision_xref, report.signed_revision_xref);
        assert_eq!(historical.signed_revision_bytes, bytes.len() as u64);
        assert_eq!(
            historical.source_object_state,
            if value.is_some() {
                SignatureObjectState::Replaced
            } else {
                SignatureObjectState::Removed
            }
        );
        assert!(
            doc.set_info_text(InfoTextField::Title, Some("must remain locked"))
                .is_err()
        );
        assert!(doc.save_incremental_to_bytes().is_err());
        assert!(doc.save_to_bytes().is_err());
    }
}

#[test]
fn removing_every_current_signature_marker_cannot_erase_source_policy() {
    let bytes = placeholder();
    let doc = open(bytes.clone());
    let table = xref(&bytes);
    let root = table.trailer().get("Root").unwrap().as_object_id().unwrap();
    let mut replacements = Vec::new();
    // Replace every signature-bearing current dictionary with null, including
    // Catalog/AcroForm and Widget. Keep a minimal valid catalog/page tree.
    let catalog = doc.resolve_object(root).unwrap();
    let pages = catalog
        .as_dict()
        .unwrap()
        .get("Pages")
        .unwrap()
        .as_object_id()
        .unwrap();
    let root_text = format!(
        "<< /Type /Catalog /Pages {} {} R >>",
        pages.obj_num(),
        pages.gen_num()
    );
    for info in table.objects().values() {
        if let Ok(PdfObject::PdfDict(dict)) = doc.resolve_object(info.object_id())
            && (dict.contains_key("ByteRange")
                || dict.contains_key("SigFlags")
                || dict
                    .get("FT")
                    .and_then(PdfObject::as_name)
                    .is_some_and(|n| n.name() == "Sig"))
        {
            replacements.push((info.object_id().obj_num(), Some("null")));
        }
    }
    replacements.retain(|(id, _)| *id != root.obj_num());
    replacements.push((root.obj_num(), Some(root_text.as_str())));
    let altered = append(bytes, &replacements);
    let doc = open(altered.clone());
    assert_eq!(doc.signature_reports().unwrap().len(), 1);
    assert!(doc.add_page(0).is_err());
    assert!(doc.save_to_bytes().is_err());
    // The same policy also applies to the generic reader API.
    let generic =
        pdf::document::PdfDocument::from_reader_with_mode(Cursor::new(altered), ParseMode::Strict)
            .unwrap();
    assert!(generic.add_page(0).is_err());
    assert!(generic.save_to_bytes().is_err());
}

#[test]
fn nested_revision_chain_deduplicates_the_same_signature() {
    let mut bytes = placeholder();
    for _ in 0..3 {
        // A new revision can repeat the catalog without changing a signature.
        let table = xref(&bytes);
        let root = table.trailer().get("Root").unwrap().as_object_id().unwrap();
        let doc = open(bytes.clone());
        let catalog = doc.resolve_object(root).unwrap();
        let pages = catalog
            .as_dict()
            .unwrap()
            .get("Pages")
            .unwrap()
            .as_object_id()
            .unwrap();
        let text = format!(
            "<< /Type /Catalog /Pages {} {} R >>",
            pages.obj_num(),
            pages.gen_num()
        );
        bytes = append(bytes, &[(root.obj_num(), Some(&text))]);
    }
    let reports = open(bytes).signature_reports().unwrap();
    assert_eq!(reports.len(), 1);
    assert_eq!(reports[0].signed_revision, 1);
    assert!(!reports[0].covers_current_document);
}

#[test]
fn signed_revision_end_must_be_a_real_chain_boundary() {
    let mut bytes = placeholder();
    let doc = open(bytes.clone());
    let range = doc.signature_reports().unwrap()[0].byte_range;
    let before = format!(
        "[{:020} {:020} {:020} {:020}]",
        range[0], range[1], range[2], range[3]
    );
    let after = format!(
        "[{:020} {:020} {:020} {:020}]",
        range[0],
        range[1],
        range[2],
        range[3] - 8
    );
    let pos = bytes
        .windows(before.len())
        .position(|w| w == before.as_bytes())
        .unwrap();
    bytes[pos..pos + before.len()].copy_from_slice(after.as_bytes());
    assert!(open(bytes).signature_reports().is_err());
}

#[test]
fn retired_object_stream_members_cannot_hide_signature_policy_markers() {
    use pdf::objects::{
        pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber, pdf_stream::PdfStream,
    };
    for marker in [false, true] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let mut dict = PdfDict::default();
        dict.try_insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new("ObjStm".into())),
        )
        .unwrap();
        dict.try_insert("N".into(), PdfObject::PdfNumber(PdfNumber::new(1.)))
            .unwrap();
        dict.try_insert("First".into(), PdfObject::PdfNumber(PdfNumber::new(6.)))
            .unwrap();
        let payload = if marker {
            b"900 0 << /FT /Sig >>".to_vec()
        } else {
            b"900 0 << /Test 1 >>".to_vec()
        };
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, payload)))
            .unwrap();
        let source = doc.save_to_bytes().unwrap();
        let original = open(source.clone());
        assert_eq!(
            original
                .set_info_text(InfoTextField::Title, Some("edit"))
                .is_err(),
            marker
        );
        for replacement in [Some("null"), None] {
            let bytes = append(source.clone(), &[(id.obj_num(), replacement)]);
            let opened = open(bytes);
            assert_eq!(
                opened
                    .set_info_text(InfoTextField::Title, Some("edit"))
                    .is_err(),
                marker
            );
        }
    }
}

#[test]
fn fake_old_eof_is_not_a_historical_boundary() {
    let bytes = placeholder();
    let signature = open(bytes.clone()).signature_reports().unwrap().remove(0);
    let old_eof = bytes.windows(5).rposition(|w| w == b"%%EOF").unwrap();
    let mut appended = append(bytes, &[(signature.object_id.obj_num(), Some("null"))]);
    appended[old_eof + 2] = b'X';
    assert!(open(appended).signature_reports().is_err());
}

#[test]
fn field_ownership_and_name_tables_share_the_callers_budget() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    for index in 0..96 {
        doc.add_text_field(0, [10., 10., 80., 30.], &format!("Field{index}"), "")
            .unwrap();
    }
    let prepared = doc.prepare_signature(Default::default()).unwrap();
    let (a, b) = prepared.signed_ranges();
    let mut bytes = a.to_vec();
    bytes.push(b'<');
    bytes.extend(std::iter::repeat_n(
        b'0',
        (prepared.byte_range()[2] - prepared.byte_range()[1] - 2) as usize,
    ));
    bytes.push(b'>');
    bytes.extend_from_slice(b);
    let small = OperationBudget::new(bytes.len() * 8 + 1024 * 1024 + 512 * 1024);
    let opened = open(bytes);
    let stamp = opened.stamp();
    assert!(opened.verify_signatures_with_budget(&small).is_err());
    assert_eq!(small.live_bytes(), 0);
    let pool = OperationBudget::new(16 * 1024 * 1024);
    assert_eq!(
        opened.verify_signatures_with_budget(&pool).unwrap().len(),
        1
    );
    assert_eq!(pool.live_bytes(), 0);
    assert!(opened.is_stamp_current(&stamp));
}

#[test]
fn active_object_alias_inside_contents_is_not_signed_document_coverage() {
    let mut bytes = placeholder();
    let report = open(bytes.clone()).signature_reports().unwrap().remove(0);
    let range = report.byte_range;
    let size = xref(&bytes)
        .trailer()
        .get("Size")
        .unwrap()
        .as_u32()
        .unwrap();
    let a = range[1] as usize;
    let b = range[2] as usize;
    bytes[a] = b'(';
    bytes[b - 1] = b')';
    let alias = format!("{size} 0 obj << /Test 1 >> endobj ");
    bytes[a + 1..a + 1 + alias.len()].copy_from_slice(alias.as_bytes());
    let marker = bytes.windows(9).rposition(|w| w == b"startxref").unwrap();
    let address: usize = std::str::from_utf8(&bytes[marker + 9..])
        .unwrap()
        .split_whitespace()
        .next()
        .unwrap()
        .parse()
        .unwrap();
    let trailer = address
        + bytes[address..]
            .windows(7)
            .position(|w| w == b"trailer")
            .unwrap();
    let mut result = bytes[..trailer].to_vec();
    result.extend_from_slice(format!("{size} 1\n{:010} 00000 n \n", a + 1).as_bytes());
    let tail = std::str::from_utf8(&bytes[trailer..]).unwrap().replacen(
        &format!("/Size {size}"),
        &format!("/Size {}", size + 1),
        1,
    );
    result.extend_from_slice(tail.as_bytes());
    let old = format!(
        "[{:020} {:020} {:020} {:020}]",
        range[0], range[1], range[2], range[3]
    );
    let new = format!(
        "[{:020} {:020} {:020} {:020}]",
        range[0],
        range[1],
        range[2],
        result.len() as u64 - range[2]
    );
    let pos = result
        .windows(old.len())
        .position(|w| w == old.as_bytes())
        .unwrap();
    result[pos..pos + old.len()].copy_from_slice(new.as_bytes());
    let doc = open(result);
    assert!(
        doc.resolve_object(pdf::objects::object_id::ObjectId::new(size, 0))
            .unwrap()
            .as_dict()
            .unwrap()
            .contains_key("Test")
    );
    assert!(
        doc.signature_reports()
            .unwrap_err()
            .to_string()
            .contains("overlaps excluded")
    );
    assert!(doc.verify_signatures().is_err());
}
