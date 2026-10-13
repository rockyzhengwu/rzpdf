use pdf::{
    budget::OperationBudget,
    document::{
        PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
        subset_text_edit::SubsetTextEditOptions,
    },
    error::{PdfError, PdfResult},
    page::content_builder::ContentBuilder,
    parser::syntax::ParseMode,
};

const FONT: &[u8] = include_bytes!("fixtures/v03/NotoSansSC-static-subset.ttf");
const LATIN: &[u8] = include_bytes!("fixtures/v03/RzpdfTestSans-composite.ttf");

fn options() -> TrueTypeEmbeddingOptions {
    TrueTypeEmbeddingOptions {
        postscript_name: "NotoSansSC-Thin".into(),
        cap_height: 733.0,
        stem_v: 80.0,
        serif: false,
    }
}

fn text(doc: &PdfDocument<std::io::Cursor<Vec<u8>>>, page: u32) -> String {
    doc.get_page(page)
        .unwrap()
        .extract_characters_paint_order()
        .unwrap()
        .into_iter()
        .map(|item| item.unicode.unwrap())
        .collect()
}

#[test]
fn static_cjk_new_glyph_edit_isolates_shared_font_and_rolls_back() -> PdfResult<()> {
    let doc = PdfDocument::new()?;
    doc.add_page(0)?;
    doc.add_page(1)?;
    let source = doc.resources().embed_truetype_font(
        0,
        FONT.to_vec(),
        "你",
        options(),
    )?;
    for page in 0..2 {
        let name = source.register_on_page(&doc, page)?;
        let mut builder = ContentBuilder::new();
        builder.begin_text();
        builder.try_set_font(&name, 24.0)?;
        builder.move_text(50.0, 500.0);
        builder.show_encoded_text(&source.encoding().encode_text("你")?)?;
        builder.end_text();
        doc.replace_page_content_builder(page, &builder)?;
    }
    assert_eq!(text(&doc, 0), "你");
    assert_eq!(text(&doc, 1), "你");
    let before = doc.save_to_bytes()?;
    let original = doc.get_page(0)?.content_program()?;
    let rolled_back: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.replace_page_text_string_with_subset(
            0,
            &original,
            3,
            0..2,
            "新",
            SubsetTextEditOptions {
                source: FONT,
                embedding: options(),
                subset_tag: *b"NEWAAA",
                omit_layout_and_auxiliary: true,
                preserve_advance: false,
            },
        )?;
        Err(PdfError::DocumentError("intentional rollback".into()))
    });
    assert!(rolled_back.is_err());
    assert_eq!(doc.save_to_bytes()?, before);
    assert_eq!(text(&doc, 0), "你");
    assert_eq!(text(&doc, 1), "你");

    let fresh = doc.get_page(0)?.content_program()?;
    let insufficient = OperationBudget::new(1);
    assert!(doc
        .replace_page_text_string_with_subset_and_budget(
            0,
            &fresh,
            3,
            0..2,
            "新",
            SubsetTextEditOptions {
                source: FONT,
                embedding: options(),
                subset_tag: *b"NEWLOW",
                omit_layout_and_auxiliary: true,
                preserve_advance: false,
            },
            &insufficient,
        )
        .is_err());
    assert_eq!(insufficient.live_bytes(), 0);
    assert_eq!(doc.save_to_bytes()?, before);
    assert_eq!(text(&doc, 1), "你");

    let fresh = doc.get_page(0)?.content_program()?;
    doc.replace_page_text_string_with_subset(
        0,
        &fresh,
        3,
        0..2,
        "新",
        SubsetTextEditOptions {
            source: FONT,
            embedding: options(),
            subset_tag: *b"NEWBBB",
            omit_layout_and_auxiliary: true,
            preserve_advance: false,
        },
    )?;
    assert_eq!(text(&doc, 0), "新");
    assert_eq!(text(&doc, 1), "你");
    let reopened = PdfDocument::from_bytes_with_mode(doc.save_to_bytes()?, ParseMode::Strict)?;
    assert_eq!(text(&reopened, 0), "新");
    assert_eq!(text(&reopened, 1), "你");
    Ok(())
}

#[test]
fn renamed_latin_font_preserves_composite_glyph_after_subset_edit() -> PdfResult<()> {
    let doc = PdfDocument::new()?;
    doc.add_page(0)?;
    let options = || TrueTypeEmbeddingOptions {
        postscript_name: "RzpdfTestSans-Regular".into(),
        cap_height: 700.0,
        stem_v: 80.0,
        serif: false,
    };
    let font = doc.resources().embed_truetype_font(0, LATIN.to_vec(), "Cafe", options())?;
    let mut builder = ContentBuilder::new();
    builder.begin_text();
    builder.try_set_font(&font.resource_name, 24.0)?;
    builder.move_text(50.0, 500.0);
    builder.show_encoded_text(&font.encoding().encode_text("Cafe")?)?;
    builder.end_text();
    doc.replace_page_content_builder(0, &builder)?;
    let snapshot = doc.get_page(0)?.content_program()?;
    doc.replace_page_text_string_with_subset(
        0,
        &snapshot,
        3,
        0..8,
        "Café",
        SubsetTextEditOptions {
            source: LATIN,
            embedding: options(),
            subset_tag: *b"COMPOS",
            omit_layout_and_auxiliary: true,
            preserve_advance: false,
        },
    )?;
    let reopened = PdfDocument::from_bytes_with_mode(doc.save_to_bytes()?, ParseMode::Strict)?;
    assert_eq!(text(&reopened, 0), "Café");
    Ok(())
}
