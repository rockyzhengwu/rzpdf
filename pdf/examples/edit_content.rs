use pdf::{
    document::{owned::Document, resource_manager::StandardFont},
    error::{PdfError, PdfResult},
    page::page_builder::PageBuilder,
    parser::syntax::ParseMode,
};

fn main() -> PdfResult<()> {
    let path = std::env::args_os()
        .nth(1)
        .unwrap_or_else(|| "output/pdf/rzpdf-edit-content-example.pdf".into());
    let doc = Document::new()?;
    doc.add_page_with_builder(0, PageBuilder::new().size(360., 400.))?;
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)?;
    let image = doc
        .resources()
        .embed_rgb_image(0, 2, 1, vec![230, 60, 50, 30, 150, 90])?;
    doc.replace_page_contents(0,format!(
        "BT /{font} 18 Tf 40 340 Td (Original content) Tj ET\nq 160 0 0 40 40 180 cm /{image} Do Q\nq 80 0 0 40 240 180 cm /{image} Do Q\n1 0 0 rg 40 80 80 50 re f\n0 0 1 rg 160 80 80 50 re f\n"
    ).into_bytes())?;
    doc.copy_page(0, 1)?;
    doc.edit_transaction(|editing| {
        let snapshot = editing.get_page(1)?.content_program()?;
        editing.replace_page_text_range(1, &snapshot, 3, None, 0..8, "Edited")?;
        let snapshot = editing.get_page(1)?.content_program()?;
        editing.transform_page_image_draw_display(1, &snapshot, 7, [0., 1., -1., 0., 320., 80.])?;
        let snapshot = editing.get_page(1)?.content_program()?;
        let second_image = snapshot
            .operations()
            .iter()
            .enumerate()
            .filter(|(_, op)| op.operator.name() == "Do")
            .nth(1)
            .map(|(index, _)| index)
            .ok_or_else(|| PdfError::ObjectError("second image missing".into()))?;
        editing.remove_page_image_draw(1, &snapshot, second_image)?;
        let snapshot = editing.get_page(1)?.content_program()?;
        let red_rectangle = snapshot
            .operations()
            .iter()
            .position(|op| {
                op.operator.name() == "re" && op.operator.get_as_f32(0).ok() == Some(40.)
            })
            .ok_or_else(|| PdfError::ObjectError("red rectangle missing".into()))?;
        editing.remove_page_path_draw(1, &snapshot, red_rectangle + 1)
    })?;
    let bytes = doc.save_to_bytes()?;
    let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict)?;
    for page in [0, 1] {
        reopened.get_page(page)?.display_list()?;
    }
    if let Some(parent) = std::path::Path::new(&path)
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
    {
        std::fs::create_dir_all(parent).map_err(|error| PdfError::IOError(error.to_string()))?;
    }
    if std::path::Path::new(&path).exists() {
        return Err(PdfError::IOError(
            "example output already exists; choose a new path".into(),
        ));
    }
    doc.save_to_file(&path)?;
    println!(
        "Saved two-page original/edited comparison to {}",
        std::path::Path::new(&path).display()
    );
    Ok(())
}
