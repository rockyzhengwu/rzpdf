//! Fixed existing/new-glyph subset edit on a shared static TrueType font.
//! Usage: v03_subset_matrix OUTPUT.pdf
use pdf::{
    document::{
        PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
        subset_text_edit::SubsetTextEditOptions,
    },
    error::{PdfError, PdfResult},
    page::content_builder::ContentBuilder,
    parser::syntax::ParseMode,
};

const FONT: &[u8] = include_bytes!("../tests/fixtures/v03/NotoSansSC-static-subset.ttf");
const LATIN: &[u8] = include_bytes!("../tests/fixtures/v03/RzpdfTestSans-composite.ttf");

fn options() -> TrueTypeEmbeddingOptions {
    TrueTypeEmbeddingOptions {
        postscript_name: "NotoSansSC-Thin".into(),
        cap_height: 733.0,
        stem_v: 80.0,
        serif: false,
    }
}

fn main() -> PdfResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err(PdfError::DocumentError(
            "Usage: v03_subset_matrix OUTPUT.pdf".into(),
        ));
    }
    let doc = PdfDocument::new()?;
    for page in 0..3 {
        doc.add_page(page)?;
    }
    let original = doc
        .resources()
        .embed_truetype_font(0, FONT.to_vec(), "你", options())?;
    for page in 0..2 {
        let name = original.register_on_page(&doc, page)?;
        let mut content = ContentBuilder::new();
        content.begin_text();
        content.try_set_font(&name, 36.0)?;
        content.move_text(50.0, 700.0);
        content.show_encoded_text(&original.encoding().encode_text("你")?)?;
        content.end_text();
        doc.replace_page_content_builder(page, &content)?;
    }
    let before = doc.save_to_bytes()?;
    let snapshot = doc.get_page(0)?.content_program()?;
    let make_edit = |tag| SubsetTextEditOptions {
        source: FONT,
        embedding: options(),
        subset_tag: tag,
        omit_layout_and_auxiliary: true,
        preserve_advance: false,
    };
    let rejected = doc.replace_page_text_string_with_subset(
        0,
        &snapshot,
        3,
        0..2,
        "你🚀",
        make_edit(*b"REJECT"),
    );
    if rejected.is_ok() || doc.save_to_bytes()? != before {
        return Err(PdfError::DocumentError(
            "unsupported glyph did not reject atomically".into(),
        ));
    }
    let fresh = doc.get_page(0)?.content_program()?;
    doc.replace_page_text_string_with_subset(0, &fresh, 3, 0..2, "你新", make_edit(*b"NEWGLY"))?;
    let latin = doc.resources().embed_truetype_font(
        2,
        LATIN.to_vec(),
        "Cafe",
        TrueTypeEmbeddingOptions {
            postscript_name: "RzpdfTestSans-Regular".into(),
            cap_height: 700.0,
            stem_v: 80.0,
            serif: false,
        },
    )?;
    let mut content = ContentBuilder::new();
    content.begin_text();
    content.try_set_font(&latin.resource_name, 36.0)?;
    content.move_text(50.0, 700.0);
    content.show_encoded_text(&latin.encoding().encode_text("Cafe")?)?;
    content.end_text();
    doc.replace_page_content_builder(2, &content)?;
    let latin_snapshot = doc.get_page(2)?.content_program()?;
    doc.replace_page_text_string_with_subset(
        2,
        &latin_snapshot,
        3,
        0..8,
        "Café",
        SubsetTextEditOptions {
            source: LATIN,
            embedding: TrueTypeEmbeddingOptions {
                postscript_name: "RzpdfTestSans-Regular".into(),
                cap_height: 700.0,
                stem_v: 80.0,
                serif: false,
            },
            subset_tag: *b"COMPOS",
            omit_layout_and_auxiliary: true,
            preserve_advance: false,
        },
    )?;
    doc.save_to_file(&args[0])?;
    let reopened = PdfDocument::open_with_mode(&args[0], ParseMode::Strict)?;
    for (page, expected) in [(0, "你新"), (1, "你"), (2, "Café")] {
        let extracted = reopened
            .get_page(page)?
            .extract_characters_paint_order()?
            .into_iter()
            .map(|item| item.unicode.unwrap_or_default())
            .collect::<String>();
        if extracted != expected {
            return Err(PdfError::DocumentError(format!(
                "page {} text mismatch: {extracted:?}",
                page + 1
            )));
        }
    }
    Ok(())
}
