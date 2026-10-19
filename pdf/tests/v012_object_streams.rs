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
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_number::PdfNumber,
        pdf_string::PdfString,
    },
    parser::{
        options::{Limits, ParseOptions},
        parser::parse_xref,
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::{
    collections::BTreeMap,
    io::{Cursor, Write},
};

const LAYOUT: SaveLayout = SaveLayout::XrefStreamWithObjectStreams;
const MAX: usize = 1024 * 1024;
fn open(bytes: Vec<u8>) -> Document {
    Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap()
}
fn xref(bytes: &[u8]) -> CrossRefTable {
    let mut parser = SyntaxParser::with_mode(
        StreamReader::try_new(Cursor::new(bytes)).unwrap(),
        ParseMode::Strict,
    );
    parser.read_header().unwrap();
    parse_xref(&mut parser).unwrap()
}
fn save(doc: &Document) -> Vec<u8> {
    doc.save_to_bytes_with_layout(Default::default(), LAYOUT)
        .unwrap()
}
fn string(n: usize) -> PdfObject {
    PdfObject::PdfString(PdfString::new(vec![b'x'; n], false))
}
fn containers(bytes: &[u8]) -> Vec<(ObjectId, usize, Vec<u8>)> {
    let table = xref(bytes);
    let doc = open(bytes.to_vec());
    let mut groups = Vec::new();
    for (&number, info) in table.objects() {
        if !matches!(info.state(), ObjectState::Normal) {
            continue;
        }
        let object = doc.resolve_object(info.object_id()).unwrap();
        let Some(stream) = object.as_stream() else {
            continue;
        };
        if !stream
            .dict()
            .get("Type")
            .and_then(PdfObject::as_name)
            .is_some_and(|n| n.name() == "ObjStm")
        {
            continue;
        }
        let data = if stream.dict().contains_key("Filter") {
            pdf::filter::apply_filter("FlateDecode", stream.raw_data(), None).unwrap()
        } else {
            stream.raw_data().to_vec()
        };
        let n = stream.dict().get("N").unwrap().as_u32().unwrap() as usize;
        let first = stream.dict().get("First").unwrap().as_u32().unwrap() as usize;
        assert!(n > 0 && n <= 100 && data.len() <= MAX && first < data.len());
        let header: Vec<usize> = std::str::from_utf8(&data[..first])
            .unwrap()
            .split_whitespace()
            .map(|s| s.parse().unwrap())
            .collect();
        assert_eq!(header.len(), 2 * n);
        assert_eq!(header[1], 0);
        for index in 0..n {
            let id = header[2 * index] as u32;
            let start = first + header[2 * index + 1];
            let end = if index + 1 == n {
                data.len()
            } else {
                first + header[2 * index + 3]
            };
            assert!(start < end);
            let mut parser = SyntaxParser::with_mode(
                StreamReader::try_new(Cursor::new(&data[start..end])).unwrap(),
                ParseMode::Strict,
            );
            let value = parser.get_object().unwrap();
            let record = table.lookup(&id).unwrap();
            assert!(matches!(record.state(), ObjectState::Compressed));
            assert_eq!(record.offset(), u64::from(number));
            assert_eq!(record.gennum(), index as u32);
            assert_eq!(doc.resolve_object(ObjectId::new(id, 0)).unwrap(), value);
        }
        groups.push((ObjectId::new(number, 0), n, data));
    }
    groups.sort_by_key(|g| g.0.obj_num());
    groups
}

#[test]
fn packs_multiple_groups_with_exact_ids_indices_values_and_no_document_mutation() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.set_info_text(InfoTextField::Title, Some("packed title"))
        .unwrap();
    let mut expected = BTreeMap::new();
    for i in 0..205 {
        let mut d = PdfDict::default();
        d.try_insert(
            "Value".into(),
            PdfObject::PdfNumber(PdfNumber::new(i as f32)),
        )
        .unwrap();
        let value = PdfObject::PdfDict(d);
        let id = doc.try_insert_object(value.clone()).unwrap();
        expected.insert(id.obj_num(), value);
    }
    let highest = *expected.keys().max().unwrap();
    let stamp = doc.stamp();
    let commit = doc.commit_revision();
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let output = doc
            .save_to_bytes_with_layout(
                SaveOptions {
                    compression,
                    ..Default::default()
                },
                LAYOUT,
            )
            .unwrap();
        let table = xref(&output);
        let groups = containers(&output);
        assert_eq!(groups.len(), 3);
        assert_eq!(groups[0].1, 100);
        assert_eq!(groups[1].1, 100);
        let reopened = open(output.clone());
        for (&id, value) in &expected {
            assert!(matches!(
                table.lookup(&id).unwrap().state(),
                ObjectState::Compressed
            ));
            assert_eq!(
                reopened.resolve_object(ObjectId::new(id, 0)).unwrap(),
                *value
            );
        }
        assert_eq!(reopened.try_page_count().unwrap(), 1);
        assert_eq!(
            reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
            Some("packed title")
        );
        assert!(
            groups
                .iter()
                .all(|(id, _, _)| doc.resolve_object(*id).is_err())
        );
    }
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.commit_revision(), commit);
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        highest + 1
    );
}

#[test]
fn exact_decoded_byte_boundary_and_large_member_fallback() {
    for (length, compressed) in [(MAX - 8, true), (MAX - 7, true), (MAX - 6, false)] {
        let doc = Document::new().unwrap();
        let object = string(length);
        let id = doc.try_insert_object(object.clone()).unwrap();
        assert_eq!(id.obj_num(), 3);
        let output = save(&doc);
        assert_eq!(
            matches!(
                xref(&output).lookup(&3).unwrap().state(),
                ObjectState::Compressed
            ),
            compressed
        );
        assert_eq!(open(output.clone()).resolve_object(id).unwrap(), object);
        let groups = containers(&output);
        if length == MAX - 7 {
            assert!(groups.iter().any(|g| g.2.len() == MAX && g.1 == 1));
        }
        assert!(groups.iter().all(|g| g.2.len() <= MAX));
    }
}

#[test]
fn byte_grouping_splits_before_count_limit() {
    let doc = Document::new().unwrap();
    for _ in 0..3 {
        doc.try_insert_object(string(600_000)).unwrap();
    }
    let output = save(&doc);
    let groups = containers(&output);
    assert_eq!(groups.len(), 3);
    assert!(groups.iter().all(|g| g.1 < 100));
}

#[test]
fn exact_99_100_101_member_group_boundaries() {
    for count in [99, 100, 101] {
        let doc = Document::new().unwrap();
        // Pages is eligible; Catalog is intentionally not.
        for _ in 1..count {
            doc.try_insert_object(PdfObject::PdfNull).unwrap();
        }
        let output = save(&doc);
        let groups = containers(&output);
        let sizes: Vec<_> = groups.iter().map(|g| g.1).collect();
        assert_eq!(
            sizes,
            if count <= 100 {
                vec![count]
            } else {
                vec![100, 1]
            }
        );
    }
}

#[test]
fn compressed_staging_budget_is_charged_released_and_retryable() {
    let doc = Document::new().unwrap();
    for _ in 0..102 {
        doc.try_insert_object(string(200)).unwrap();
    }
    let options = SaveOptions {
        compression: StreamCompression::FlateUnencoded,
        ..Default::default()
    };
    let pool = OperationBudget::new(32 * 1024 * 1024);
    let output = doc
        .save_with_layout_and_budget(options, LAYOUT, &pool)
        .unwrap();
    let peak = pool.peak_bytes();
    let expected = output.as_slice().to_vec();
    assert!(
        expected
            .windows(b"/Filter /FlateDecode".len())
            .any(|w| w == b"/Filter /FlateDecode")
    );
    drop(output);
    assert_eq!(pool.live_bytes(), 0);
    let short = OperationBudget::new(peak - 1);
    assert!(matches!(
        doc.save_with_layout_and_budget(options, LAYOUT, &short),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(short.live_bytes(), 0);
    assert_eq!(
        doc.save_to_bytes_with_layout(options, LAYOUT).unwrap(),
        expected
    );
}

#[test]
fn generated_xref_payload_intersects_both_stream_byte_limits_exactly() {
    for raw in [false, true] {
        for (maximum, success) in [(64, false), (65, true)] {
            let mut limits = Limits::default();
            if raw {
                limits.max_raw_stream_bytes = maximum;
            } else {
                limits.max_decoded_bytes = maximum;
            }
            let doc = Document::new_with_version_and_options(
                PdfVersion::V1_7,
                ParseOptions {
                    mode: ParseMode::Strict,
                    limits,
                },
            )
            .unwrap();
            assert!(
                doc.save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStream)
                    .is_ok()
            );
            let pool = OperationBudget::new(1024 * 1024);
            let result = doc.save_with_layout_and_budget(Default::default(), LAYOUT, &pool);
            assert_eq!(
                result.is_ok(),
                success,
                "raw={raw}, max={maximum}, result={result:?}"
            );
            drop(result);
            assert_eq!(pool.live_bytes(), 0);
        }
    }
}

fn source(objects: &[(u32, u32, Vec<u8>)], trailer_extra: &str, size: u32) -> Vec<u8> {
    let mut out = b"%PDF-1.7\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = BTreeMap::new();
    for (number, generation, body) in objects {
        offsets.insert(*number, (out.len(), *generation));
        writeln!(out, "{number} {generation} obj").unwrap();
        out.extend(body);
        out.extend(b"\nendobj\n");
    }
    let start = out.len();
    write!(out, "xref\n0 {size}\n0000000000 65535 f \n").unwrap();
    for n in 1..size {
        if let Some((offset, generation)) = offsets.get(&n) {
            writeln!(out, "{offset:010} {generation:05} n ").unwrap();
        } else {
            out.extend(b"0000000000 00000 f \n");
        }
    }
    write!(
        out,
        "trailer\n<< /Size {size} /Root 1 0 R {trailer_extra} >>\nstartxref\n{start}\n%%EOF\n"
    )
    .unwrap();
    out
}
fn base() -> Vec<(u32, u32, Vec<u8>)> {
    vec![
        (1, 0, b"<< /Type /Catalog /Pages 2 0 R >>".to_vec()),
        (2, 0, b"<< /Type /Pages /Kids [] /Count 0 >>".to_vec()),
    ]
}

#[test]
fn streams_generation_length_targets_and_reference_only_objects_stay_standalone() {
    let mut objects = base();
    objects.extend([
        (3, 7, b"(generation seven)".to_vec()),
        (
            4,
            0,
            b"<< /Length 5 0 R >>\nstream\nabc\nendstream".to_vec(),
        ),
        (5, 0, b"3".to_vec()),
        (6, 0, b"5 0 R".to_vec()),
    ]);
    let doc = open(source(&objects, "", 7));
    let output = save(&doc);
    let table = xref(&output);
    for id in [1, 3, 4, 5, 6] {
        assert!(matches!(
            table.lookup(&id).unwrap().state(),
            ObjectState::Normal
        ));
    }
    assert_eq!(table.lookup(&3).unwrap().gennum(), 7);
    assert_eq!(
        open(output).resolve_object(ObjectId::new(3, 7)).unwrap(),
        doc.resolve_object(ObjectId::new(3, 7)).unwrap()
    );
}

#[test]
fn packed_sources_can_be_reopened_modified_repacked_and_saved_classically() {
    let doc = Document::new().unwrap();
    let id = doc.try_insert_object(string(10)).unwrap();
    let first = save(&doc);
    let old = containers(&first);
    let reopened = open(first.clone());
    reopened.update_object(id, string(25)).unwrap();
    let incremental = reopened.save_incremental_to_bytes().unwrap();
    assert!(incremental.starts_with(&first));
    assert!(matches!(
        xref(&incremental).lookup(&id.obj_num()).unwrap().state(),
        ObjectState::Normal
    ));
    assert_eq!(open(incremental).resolve_object(id).unwrap(), string(25));
    let second = save(&reopened);
    let table = xref(&second);
    for (id, _, _) in old {
        assert!(matches!(
            table.lookup(&id.obj_num()).unwrap().state(),
            ObjectState::Free
        ));
    }
    assert_eq!(containers(&second).len(), 1);
    assert_eq!(open(second).resolve_object(id).unwrap(), string(25));
    let classic = reopened.save_to_bytes().unwrap();
    assert_eq!(open(classic).resolve_object(id).unwrap(), string(25));
}

#[test]
fn source_size_high_water_and_generation_survive_output_only_allocations() {
    let mut objects = base();
    objects.push((3, 7, b"(generation seven)".to_vec()));
    let doc = open(source(&objects, "", 16));
    let stamp = doc.stamp();
    let output = save(&doc);
    let table = xref(&output);
    assert_eq!(table.trailer().get("Size").unwrap().as_u32().unwrap(), 18);
    assert_eq!(table.lookup(&3).unwrap().gennum(), 7);
    assert!(matches!(
        table.lookup(&16).unwrap().state(),
        ObjectState::Normal
    ));
    assert_eq!(containers(&output)[0].0, ObjectId::new(17, 0));
    assert!(doc.resolve_object(ObjectId::new(16, 0)).is_err());
    assert!(doc.resolve_object(ObjectId::new(17, 0)).is_err());
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap(),
        ObjectId::new(16, 0)
    );
}

#[test]
fn unknown_xref_container_semantics_are_not_silently_removed() {
    let mut objects = base();
    objects.push((
        3,
        0,
        b"<< /Type /XRef /Size 4 /W [1 8 4] /Length 0 /Private (retain me) >>\nstream\n\nendstream"
            .to_vec(),
    ));
    let doc = open(source(&objects, "", 4));
    assert!(matches!(
        doc.save_to_bytes_with_layout(Default::default(), LAYOUT),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert!(
        doc.save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStream)
            .is_ok()
    );
}

#[test]
fn extends_unknown_container_fields_and_live_container_references_refuse_without_host_writes() {
    for (extra, owner, trailer) in [
        ("/Extends 4 0 R", "", ""),
        ("/RzPrivate (semantic)", "", ""),
        ("", "/Container 3 0 R", ""),
        ("", "", "/RzContainer 3 0 R"),
    ] {
        let mut objects = base();
        objects.push((
            3,
            0,
            format!("<< /Type /ObjStm /N 0 /First 0 /Length 0 {extra} >>\nstream\n\nendstream")
                .into_bytes(),
        ));
        objects.push((4, 0, format!("<< {owner} >>").into_bytes()));
        let doc = open(source(&objects, trailer, 5));
        let stamp = doc.stamp();
        let mut host = b"keep".to_vec();
        assert!(matches!(
            doc.save_to_writer_with_layout(&mut host, Default::default(), LAYOUT),
            Err(PdfError::UnsupportedFeature(_))
        ));
        assert_eq!(host, b"keep");
        assert!(doc.is_stamp_current(&stamp));
        assert!(
            doc.save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStream)
                .is_ok()
        );
    }
}

#[test]
fn dangling_reference_cannot_alias_later_allocated_container() {
    let mut objects = base();
    objects.push((3, 0, b"<< /Dangling 5 0 R >>".to_vec())); // xref 4, first container 5
    let doc = open(source(&objects, "", 4));
    let mut host = b"keep".to_vec();
    assert!(matches!(
        doc.save_to_writer_with_layout(&mut host, Default::default(), LAYOUT),
        Err(PdfError::InvalidReference(_))
    ));
    assert_eq!(host, b"keep");
}

#[test]
fn exact_output_limit_and_budget_failure_release_and_allow_retry() {
    let doc = Document::new().unwrap();
    for _ in 0..104 {
        doc.try_insert_object(string(50)).unwrap();
    }
    let stamp = doc.stamp();
    let expected = save(&doc);
    let exact = SaveOptions {
        max_output_bytes: expected.len(),
        ..Default::default()
    };
    assert_eq!(
        doc.save_to_bytes_with_layout(exact, LAYOUT).unwrap(),
        expected
    );
    assert!(matches!(
        doc.save_to_bytes_with_layout(
            SaveOptions {
                max_output_bytes: expected.len() - 1,
                ..exact
            },
            LAYOUT
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    let pool = OperationBudget::new(32 * 1024 * 1024);
    let result = doc
        .save_with_layout_and_budget(Default::default(), LAYOUT, &pool)
        .unwrap();
    assert_eq!(result.as_slice(), expected);
    let peak = pool.peak_bytes();
    assert!(pool.live_bytes() >= result.as_slice().len());
    drop(result);
    assert_eq!(pool.live_bytes(), 0);
    for maximum in [1, peak - 1] {
        let small = OperationBudget::new(maximum);
        assert!(matches!(
            doc.save_with_layout_and_budget(Default::default(), LAYOUT, &small),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(small.live_bytes(), 0);
    }
    assert_eq!(save(&doc), expected);
    assert!(doc.is_stamp_current(&stamp));
}

#[test]
fn new_container_counts_toward_size_limit_and_stream_quotas() {
    for (maximum, success) in [(14, false), (15, true)] {
        let doc = Document::new_with_version_and_options(
            PdfVersion::V1_7,
            ParseOptions {
                mode: ParseMode::Strict,
                limits: Limits {
                    max_objects: maximum,
                    ..Default::default()
                },
            },
        )
        .unwrap();
        for _ in 0..10 {
            doc.try_insert_object(PdfObject::PdfNull).unwrap();
        }
        let pool = OperationBudget::new(32 * 1024 * 1024);
        assert!(
            doc.save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStream)
                .is_ok()
        );
        assert_eq!(
            doc.save_with_layout_and_budget(Default::default(), LAYOUT, &pool)
                .is_ok(),
            success
        );
        assert_eq!(pool.live_bytes(), 0);
    }
    // Member/decoded cap below 1 MiB; oversized regular objects remain normal.
    let doc = Document::new_with_version_and_options(
        PdfVersion::V1_7,
        ParseOptions {
            mode: ParseMode::Strict,
            limits: Limits {
                max_raw_stream_bytes: 2048,
                ..Default::default()
            },
        },
    )
    .unwrap();
    let id = doc.try_insert_object(string(3000)).unwrap();
    let output = save(&doc);
    assert!(matches!(
        xref(&output).lookup(&id.obj_num()).unwrap().state(),
        ObjectState::Normal
    ));
    assert!(containers(&output).iter().all(|g| g.2.len() <= 2048));
}

#[test]
fn low_version_protection_and_signed_source_guards_apply_to_packed_layout() {
    let doc = Document::new_with_version(PdfVersion::V1_4).unwrap();
    assert!(matches!(
        doc.save_to_bytes_with_layout(Default::default(), LAYOUT),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert!(
        doc.save_to_bytes_with_layout(
            SaveOptions {
                version: Some(PdfVersion::V1_5),
                ..Default::default()
            },
            LAYOUT
        )
        .is_ok()
    );
    let encrypted = Document::new()
        .unwrap()
        .save_to_bytes_with_security(
            Default::default(),
            pdf::security::SaveSecurity::Set(
                &pdf::security::EncryptionOptions::new("user", "owner").unwrap(),
            ),
        )
        .unwrap();
    let encrypted = Document::from_bytes_with_password_and_options(
        encrypted,
        "owner",
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )
    .unwrap();
    let protected = encrypted
        .save_to_bytes_with_layout(Default::default(), LAYOUT)
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
    let hex: Vec<_> = include_str!("fixtures/v011/workflow-certified.pdf.hex")
        .bytes()
        .filter(|b| !b.is_ascii_whitespace())
        .collect();
    let signed = open(
        hex.as_chunks::<2>()
            .0
            .iter()
            .map(|p| u8::from_str_radix(std::str::from_utf8(p).unwrap(), 16).unwrap())
            .collect(),
    );
    assert!(
        signed
            .save_to_bytes_with_layout(Default::default(), LAYOUT)
            .is_err()
    );
}

#[test]
fn host_writer_failure_does_not_consume_source_or_output_identity() {
    struct Fail;
    impl Write for Fail {
        fn write(&mut self, _: &[u8]) -> std::io::Result<usize> {
            Err(std::io::Error::other("host failure"))
        }
        fn flush(&mut self) -> std::io::Result<()> {
            Ok(())
        }
    }
    let doc = Document::new().unwrap();
    let expected = save(&doc);
    let stamp = doc.stamp();
    assert!(
        doc.save_to_writer_with_layout(&mut Fail, Default::default(), LAYOUT)
            .is_err()
    );
    assert_eq!(save(&doc), expected);
    assert!(doc.is_stamp_current(&stamp));
}
