//! Explicit layer states replayed through the existing Skia backend.
use pdf::{
    document::{
        PdfDocument,
        optional_content::{OptionalContentIntent, OptionalContentOptions},
    },
    error::{PdfError, PdfResult},
    page::annotations::{AnnotationRenderIntent, AnnotationRenderOptions},
    parser::syntax::ParseMode,
};
use render::render_device::SkiaRender;
fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err(PdfError::DocumentError(
            "input.pdf new-output-directory default|config-index view|print".into(),
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
    let (intent, annot_intent) = match args[3].as_str() {
        "view" => (OptionalContentIntent::View, AnnotationRenderIntent::Screen),
        "print" => (OptionalContentIntent::Print, AnnotationRenderIntent::Print),
        _ => return Err(PdfError::DocumentError("view or print".into())),
    };
    let doc = PdfDocument::from_bytes_with_mode(std::fs::read(&args[0])?, ParseMode::Strict)?;
    let stamp = doc.stamp();
    let visibility = doc.optional_content_visibility(&OptionalContentOptions {
        configuration,
        intent,
        ..Default::default()
    })?;
    let output = std::path::PathBuf::from(&args[1]);
    std::fs::create_dir(&output)?;
    for i in 0..doc.try_page_count()? {
        let list = doc
            .get_page(i as u32)?
            .display_list_with_annotations_and_optional_content(
                AnnotationRenderOptions {
                    intent: annot_intent,
                    ..Default::default()
                },
                &visibility,
            )?;
        let mut device = SkiaRender::new(72., 72.)?;
        list.replay(&mut device)?;
        device.save(
            output
                .join(format!("page-{}.png", i + 1))
                .to_str()
                .ok_or_else(|| PdfError::DocumentError("UTF8 path required".into()))?,
        )?;
    }
    assert!(doc.is_stamp_current(&stamp));
    Ok(())
}
