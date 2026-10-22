//! Native reference for WA-03 browser pixel comparisons.
use pdf::{document::Document, parser::syntax::ParseMode};
use render::render_device::SkiaRender;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: rgba_reference INPUT OUTPUT_DIR".into());
    }
    let doc = Document::from_bytes_with_mode(std::fs::read(&args[1])?, ParseMode::Strict)?;
    std::fs::create_dir_all(&args[2])?;
    let root = std::path::Path::new(&args[2]);
    let mut entries = Vec::new();
    let mut renderer = SkiaRender::new(72., 72.)?;
    if let Some(font) = std::env::var_os("RZPDF_SUBSTITUTE_FONT") {
        let index = std::env::var("RZPDF_SUBSTITUTE_FONT_INDEX")
            .ok()
            .map(|value| value.parse::<usize>())
            .transpose()?
            .unwrap_or(0);
        renderer.set_substitute_font(&std::fs::read(font)?, index)?;
    }
    for page in 0..doc.page_count() {
        if doc.optional_content()?.is_some() {
            let visibility = doc.optional_content_visibility(&Default::default())?;
            doc.get_page(page as u32)?
                .display_list_with_annotations_and_optional_content(
                    Default::default(),
                    &visibility,
                )?
                .replay(&mut renderer)?;
        } else {
            doc.get_page(page as u32)?
                .display_with_annotations(&mut renderer, Default::default())?;
        }
        let image = renderer.to_rgba()?;
        std::fs::write(root.join(format!("{page}.rgba")), &image.pixels)?;
        entries.push(format!(
            "{{\"width\":{},\"height\":{}}}",
            image.width, image.height
        ));
    }
    std::fs::write(
        root.join("pages.json"),
        format!("[{}]\n", entries.join(",")),
    )?;
    Ok(())
}
