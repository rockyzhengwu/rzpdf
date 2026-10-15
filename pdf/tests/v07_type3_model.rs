use pdf::{
    budget::OperationBudget,
    document::{Document, resource_manager::ResourceKind},
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream},
    parser::{
        options::{Limits, ParseOptions},
        syntax::SyntaxParser,
    },
};
use std::io::Cursor;

const FONT: &str = "<< /Type /Font /Subtype /Type3 /FontMatrix [.002 .001 0 .003 3 4] /FontBBox [0 0 600 700] /FirstChar 65 /LastChar 66 /Widths [600 700] /Encoding << /Differences [65 /A /B] >> >>";

fn dict(source: &str) -> PdfDict {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes())).unwrap())
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap()
}

fn document(
    source: &str,
    glyph: &[u8],
    options: ParseOptions,
) -> (Document, String, pdf::objects::object_id::ObjectId) {
    let doc = Document::new_with_options(options).unwrap();
    doc.add_page(0).unwrap();
    let a = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            Default::default(),
            glyph.to_vec(),
        )))
        .unwrap();
    let b = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            Default::default(),
            b"700 0 0 0 600 700 d1 0 0 600 700 re f".to_vec(),
        )))
        .unwrap();
    let mut procs = PdfDict::default();
    procs.insert(
        "A".into(),
        PdfObject::PdfReference(PdfReference::from_id(a)),
    );
    procs.insert(
        "B".into(),
        PdfObject::PdfReference(PdfReference::from_id(b)),
    );
    let mut font = dict(source);
    font.insert("CharProcs".into(), PdfObject::PdfDict(procs));
    let id = doc.try_insert_object(PdfObject::PdfDict(font)).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Font, id)
        .unwrap();
    (doc, name, id)
}

#[test]
fn metrics_use_font_matrix_not_fixed_thousandths_and_keep_d0_d1_distinct() {
    let (doc, name, _) = document(
        FONT,
        b"600 0 d0 1 0 0 rg 0 0 600 700 re f",
        Default::default(),
    );
    let page = doc.get_page(0).unwrap();
    let font = page.inspect_type3_font(&name).unwrap();
    assert_eq!(font.font_matrix(), [0.002, 0.001, 0., 0.003, 3., 4.]);
    assert_eq!(font.font_bbox(), [0., 0., 600., 700.]);
    assert_eq!(font.code_range(), 65..=66);
    assert_eq!(font.glyph_name(65), Some("A"));
    assert_eq!(font.width(65).unwrap(), 600.);
    let advance = font.text_space_advance(65).unwrap();
    assert!((advance[0] - 1.2).abs() < 1e-6 && (advance[1] - 0.6).abs() < 1e-6);
    assert!(font.width(64).is_err());
    let a = page.inspect_type3_glyph(&font, 65).unwrap();
    assert_eq!(a.metrics().width, [600., 0.]);
    assert_eq!(a.metrics().bbox, None);
    let b = page.inspect_type3_glyph(&font, 66).unwrap();
    assert_eq!(b.metrics().bbox, Some([0., 0., 600., 700.]));
    assert_eq!(b.program().operations()[0].operator.name(), "d1");
    assert_eq!(b.program().operations().len(), 3);
}

#[test]
fn font_and_glyph_share_retained_budget_and_retry_without_neighbors() {
    let (doc, name, _) = document(FONT, b"600 0 d0 0 0 600 700 re f", Default::default());
    let page = doc.get_page(0).unwrap();
    let probe = OperationBudget::new(1024 * 1024);
    let font = page.inspect_type3_font_with_budget(&name, &probe).unwrap();
    let glyph = page.inspect_type3_glyph(&font, 65).unwrap();
    let peak = probe.peak_bytes();
    drop(font);
    assert!(probe.live_bytes() > 0);
    assert_eq!(glyph.program().operations().len(), 3);
    drop(glyph);
    assert_eq!(probe.live_bytes(), 0);
    let pool = OperationBudget::new(peak);
    let neighbor = pool.reserve(1).unwrap();
    let failed = match page.inspect_type3_font_with_budget(&name, &pool) {
        Err(PdfError::ResourceLimit(_)) => true,
        Ok(font) => {
            let result = page.inspect_type3_glyph(&font, 65);
            let failed = matches!(result, Err(PdfError::ResourceLimit(_)));
            drop(result);
            drop(font);
            failed
        }
        Err(error) => panic!("unexpected {error:?}"),
    };
    assert!(failed);
    assert_eq!(pool.live_bytes(), 1);
    drop(neighbor);
    let font = page.inspect_type3_font_with_budget(&name, &pool).unwrap();
    drop(page.inspect_type3_glyph(&font, 65).unwrap());
    drop(font);
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn invalid_fields_reject_and_release_every_reservation() {
    for (key, value) in [
        ("Type", "/NotFont"),
        ("Subtype", "/Type1"),
        ("FontMatrix", "[1 0 0 1 0]"),
        ("FontMatrix", "[1 0 0 1 0 /Bad]"),
        ("FontBBox", "[2 0 1 3]"),
        ("FirstChar", "65.5"),
        ("FirstChar", "-1"),
        ("LastChar", "256"),
        ("FirstChar", "67"),
        ("Widths", "[600]"),
        ("Widths", "[600 /Bad]"),
        ("Encoding", "null"),
        ("CharProcs", "[]"),
        ("Resources", "12"),
    ] {
        let (doc, name, id) = document(FONT, b"600 0 d0", Default::default());
        let mut font = doc.resolve_object(id).unwrap().into_dict().unwrap();
        font.insert(
            key.into(),
            dict(&format!("<< /V {value} >>")).get("V").unwrap().clone(),
        );
        doc.update_object(id, PdfObject::PdfDict(font)).unwrap();
        let pool = OperationBudget::new(1024 * 1024);
        assert!(
            doc.get_page(0)
                .unwrap()
                .inspect_type3_font_with_budget(&name, &pool)
                .is_err(),
            "{key} {value}"
        );
        assert_eq!(pool.live_bytes(), 0, "{key} {value}");
    }
}

#[test]
fn indirect_fields_and_font_resources_are_resolved_in_the_same_pool() {
    let (doc, name, id) = document(FONT, b"600 0 d0", Default::default());
    let mut font = doc.resolve_object(id).unwrap().into_dict().unwrap();
    let width = doc
        .try_insert_object(dict("<< /V 625 >>").get("V").unwrap().clone())
        .unwrap();
    let mut values = dict("<< /V [0 700] >>").get("V").unwrap().clone();
    let array = values.as_array().unwrap();
    let mut entries: Vec<_> = array.into_iter().cloned().collect();
    entries[0] = PdfObject::PdfReference(PdfReference::from_id(width));
    let mut array = pdf::objects::pdf_array::PdfArray::default();
    for entry in entries {
        array.add_obj(entry);
    }
    values = PdfObject::PdfArray(array);
    font.insert("Widths".into(), values);
    for key in [
        "FontMatrix",
        "FontBBox",
        "Widths",
        "FirstChar",
        "LastChar",
        "Encoding",
        "CharProcs",
    ] {
        let reference = doc
            .try_insert_object(font.get(key).unwrap().clone())
            .unwrap();
        font.insert(
            key.into(),
            PdfObject::PdfReference(PdfReference::from_id(reference)),
        );
    }
    let resource = doc
        .try_insert_object(PdfObject::PdfDict(dict("<< /ProcSet [/PDF] >>")))
        .unwrap();
    font.insert(
        "Resources".into(),
        PdfObject::PdfReference(PdfReference::from_id(resource)),
    );
    doc.update_object(id, PdfObject::PdfDict(font)).unwrap();
    let page = doc.get_page(0).unwrap();
    let pool = OperationBudget::new(1024 * 1024);
    let font = page.inspect_type3_font_with_budget(&name, &pool).unwrap();
    assert_eq!(font.width(65).unwrap(), 625.);
    assert!(font.resources().unwrap().get("ProcSet").is_some());
    // CharProc metrics remain independent of the width used for text advance.
    assert_eq!(
        page.inspect_type3_glyph(&font, 65).unwrap().metrics().width,
        [600., 0.]
    );
    drop(font);
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn saved_and_imported_charprocs_remain_readable_after_source_drop() {
    let (doc, name, _) = document(
        FONT,
        b"600 0 d0 1 0 0 rg 0 0 600 700 re f",
        Default::default(),
    );
    let bytes = doc.save_to_bytes().unwrap();
    let reopened = Document::from_bytes(bytes).unwrap();
    let target = Document::new().unwrap();
    target.import_pages_from(&reopened, &[0], 0).unwrap();
    drop(reopened);
    drop(doc);
    let target = Document::from_bytes(target.save_to_bytes().unwrap()).unwrap();
    let page = target.get_page(0).unwrap();
    let font = page.inspect_type3_font(&name).unwrap();
    assert_eq!(font.width(65).unwrap(), 600.);
    let glyph = page.inspect_type3_glyph(&font, 65).unwrap();
    assert_eq!(glyph.program().operations()[1].operator.name(), "rg");
}

#[test]
fn cyclic_missing_and_corrupt_charprocs_fail_without_retaining_temporary_data() {
    for case in 0..4 {
        let options = ParseOptions {
            limits: Limits {
                max_depth: 8,
                ..Default::default()
            },
            ..Default::default()
        };
        let (doc, name, id) = document(FONT, b"600 0 d0", options);
        let mut dictionary = doc.resolve_object(id).unwrap().into_dict().unwrap();
        let proc_id = dictionary
            .get("CharProcs")
            .unwrap()
            .as_dict()
            .unwrap()
            .get("A")
            .unwrap()
            .as_object_id()
            .unwrap();
        match case {
            0 => doc
                .update_object(
                    proc_id,
                    PdfObject::PdfReference(PdfReference::from_id(proc_id)),
                )
                .unwrap(),
            1 => doc.update_object(proc_id, PdfObject::PdfNull).unwrap(),
            2 => doc
                .update_object(
                    proc_id,
                    PdfObject::PdfStream(PdfStream::new(
                        dict("<< /Filter /FlateDecode >>"),
                        vec![0x78, 0x9c, 0],
                    )),
                )
                .unwrap(),
            _ => {
                dictionary.insert(
                    "Encoding".into(),
                    PdfObject::PdfDict(dict("<< /Differences [65 /Absent] >>")),
                );
                doc.update_object(id, PdfObject::PdfDict(dictionary))
                    .unwrap();
            }
        }
        let page = doc.get_page(0).unwrap();
        let pool = OperationBudget::new(1024 * 1024);
        let font = page.inspect_type3_font_with_budget(&name, &pool).unwrap();
        let retained = pool.live_bytes();
        assert!(page.inspect_type3_glyph(&font, 65).is_err(), "case {case}");
        assert_eq!(pool.live_bytes(), retained);
        drop(font);
        assert_eq!(pool.live_bytes(), 0);
    }
}

#[test]
fn malformed_charproc_metrics_do_not_leak_or_silently_render() {
    for source in [
        b"".as_slice(),
        b"q 600 0 d0 Q",
        b"600 d0",
        b"600 0 1 d0",
        b"600 1 d0",
        b"600 0 2 0 1 3 d1",
        b"600 0 d0 600 0 d0",
        b"600 0 d0 600 0 0 0 1 1 d1",
    ] {
        let (doc, name, _) = document(FONT, source, Default::default());
        let page = doc.get_page(0).unwrap();
        let pool = OperationBudget::new(1024 * 1024);
        let font = page.inspect_type3_font_with_budget(&name, &pool).unwrap();
        let retained = pool.live_bytes();
        assert!(page.inspect_type3_glyph(&font, 65).is_err(), "{source:?}");
        assert_eq!(pool.live_bytes(), retained);
        drop(font);
        assert_eq!(pool.live_bytes(), 0);
    }
}

#[test]
fn stale_and_foreign_documents_cannot_resolve_snapshot_references() {
    let (doc, name, _) = document(FONT, b"600 0 d0", Default::default());
    let font = doc.get_page(0).unwrap().inspect_type3_font(&name).unwrap();
    let (other, _, _) = document(FONT, b"600 0 d0", Default::default());
    assert!(
        other
            .get_page(0)
            .unwrap()
            .inspect_type3_glyph(&font, 65)
            .is_err()
    );
    doc.add_page(1).unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .inspect_type3_glyph(&font, 65)
            .is_err()
    );
    let refreshed = doc.get_page(0).unwrap().inspect_type3_font(&name).unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .inspect_type3_glyph(&refreshed, 65)
            .is_ok()
    );
}

#[test]
fn glyph_operation_limit_is_enforced_without_changing_document() {
    let options = ParseOptions {
        limits: Limits {
            max_content_operations: 2,
            ..Default::default()
        },
        ..Default::default()
    };
    let (doc, name, _) = document(FONT, b"600 0 d0 0 0 1 1 re f", options);
    let before = doc.save_to_bytes().unwrap();
    let page = doc.get_page(0).unwrap();
    let font = page.inspect_type3_font(&name).unwrap();
    assert!(matches!(
        page.inspect_type3_glyph(&font, 65),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(doc.save_to_bytes().unwrap(), before);
}

#[test]
fn inspect_does_not_enable_unimplemented_type3_page_execution() {
    let (doc, name, _) = document(FONT, b"600 0 d0", Default::default());
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
        .unwrap();
    let mut page = doc.get_page(0).unwrap();
    let font = page.inspect_type3_font(&name).unwrap();
    assert!(page.inspect_type3_glyph(&font, 65).is_ok());
    assert!(matches!(
        page.extract_characters_paint_order(),
        Err(PdfError::UnsupportedFeature(_))
    ));
}
