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
    assert!((advance[0] - 1.2).abs() < 1e-6 && advance[1] == 0.);
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
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
        .unwrap();
    let bytes = doc.save_to_bytes().unwrap();
    let reopened = Document::from_bytes(bytes).unwrap();
    let target = Document::new().unwrap();
    target.import_pages_from(&reopened, &[0], 0).unwrap();
    drop(reopened);
    drop(doc);
    let target = Document::from_bytes(target.save_to_bytes().unwrap()).unwrap();
    let mut page = target.get_page(0).unwrap();
    let font = page.inspect_type3_font(&name).unwrap();
    assert_eq!(font.width(65).unwrap(), 600.);
    let glyph = page.inspect_type3_glyph(&font, 65).unwrap();
    assert_eq!(glyph.program().operations()[1].operator.name(), "rg");
    assert_eq!(
        page.extract_characters_paint_order().unwrap()[0]
            .unicode
            .as_deref(),
        Some("A")
    );
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
fn inspection_and_page_execution_share_the_type3_model() {
    let (doc, name, _) = document(FONT, b"600 0 d0", Default::default());
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
        .unwrap();
    let mut page = doc.get_page(0).unwrap();
    let font = page.inspect_type3_font(&name).unwrap();
    assert!(page.inspect_type3_glyph(&font, 65).is_ok());
    let chars = page.extract_characters_paint_order().unwrap();
    assert_eq!(chars.len(), 1);
    assert_eq!(chars[0].unicode.as_deref(), Some("A"));
}

#[test]
fn type3_text_advance_and_draw_sources_are_separate() {
    use pdf::page::page_object::PageObject;
    let (doc, name, _) = document(FONT, b"600 0 d0 0 0 500 700 re f", Default::default());
    doc.replace_page_contents(
        0,
        format!("BT /{name} 12 Tf 50 Tz 1 Tc 1 0 0 1 20 30 Tm [(A) 100 (B)] TJ ET 0 0 2 2 re f")
            .into_bytes(),
    )
    .unwrap();
    let mut page = doc.get_page(0).unwrap();
    let characters = page.extract_characters_paint_order().unwrap();
    assert_eq!(characters.len(), 2);
    assert_eq!(characters[0].unicode.as_deref(), Some("A"));
    assert_eq!(characters[1].unicode.as_deref(), Some("B"));
    assert!((characters[1].position.x() - 27.1).abs() < 0.0001);
    assert!((characters[1].position.y() - 762.0).abs() < 0.0001);
    assert_eq!(characters[0].array_item_index, Some(0));
    assert_eq!(characters[1].array_item_index, Some(2));
    let list = page.display_list().unwrap();
    let paths: Vec<_> = list
        .objects()
        .iter()
        .filter_map(|object| {
            if let PageObject::Path(path) = object {
                Some(path)
            } else {
                None
            }
        })
        .collect();
    assert_eq!(paths.len(), 3);
    for glyph in &paths[..2] {
        assert!(glyph.source().is_none() && glyph.nested_source().is_none());
    }
    assert!(paths[2].source().is_some());
    let glyph = paths[0].matrix();
    assert!((glyph.a() - 0.012).abs() < 1e-6);
    assert!((glyph.b() + 0.012).abs() < 1e-6);
    assert!((glyph.e() - 38.).abs() < 1e-6);
    assert!((glyph.f() - 714.).abs() < 1e-6);
}

#[test]
fn type3_tounicode_preserves_multiple_scalars_and_missing_mapping() {
    let (doc, name, id) = document(FONT, b"600 0 d0", Default::default());
    let mut font = doc.resolve_object(id).unwrap().into_dict().unwrap();
    let cmap = b"1 begincodespacerange <00> <FF> endcodespacerange 1 beginbfchar <41> <D83DDE000041> endbfchar";
    let map = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            Default::default(),
            cmap.to_vec(),
        )))
        .unwrap();
    font.insert(
        "ToUnicode".into(),
        PdfObject::PdfReference(PdfReference::from_id(map)),
    );
    doc.update_object(id, PdfObject::PdfDict(font)).unwrap();
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf (AB) Tj ET").into_bytes())
        .unwrap();
    let chars = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(chars[0].unicode.as_deref(), Some("😀A"));
    assert_eq!(chars[1].unicode, None);
}

#[test]
fn type3_d1_rejects_colour_and_nested_text_before_success() {
    for glyph in [
        b"600 0 0 0 600 700 d1 1 0 0 rg 0 0 600 700 re f".as_slice(),
        b"600 0 d0 BT ET",
        b"600 0 d0 unknown",
    ] {
        let (doc, name, _) = document(FONT, glyph, Default::default());
        doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
            .unwrap();
        let before = doc.save_to_bytes().unwrap();
        assert!(doc.get_page(0).unwrap().display_list().is_err());
        assert_eq!(doc.save_to_bytes().unwrap(), before);
    }
}

#[test]
fn type3_shared_logical_work_counts_each_glyph_invocation() {
    let options = ParseOptions {
        limits: Limits {
            max_content_operations: 12,
            ..Default::default()
        },
        ..Default::default()
    };
    let (doc, name, _) = document(FONT, b"600 0 d0 0 0 500 700 re f", options);
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf (AAAA) Tj ET").into_bytes())
        .unwrap();
    assert!(matches!(
        doc.get_page(0).unwrap().display_list(),
        Err(PdfError::ResourceLimit(_))
    ));
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
        .unwrap();
    assert!(doc.get_page(0).unwrap().display_list().is_ok());
}

#[test]
fn type3_form_resources_and_d1_rules_are_preserved() {
    for uncoloured in [false, true] {
        let program = if uncoloured {
            b"600 0 0 0 600 700 d1 /Inner Do".as_slice()
        } else {
            b"600 0 d0 /Inner Do"
        };
        let (doc, name, id) = document(FONT, program, Default::default());
        let mut inner = dict("<< /Subtype /Form /BBox [0 0 600 700] >>");
        let form = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                inner.clone(),
                b"1 0 0 rg 0 0 500 700 re f".to_vec(),
            )))
            .unwrap();
        let mut resources = PdfDict::default();
        let mut xobjects = PdfDict::default();
        xobjects.insert(
            "Inner".into(),
            PdfObject::PdfReference(PdfReference::from_id(form)),
        );
        resources.insert("XObject".into(), PdfObject::PdfDict(xobjects));
        let mut font = doc.resolve_object(id).unwrap().into_dict().unwrap();
        font.insert("Resources".into(), PdfObject::PdfDict(resources.clone()));
        doc.update_object(id, PdfObject::PdfDict(font)).unwrap();
        doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
            .unwrap();
        assert_eq!(doc.get_page(0).unwrap().display_list().is_ok(), !uncoloured);
        // Reentering a Form through its own explicitly declared resource scope
        // must be diagnosed before expanding an unbounded invocation chain.
        inner.insert("Resources".into(), PdfObject::PdfDict(resources));
        inner.insert(
            "Length".into(),
            dict("<< /V 9 >>").get("V").unwrap().clone(),
        );
        doc.update_object(
            form,
            PdfObject::PdfStream(PdfStream::new(inner, b"/Inner Do".to_vec())),
        )
        .unwrap();
        assert!(matches!(
            doc.get_page(0).unwrap().display_list(),
            Err(PdfError::PageResourceError(message)) if message.contains("Cyclic")
        ));
    }
}

#[test]
fn type3_image_resources_distinguish_omitted_empty_and_local_scopes() {
    for scope in 0..3 {
        let (doc, name, id) = document(FONT, b"600 0 d0", Default::default());
        let image_dict = dict(
            "<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB >>",
        );
        let image = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                image_dict.clone(),
                vec![255, 0, 0],
            )))
            .unwrap();
        let image_name = doc
            .resources()
            .register_existing(0, ResourceKind::Image, image)
            .unwrap();
        let mut font = doc.resolve_object(id).unwrap().into_dict().unwrap();
        let glyph_id = font
            .get("CharProcs")
            .unwrap()
            .as_dict()
            .unwrap()
            .get("A")
            .unwrap()
            .as_object_id()
            .unwrap();
        doc.update_object(
            glyph_id,
            PdfObject::PdfStream(PdfStream::new(
                Default::default(),
                format!("600 0 d0 /{image_name} Do").into_bytes(),
            )),
        )
        .unwrap();
        if scope != 0 {
            let mut resources = PdfDict::default();
            if scope == 2 {
                let local = doc
                    .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                        image_dict,
                        vec![0, 0, 255],
                    )))
                    .unwrap();
                let mut images = PdfDict::default();
                images.insert(
                    image_name.clone(),
                    PdfObject::PdfReference(PdfReference::from_id(local)),
                );
                resources.insert("XObject".into(), PdfObject::PdfDict(images));
            }
            font.insert("Resources".into(), PdfObject::PdfDict(resources));
            doc.update_object(id, PdfObject::PdfDict(font)).unwrap();
        }
        doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
            .unwrap();
        let images = doc.get_page(0).unwrap().extract_images_paint_order();
        if scope == 1 {
            assert!(images.is_err());
        } else {
            let images = images.unwrap();
            assert_eq!(images.len(), 1);
            assert!(images[0].source().is_none() && images[0].nested_source().is_none());
            assert_eq!(
                images[0].image.data(),
                if scope == 0 { [255, 0, 0] } else { [0, 0, 255] }
            );
        }
    }
}

#[test]
fn type3_d1_images_require_stencils_in_both_inline_and_xobject_paths() {
    for inline in [false, true] {
        for stencil in [false, true] {
            let program = if inline {
                let parameters = if stencil { "/IM true" } else { "/BPC 8 /CS /G" };
                format!("600 0 0 0 600 700 d1 BI /W 1 /H 1 {parameters} /F /AHx ID 00> EI")
                    .into_bytes()
            } else {
                b"600 0 0 0 600 700 d1 /Img Do".to_vec()
            };
            let (doc, name, id) = document(FONT, &program, Default::default());
            if !inline {
                let parameters = if stencil {
                    "/ImageMask true"
                } else {
                    "/BitsPerComponent 8 /ColorSpace /DeviceGray"
                };
                let image = doc
                    .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                        dict(&format!(
                            "<< /Subtype /Image /Width 1 /Height 1 {parameters} >>"
                        )),
                        vec![0],
                    )))
                    .unwrap();
                let mut xobjects = PdfDict::default();
                xobjects.insert(
                    "Img".into(),
                    PdfObject::PdfReference(PdfReference::from_id(image)),
                );
                let mut resources = PdfDict::default();
                resources.insert("XObject".into(), PdfObject::PdfDict(xobjects));
                let mut font = doc.resolve_object(id).unwrap().into_dict().unwrap();
                font.insert("Resources".into(), PdfObject::PdfDict(resources));
                doc.update_object(id, PdfObject::PdfDict(font)).unwrap();
            }
            doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
                .unwrap();
            assert_eq!(
                doc.get_page(0).unwrap().display_list().is_ok(),
                stencil,
                "inline={inline}, stencil={stencil}"
            );
        }
    }
}

#[test]
fn type3_d1_inherits_distinct_stroke_and_fill_colours() {
    use pdf::page::page_object::PageObject;
    let (doc, name, _) = document(
        FONT,
        b"600 0 0 0 600 700 d1 0 0 500 700 re S",
        Default::default(),
    );
    doc.replace_page_contents(
        0,
        format!("0 0 1 rg 0 1 0 RG BT /{name} 12 Tf (A) Tj ET").into_bytes(),
    )
    .unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("glyph path");
    };
    let colour = &path.graphic_state().color_state;
    let fill = colour
        .fill_color_space
        .rgb(&colour.fill_color_value)
        .unwrap();
    let stroke = colour
        .stroke_color_space
        .rgb(&colour.stroke_color_value)
        .unwrap();
    assert_eq!((fill.r(), fill.g(), fill.b()), (0, 0, 255));
    assert_eq!((stroke.r(), stroke.g(), stroke.b()), (0, 255, 0));
}

#[test]
fn type3_nondefault_rendering_modes_and_mismatched_metrics_are_not_silent_success() {
    let (doc, name, id) = document(FONT, b"600 0 d0 0 0 500 700 re f", Default::default());
    for mode in 1..=7 {
        doc.replace_page_contents(
            0,
            format!("BT /{name} 12 Tf {mode} Tr (A) Tj ET").into_bytes(),
        )
        .unwrap();
        assert!(matches!(
            doc.get_page(0).unwrap().display_list(),
            Err(PdfError::UnsupportedFeature(_))
        ));
    }
    let mut font = doc.resolve_object(id).unwrap().into_dict().unwrap();
    font.insert(
        "Widths".into(),
        dict("<< /V [625 700] >>").get("V").unwrap().clone(),
    );
    doc.update_object(id, PdfObject::PdfDict(font)).unwrap();
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
        .unwrap();
    assert!(matches!(
        doc.get_page(0).unwrap().display_list(),
        Err(PdfError::FontError(_))
    ));
}

#[test]
fn type3_without_resources_uses_page_scope_even_inside_a_form() {
    let (doc, font_name, font_id) = document(FONT, b"600 0 d0", Default::default());
    let image_dict = dict(
        "<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB >>",
    );
    let page_image = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            image_dict.clone(),
            vec![255, 0, 0],
        )))
        .unwrap();
    let image_name = doc
        .resources()
        .register_existing(0, ResourceKind::Image, page_image)
        .unwrap();
    let local_image = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            image_dict,
            vec![0, 0, 255],
        )))
        .unwrap();
    let font = doc.resolve_object(font_id).unwrap();
    let glyph = font
        .get_attr("CharProcs")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("A")
        .unwrap()
        .as_object_id()
        .unwrap();
    doc.update_object(
        glyph,
        PdfObject::PdfStream(PdfStream::new(
            Default::default(),
            format!("600 0 d0 /{image_name} Do").into_bytes(),
        )),
    )
    .unwrap();
    let mut fonts = PdfDict::default();
    fonts.insert(
        font_name.clone(),
        PdfObject::PdfReference(PdfReference::from_id(font_id)),
    );
    let mut images = PdfDict::default();
    images.insert(
        image_name,
        PdfObject::PdfReference(PdfReference::from_id(local_image)),
    );
    let mut resources = PdfDict::default();
    resources.insert("Font".into(), PdfObject::PdfDict(fonts));
    resources.insert("XObject".into(), PdfObject::PdfDict(images));
    let mut form = dict("<< /Subtype /Form /BBox [0 0 1000 1000] >>");
    form.insert("Resources".into(), PdfObject::PdfDict(resources));
    let form_id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            form,
            format!("BT /{font_name} 12 Tf (A) Tj ET").into_bytes(),
        )))
        .unwrap();
    let form_name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form_id)
        .unwrap();
    doc.replace_page_contents(0, format!("/{form_name} Do").into_bytes())
        .unwrap();
    let images = doc
        .get_page(0)
        .unwrap()
        .extract_images_paint_order()
        .unwrap();
    assert_eq!(images.len(), 1);
    assert_eq!(images[0].image.data(), [255, 0, 0]);
    assert!(images[0].source().is_none() && images[0].nested_source().is_none());
}

#[test]
fn type3_text_removal_preserves_neighbors_and_shared_pages() {
    let (doc, name, _) = document(FONT, b"600 0 d0 0 0 500 700 re f", Default::default());
    doc.replace_page_contents(
        0,
        format!("BT /{name} 12 Tf 1 0 0 1 20 30 Tm (A) Tj 1 0 0 1 80 30 Tm (B) Tj ET").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 1).unwrap();
    let before = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    let program = doc.get_page(0).unwrap().content_program().unwrap();
    let operation = before[0].source.unwrap().operation_index;
    doc.remove_page_text_draw(0, &program, operation).unwrap();
    assert!(doc.remove_page_text_draw(0, &program, operation).is_err());
    let reopened = Document::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
    for document in [&doc, &reopened] {
        let changed = document
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        let unchanged = document
            .get_page(1)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(changed.len(), 1);
        assert_eq!(changed[0].unicode.as_deref(), Some("B"));
        assert_eq!(
            (changed[0].position.x(), changed[0].position.y()),
            (before[1].position.x(), before[1].position.y())
        );
        assert_eq!(unchanged.len(), 2);
        assert_eq!(
            (unchanged[0].position.x(), unchanged[0].position.y()),
            (before[0].position.x(), before[0].position.y())
        );
        assert_eq!(
            (unchanged[1].position.x(), unchanged[1].position.y()),
            (before[1].position.x(), before[1].position.y())
        );
    }
}

#[test]
fn type3_r6_decrypts_charprocs_once_and_keeps_edit_permission_guards() {
    use pdf::security::{EncryptionOptions, SaveSecurity};
    let (doc, name, _) = document(FONT, b"600 0 d0 0 0 500 700 re f", Default::default());
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
        .unwrap();
    let mut options = EncryptionOptions::new("user", "owner").unwrap();
    options.permissions.modify = false;
    let bytes = doc
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&options))
        .unwrap();
    let user =
        Document::from_bytes_with_password_and_options(bytes.clone(), "user", Default::default())
            .unwrap();
    let chars = user
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(chars[0].unicode.as_deref(), Some("A"));
    let operation = chars[0].source.unwrap().operation_index;
    let program = user.get_page(0).unwrap().content_program().unwrap();
    let stamp = user.stamp();
    assert!(matches!(
        user.remove_page_text_draw(0, &program, operation),
        Err(PdfError::PermissionDenied(_))
    ));
    assert!(user.is_stamp_current(&stamp));
    let owner =
        Document::from_bytes_with_password_and_options(bytes, "owner", Default::default()).unwrap();
    let program = owner.get_page(0).unwrap().content_program().unwrap();
    owner.remove_page_text_draw(0, &program, operation).unwrap();
    assert!(
        owner
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()
            .is_empty()
    );
}

#[test]
fn type3_metadata_is_retained_for_extraction_but_not_painted_twice() {
    use pdf::{
        device::trace::TraceDevice,
        page::{page_object::PageObject, text_object::TextObject},
    };
    let (doc, name, _) = document(FONT, b"600 0 d0 0 0 500 700 re f", Default::default());
    doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
        .unwrap();
    let mut page = doc.get_page(0).unwrap();
    let list = page.display_list().unwrap();
    assert_eq!(list.objects().len(), 2);
    let PageObject::Text(text) = &list.objects()[1] else {
        panic!("text metadata");
    };
    assert!(text.is_type3_metadata());
    assert!(!TextObject::new(text.state.clone()).is_type3_metadata());
    for replay in [false, true] {
        let mut painter = TraceDevice::new();
        if replay {
            list.replay(&mut painter).unwrap();
        } else {
            page.display(&mut painter).unwrap();
        }
        let painted = painter.into_display_list();
        assert_eq!(painted.objects().len(), 1);
        assert!(matches!(painted.objects()[0], PageObject::Path(_)));
    }
}
