//! Usage: edit_page_order INPUT.pdf OUTPUT.pdf
//! Import pages in reverse, copy the last source page, then restore source
//! order. Output contains all original pages plus a copy of the last page.
//! Page-only workflow: catalog metadata, annotations and outlines are not copied.
use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(PdfError::DocumentError(
            "Usage: edit_page_order INPUT.pdf OUTPUT.pdf".into(),
        ));
    }
    let source = PdfDocument::open_with_mode(&args[0], ParseMode::Strict)?;
    let count = u32::try_from(source.try_page_count()?)
        .map_err(|_| PdfError::ResourceLimit("Page count".into()))?;
    if count == 0 {
        return Err(PdfError::DocumentError("Input must contain a page".into()));
    }
    let expected = count
        .checked_add(1)
        .ok_or_else(|| PdfError::ResourceLimit("Copied page count".into()))?;
    let target = PdfDocument::new_with_version(source.effective_version()?)?;
    let mut order = Vec::new();
    order
        .try_reserve_exact(expected as usize)
        .map_err(|_| PdfError::ResourceLimit("Page selection allocation".into()))?;
    order.extend((0..count).rev());
    target.import_pages_from(&source, &order, 0)?;
    target.copy_page(0, count)?;
    order.push(count);
    target.reorder_pages(&order)?;
    // All copied resources must survive without the source's reader/context.
    drop(source);
    for index in 0..expected {
        target.get_page(index)?.display_list()?;
    }
    target.save_to_file(&args[1])?;
    drop(target);
    let reopened = PdfDocument::open_with_mode(&args[1], ParseMode::Strict)?;
    if reopened.try_page_count()? != expected as usize {
        return Err(PdfError::DocumentError("Edited page count mismatch".into()));
    }
    for index in 0..expected {
        reopened.get_page(index)?.display_list()?;
    }
    Ok(())
}
