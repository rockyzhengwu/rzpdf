use pdf::{
    document::{Document, resource_manager::ResourceKind},
    font::{CharCode, GlyphDesc, cmap::Cmap},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
    parser::syntax::SyntaxParser,
};
use std::io::Cursor;

fn dict(source: &str) -> PdfDict {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes())).unwrap())
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap()
}

fn inspect(doc: &Document, mut source: PdfDict) -> pdf::page::InspectedFont {
    source.insert(
        "Type".into(),
        PdfObject::PdfName(pdf::objects::pdf_name::PdfName::new("Font".into())),
    );
    if doc.try_page_count().unwrap() == 0 {
        doc.add_page(0).unwrap();
    }
    let id = doc.try_insert_object(PdfObject::PdfDict(source)).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Font, id)
        .unwrap();
    doc.get_page(0).unwrap().inspect_font(&name).unwrap()
}

#[test]
fn unicode_and_cid_keep_leading_zero_code_lengths_separate() {
    let map = Cmap::try_new(b"2 begincodespacerange <40> <7F> <0000> <00FF> endcodespacerange 2 beginbfchar <41> <00660069> <0041> <D83DDE00> endbfchar 2 begincidchar <41> 7 <0041> 19 endcidchar".to_vec()).unwrap();
    let chars = map.chars(&[0x41, 0, 0x41]).unwrap();
    assert_eq!(chars.len(), 2);
    assert_eq!(map.unicode(&chars[0]), Some("fi"));
    assert_eq!(map.unicode(&chars[1]), Some("😀"));
    assert_eq!(map.cid_for_char(&chars[0]), 7);
    assert_eq!(map.cid_for_char(&chars[1]), 19);
    assert_eq!(map.cid(65), 0); // ambiguous lengthless inspection
    assert_eq!(map.unicode(&CharCode::new(65, 3, 0.)), None);
    assert_eq!(map.cid_for_char(&CharCode::new(65, 3, 0.)), 0);
}

#[test]
fn unicode_does_not_alias_wrong_length_even_without_codespace() {
    let map = Cmap::try_new(b"1 beginbfchar <0041> <0042> endbfchar".to_vec()).unwrap();
    assert_eq!(map.unicode(&CharCode::new(65, 1, 0.)), None);
    assert_eq!(map.unicode(&CharCode::new(65, 2, 0.)), Some("B"));
}

#[test]
fn range_sources_require_matching_lengths() {
    for map in [
        "1 beginbfrange <01> <0002> <0041> endbfrange",
        "1 begincidrange <01> <0002> 1 endcidrange",
    ] {
        assert!(Cmap::try_new(map.as_bytes().to_vec()).is_err());
    }
}

#[test]
fn nonidentity_code_cid_gid_and_vertical_origins_are_independent_of_unicode() {
    let doc = Document::new().unwrap();
    let mut descendant = dict(
        "<< /Subtype /CIDFontType2 /BaseFont /Test /DW 500 /W [7 [610] 19 [720]] /W2 [7 [-900 300 800] 19 [-1100 360 850]] >>",
    );
    let mut gids = vec![0u8; 40];
    gids[14..16].copy_from_slice(&3u16.to_be_bytes());
    gids[38..40].copy_from_slice(&5u16.to_be_bytes());
    descendant.insert(
        "CIDToGIDMap".into(),
        PdfObject::PdfStream(PdfStream::new(Default::default(), gids)),
    );
    let mut source = dict("<< /Subtype /Type0 /BaseFont /Test >>");
    let mut descendants = pdf::objects::pdf_array::PdfArray::default();
    descendants.add_obj(PdfObject::PdfDict(descendant));
    source.insert("DescendantFonts".into(), PdfObject::PdfArray(descendants));
    source.insert("Encoding".into(), PdfObject::PdfStream(PdfStream::new(Default::default(), b"/WMode 1 def 2 begincodespacerange <40> <7F> <0000> <00FF> endcodespacerange 2 begincidchar <41> 7 <0041> 19 endcidchar".to_vec())));
    source.insert(
        "ToUnicode".into(),
        PdfObject::PdfStream(PdfStream::new(
            Default::default(),
            b"2 beginbfchar <41> <00660069> <0041> <D83DDE00> endbfchar".to_vec(),
        )),
    );
    let font = inspect(&doc, source);
    let chars = font.chars(&[65, 0, 65]).unwrap();
    for (ch, width, origin, gid, text) in [
        (&chars[0], -900., [300., 800.], 3, "fi"),
        (&chars[1], -1100., [360., 850.], 5, "😀"),
    ] {
        assert_eq!(ch.width(), width);
        assert_eq!([ch.origin_x(), ch.origin_y()], origin);
        assert!(matches!(font.get_glyph(ch), Some(GlyphDesc::Gid(actual)) if actual == gid));
        assert_eq!(font.unicode_mapping(ch).unwrap().as_deref(), Some(text));
    }
}

#[test]
fn simple_differences_choose_glyph_independently_from_tounicode() {
    let doc = Document::new().unwrap();
    let mut source = dict(
        "<< /Subtype /Type1 /BaseFont /Courier /Encoding << /BaseEncoding /WinAnsiEncoding /Differences [65 /B] >> >>",
    );
    source.insert(
        "ToUnicode".into(),
        PdfObject::PdfStream(PdfStream::new(
            Default::default(),
            b"1 beginbfchar <41> <00660069> endbfchar".to_vec(),
        )),
    );
    let font = inspect(&doc, source);
    let b = inspect(&doc, dict("<< /Subtype /Type1 /BaseFont /Courier >>"));
    let ch = &font.chars(b"A").unwrap()[0];
    assert_eq!(font.unicode_mapping(ch).unwrap().as_deref(), Some("fi"));
    assert_eq!(
        format!("{}", font.get_glyph(ch).unwrap()),
        format!("{}", b.get_glyph(&b.chars(b"B").unwrap()[0]).unwrap())
    );
    assert_eq!(
        font.unicode_mapping(&font.chars(b"B").unwrap()[0]).unwrap(),
        None
    );
}

#[test]
fn fixed_cff_opentype_truetype_type1_matrix_keeps_mapping_after_save() {
    let doc = Document::from_bytes_with_mode(
        include_bytes!("fixtures/v07/fonts.pdf").to_vec(),
        pdf::parser::syntax::ParseMode::Strict,
    )
    .unwrap();
    let before: Vec<_> = (0..8).map(|index| {
        let font = doc.get_page(index).unwrap().inspect_font("F0").unwrap();
        let characters = doc.get_page(index).unwrap().extract_characters_paint_order().unwrap();
        assert_eq!(characters.len(), if index == 5 { 4 } else { 2 });
        for (position, ch) in characters.iter().enumerate() {
            let code = ch.charcode.to_be_bytes();
            let chars = font.chars(&code[4-ch.code_length as usize..]).unwrap();
            assert!(matches!(font.get_glyph(&chars[0]), Some(GlyphDesc::Gid(gid)) if gid > 0));
            assert_eq!(font.unicode_mapping(&chars[0]).unwrap(), ch.unicode);
            if index < 6 { assert!(matches!(font.get_glyph(&chars[0]), Some(GlyphDesc::Gid(gid)) if gid == 1 + (position % 2) as u32)); }
        }
        characters.into_iter().map(|ch| (ch.charcode, ch.code_length, ch.unicode, ch.position.x(), ch.position.y(), ch.vertical_origin)).collect::<Vec<_>>()
    }).collect();
    let saved = Document::from_bytes_with_mode(
        doc.save_to_bytes().unwrap(),
        pdf::parser::syntax::ParseMode::Strict,
    )
    .unwrap();
    for (page, expected) in before.iter().enumerate() {
        let actual: Vec<_> = saved
            .get_page(page as u32)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()
            .into_iter()
            .map(|ch| {
                (
                    ch.charcode,
                    ch.code_length,
                    ch.unicode,
                    ch.position.x(),
                    ch.position.y(),
                    ch.vertical_origin,
                )
            })
            .collect();
        assert_eq!(&actual, expected);
    }
}

#[test]
fn inspected_font_retains_shared_quota_and_outlives_document() {
    use pdf::budget::OperationBudget;
    let doc = Document::from_bytes_with_mode(
        include_bytes!("fixtures/v07/fonts.pdf").to_vec(),
        pdf::parser::syntax::ParseMode::Strict,
    )
    .unwrap();
    // Both measured calls use the same warm object/decoded-stream cache state.
    drop(doc.get_page(0).unwrap().inspect_font("F0").unwrap());
    let probe = OperationBudget::new(4 * 1024 * 1024);
    let font = doc
        .get_page(0)
        .unwrap()
        .inspect_font_with_budget("F0", &probe)
        .unwrap();
    let peak = probe.peak_bytes();
    assert!(probe.live_bytes() > 0);
    drop(font);
    assert_eq!(probe.live_bytes(), 0);
    let pool = OperationBudget::new(peak);
    let neighbor = pool.reserve(1).unwrap();
    assert!(matches!(
        doc.get_page(0)
            .unwrap()
            .inspect_font_with_budget("F0", &pool),
        Err(pdf::error::PdfError::ResourceLimit(_))
    ));
    assert_eq!(pool.live_bytes(), 1);
    drop(neighbor);
    let font = doc
        .get_page(0)
        .unwrap()
        .inspect_font_with_budget("F0", &pool)
        .unwrap();
    drop(doc);
    assert_eq!(
        font.unicode_mapping(&font.chars(b"A").unwrap()[0])
            .unwrap()
            .as_deref(),
        Some("fi")
    );
    drop(font);
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn vertical_text_and_outline_origins_remain_distinct_under_transform() {
    let doc = Document::from_bytes_with_mode(
        include_bytes!("fixtures/v07/fonts.pdf").to_vec(),
        pdf::parser::syntax::ParseMode::Strict,
    )
    .unwrap();
    doc.replace_page_contents(
        4,
        b"BT /F0 20 Tf 50 Tz 1 .2 .3 1 30 130 Tm <41> Tj ET".to_vec(),
    )
    .unwrap();
    let characters = doc
        .get_page(4)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap();
    let ch = &characters[0];
    assert_eq!(ch.vertical_origin, [300., 800.]);
    assert_eq!((ch.position.x(), ch.position.y()), (30., 50.));
    let origin = ch.glyph_origin_display().unwrap();
    assert!((origin.x() - 22.2).abs() < 1e-4);
    assert!((origin.y() - 66.6).abs() < 1e-4);
    let em = ch.em_to_display_matrix().unwrap();
    assert_eq!((em.e(), em.f()), (origin.x(), origin.y()));
    let list = doc.get_page(4).unwrap().display_list().unwrap();
    let text = list
        .objects()
        .iter()
        .find_map(|object| {
            if let pdf::page::page_object::PageObject::Text(text) = object {
                Some(text)
            } else {
                None
            }
        })
        .unwrap();
    let outline = text
        .character_outline_display(0, Default::default())
        .unwrap();
    let pdf::font::truetype::tables::glyf::GlyphOutlineCommand::Move(point) = outline[0] else {
        panic!("expected rectangle")
    };
    assert!((point[0] - f64::from(origin.x())).abs() < 1e-4);
    assert!((point[1] - f64::from(origin.y())).abs() < 1e-4);
}
