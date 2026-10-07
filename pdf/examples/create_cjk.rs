//! Usage: create_cjk FONT.ttf POSTSCRIPT_NAME CAP_HEIGHT STEM_V OUTPUT.pdf [--overlay] [--images]
//! Supply licensed static TrueType data and descriptor metrics in 1000 units.
use pdf::{
    document::{
        PdfDocument, embedded_font::TrueTypeEmbeddingOptions, metadata::InfoTextField,
        page_overlay::PageNumberFormat,
    },
    error::{PdfError, PdfResult},
    page::{content_builder::ContentBuilder, page_builder::PageBuilder},
};

fn main() -> PdfResult<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let flags = if args.len() >= 5 { &args[5..] } else { &[] };
    if args.len() < 5
        || flags
            .iter()
            .any(|flag| flag != "--overlay" && flag != "--images")
        || flags
            .iter()
            .filter(|flag| flag.as_str() == "--overlay")
            .count()
            > 1
        || flags
            .iter()
            .filter(|flag| flag.as_str() == "--images")
            .count()
            > 1
    {
        return Err(PdfError::DocumentError(
            "Usage: create_cjk FONT.ttf POSTSCRIPT_NAME CAP_HEIGHT STEM_V OUTPUT.pdf [--overlay] [--images]"
                .into(),
        ));
    }
    let metric = |value: &str| {
        value
            .parse::<f64>()
            .map_err(|_| PdfError::FontError("Descriptor metric must be numeric".into()))
    };
    let doc = PdfDocument::new()?;
    for index in 0..2 {
        doc.add_page_with_builder(index, PageBuilder::new().size(595.0, 842.0))?;
    }
    let lines = [
        "rzpdf - 中文字体嵌入",
        "你好，世界！Hello PDF",
        "第二页 - 同一字体跨页复用",
        "中文可显示，也应正确提取。",
    ];
    let overlays = flags.iter().any(|flag| flag == "--overlay");
    let images = flags.iter().any(|flag| flag == "--images");
    let mut repertoire = lines.join("");
    if overlays {
        repertoire.push_str("草稿第 页 / 共 12");
    }
    let font = doc.resources().embed_truetype_font(
        0,
        std::fs::read(&args[0]).map_err(|error| PdfError::DocumentError(error.to_string()))?,
        &repertoire,
        TrueTypeEmbeddingOptions {
            postscript_name: args[1].clone(),
            cap_height: metric(&args[2])?,
            stem_v: metric(&args[3])?,
            serif: false,
        },
    )?;
    for index in 0..2 {
        let resource = font.register_on_page(&doc, index)?;
        let mut content = ContentBuilder::new();
        for (line_index, line) in lines[index as usize * 2..index as usize * 2 + 2]
            .iter()
            .enumerate()
        {
            content.begin_text();
            content.try_set_font(&resource, if line_index == 0 { 24.0 } else { 16.0 })?;
            content.move_text(48.0, 770.0 - line_index as f32 * 48.0);
            content.show_encoded_text(&font.encoding().encode_text(line)?)?;
            content.end_text();
        }
        if images {
            // Top row red/green, bottom row blue/yellow. The second page uses
            // varying alpha to exercise a soft mask in the same CID document.
            let image = if index == 0 {
                doc.resources().embed_rgb_image(
                    index,
                    2,
                    2,
                    vec![230, 60, 50, 30, 150, 90, 40, 100, 210, 240, 190, 40],
                )?
            } else {
                doc.resources().embed_rgba_image(
                    index,
                    2,
                    2,
                    vec![
                        230, 60, 50, 255, 30, 150, 90, 192, 40, 100, 210, 128, 240, 190, 40, 0,
                    ],
                )?
            };
            content.draw_image(&image, [240.0, 0.0, 0.0, 160.0, 48.0, 500.0])?;
        }
        doc.replace_page_content_builder(index, &content)?;
    }
    if overlays {
        doc.edit_transaction(|doc| {
            doc.watermark_pages_cid(
                &[0, 1],
                "草稿",
                &font,
                48.0,
                [
                    0.70710677,
                    0.70710677,
                    -0.70710677,
                    0.70710677,
                    170.0,
                    300.0,
                ],
                [0.75; 3],
            )?;
            doc.number_pages_cid(
                &[0, 1],
                &PageNumberFormat {
                    start: 1,
                    prefix: "第 ",
                    total_separator: Some(" 页 / 共 "),
                    suffix: " 页",
                },
                &font,
                12.0,
                [1.0, 0.0, 0.0, 1.0, 48.0, 32.0],
                [0.2; 3],
            )
        })?;
    }
    doc.set_info_text(InfoTextField::Title, Some("rzpdf 中文构建验证"))?;
    doc.save_to_file(&args[4])?;
    let reopened = PdfDocument::open_with_mode(&args[4], pdf::parser::syntax::ParseMode::Strict)?;
    if reopened.try_page_count()? != 2 {
        return Err(PdfError::DocumentError(
            "CJK example page count mismatch".into(),
        ));
    }
    for index in 0..2 {
        reopened.get_page(index)?.display_list()?;
    }
    Ok(())
}
