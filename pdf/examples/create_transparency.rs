use pdf::{
    document::{
        PdfDocument,
        save_options::{SaveOptions, StreamCompression},
    },
    error::{PdfError, PdfResult},
    page::{content_builder::ContentBuilder, page_builder::PageBuilder},
};

fn main() -> PdfResult<()> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().unwrap_or_else(|| "transparency.pdf".into());
    let via_png = match args.next() {
        None => false,
        Some(flag) if flag == "--png" => true,
        Some(_) => {
            return Err(PdfError::DocumentError(
                "expected optional --png argument".into(),
            ));
        }
    };
    if args.next().is_some() {
        return Err(PdfError::DocumentError("too many arguments".into()));
    }
    let doc = PdfDocument::new()?;
    doc.add_page_with_builder(0, PageBuilder::new().size(300.0, 100.0))?;
    let pixels = vec![200, 40, 20, 0, 200, 40, 20, 128, 200, 40, 20, 255];
    let name = if via_png {
        let mut encoded = Vec::new();
        {
            let mut encoder = png::Encoder::new(&mut encoded, 3, 1);
            encoder.set_color(png::ColorType::Rgba);
            encoder.set_depth(png::BitDepth::Eight);
            let mut writer = encoder
                .write_header()
                .map_err(|_| PdfError::ObjectError("example PNG header encoding failed".into()))?;
            writer
                .write_image_data(&pixels)
                .map_err(|_| PdfError::ObjectError("example PNG encoding failed".into()))?;
            writer
                .finish()
                .map_err(|_| PdfError::ObjectError("example PNG finalization failed".into()))?;
        }
        doc.resources().embed_png_image(0, encoded)?
    } else {
        doc.resources().embed_rgba_image(0, 3, 1, pixels)?
    };
    let mut content = ContentBuilder::new();
    content.draw_image(&name, [300.0, 0.0, 0.0, 100.0, 0.0, 0.0])?;
    doc.replace_page_content_builder(0, &content)?;
    doc.save_to_file_with_options(
        path,
        SaveOptions {
            compression: StreamCompression::FlateUnencoded,
            ..Default::default()
        },
    )
}
