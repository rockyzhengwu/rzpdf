//! Usage: extract_pages OUTPUT.pdf INPUT.pdf INDEX [INDEX ...]
//! Indices are zero-based and unique; output order follows the arguments.
use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() < 3 {
        return Err(PdfError::DocumentError(
            "Usage: extract_pages OUTPUT.pdf INPUT.pdf INDEX [INDEX ...]".into(),
        ));
    }
    let mut indices = Vec::new();
    indices
        .try_reserve_exact(args.len() - 2)
        .map_err(|_| PdfError::ResourceLimit("Extract selection allocation".into()))?;
    for argument in &args[2..] {
        indices.push(
            argument
                .to_str()
                .and_then(|value| value.parse::<u32>().ok())
                .ok_or_else(|| {
                    PdfError::DocumentError("Page index must be a nonnegative u32 integer".into())
                })?,
        );
    }
    let source = PdfDocument::open_with_mode(&args[1], ParseMode::Strict)?;
    let result = source.extract_pages(&indices)?;
    // Extraction must own all resources independently of the source reader.
    drop(source);
    result.save_to_file(&args[0])?;
    let reopened = PdfDocument::open_with_mode(&args[0], ParseMode::Strict)?;
    if reopened.try_page_count()? != indices.len() {
        return Err(PdfError::DocumentError(
            "Extracted page count mismatch".into(),
        ));
    }
    for index in 0..indices.len() {
        reopened
            .get_page(
                u32::try_from(index)
                    .map_err(|_| PdfError::ResourceLimit("Extracted page index".into()))?,
            )?
            .display_list()?;
    }
    Ok(())
}
