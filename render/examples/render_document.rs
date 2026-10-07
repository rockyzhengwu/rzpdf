//! Render every page without overwriting existing output files.
use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
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
            .ok_or_else(|| PdfError::DocumentError("expected output directory".into()))?,
    );
    let parse_dpi = |value: Option<std::ffi::OsString>| -> PdfResult<f32> {
        match value {
            None => Ok(72.0),
            Some(value) => value
                .to_str()
                .and_then(|s| s.parse::<f32>().ok())
                .filter(|dpi| dpi.is_finite() && *dpi > 0.0)
                .ok_or_else(|| PdfError::DocumentError("DPI must be finite and positive".into())),
        }
    };
    let xdpi = parse_dpi(args.next())?;
    let ydpi = parse_dpi(args.next())?;
    if args.next().is_some() {
        return Err(PdfError::DocumentError("too many arguments".into()));
    }
    let doc = PdfDocument::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?;
    std::fs::create_dir_all(&output)?;
    for index in 0..doc.try_page_count()? {
        let path = output.join(format!("skia-{}.png", index + 1));
        if path.exists() {
            return Err(PdfError::DocumentError(
                "render output already exists".into(),
            ));
        }
        let mut renderer = SkiaRender::new(xdpi, ydpi)?;
        let page = u32::try_from(index)
            .map_err(|_| PdfError::ResourceLimit("page index exceeds u32".into()))?;
        doc.get_page(page)?.display(&mut renderer)?;
        renderer.save(
            path.to_str()
                .ok_or_else(|| PdfError::DocumentError("output path must be UTF-8".into()))?,
        )?;
    }
    Ok(())
}
