//! Isolated single-page worker for scripts/sample_pdf_corpus.py.
use pdf::{document::Document, parser::syntax::ParseMode};
use render::render_device::SkiaRender;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 5 {
        return Err("Usage: check_render INPUT.pdf OUTPUT.png PAGE strict|compatible DPI".into());
    }
    let number: u32 = args[2].to_str().ok_or("Invalid page")?.parse()?;
    let index = number.checked_sub(1).ok_or("Page numbers start at 1")?;
    let mode = match args[3].to_str() {
        Some("strict") => ParseMode::Strict,
        Some("compatible") => ParseMode::Compatible,
        _ => return Err("Invalid parse mode".into()),
    };
    let dpi: f32 = args[4].to_str().ok_or("Invalid DPI")?.parse()?;
    if !dpi.is_finite() || dpi <= 0. {
        return Err("DPI must be finite and positive".into());
    }
    let output = std::path::Path::new(&args[1]);
    if output.exists() {
        return Err("Output already exists".into());
    }
    let document = Document::open_with_mode(&args[0], mode)?;
    let mut renderer = SkiaRender::new(dpi, dpi)?;
    if let Some(path) = std::env::var_os("RZPDF_SUBSTITUTE_FONT") {
        renderer.set_substitute_font(&std::fs::read(path)?, 0)?;
    }
    document.get_page(index)?.display(&mut renderer)?;
    renderer.save(output.to_str().ok_or("Output path must be UTF-8")?)?;
    println!("page={number}");
    Ok(())
}
