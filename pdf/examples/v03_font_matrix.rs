//! Create the fixed v0.3 static TrueType Identity-H/V sample.
//! Usage: v03_font_matrix OUTPUT.pdf
use pdf::{
    document::{
        PdfDocument, embedded_font::TrueTypeEmbeddingOptions,
        resource_manager::StandardFont,
    },
    error::{PdfError, PdfResult},
    page::{content_builder::ContentBuilder, page_builder::PageBuilder},
    parser::syntax::ParseMode,
};

fn main() -> PdfResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 1 {
        return Err(PdfError::DocumentError(
            "Usage: v03_font_matrix OUTPUT.pdf".into(),
        ));
    }
    let data = include_bytes!("../tests/fixtures/v03/NotoSansSC-static-subset.ttf");
    let doc = PdfDocument::new()?;
    for index in 0..3 {
        doc.add_page_with_builder(index, PageBuilder::new().size(300.0, 300.0))?;
    }
    let options = || TrueTypeEmbeddingOptions {
        postscript_name: "NotoSansSC-Thin".into(),
        cap_height: 733.0,
        stem_v: 80.0,
        serif: false,
    };
    let horizontal = doc.resources().embed_truetype_font(
        0,
        data.to_vec(),
        "你好，世界！Hello PDF",
        options(),
    )?;
    let vertical = doc.resources().embed_truetype_vertical_font(
        1,
        data.to_vec(),
        "竖排测试",
        options(),
    )?;
    for (index, font, text) in [
        (0, &horizontal, "你好，世界！Hello PDF"),
        (1, &vertical, "竖排测试"),
    ] {
        let resource = font.register_on_page(&doc, index)?;
        let mut content = ContentBuilder::new();
        content.begin_text();
        content.try_set_font(&resource, 24.0)?;
        content.move_text(if index == 0 { 20.0 } else { 150.0 }, 250.0);
        content.show_encoded_text(&font.encoding().encode_text(text)?)?;
        content.end_text();
        doc.replace_page_content_builder(index, &content)?;
    }
    let courier = doc.resources().register_standard_font(2, StandardFont::Courier)?;
    let mut content = ContentBuilder::new();
    content.begin_text();
    content.try_set_font(&courier, 24.0)?;
    content.move_text(20.0, 250.0);
    content.show_encoded_text(&StandardFont::Courier.encode_text("Courier: Café 123")?)?;
    content.end_text();
    doc.replace_page_content_builder(2, &content)?;
    doc.save_to_file(&args[0])?;
    let reopened = PdfDocument::open_with_mode(&args[0], ParseMode::Strict)?;
    if reopened.try_page_count()? != 3 {
        return Err(PdfError::DocumentError("font matrix page count mismatch".into()));
    }
    for index in 0..3 {
        reopened.get_page(index)?.display_list()?;
    }
    Ok(())
}
