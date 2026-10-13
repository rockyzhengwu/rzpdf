//! Two invocations of one Form; edit only the first on one of two shared pages.
//! Usage: edit_form_v02 OUTPUT.pdf
use pdf::{
    document::{
        owned::Document,
        resource_manager::{ResourceKind, StandardFont},
    },
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::{page_builder::PageBuilder, page_object::PageObject},
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

fn main() -> PdfResult<()> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or_else(|| PdfError::DocumentError("Usage: edit_form_v02 OUTPUT.pdf".into()))?;
    if std::path::Path::new(&output).exists() {
        return Err(PdfError::IOError("output already exists".into()));
    }
    let doc = Document::new()?;
    doc.add_page_with_builder(0, PageBuilder::new().size(230., 110.))?;
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)?;
    let red = doc
        .resources()
        .embed_rgb_image(0, 2, 1, vec![255, 0, 0, 255, 0, 0])?;
    let mut form_dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(
        b"<< /Subtype /Form /BBox [0 0 100 100] >>".to_vec(),
    ))?)
    .get_object()?
    .into_dict()
    .ok_or_else(|| PdfError::ObjectError("Form dictionary missing".into()))?;
    form_dict.try_insert(
        "Resources".into(),
        PdfObject::PdfDict(doc.get_page(0)?.resource()?),
    )?;
    let body = format!(
        "BT /{font} 16 Tf 10 75 Td (AB) Tj ET\nq 40 0 0 20 20 20 cm /{red} Do Q\n0 0 0 RG 20 20 40 20 re S\n"
    );
    let form = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
        form_dict,
        body.into_bytes(),
    )?))?;
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form)?;
    doc.replace_page_contents(
        0,
        format!("q 1 0 0 1 0 0 cm /{name} Do Q\nq 1 0 0 1 110 0 cm /{name} Do Q\n").into_bytes(),
    )?;
    doc.copy_page(0, 1)?;

    doc.edit_transaction(|editing| {
        let blue = editing
            .resources()
            .embed_rgb_image(0, 2, 1, vec![0, 0, 255, 0, 0, 255])?;
        let resources = editing.get_page(0)?.resource()?;
        let blue_id = resources
            .get("XObject")
            .and_then(|value| value.as_dict())
            .and_then(|xobjects| xobjects.get(&blue))
            .and_then(|value| value.as_object_id())
            .ok_or_else(|| PdfError::ObjectError("blue image missing".into()))?;
        let blue_handle = editing.object_handle(blue_id)?;

        let mut page = editing.get_page(0)?;
        let root = page.content_program()?;
        let text = page.extract_characters_paint_order_report()?;
        if !text.matches_program(&root) || !editing.is_stamp_current(text.source_stamp()) {
            return Err(PdfError::DocumentError("Form text source is stale".into()));
        }
        let source = text
            .characters
            .first()
            .and_then(|ch| ch.nested_source.as_ref())
            .ok_or_else(|| PdfError::DocumentError("Form text source missing".into()))?;
        editing.replace_nested_text(0, &root, source, "CD")?;

        let mut page = editing.get_page(0)?;
        let root = page.content_program()?;
        let images = page.extract_images_paint_order_report()?;
        if !images.matches_program(&root) || !editing.is_stamp_current(images.source_stamp()) {
            return Err(PdfError::DocumentError("Form image source is stale".into()));
        }
        let source = images
            .images
            .first()
            .and_then(|image| image.nested_source())
            .ok_or_else(|| PdfError::DocumentError("Form image source missing".into()))?;
        editing.replace_nested_image_draw(0, &root, source, &blue_handle)?;

        let mut page = editing.get_page(0)?;
        let root = page.content_program()?;
        let display = page.display_list()?;
        if !display.matches_program(&root) {
            return Err(PdfError::DocumentError("Form path source is stale".into()));
        }
        let source = display
            .objects()
            .iter()
            .find_map(|object| {
                if let PageObject::Path(path) = object {
                    path.nested_source()
                } else {
                    None
                }
            })
            .ok_or_else(|| PdfError::DocumentError("Form path source missing".into()))?;
        let geometry = *source
            .geometry_operation_indices
            .first()
            .ok_or_else(|| PdfError::DocumentError("Form path geometry missing".into()))?;
        editing.replace_nested_path_numbers(0, &root, source, geometry, &[20., 20., 50., 20.])
    })?;

    let reopened = Document::from_bytes_with_mode(doc.save_to_bytes()?, ParseMode::Strict)?;
    if reopened.try_page_count()? != 2 {
        return Err(PdfError::DocumentError("Form page count mismatch".into()));
    }
    for index in 0..2 {
        reopened.get_page(index)?.display_list()?;
    }
    doc.save_to_file(output)
}
