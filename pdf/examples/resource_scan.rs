//! Deterministic large sequential-JPEG scan for browser resource acceptance.
use pdf::{document::PdfDocument, geom::rectangle::Rectangle};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or("provide a NEW output directory")?;
    let output = std::path::PathBuf::from(output);
    std::fs::create_dir(&output)?;
    let mut encoder = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    encoder.set_fastest_defaults();
    encoder.set_size(2500, 3500);
    let mut encoder = encoder.start_compress(Vec::new())?;
    let row = vec![120; 2500 * 3];
    for _ in 0..3500 {
        encoder.write_scanlines(&row)?;
    }
    let doc = PdfDocument::new()?;
    doc.add_page(0)?;
    doc.set_page_crop_box(0, Some(Rectangle::new(0., 0., 350., 490.)))?;
    let name = doc.resources().embed_jpeg_image(0, encoder.finish()?)?;
    doc.replace_page_contents(0, format!("q 350 0 0 490 0 0 cm /{name} Do Q").into_bytes())?;
    std::fs::write(output.join("scan.pdf"), doc.save_to_bytes()?)?;
    let blank = PdfDocument::new()?;
    blank.add_page(0)?;
    blank.replace_page_contents(0, b"0 0 1000 1000 re f".to_vec())?;
    std::fs::write(output.join("solid.pdf"), blank.save_to_bytes()?)?;
    Ok(())
}
