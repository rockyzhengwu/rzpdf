//! A read-only independent input becomes a new copy/reorder/delete/edit/import result.
use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};
fn main() -> PdfResult<()> {
    let mut args = std::env::args_os().skip(1);
    let input = args.next().ok_or_else(|| {
        PdfError::DocumentError("expected two-page-input.pdf new-output.pdf".into())
    })?;
    let output = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("expected new-output.pdf".into()))?;
    if args.next().is_some() {
        return Err(PdfError::DocumentError("too many arguments".into()));
    }
    let doc = PdfDocument::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?;
    if doc.try_page_count()? != 2 {
        return Err(PdfError::DocumentError("expected two-page input".into()));
    }
    doc.copy_page(0, 2)?;
    doc.reorder_pages(&[2, 1, 0])?;
    doc.delete_page(2)?;
    let program = doc.get_page(0)?.content_program()?;
    let operation = program
        .operations()
        .iter()
        .position(|o| o.operator.name() == "Tj")
        .ok_or_else(|| PdfError::DocumentError("expected a top-level Tj".into()))?;
    doc.replace_page_text_codes(0, &program, operation, b"Z")?;
    let target = PdfDocument::new_with_version(doc.effective_version()?)?;
    target.import_pages_from(&doc, &[0, 1], 0)?;
    target.structure_associations()?;
    let logical = target
        .extract_text_structure_order(Default::default())?
        .unwrap()
        .text(&Default::default())?;
    let bytes = target.save_to_bytes()?;
    let reopened = PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
    reopened.structure_associations()?;
    if reopened
        .extract_text_structure_order(Default::default())?
        .unwrap()
        .text(&Default::default())?
        != logical
    {
        return Err(PdfError::DocumentError(
            "strict reopen changed declared text".into(),
        ));
    }
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&bytes)?;
    println!("structure={logical:?} pages=2");
    Ok(())
}
