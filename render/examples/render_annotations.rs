//! Opt-in annotation rendering; does not modify page Contents or the document.
use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    page::annotations::{AnnotationRenderIntent, AnnotationRenderOptions},
    parser::syntax::ParseMode,
};
use render::render_device::SkiaRender;
fn main() -> PdfResult<()> {
    let mut args = std::env::args_os().skip(1);
    let input = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("expected input PDF".into()))?;
    let output = std::path::PathBuf::from(
        args.next()
            .ok_or_else(|| PdfError::DocumentError("expected new output directory".into()))?,
    );
    let intent = match args.next().and_then(|v| v.into_string().ok()).as_deref() {
        None | Some("screen") => AnnotationRenderIntent::Screen,
        Some("print") => AnnotationRenderIntent::Print,
        _ => return Err(PdfError::DocumentError("expected screen or print".into())),
    };
    let zoom = match args.next() {
        None => 1.,
        Some(v) => v
            .to_str()
            .and_then(|s| s.parse::<f32>().ok())
            .filter(|v| v.is_finite() && *v > 0.)
            .ok_or_else(|| PdfError::DocumentError("positive zoom required".into()))?,
    };
    if args.next().is_some() {
        return Err(PdfError::DocumentError("too many arguments".into()));
    }
    std::fs::create_dir(&output)?;
    let doc = PdfDocument::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?;
    let revision = doc.commit_revision();
    for i in 0..doc.try_page_count()? {
        let mut device = SkiaRender::new(72. * zoom, 72. * zoom)?;
        doc.get_page(i as u32)?.display_with_annotations(
            &mut device,
            AnnotationRenderOptions {
                intent,
                include_widgets: true,
                view_zoom: zoom,
                ..Default::default()
            },
        )?;
        device.save(
            output
                .join(format!("page-{}.png", i + 1))
                .to_str()
                .ok_or_else(|| PdfError::DocumentError("UTF-8 output path required".into()))?,
        )?;
    }
    assert_eq!(doc.commit_revision(), revision);
    Ok(())
}
