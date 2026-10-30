//! Render bounded previews with the browser's default image/operation budgets.
use pdf::{
    document::{Document, prepared_page::ImageDecodePolicy},
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};
use render::render_device::SkiaRender;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: masked_preview INPUT.pdf OUTPUT_DIRECTORY".into());
    }
    let doc = Document::from_bytes_with_options(
        std::fs::read(&args[1])?,
        ParseOptions {
            mode: ParseMode::Compatible,
            limits: Limits {
                max_image_pixels: 4_000_000,
                max_decoded_bytes: 16 * 1024 * 1024,
                max_operation_bytes: 64 * 1024 * 1024,
                max_cached_stream_bytes: 16 * 1024 * 1024,
                ..Default::default()
            },
        },
    )?;
    std::fs::create_dir_all(&args[2])?;
    for dpi in [72., 36., 18.] {
        let mut renderer = SkiaRender::new(dpi, dpi)?;
        doc.prepare_page_for_render_with_policy(0, None, dpi, ImageDecodePolicy::TargetResolution)?
            .replay(&doc, &mut renderer)?;
        renderer.save(&format!("{}/{}.png", args[2], dpi as u32))?;
        println!("rendered page 1 at {dpi} DPI");
    }
    Ok(())
}
