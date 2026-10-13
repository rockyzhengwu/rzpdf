//! Edit Tj, TJ, single-quote and double-quote shows by reported source.
//! Usage: edit_text_operators_v02 OUTPUT.pdf
use pdf::{
    document::{owned::Document, resource_manager::StandardFont},
    error::{PdfError, PdfResult},
    page::{page_builder::PageBuilder, text_extraction::TextExtractionReport},
    parser::syntax::ParseMode,
};

fn select(report: &TextExtractionReport, first: &str) -> PdfResult<(usize, Option<usize>)> {
    let character = report
        .characters
        .iter()
        .find(|character| character.unicode.as_deref() == Some(first))
        .ok_or_else(|| PdfError::DocumentError(format!("character {first} missing")))?;
    let source = character
        .source
        .ok_or_else(|| PdfError::DocumentError("text show source missing".into()))?;
    Ok((source.operation_index, character.array_item_index))
}

fn main() -> PdfResult<()> {
    let output = std::env::args_os().nth(1).ok_or_else(|| {
        PdfError::DocumentError("Usage: edit_text_operators_v02 OUTPUT.pdf".into())
    })?;
    if std::path::Path::new(&output).exists() {
        return Err(PdfError::IOError("output already exists".into()));
    }
    let doc = Document::new()?;
    doc.add_page_with_builder(0, PageBuilder::new().size(200., 120.))?;
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)?;
    doc.replace_page_contents(0, format!(
        "BT /{font} 16 Tf 20 100 Td (One) Tj 0 -20 Td [(T) -50 (J)] TJ 20 TL (Quote) ' 0 0 (Double) \" ET"
    ).into_bytes())?;
    doc.copy_page(0, 1)?;
    doc.edit_transaction(|editing| {
        for (first, replacement) in [("O", "Uno"), ("T", "X"), ("Q", "Prime"), ("D", "Second")] {
            let mut page = editing.get_page(0)?;
            let program = page.content_program()?;
            let report = page.extract_characters_paint_order_report()?;
            if !report.matches_program(&program) || !editing.is_stamp_current(report.source_stamp())
            {
                return Err(PdfError::DocumentError("text source is stale".into()));
            }
            let (operation, item) = select(&report, first)?;
            match item {
                Some(item) => editing.replace_page_text_array_item(
                    0,
                    &program,
                    operation,
                    item,
                    replacement,
                )?,
                None => editing.replace_page_text(0, &program, operation, replacement)?,
            }
        }
        Ok(())
    })?;
    let reopened = Document::from_bytes_with_mode(doc.save_to_bytes()?, ParseMode::Strict)?;
    for index in 0..2 {
        reopened.get_page(index)?.display_list()?;
    }
    doc.save_to_file(output)
}
