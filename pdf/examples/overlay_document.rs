use pdf::{
    document::{PdfDocument, page_overlay::PageNumberFormat, resource_manager::StandardFont},
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};

/// Add a diagonal DRAFT watermark and numbered footer to an existing document.
/// Positions are fixed default user-space coordinates, not automatic layout.
fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(PdfError::DocumentError(
            "usage: overlay_document INPUT.pdf OUTPUT.pdf".into(),
        ));
    }
    if std::path::Path::new(&args[0]) == std::path::Path::new(&args[1]) {
        return Err(PdfError::DocumentError("Use a distinct output path".into()));
    }
    let doc = PdfDocument::open_with_mode(&args[0], ParseMode::Strict)?;
    let count = doc.try_page_count()?;
    let mut pages = Vec::new();
    pages
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("Overlay example selection".into()))?;
    for index in 0..count {
        pages.push(
            u32::try_from(index)
                .map_err(|_| PdfError::ResourceLimit("Page index overflow".into()))?,
        );
    }
    doc.edit_transaction(|doc| {
        doc.watermark_pages_standard(
            &pages,
            "DRAFT",
            StandardFont::Helvetica,
            48.0,
            [
                0.70710677,
                0.70710677,
                -0.70710677,
                0.70710677,
                170.0,
                300.0,
            ],
            [0.75; 3],
        )?;
        doc.number_pages_standard(
            &pages,
            &PageNumberFormat {
                start: 1,
                prefix: "Page ",
                total_separator: Some(" / "),
                suffix: "",
            },
            StandardFont::Helvetica,
            12.0,
            [1.0, 0.0, 0.0, 1.0, 48.0, 32.0],
            [0.2; 3],
        )
    })?;
    // Validate all output pages before writing the destination.
    let bytes = doc.save_to_bytes()?;
    let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict)?;
    for index in &pages {
        reopened.get_page(*index)?.display_list()?;
    }
    doc.save_to_file(&args[1])
}
