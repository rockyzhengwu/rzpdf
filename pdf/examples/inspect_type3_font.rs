//! Inspect Type3 metrics/programs without interpreting their drawing operators.
use pdf::{
    document::Document,
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};
use std::io::Write;

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !matches!(args.len(), 4 | 5) {
        return Err(PdfError::DocumentError(
            "Usage: inspect_type3_font INPUT.pdf PAGE_INDEX FONT_NAME CODE [NEW_OUTPUT.pdf]".into(),
        ));
    }
    let page: u32 = args[1]
        .parse()
        .map_err(|_| PdfError::DocumentError("invalid page index".into()))?;
    let code: u8 = args[3]
        .parse()
        .map_err(|_| PdfError::DocumentError("code must be 0..255".into()))?;
    let doc = Document::open_with_mode(&args[0], ParseMode::Strict)?;
    let page = doc.get_page(page)?;
    let font = page.inspect_type3_font(&args[2])?;
    let glyph = page.inspect_type3_glyph(&font, code)?;
    let metrics = glyph.metrics();
    let bbox = metrics
        .bbox
        .map(|b| format!("{b:?}"))
        .unwrap_or_else(|| "null".into());
    println!(
        "{{\"font_matrix\":{:?},\"font_bbox\":{:?},\"width\":{},\"advance\":{:?},\"glyph_width\":{:?},\"glyph_bbox\":{},\"operations\":{}}}",
        font.font_matrix(),
        font.font_bbox(),
        font.width(code)?,
        font.text_space_advance(code)?,
        metrics.width,
        bbox,
        glyph.program().operations().len()
    );
    if let Some(output) = args.get(4) {
        let bytes = doc.save_to_bytes()?;
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?;
        file.write_all(&bytes)?;
    }
    Ok(())
}
