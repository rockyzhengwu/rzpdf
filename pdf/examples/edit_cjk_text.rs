//! Usage: edit_cjk_text INPUT.pdf OUTPUT.pdf
//! Fixed create_cjk (without --overlay) sample: copy page 0, replace 世界 with
//! 你好 in the second Tj, preserving the original two pages and their images.
use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(PdfError::DocumentError(
            "Usage: edit_cjk_text INPUT.pdf OUTPUT.pdf".into(),
        ));
    }
    let doc = PdfDocument::open_with_mode(&args[0], ParseMode::Strict)?;
    if doc.try_page_count()? != 2 {
        return Err(PdfError::DocumentError(
            "Expected two-page CJK sample".into(),
        ));
    }
    let original = doc.get_page(0)?.content_program()?;
    doc.copy_page(0, 2)?;
    let snapshot = doc.get_page(2)?.content_program()?;
    let operation = snapshot
        .operations()
        .iter()
        .enumerate()
        .filter(|(_, op)| op.operator.name() == "Tj")
        .nth(1)
        .map(|(index, _)| index)
        .ok_or_else(|| PdfError::ObjectError("Second Tj missing".into()))?;
    let font_name = snapshot.operations()[..operation]
        .iter()
        .rev()
        .find(|op| op.operator.name() == "Tf")
        .ok_or_else(|| PdfError::FontError("Active font missing".into()))?
        .operator
        .get_as_name(0)?
        .name()
        .to_owned();
    let page = doc.get_page(2)?;
    let old = snapshot.operations()[operation]
        .operator
        .get_as_string(0)?
        .bytes();
    if old != page.encode_text_for_font(&font_name, "你好，世界！Hello PDF")? {
        return Err(PdfError::ObjectError(
            "Expected original CJK sentence".into(),
        ));
    }
    // Range coordinates are decoded character-code bytes, not Unicode indices.
    let start = page.encode_text_for_font(&font_name, "你好，")?.len();
    let end = start + page.encode_text_for_font(&font_name, "世界")?.len();
    drop(page);
    let stamp = doc.stamp();
    if doc
        .replace_page_text_range(2, &snapshot, operation, None, start..end, "🚀")
        .is_ok()
        || !doc.is_stamp_current(&stamp)
        || doc.get_page(2)?.content_program()?.source_bytes() != snapshot.source_bytes()
    {
        return Err(PdfError::DocumentError(
            "Unmapped character rejection failed".into(),
        ));
    }
    let rollback: PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_page_text_range(2, &snapshot, operation, None, start..end, "你好")?;
        Err(PdfError::DocumentError("Injected outer rollback".into()))
    });
    if !matches!(rollback, Err(PdfError::DocumentError(ref message)) if message == "Injected outer rollback")
        || doc.get_page(2)?.content_program()?.source_bytes() != snapshot.source_bytes()
    {
        return Err(PdfError::DocumentError("CJK rollback failed".into()));
    }
    // A rollback invalidates old snapshots; retry from the current revision.
    let snapshot = doc.get_page(2)?.content_program()?;
    doc.replace_page_text_range(2, &snapshot, operation, None, start..end, "你好")?;
    if doc.get_page(0)?.content_program()?.source_bytes() != original.source_bytes() {
        return Err(PdfError::DocumentError(
            "Original shared page changed".into(),
        ));
    }
    doc.save_to_file(&args[1])?;
    drop(doc);
    let reopened = PdfDocument::open_with_mode(&args[1], ParseMode::Strict)?;
    if reopened.try_page_count()? != 3 {
        return Err(PdfError::DocumentError("Edited page count mismatch".into()));
    }
    for index in 0..3 {
        reopened.get_page(index)?.display_list()?;
    }
    Ok(())
}
