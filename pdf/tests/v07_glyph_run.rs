use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        embedded_font::TrueTypeEmbeddingOptions,
        glyph_run::{GlyphRun, PositionedGlyph},
        resource_manager::StandardFont,
    },
    error::{PdfError, PdfResult},
    page::content_builder::ContentBuilder,
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};

const FONT: &[u8] = include_bytes!("fixtures/v03/NotoSansSC-static-subset.ttf");

fn glyph(code: &[u8], position: [f32; 2]) -> PositionedGlyph<'_> {
    PositionedGlyph {
        code,
        expected_gid: None,
        unicode: None,
        position,
    }
}
fn run<'a>(name: &'a str, glyphs: &'a [PositionedGlyph<'a>]) -> GlyphRun<'a> {
    GlyphRun {
        font_name: name,
        font_size: 20.,
        matrix: [1., 0., 0., 1., 0., 0.],
        color: [0., 0., 0.],
        glyphs,
    }
}
fn document(options: ParseOptions) -> (Document, String) {
    let doc = Document::new_with_options(options).unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    (doc, name)
}

#[test]
fn explicit_origins_reset_inherited_text_state_and_survive_strict_save() -> PdfResult<()> {
    let (doc, name) = document(Default::default());
    doc.replace_page_contents(
        0,
        format!("BT /{name} 9 Tf 3 Tc 7 Tw 30 Tz 11 Ts 3 Tr ET 2 0 0 2 60 70 cm 0 0 1 1 re W n")
            .into_bytes(),
    )?;
    let glyphs = [
        PositionedGlyph {
            unicode: Some("A"),
            ..glyph(b"A", [40., 60.])
        },
        glyph(b"B", [70., 80.]),
    ];
    let mut input = run(&name, &glyphs);
    input.matrix = [0., 1., -1., 0., 150., 20.];
    doc.overlay_page_glyph_run(0, &input)?;
    let reopened = Document::from_bytes_with_mode(doc.save_to_bytes()?, ParseMode::Strict)?;
    for document in [&doc, &reopened] {
        let chars = document.get_page(0)?.extract_characters_paint_order()?;
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].unicode.as_deref(), Some("A"));
        assert_eq!(chars[1].unicode.as_deref(), Some("B"));
        assert_eq!(chars[0].font_size, 20.);
        assert_eq!(chars[0].horizontal_scale, 1.);
    }
    Ok(())
}

#[test]
fn matrix_positions_are_exact_and_not_implicit_advances() {
    let (doc, name) = document(Default::default());
    let glyphs = [glyph(b"A", [40., 60.]), glyph(b"B", [70., 80.])];
    let mut input = run(&name, &glyphs);
    input.matrix = [0., 1., -1., 0., 150., 20.];
    doc.overlay_page_glyph_run(0, &input).unwrap();
    let height = doc.get_page(0).unwrap().geometry().unwrap().display_height;
    let check = |doc: &Document| {
        let chars = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        for (ch, expected) in chars.iter().zip([[90., height - 60.], [70., height - 90.]]) {
            assert!((ch.position.x() - expected[0]).abs() < 1e-4);
            assert!((ch.position.y() - expected[1]).abs() < 1e-4);
        }
    };
    check(&doc);
    check(
        &Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap(),
    );
}

#[test]
fn invalid_code_gid_unicode_geometry_and_missing_font_leave_bytes_unchanged() {
    let (doc, name) = document(Default::default());
    let before = doc.save_to_bytes().unwrap();
    let variants = [
        glyph(b"", [0., 0.]),
        glyph(b"AB", [0., 0.]),
        glyph(b"\0", [0., 0.]),
        glyph(b"A", [f32::INFINITY, 0.]),
        PositionedGlyph {
            expected_gid: Some(u32::MAX),
            ..glyph(b"A", [0., 0.])
        },
        PositionedGlyph {
            unicode: Some("B"),
            ..glyph(b"A", [0., 0.])
        },
        PositionedGlyph {
            unicode: Some(""),
            ..glyph(b"A", [0., 0.])
        },
    ];
    for invalid in variants {
        // First valid glyph must not be published when the second fails.
        assert!(
            doc.overlay_page_glyph_run(0, &run(&name, &[glyph(b"B", [20., 30.]), invalid]))
                .is_err()
        );
        assert_eq!(doc.save_to_bytes().unwrap(), before);
    }
    let glyphs = [glyph(b"A", [20., 30.])];
    let mut input = run(&name, &glyphs);
    input.font_name = "missing";
    assert!(doc.overlay_page_glyph_run(0, &input).is_err());
    input.font_name = &name;
    for size in [0., -1., f32::NAN, f32::INFINITY] {
        input.font_size = size;
        assert!(doc.overlay_page_glyph_run(0, &input).is_err());
    }
    input.font_size = 20.;
    input.matrix[0] = f32::MAX;
    assert!(doc.overlay_page_glyph_run(0, &input).is_err());
    assert!(doc.overlay_page_glyph_run(0, &run(&name, &[])).is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), before);
}

#[test]
fn shared_content_and_resources_are_isolated_and_outer_transaction_rolls_back() {
    let (doc, name) = document(Default::default());
    let mut content = ContentBuilder::new();
    content
        .begin_text()
        .set_font(&name, 20.)
        .move_text(30., 50.);
    content.show_encoded_text(b"A").unwrap().end_text();
    doc.replace_page_content_builder(0, &content).unwrap();
    doc.copy_page(0, 1).unwrap();
    let before = doc.save_to_bytes().unwrap();
    let other = doc
        .get_page(1)
        .unwrap()
        .content_program()
        .unwrap()
        .source_bytes()
        .to_vec();
    let glyphs = [glyph(b"B", [50., 70.])];
    let input = run(&name, &glyphs);
    let failed: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.overlay_page_glyph_run(0, &input)?;
        Err(PdfError::DocumentError("rollback".into()))
    });
    assert!(failed.is_err());
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    doc.overlay_page_glyph_run(0, &input).unwrap();
    assert_eq!(
        doc.get_page(1)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        other
    );
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()
            .len(),
        2
    );
    assert_eq!(
        doc.get_page(1)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()
            .len(),
        1
    );
}

#[test]
fn operation_pool_neighbor_failure_releases_and_retries() {
    let (doc, name) = document(Default::default());
    let before = doc.save_to_bytes().unwrap();
    let glyphs = [glyph(b"A", [50., 70.]), glyph(b"B", [80., 70.])];
    let input = run(&name, &glyphs);
    let probe = OperationBudget::new(8 * 1024 * 1024);
    let rolled_back: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.overlay_page_glyph_run_with_budget(0, &input, &probe)?;
        Err(PdfError::DocumentError(
            "rollback after peak measurement".into(),
        ))
    });
    assert!(rolled_back.is_err());
    assert_eq!(probe.live_bytes(), 0);
    let pool = OperationBudget::new(probe.peak_bytes());
    let neighbor = pool.reserve(1).unwrap();
    assert!(matches!(
        doc.overlay_page_glyph_run_with_budget(0, &input, &pool),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(pool.live_bytes(), 1);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    drop(neighbor);
    doc.overlay_page_glyph_run_with_budget(0, &input, &pool)
        .unwrap();
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn bounded_count_rejects_before_modification() {
    let (doc, name) = document(ParseOptions {
        limits: Limits {
            max_text_chars: 1,
            ..Default::default()
        },
        ..Default::default()
    });
    let before = doc.save_to_bytes().unwrap();
    assert!(matches!(
        doc.overlay_page_glyph_run(
            0,
            &run(&name, &[glyph(b"A", [0., 0.]), glyph(b"B", [0., 0.])])
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(doc.save_to_bytes().unwrap(), before);
}

#[test]
fn horizontal_vertical_static_subsets_keep_original_gids_and_w2_origins() {
    for vertical in [false, true] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let options = TrueTypeEmbeddingOptions {
            postscript_name: "NotoSansSC-Thin".into(),
            cap_height: 733.,
            stem_v: 80.,
            serif: false,
        };
        let pool = OperationBudget::new(16 * 1024 * 1024);
        let subset = if vertical {
            doc.resources().embed_truetype_vertical_subset_font(
                0, FONT, "你好", options, *b"RUNVER", true, &pool,
            )
        } else {
            doc.resources()
                .embed_truetype_subset_font(0, FONT, "你好", options, *b"RUNHOR", true, &pool)
        }
        .unwrap();
        let codes = subset.font.encoding().encode_text("你好").unwrap();
        let glyphs = [
            PositionedGlyph {
                unicode: Some("你"),
                ..glyph(&codes[..2], [80., 100.])
            },
            PositionedGlyph {
                unicode: Some("好"),
                ..glyph(&codes[2..], [120., 100.])
            },
        ];
        doc.overlay_page_glyph_run(0, &run(&subset.font.resource_name, &glyphs))
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let chars = reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(chars.len(), 2);
        assert_eq!(chars[0].unicode.as_deref(), Some("你"));
        assert_eq!(chars[1].unicode.as_deref(), Some("好"));
        assert_eq!(
            chars[0].writing_mode,
            Some(if vertical {
                pdf::font::WritingMode::Vertical
            } else {
                pdf::font::WritingMode::Horizontal
            })
        );
        assert!((chars[1].position.x() - chars[0].position.x() - 40.).abs() < 1e-4);
    }
}

#[test]
fn r6_owner_can_write_but_user_denial_keeps_revision_unchanged() {
    use pdf::security::{EncryptionOptions, SaveSecurity};
    let (doc, name) = document(Default::default());
    let mut security = EncryptionOptions::new("user", "owner").unwrap();
    security.permissions.modify = false;
    let bytes = doc
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&security))
        .unwrap();
    let user =
        Document::from_bytes_with_password_and_options(bytes.clone(), "user", Default::default())
            .unwrap();
    let stamp = user.stamp();
    let glyphs = [glyph(b"A", [50., 70.])];
    let input = run(&name, &glyphs);
    let pool = OperationBudget::new(0);
    assert!(matches!(
        user.overlay_page_glyph_run_with_budget(0, &input, &pool),
        Err(PdfError::PermissionDenied(_))
    ));
    assert_eq!(pool.live_bytes(), 0);
    assert!(user.is_stamp_current(&stamp));
    let owner =
        Document::from_bytes_with_password_and_options(bytes, "owner", Default::default()).unwrap();
    owner.overlay_page_glyph_run(0, &input).unwrap();
    let reopened = Document::from_bytes_with_password_and_options(
        owner.save_to_bytes().unwrap(),
        "owner",
        Default::default(),
    )
    .unwrap();
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()[0]
            .unicode
            .as_deref(),
        Some("A")
    );
}

#[test]
fn glyph_run_supports_incremental_save_and_rejects_signature_source_before_budget() {
    let (doc, name) = document(Default::default());
    let original = doc.save_to_bytes().unwrap();
    let opened = Document::from_bytes_with_mode(original.clone(), ParseMode::Strict).unwrap();
    let glyphs = [glyph(b"A", [50., 70.])];
    let input = run(&name, &glyphs);
    opened.overlay_page_glyph_run(0, &input).unwrap();
    let appended = opened.save_incremental_to_bytes().unwrap();
    assert!(appended.starts_with(&original));
    let reopened = Document::from_bytes_with_mode(appended, ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()[0]
            .unicode
            .as_deref(),
        Some("A")
    );

    let (doc, name) = document(Default::default());
    use pdf::objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName};
    let mut signature = PdfDict::default();
    signature.insert(
        "Type".into(),
        PdfObject::PdfName(PdfName::new("Sig".into())),
    );
    doc.try_insert_object(PdfObject::PdfDict(signature))
        .unwrap();
    let signed =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let stamp = signed.stamp();
    let pool = OperationBudget::new(0);
    assert!(matches!(
        signed.overlay_page_glyph_run_with_budget(0, &run(&name, &glyphs), &pool),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert!(signed.is_stamp_current(&stamp));
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn missing_substitute_type3_and_out_of_range_gid_are_not_successful_writes() {
    use pdf::{
        document::resource_manager::ResourceKind,
        objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_stream::PdfStream},
    };
    let (doc, _) = document(Default::default());
    let parse = |source: &str| {
        pdf::parser::syntax::SyntaxParser::new(
            pdf::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(source.as_bytes()))
                .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap()
    };
    for source in [
        "<< /Type /Font /Subtype /Type1 /BaseFont /Unlisted-Font /FirstChar 65 /LastChar 65 /Widths [600] >>",
        "<< /Type /Font /Subtype /Type1 /BaseFont /Arial >>",
    ] {
        let id = doc
            .try_insert_object(PdfObject::PdfDict(parse(source)))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Font, id)
            .unwrap();
        let before = doc.save_to_bytes().unwrap();
        assert!(
            doc.overlay_page_glyph_run(0, &run(&name, &[glyph(b"A", [0., 0.])]))
                .is_err()
        );
        assert_eq!(doc.save_to_bytes().unwrap(), before);
    }
    let doc = Document::from_bytes_with_mode(
        include_bytes!("fixtures/v07/fonts.pdf").to_vec(),
        ParseMode::Strict,
    )
    .unwrap();
    let root = doc
        .get_page(3)
        .unwrap()
        .get_resource_font("F0")
        .unwrap()
        .unwrap()
        .into_dict()
        .unwrap();
    let descendants = root.get("DescendantFonts").unwrap().as_array().unwrap();
    let id = descendants.get(0).unwrap().as_reference().unwrap().id();
    let mut descendant = doc.resolve_object(id).unwrap().into_dict().unwrap();
    let mut map = vec![0u8; 40];
    map[14..16].copy_from_slice(&5000u16.to_be_bytes());
    let map_id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            map,
        )))
        .unwrap();
    descendant.insert(
        "CIDToGIDMap".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(map_id)),
    );
    doc.update_object(id, PdfObject::PdfDict(descendant))
        .unwrap();
    let before = doc.save_to_bytes().unwrap();
    assert!(
        doc.overlay_page_glyph_run(3, &run("F0", &[glyph(b"A", [20., 30.])]))
            .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), before);

    // A minimally inspectable Type3 remains outside this writing API.
    let mut font = parse(
        "<< /Type /Font /Subtype /Type3 /FontMatrix [.001 0 0 .001 0 0] /FontBBox [0 0 600 700] /FirstChar 65 /LastChar 65 /Widths [600] /Encoding << /Differences [65 /A] >> >>",
    );
    let mut procs = PdfDict::default();
    let glyph_id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            b"600 0 d0".to_vec(),
        )))
        .unwrap();
    procs.insert(
        "A".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(glyph_id)),
    );
    font.insert("CharProcs".into(), PdfObject::PdfDict(procs));
    font.insert(
        "Type".into(),
        PdfObject::PdfName(PdfName::new("Font".into())),
    );
    let id = doc.try_insert_object(PdfObject::PdfDict(font)).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Font, id)
        .unwrap();
    let before = doc.save_to_bytes().unwrap();
    assert!(matches!(
        doc.overlay_page_glyph_run(0, &run(&name, &[glyph(b"A", [0., 0.])])),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert_eq!(doc.save_to_bytes().unwrap(), before);
}

#[test]
fn form_local_font_is_not_rebound_by_page_glyph_run() {
    use pdf::{
        document::resource_manager::ResourceKind,
        objects::{PdfObject, pdf_stream::PdfStream},
    };
    let (doc, name) = document(Default::default());
    let source = format!(
        "<< /Type /XObject /Subtype /Form /BBox [0 0 300 300] /Resources << /Font << /{name} << /Type /Font /Subtype /Type1 /BaseFont /Helvetica >> >> >> >>"
    );
    let dict = pdf::parser::syntax::SyntaxParser::new(
        pdf::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(source.as_bytes()))
            .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    let body = format!("BT /{name} 12 Tf 20 30 Td (B) Tj ET").into_bytes();
    let form = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, body)))
        .unwrap();
    let form_name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form)
        .unwrap();
    doc.replace_page_contents(0, format!("/{form_name} Do").into_bytes())
        .unwrap();
    let before = doc.resolve_object(form).unwrap();
    doc.overlay_page_glyph_run(
        0,
        &run(
            &name,
            &[PositionedGlyph {
                unicode: Some("A"),
                ..glyph(b"A", [50., 70.])
            }],
        ),
    )
    .unwrap();
    assert_eq!(doc.resolve_object(form).unwrap(), before);
    let chars = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    assert_eq!(chars.len(), 2);
    assert_eq!(chars[0].font_name.as_deref(), Some("Helvetica"));
    assert_eq!(chars[1].font_name.as_deref(), Some("Courier"));
    assert!(chars[0].nested_source.is_some());
    assert!(chars[1].nested_source.is_none());
}
