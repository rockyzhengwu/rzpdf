use pdf::{
    document::{owned::Document, resource_manager::StandardFont},
    error::{PdfError, PdfResult},
    page::{page_builder::PageBuilder, page_object::PageObject},
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
        let mut page = editing.get_page(1)?;
        let snapshot = page.content_program()?;
        let report = page.extract_characters_paint_order_report()?;
        if !report.matches_program(&snapshot) || !editing.is_stamp_current(report.source_stamp()) {
            return Err(PdfError::DocumentError("text source is stale".into()));
        }
        let prefix = report
            .characters
            .get(..8)
            .ok_or_else(|| PdfError::DocumentError("expected title prefix is missing".into()))?;
        if prefix
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>()
            != "Original"
        {
            return Err(PdfError::DocumentError("unexpected title prefix".into()));
        }
        let source = prefix[0]
            .source
            .ok_or_else(|| PdfError::DocumentError("title has no page source".into()))?;
        let start = prefix[0]
            .decoded_code_range
            .as_ref()
            .ok_or_else(|| PdfError::DocumentError("title has no character range".into()))?
            .start;
        let end = prefix[7]
            .decoded_code_range
            .as_ref()
            .ok_or_else(|| PdfError::DocumentError("title has no character range".into()))?
            .end;
        if !prefix
            .iter()
            .all(|ch| ch.source == Some(source) && ch.array_item_index.is_none())
        {
            return Err(PdfError::DocumentError(
                "title spans several text operations".into(),
            ));
        }
        editing.replace_page_text_range(
            1,
            &snapshot,
            source.operation_index,
            None,
            start..end,
            "Edited",
        )?;
        let mut page = editing.get_page(1)?;
        let snapshot = page.content_program()?;
        let display = page.display_list()?;
        if !display.matches_program(&snapshot) {
            return Err(PdfError::DocumentError(
                "image display source is stale".into(),
            ));
        }
        let first_image = display
            .objects()
            .iter()
            .find_map(|object| {
                if let PageObject::Image(image) = object {
                    image.source()
                } else {
                    None
                }
            })
            .ok_or_else(|| PdfError::DocumentError("first image is missing".into()))?;
        editing.transform_page_image_draw_display(
            1,
            &snapshot,
            first_image.operation_index,
            [0., 1., -1., 0., 320., 80.],
        )?;
        let mut page = editing.get_page(1)?;
        let snapshot = page.content_program()?;
        let report = page.extract_images_paint_order_report()?;
        if !report.matches_program(&snapshot) || !editing.is_stamp_current(report.source_stamp()) {
            return Err(PdfError::DocumentError("image source is stale".into()));
        }
        let second_image = report
            .images
            .into_iter()
            .filter_map(|draw| draw.source())
            .nth(1)
            .map(|source| source.operation_index)
            .ok_or_else(|| PdfError::ObjectError("second image missing".into()))?;
        editing.remove_page_image_draw(1, &snapshot, second_image)?;
        let mut page = editing.get_page(1)?;
        let snapshot = page.content_program()?;
        let display = page.display_list()?;
        if !display.matches_program(&snapshot) {
            return Err(PdfError::DocumentError(
                "path display source is stale".into(),
            ));
        }
        let red_path = display
            .objects()
            .iter()
            .find_map(|object| {
                if let PageObject::Path(path) = object {
                    path.source()
                } else {
                    None
                }
            })
            .ok_or_else(|| PdfError::ObjectError("red path missing".into()))?;
        editing.remove_page_path_draw(1, &snapshot, red_path.painting_operation_index)
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
