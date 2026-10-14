//! Independent-check entry point: export raw RGBA and preserve/import images.
use pdf::{
    document::Document,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber},
    page::{content_builder::ContentBuilder, inline_image::InlineImage},
    parser::{options::Limits, syntax::ParseMode},
    security::{EncryptionOptions, SaveSecurity},
};
use std::{io::Write, path::PathBuf};
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: v06_images INPUT OUTPUT_DIRECTORY (directory must not exist)".into());
    }
    let output = PathBuf::from(&args[1]);
    if output.exists() {
        return Err("refusing to overwrite output directory".into());
    }
    let bytes = std::fs::read(&args[0])?;
    if bytes.len() > 64 * 1024 * 1024 {
        return Err("example input limit: 64 MiB".into());
    }
    let doc = Document::from_bytes_with_mode(bytes, ParseMode::Strict)?;
    std::fs::create_dir(&output)?;
    let mut info = std::fs::File::create(output.join("images.tsv"))?;
    for index in 0..doc.try_page_count()? {
        let report = doc
            .get_page(index as u32)?
            .extract_images_paint_order_report()?;
        if !report.diagnostics.is_empty() {
            return Err("image interpretation diagnostics are not accepted".into());
        }
        for (draw, image) in report.images.iter().enumerate() {
            let filename = format!("page-{}-image-{}.rgba", index + 1, draw + 1);
            std::fs::write(output.join(&filename), image.image.rgba_buffer()?)?;
            writeln!(
                info,
                "{}\t{}\t{}\t{}\t{}",
                index + 1,
                draw + 1,
                image.image.width(),
                image.image.height(),
                filename
            )?;
        }
    }
    doc.save_to_file(output.join("roundtrip.pdf"))?;
    let imported = Document::new_with_version(pdf::document::save_options::PdfVersion::V1_5)?;
    let pages: Vec<u32> = (0..doc.try_page_count()?).map(|i| i as u32).collect();
    imported.import_pages_from(&doc, &pages, 0)?;
    drop(doc);
    imported.save_to_file(output.join("imported.pdf"))?;

    let inline = Document::new()?;
    inline.add_page(0)?;
    let image = |pixels: Vec<u8>| -> Result<InlineImage, Box<dyn std::error::Error>> {
        let mut dict = PdfDict::default();
        for (key, value) in [("W", 2.), ("H", 2.), ("BPC", 8.)] {
            dict.insert(key.into(), PdfObject::PdfNumber(PdfNumber::new(value)));
        }
        dict.insert("CS".into(), PdfObject::PdfName(PdfName::new("RGB".into())));
        Ok(InlineImage::new(dict, pixels, Limits::default())?)
    };
    let original = image(vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0])?;
    let replacement = image(vec![0, 255, 255, 255, 0, 255, 255, 255, 255, 0, 0, 0])?;
    let mut builder = ContentBuilder::new();
    builder.draw_inline_image(
        &original,
        [120., 0., 0., 120., 30., 500.],
        Limits::default(),
    )?;
    builder.draw_inline_image(&original, [60., 0., 0., 60., 200., 500.], Limits::default())?;
    inline.replace_page_contents(0, builder.as_bytes().to_vec())?;
    inline.save_to_file(output.join("inline.pdf"))?;
    let encryption = EncryptionOptions::new("user", "owner")?;
    std::fs::write(
        output.join("inline-r6.pdf"),
        inline.save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&encryption))?,
    )?;
    let program = inline.get_page(0)?.content_program()?;
    inline.replace_page_inline_image_draw(0, &program, 2, &replacement)?;
    inline.save_to_file(output.join("inline-replaced.pdf"))?;
    let program = inline.get_page(0)?.content_program()?;
    inline.remove_page_image_draw(0, &program, 2)?;
    inline.save_to_file(output.join("inline-removed.pdf"))?;
    Ok(())
}
