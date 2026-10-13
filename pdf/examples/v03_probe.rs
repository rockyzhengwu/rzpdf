//! Inspect a fixed external PDF matrix and save a round-trip copy.
//! Usage: v03_probe INPUT.pdf OUTPUT.pdf [--strict|--compat]
use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};

fn main() -> PdfResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.len() < 2 || args.len() > 3 {
        return Err(PdfError::DocumentError(
            "Usage: v03_probe INPUT.pdf OUTPUT.pdf [--strict|--compat]".into(),
        ));
    }
    let mode = match args.get(2).map(String::as_str) {
        None | Some("--compat") => ParseMode::Compatible,
        Some("--strict") => ParseMode::Strict,
        _ => {
            return Err(PdfError::DocumentError(
                "mode must be --strict or --compat".into(),
            ));
        }
    };
    let doc = PdfDocument::open_with_mode(&args[0], mode)?;
    let pages = doc.try_page_count()?;
    let mut original_events = Vec::new();
    println!("pages={pages}");
    for index in 0..pages {
        let mut page = doc.get_page(u32::try_from(index).map_err(|_| {
            PdfError::ResourceLimit("v0.3 probe page index".into())
        })?)?;
        let display = page.display_list();
        let extraction = page.extract_characters_paint_order_report();
        match (display, extraction) {
            (Ok(_), Ok(report)) => {
                let mapped = report
                    .characters
                    .iter()
                    .filter(|item| item.unicode.is_some())
                    .count();
                println!(
                    "page={} characters={} mapped={} diagnostics={}",
                    index + 1,
                    report.characters.len(),
                    mapped,
                    report.diagnostics.len()
                );
                for diagnostic in report.diagnostics.iter().take(3) {
                    println!("  diagnostic={diagnostic:?}");
                }
                if mapped != report.characters.len() {
                    return Err(PdfError::FontError(format!(
                        "page {} has {} characters without Unicode",
                        index + 1,
                        report.characters.len() - mapped
                    )));
                }
                original_events.push(
                    report.characters.into_iter().map(|item| {
                        (
                            item.unicode,
                            item.charcode,
                            item.position.x().to_bits(),
                            item.position.y().to_bits(),
                        )
                    }).collect::<Vec<_>>()
                );
            }
            (display, extraction) => {
                println!(
                    "page={} display_error={:?} extraction_error={:?}",
                    index + 1,
                    display.err(),
                    extraction.err()
                );
                return Err(PdfError::DocumentError("page interpretation failed".into()));
            }
        }
    }
    doc.save_to_file(&args[1])?;
    let reopened = PdfDocument::open_with_mode(&args[1], ParseMode::Strict)?;
    if reopened.try_page_count()? != pages {
        return Err(PdfError::DocumentError("round-trip page count changed".into()));
    }
    for (index, expected) in original_events.iter().enumerate() {
        let page = u32::try_from(index)
            .map_err(|_| PdfError::ResourceLimit("v0.3 probe page index".into()))?;
        let report = reopened.get_page(page)?.extract_characters_paint_order_report()?;
        let actual = report.characters.into_iter().map(|item| {
            (
                item.unicode,
                item.charcode,
                item.position.x().to_bits(),
                item.position.y().to_bits(),
            )
        }).collect::<Vec<_>>();
        if !report.diagnostics.is_empty() || actual != *expected {
            return Err(PdfError::DocumentError(format!(
                "page {} text or position changed after save",
                index + 1
            )));
        }
    }
    println!("roundtrip=strict-ok");
    Ok(())
}
