//! Usage: create_subset FONT.ttf POSTSCRIPT_NAME CAP_HEIGHT STEM_V OUTPUT.pdf
//! Fixed Latin composite fixture; metrics and font license are caller supplied.
use pdf::{
    budget::OperationBudget,
    document::{PdfDocument, embedded_font::TrueTypeEmbeddingOptions},
    error::{PdfError, PdfResult},
    page::{content_builder::ContentBuilder, page_builder::PageBuilder},
    parser::syntax::ParseMode,
};

fn main() -> PdfResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() != 5 {
        return Err(PdfError::DocumentError(
            "Usage: create_subset FONT.ttf POSTSCRIPT_NAME CAP_HEIGHT STEM_V OUTPUT.pdf".into(),
        ));
    }
    let metric = |value: &str| {
        value
            .parse::<f64>()
            .map_err(|_| PdfError::FontError("Invalid descriptor metric".into()))
    };
    let source =
        std::fs::read(&args[0]).map_err(|error| PdfError::DocumentError(error.to_string()))?;
    let text = "Composite subset - é Å ö ñ ç é";
    let doc = PdfDocument::new()?;
    doc.add_page_with_builder(0, PageBuilder::new().size(595.0, 842.0))?;
    let budget = OperationBudget::new(128 * 1024 * 1024);
    let subset = doc.resources().embed_truetype_subset_font(
        0,
        &source,
        text,
        TrueTypeEmbeddingOptions {
            postscript_name: args[1].clone(),
            cap_height: metric(&args[2])?,
            stem_v: metric(&args[3])?,
            serif: false,
        },
        *b"RZPDFY",
        true,
        &budget,
    )?;
    eprintln!(
        "Omitted tables: {:?}; peak: {}",
        subset.omitted_tables(),
        budget.peak_bytes()
    );
    let mut builder = ContentBuilder::new();
    builder.begin_text();
    builder.try_set_font(&subset.font.resource_name, 24.0)?;
    builder.move_text(48.0, 760.0);
    builder.show_encoded_text(&subset.font.encoding().encode_text(text)?)?;
    builder.end_text();
    doc.replace_page_content_builder(0, &builder)?;
    doc.save_to_file(&args[4])?;
    let reopened = PdfDocument::open_with_mode(&args[4], ParseMode::Strict)?;
    let characters = reopened.get_page(0)?.extract_characters_paint_order()?;
    if characters
        .iter()
        .filter_map(|c| c.unicode.as_deref())
        .collect::<String>()
        != text
    {
        return Err(PdfError::DocumentError(
            "Subset round-trip text mismatch".into(),
        ));
    }
    Ok(())
}
