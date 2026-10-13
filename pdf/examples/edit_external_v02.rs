//! Edit the controlled independent MuPDF fixture from pdf/tests/fixtures/v01.
//! Usage: edit_external_v02 INPUT.pdf OUTPUT.pdf
use pdf::{
    document::owned::Document,
    error::{PdfError, PdfResult},
    page::page_object::PageObject,
    parser::syntax::ParseMode,
};

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err(PdfError::DocumentError(
            "Usage: edit_external_v02 INPUT.pdf OUTPUT.pdf".into(),
        ));
    }
    if std::path::Path::new(&args[1]).exists() {
        return Err(PdfError::IOError("output already exists".into()));
    }
    let doc = Document::open_with_mode(&args[0], ParseMode::Strict)?;
    if doc.try_page_count()? != 2 {
        return Err(PdfError::DocumentError("expected two source pages".into()));
    }
    doc.copy_page(0, 2)?;
    doc.edit_transaction(|editing| {
        let green = editing
            .resources()
            .embed_rgb_image(2, 2, 1, vec![0, 170, 70, 0, 170, 70])?;
        let resources = editing.get_page(2)?.resource()?;
        let green_id = resources
            .get("XObject")
            .and_then(|value| value.as_dict())
            .and_then(|xobjects| xobjects.get(&green))
            .and_then(|value| value.as_object_id())
            .ok_or_else(|| PdfError::ObjectError("green image resource missing".into()))?;
        let green_handle = editing.object_handle(green_id)?;
        let mut page = editing.get_page(2)?;
        let program = page.content_program()?;
        let text = page.extract_characters_paint_order_report()?;
        if !text.matches_program(&program) || !editing.is_stamp_current(text.source_stamp()) {
            return Err(PdfError::DocumentError("text source is stale".into()));
        }
        let selected = text
            .characters
            .get(..8)
            .ok_or_else(|| PdfError::DocumentError("expected External prefix".into()))?;
        if selected
            .iter()
            .filter_map(|ch| ch.unicode.as_deref())
            .collect::<String>()
            != "External"
        {
            return Err(PdfError::DocumentError("unexpected source text".into()));
        }
        let source = selected[0]
            .source
            .ok_or_else(|| PdfError::DocumentError("no text source".into()))?;
        if !selected.iter().all(|ch| ch.source == Some(source)) {
            return Err(PdfError::DocumentError("text spans operations".into()));
        }
        let start = selected[0]
            .decoded_code_range
            .as_ref()
            .ok_or_else(|| PdfError::DocumentError("no start code range".into()))?
            .start;
        let end = selected[7]
            .decoded_code_range
            .as_ref()
            .ok_or_else(|| PdfError::DocumentError("no end code range".into()))?
            .end;
        editing.replace_page_text_range(
            2,
            &program,
            source.operation_index,
            None,
            start..end,
            "Edited",
        )?;

        let mut page = editing.get_page(2)?;
        let program = page.content_program()?;
        let images = page.extract_images_paint_order_report()?;
        if !images.matches_program(&program) || !editing.is_stamp_current(images.source_stamp()) {
            return Err(PdfError::DocumentError("image source is stale".into()));
        }
        let image = images
            .images
            .first()
            .and_then(|image| image.source())
            .ok_or_else(|| PdfError::DocumentError("expected page image".into()))?;
        editing.replace_page_image_draw(2, &program, image.operation_index, &green_handle)?;

        let mut page = editing.get_page(2)?;
        let program = page.content_program()?;
        let images = page.extract_images_paint_order_report()?;
        if !images.matches_program(&program) || !editing.is_stamp_current(images.source_stamp()) {
            return Err(PdfError::DocumentError("image source is stale".into()));
        }
        let image = images
            .images
            .first()
            .and_then(|image| image.source())
            .ok_or_else(|| PdfError::DocumentError("replacement image missing".into()))?;
        editing.translate_page_image_draw_display(2, &program, image.operation_index, 100., 0.)?;

        let mut page = editing.get_page(2)?;
        let program = page.content_program()?;
        let display = page.display_list()?;
        if !display.matches_program(&program) {
            return Err(PdfError::DocumentError("path source is stale".into()));
        }
        let path = display
            .objects()
            .iter()
            .find_map(|object| {
                if let PageObject::Path(path) = object {
                    path.source()
                } else {
                    None
                }
            })
            .ok_or_else(|| PdfError::DocumentError("expected rectangle path".into()))?;
        let rectangle = *path
            .geometry_operation_indices
            .first()
            .ok_or_else(|| PdfError::DocumentError("path geometry missing".into()))?;
        editing.replace_page_path_numbers(2, &program, rectangle, &[120., 30., 80., 40.])
    })?;
    let reopened = Document::from_bytes_with_mode(doc.save_to_bytes()?, ParseMode::Strict)?;
    if reopened.try_page_count()? != 3 {
        return Err(PdfError::DocumentError("edited page count mismatch".into()));
    }
    for index in 0..3 {
        reopened.get_page(index)?.display_list()?;
    }
    doc.save_to_file(&args[1])
}
