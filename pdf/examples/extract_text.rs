//! Print Unicode in paint order, preserving interpreted text-run boundaries.
//! Not a reading-order or layout reconstruction algorithm. Missing mappings fail.
use pdf::{
    document::Document,
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 1 {
        return Err(PdfError::DocumentError(
            "Usage: extract_text INPUT.pdf".into(),
        ));
    }
    let doc = Document::open_with_mode(&args[0], ParseMode::Strict)?;
    for index in 0..doc.try_page_count()? {
        let index = u32::try_from(index)
            .map_err(|_| PdfError::ResourceLimit("Text extraction page index".into()))?;
        let report = doc
            .get_page(index)?
            .extract_characters_paint_order_report()?;
        if !report.diagnostics.is_empty() {
            return Err(PdfError::DocumentError(format!(
                "Page {index}: content interpretation warnings"
            )));
        }
        let chars = report.characters;
        // Validate the page before printing it; prior page output may exist on failure.
        if chars.iter().any(|item| item.unicode.is_none()) {
            return Err(PdfError::FontError(format!(
                "Page {index}: missing Unicode mapping"
            )));
        }
        let mut previous = None;
        for item in chars {
            if previous.is_some_and(|object| object != item.object_index) {
                println!();
            }
            print!("{}", item.unicode.as_deref().unwrap_or(""));
            previous = Some(item.object_index);
        }
        println!();
        print!("\x0c");
    }
    Ok(())
}
