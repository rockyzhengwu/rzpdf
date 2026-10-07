//! A small PDF 1.6 fixture for independent UserUnit/rotation inspection.
use pdf::{
    document::{PdfDocument, resource_manager::StandardFont, save_options::PdfVersion},
    error::PdfResult,
    geom::rectangle::Rectangle,
    page::page_builder::PageBuilder,
};

fn main() -> PdfResult<()> {
    let path = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "user-unit.pdf".into());
    let doc = PdfDocument::new_with_version(PdfVersion::V1_6)?;
    for (index, (unit, rotation)) in [
        (1.5, 0),
        (1.5, 90),
        (1.5, 180),
        (1.5, 270),
        (0.5, 0),
        (2.5, 0),
    ]
    .into_iter()
    .enumerate()
    {
        let index = index as u32;
        doc.add_page_with_builder(
            index,
            PageBuilder::new()
                .media_box(Rectangle::new(0., 0., 200., 140.))
                .crop_box(Rectangle::new(10., 20., 170., 100.))
                .rotation(rotation)
                .user_unit(unit),
        )?;
        let font = doc
            .resources()
            .register_standard_font(index, StandardFont::Helvetica)?;
        let image = doc
            .resources()
            .embed_rgb_image(index, 2, 1, vec![255, 0, 0, 0, 255, 0])?;
        doc.replace_page_contents(index, format!(
            "0 0 0 rg BT /{font} 12 Tf 1 0 0 1 20 80 Tm (UserUnit {unit}, Rotate {rotation}) Tj ET q 40 0 0 20 20 45 cm /{image} Do Q 0 0 1 rg 100 30 50 20 re f"
        ).into_bytes())?;
    }
    doc.save_to_file(path)
}
