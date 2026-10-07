use pdf::{
    document::PdfDocument,
    filter::{FilterContext, apply_filter_with_context},
    io::stream_reader::StreamReader,
    page::content_parser::ContentParser,
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;

pub const MAX_INPUT: usize = 64 * 1024;

/// Minimal sfnt/cmap parser fixture, not a complete renderable TrueType font.
pub fn font_seed() -> Vec<u8> {
    let mut bytes = vec![0, 1, 0, 0, 0, 1, 0, 16, 0, 0, 0, 0];
    bytes.extend_from_slice(b"cmap");
    bytes.extend_from_slice(&0u32.to_be_bytes());
    bytes.extend_from_slice(&28u32.to_be_bytes());
    bytes.extend_from_slice(&52u32.to_be_bytes());
    for word in [0u16, 1, 3, 1] {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    bytes.extend_from_slice(&12u32.to_be_bytes());
    for word in [
        4u16, 40, 0, 6, 4, 1, 2, 65, 70, 65535, 0, 60, 67, 65535, 1, 65534, 1, 0, 0, 0,
    ] {
        bytes.extend_from_slice(&word.to_be_bytes());
    }
    bytes
}

pub fn fonts(data: &[u8]) {
    use pdf::font::truetype::{
        reader::TrueTypeReader, tables::cmap::TCmap, true_type_program::TrueTypeProgram,
    };
    if data.len() > MAX_INPUT {
        return;
    }
    let mut queries = vec![0, 1, 60, 65, 66, 67, 70, 71, 255, 256, u16::MAX];
    queries.extend(
        data.chunks_exact(2)
            .take(64)
            .map(|pair| u16::from_be_bytes([pair[0], pair[1]])),
    );
    if let Ok(mut font) =
        TrueTypeProgram::try_new_with_limits(data, options(ParseMode::Strict).limits)
    {
        for symbolic in [false, true] {
            font.selct_charmap(symbolic);
            if let Some(cmap) = font.current_cmap() {
                for &query in &queries {
                    let _ = cmap.get_gid(query);
                }
            }
        }
    }
    // A direct subtable path reaches malformed cmap data without requiring a
    // successful sfnt directory first. Its input is still capped above.
    if let Ok(cmap) = TCmap::try_new(3, 1, 0, &mut TrueTypeReader::new(data)) {
        for &query in &queries {
            let _ = cmap.get_gid(query);
        }
    }
}

/// Project-generated seed: pages, text, a path, RGB image and soft mask.
pub fn document_seed() -> pdf::error::PdfResult<Vec<u8>> {
    use pdf::{document::resource_manager::StandardFont, page::content_builder::ContentBuilder};
    let doc = PdfDocument::new()?;
    for index in 0..2 {
        doc.add_page(index)?;
        let font = doc
            .resources()
            .register_standard_font(index, StandardFont::Helvetica)?;
        let image = doc.resources().embed_rgba_image(
            index,
            2,
            1,
            vec![200, 40, 20, 128, 20, 80, 200, 255],
        )?;
        let mut content = ContentBuilder::new();
        content.begin_text();
        content.try_set_font(&font, 12.0)?;
        content.move_text(20.0, 40.0);
        content.show_encoded_text(b"rzpdf fuzz seed")?;
        content.end_text();
        content.rectangle(20.0, 20.0, 40.0, 10.0).stroke();
        content.draw_image(&image, [20.0, 0.0, 0.0, 10.0, 80.0, 20.0])?;
        doc.replace_page_content_builder(index, &content)?;
    }
    doc.save_to_bytes()
}

fn options(mode: ParseMode) -> ParseOptions {
    ParseOptions {
        mode,
        limits: Limits {
            // Also used for built-in font metrics; permit Helvetica's character records.
            max_objects: 1024,
            max_depth: 16,
            max_decoded_bytes: MAX_INPUT * 4,
            max_raw_stream_bytes: MAX_INPUT,
            max_cached_stream_bytes: MAX_INPUT * 2,
            max_token_bytes: 4096,
            max_content_operations: 1024,
            max_content_diagnostics: 32,
            max_content_diagnostic_bytes: 4096,
            max_image_pixels: 4096,
            max_text_chars: 4096,
            max_path_segments: 4096,
            max_clip_paths: 32,
            ..Limits::default()
        },
    }
}

fn parser(data: &[u8], mode: ParseMode) -> SyntaxParser<Cursor<Vec<u8>>> {
    SyntaxParser::with_options(
        StreamReader::try_new(Cursor::new(data.to_vec())).unwrap(),
        options(mode),
    )
}

pub fn syntax(data: &[u8]) {
    if data.len() > MAX_INPUT {
        return;
    }
    for mode in [ParseMode::Strict, ParseMode::Compatible] {
        let mut parser = parser(data, mode);
        for _ in 0..1024 {
            if parser.is_eof() {
                break;
            }
            let before = parser.position();
            match parser.get_object() {
                Ok(_) => assert!(
                    parser.position() > before,
                    "successful syntax read must advance"
                ),
                Err(_) => break,
            }
        }
    }
}

pub fn content(data: &[u8]) {
    if data.len() > MAX_INPUT {
        return;
    }
    for mode in [ParseMode::Strict, ParseMode::Compatible] {
        let mut parser = ContentParser::new(parser(data, mode));
        for _ in 0..1024 {
            match parser.read_operator() {
                Ok(Some(_)) => {}
                Ok(None) | Err(_) => break,
            }
        }
        // Also reach the interpreter, not only content tokenization.
        let document = PdfDocument::new_with_options(options(mode)).unwrap();
        document.add_page(0).unwrap();
        // Make valid text/image seed operators reach interpretation, not only
        // missing-resource errors. Registration deterministically yields F1/Im1.
        let font = document
            .resources()
            .register_standard_font(0, pdf::document::resource_manager::StandardFont::Helvetica)
            .unwrap();
        let image = document
            .resources()
            .embed_rgba_image(0, 2, 1, vec![200, 40, 20, 128, 20, 80, 200, 255])
            .unwrap();
        assert_eq!(font, "F1");
        assert_eq!(image, "Im1");
        document.replace_page_contents(0, data.to_vec()).unwrap();
        let _ = document.get_page(0).unwrap().display_list();
    }
}

pub fn document(data: &[u8]) {
    if data.len() > MAX_INPUT {
        return;
    }
    for mode in [ParseMode::Strict, ParseMode::Compatible] {
        if let Ok(document) = PdfDocument::from_bytes_with_options(data.to_vec(), options(mode)) {
            if let Ok(count) = document.try_page_count() {
                for index in 0..count.min(8) {
                    if let Ok(mut page) = document.get_page(index as u32) {
                        let _ = page.display_list();
                    }
                }
            }
        }
    }
}

pub fn filters(data: &[u8]) {
    if data.len() > MAX_INPUT {
        return;
    }
    let Some((&selector, payload)) = data.split_first() else {
        return;
    };
    let names = [
        "FlateDecode",
        "LZWDecode",
        "ASCIIHexDecode",
        "ASCII85Decode",
        "RunLengthDecode",
    ];
    let limit = payload.first().copied().unwrap_or(0) as usize * 16;
    let context = FilterContext {
        max_decoded_bytes: limit,
        max_image_pixels: 4096,
        ..Default::default()
    };
    if let Ok(output) = apply_filter_with_context(
        names[selector as usize % names.len()],
        payload,
        None,
        context,
    ) {
        assert!(output.len() <= limit, "decoded output exceeded budget");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn font_seed_reaches_directory_subtable_and_glyph_lookup() {
        use pdf::font::truetype::true_type_program::TrueTypeProgram;
        let seed = font_seed();
        let mut font =
            TrueTypeProgram::try_new_with_limits(&seed, options(ParseMode::Strict).limits).unwrap();
        assert_eq!(font.num_cmap(), 1);
        font.selct_charmap(false);
        let cmap = font.current_cmap().unwrap();
        for (code, expected) in [
            (0, 0),
            (60, 61),
            (65, 66),
            (66, 0),
            (67, 65),
            (70, 68),
            (71, 0),
            (65535, 0),
        ] {
            assert_eq!(cmap.get_gid(code), expected);
        }
        fonts(&seed);
        fonts(&seed[40..]);
        for end in 0..seed.len() {
            fonts(&seed[..end]);
        }
        fonts(&vec![0; MAX_INPUT + 1]);
    }

    #[test]
    fn harnesses_replay_generated_seeds() {
        for seed in [
            b"".as_slice(),
            b"[[null]]",
            b"BT /F 12 Tf (A) Tj ET",
            b"q 0 0 m 1 1 l S Q",
        ] {
            syntax(seed);
            content(seed);
            document(seed);
            filters(seed);
        }
        let seed = document_seed().unwrap();
        assert!(seed.len() <= MAX_INPUT);
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let doc = PdfDocument::from_bytes_with_options(seed.clone(), options(mode)).unwrap();
            assert_eq!(doc.try_page_count().unwrap(), 2);
            for index in 0..2 {
                let list = doc.get_page(index).unwrap().display_list().unwrap();
                assert_eq!(list.objects().len(), 3);
            }
        }
        document(&seed);
        syntax(&vec![0; MAX_INPUT + 1]);
        content(&vec![0; MAX_INPUT + 1]);
        document(&vec![0; MAX_INPUT + 1]);
        filters(&vec![0; MAX_INPUT + 1]);
    }

    #[test]
    fn content_seed_reaches_text_path_and_image_interpretation() {
        let doc = PdfDocument::new_with_options(options(ParseMode::Strict)).unwrap();
        doc.add_page(0).unwrap();
        doc.resources()
            .register_standard_font(0, pdf::document::resource_manager::StandardFont::Helvetica)
            .unwrap();
        doc.resources()
            .embed_rgba_image(0, 2, 1, vec![200, 40, 20, 128, 20, 80, 200, 255])
            .unwrap();
        let seed = include_bytes!("../seeds/content/path");
        doc.replace_page_contents(0, seed.to_vec()).unwrap();
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(list.objects().len(), 3);
        assert!(list.diagnostics().is_empty());
        content(seed);
    }
}
