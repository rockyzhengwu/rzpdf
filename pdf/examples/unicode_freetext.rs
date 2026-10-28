//! Explicit-font FreeText corpus and runnable API example. Project-owned geometry,
//! using the repository's licensed static Noto fixture; no system fonts.
use pdf::{
    document::{
        Document,
        annotations::{Annotation, AnnotationKind, TextAlignment, UnicodeFreeTextFont},
        embedded_font::TrueTypeEmbeddingOptions,
        save_options::{PdfVersion, SaveLayout, SaveOptions},
    },
    geom::rectangle::Rectangle,
    page::page_builder::PageBuilder,
    parser::syntax::ParseMode,
    security::{EncryptionOptions, SaveSecurity},
};
use std::{fs, path::Path};
fn font(bytes: &[u8]) -> UnicodeFreeTextFont<'_> {
    UnicodeFreeTextFont {
        bytes,
        embedding: TrueTypeEmbeddingOptions {
            postscript_name: "NotoSansSC-Regular".into(),
            cap_height: 733.,
            stem_v: 80.,
            serif: false,
        },
        omit_layout_and_auxiliary: true,
    }
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 2 {
        return Err("usage: unicode_freetext NEW_OUTPUT_DIRECTORY".into());
    }
    let out = Path::new(&args[1]);
    fs::create_dir(out)?;
    let bytes = include_bytes!("../tests/fixtures/v03/NotoSansSC-static-subset.ttf");
    let d = Document::new()?;
    for (page, angle) in [0, 90, 180, 270].into_iter().enumerate() {
        d.add_page_with_builder(
            page as u32,
            PageBuilder::new()
                .size(380., 240.)
                .crop_box(Rectangle::new(10., 20., 360., 220.))
                .rotation(angle)
                .user_unit(if page == 3 { 1.5 } else { 1. }),
        )?;
    }
    let blank = d.save_to_bytes()?;
    fs::write(out.join("base.pdf"), &blank)?;
    for (name, allowed) in [("allowed", true), ("denied", false)] {
        let base = Document::from_bytes_with_mode(blank.clone(), ParseMode::Strict)?;
        let mut encryption = EncryptionOptions::new("user", "owner")?;
        encryption.permissions.annotate = allowed;
        encryption.permissions.modify = !allowed;
        fs::write(
            out.join(format!("{name}.pdf")),
            base.save_to_bytes_with_layout_and_security(
                SaveOptions {
                    version: Some(PdfVersion::V2_0),
                    ..Default::default()
                },
                SaveLayout::Classic,
                SaveSecurity::Set(&encryption),
            )?,
        )?;
    }
    for page in 0..4 {
        for (row, alignment) in [
            TextAlignment::Left,
            TextAlignment::Center,
            TextAlignment::Right,
        ]
        .into_iter()
        .enumerate()
        {
            let y = 30. + row as f32 * 60.;
            let mut a = Annotation::new(
                AnnotationKind::FreeText {
                    font_size: 14.,
                    alignment,
                },
                [30., y, 350., y + 50.],
            );
            a.contents = "你好，世界！\nHello PDF".into();
            a.color = match row {
                0 => [0.1, 0.1, 0.7],
                1 => [0.1, 0.5, 0.1],
                _ => [0.7, 0.1, 0.1],
            };
            a.opacity = if row == 1 { 0.65 } else { 1. };
            d.add_unicode_freetext(page, &a, font(bytes))?;
        }
    }
    fs::write(out.join("created.pdf"), d.save_to_bytes()?)?;
    let next = Document::from_bytes_with_mode(d.save_to_bytes()?, ParseMode::Strict)?;
    for page in 0..4 {
        let records = next.page_annotations(page)?;
        let h = records[0].handle.as_ref().ok_or("missing handle")?;
        let mut a = records[0].annotation.clone().ok_or("missing typed view")?;
        a.contents = "Hello PDF\n世界".into();
        a.kind = AnnotationKind::FreeText {
            font_size: 13.,
            alignment: TextAlignment::Right,
        };
        next.update_unicode_freetext(page, h, &a, None)?;
    }
    fs::write(out.join("updated.pdf"), next.save_to_bytes()?)?;
    fs::write(out.join("font.ttf"), bytes)?;
    println!(
        "Unicode FreeText: 4 rotations × 3 alignments; crop, UserUnit, opacity, create/strict reopen/update and R6 source fixtures in {}",
        out.display()
    );
    Ok(())
}
