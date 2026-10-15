//! Fixed-matrix validation driver, not a general layout/shaping tool.
use pdf::{
    document::{
        Document,
        glyph_run::{GlyphRun, PositionedGlyph},
    },
    error::{PdfError, PdfResult},
    font::GlyphDesc,
    parser::syntax::ParseMode,
};
use std::io::Write;

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02X}")).collect()
}

fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 2 {
        return Err(PdfError::DocumentError(
            "usage: v07_font_roundtrip INPUT NEW_OUTPUT".into(),
        ));
    }
    let doc = Document::open_with_mode(&args[0], ParseMode::Strict)?;
    for page_index in 0..doc.try_page_count()? as u32 {
        let mut page = doc.get_page(page_index)?;
        let font = page.inspect_font("F0")?;
        let chars = page.extract_characters_paint_order()?;
        let mut inputs = Vec::new();
        for (index, ch) in chars.iter().enumerate() {
            let code = ch.charcode.to_be_bytes();
            let source = &code[4 - ch.code_length as usize..];
            let parsed = font.chars(source)?;
            let gid = match font.get_glyph(&parsed[0]) {
                Some(GlyphDesc::Gid(gid)) => gid,
                _ => return Err(PdfError::FontError("matrix expects numeric GID".into())),
            };
            println!(
                "MAP\t{page_index}\t{index}\t{}\t{gid}\t{}\t{}\t{}\t{}\t{}\t{}",
                hex(source),
                parsed[0].width(),
                parsed[0].origin_x(),
                parsed[0].origin_y(),
                ch.position.x(),
                ch.position.y(),
                ch.unicode
                    .as_ref()
                    .map(|s| hex(s.as_bytes()))
                    .unwrap_or_else(|| "-".into())
            );
            inputs.push((code, ch.code_length, gid, ch.unicode.clone()));
        }
        let glyphs: Vec<_> = inputs
            .iter()
            .enumerate()
            .map(|(index, (code, length, gid, unicode))| PositionedGlyph {
                code: &code[4 - *length as usize..],
                expected_gid: Some(*gid),
                unicode: unicode.as_deref(),
                position: [30. + index as f32 * 30., 40. + (index % 2) as f32 * 15.],
            })
            .collect();
        let run = GlyphRun {
            font_name: "F0",
            font_size: 20.,
            matrix: [1., 0., 0., 1., 0., 0.],
            color: [0., 0., 0.],
            glyphs: &glyphs,
        };
        doc.overlay_page_glyph_run(page_index, &run)?;
    }
    let bytes = doc.save_to_bytes()?;
    let reopened = Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
    for page_index in 0..reopened.try_page_count()? as u32 {
        for (index, ch) in reopened
            .get_page(page_index)?
            .extract_characters_paint_order()?
            .iter()
            .enumerate()
        {
            println!(
                "CHAR\t{page_index}\t{index}\t{}\t{}\t{}",
                ch.position.x(),
                ch.position.y(),
                ch.unicode
                    .as_ref()
                    .map(|s| hex(s.as_bytes()))
                    .unwrap_or_else(|| "-".into())
            );
            let origin = ch.glyph_origin_display()?;
            println!(
                "GLYPH\t{page_index}\t{index}\t{}\t{}",
                origin.x(),
                origin.y()
            );
        }
    }
    std::fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&args[1])?
        .write_all(&bytes)?;
    Ok(())
}
