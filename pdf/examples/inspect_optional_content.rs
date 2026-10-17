//! Explicit configuration selection; no source mutation or layer flattening.
use pdf::{
    document::{
        PdfDocument,
        optional_content::{OptionalContentIntent, OptionalContentOptions},
    },
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};
fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(PdfError::DocumentError(
            "input.pdf new-output.pdf default|config-index view|print".into(),
        ));
    }
    let configuration = if args[2] == "default" {
        None
    } else {
        Some(
            args[2]
                .parse()
                .map_err(|_| PdfError::DocumentError("configuration index".into()))?,
        )
    };
    let intent = match args[3].as_str() {
        "view" => OptionalContentIntent::View,
        "print" => OptionalContentIntent::Print,
        _ => return Err(PdfError::DocumentError("view or print".into())),
    };
    let doc = PdfDocument::from_bytes_with_mode(std::fs::read(&args[0])?, ParseMode::Strict)?;
    let stamp = doc.stamp();
    let options = OptionalContentOptions {
        configuration,
        intent,
        ..Default::default()
    };
    let visibility = doc.optional_content_visibility(&options)?;
    if let Some(model) = doc.optional_content()? {
        println!(
            "groups={} configurations={}",
            model.groups.len(),
            model.configurations.len()
        );
        for group in model.groups {
            println!(
                "group={:?} state={}",
                group.name,
                visibility.group_state(&group.handle)?
            );
        }
    }
    for i in 0..doc.try_page_count()? {
        let mut page = doc.get_page(i as u32)?;
        let list = page.display_list_with_optional_content(&visibility)?;
        if !list.diagnostics().is_empty() {
            return Err(PdfError::UnsupportedFeature(
                "optional-content display diagnostics".into(),
            ));
        }
        let report =
            page.extract_marked_text_with_optional_content(Default::default(), &visibility)?;
        println!(
            "page={} text={:?}",
            i + 1,
            report.text(&Default::default())?
        );
    }
    assert!(doc.is_stamp_current(&stamp));
    let bytes = doc.save_to_bytes()?;
    let reopened = PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
    reopened.optional_content_visibility(&options)?;
    use std::io::Write;
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[1])?
        .write_all(&bytes)?;
    Ok(())
}
