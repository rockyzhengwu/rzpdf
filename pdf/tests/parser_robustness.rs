//! Deterministic malformed-input smoke tests; not a replacement for coverage-guided fuzzing.
use pdf::{
    io::stream_reader::StreamReader,
    page::content_parser::ContentParser,
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;

fn syntax(bytes: Vec<u8>, mode: ParseMode) -> SyntaxParser<Cursor<Vec<u8>>> {
    SyntaxParser::with_options(
        StreamReader::try_new(Cursor::new(bytes)).unwrap(),
        ParseOptions {
            mode,
            limits: Limits {
                max_depth: 16,
                max_objects: 128,
                max_token_bytes: 1024,
                max_raw_stream_bytes: 1024,
                ..Default::default()
            },
        },
    )
}

#[test]
fn deterministic_mutations_do_not_panic_and_successful_reads_advance() {
    let seeds: &[&[u8]] = &[
        b"<< /Length 3 /Name /A#20B /Nums [1 0 R -2 .5] >>",
        b"(escaped \\( nested (text) \\123) <00FF1> true null",
        b"0 0 m 10 10 l S BT /F 12 Tf [(A) 100 (B)] TJ ET",
        b"<< /Length 4 >> stream\nABCD\nendstream",
        b"[[[[1]]]] % comment\r\n/Name#FE",
    ];
    let mut random = 0x726a7064u32;
    for case in 0..1024 {
        let mut bytes = seeds[case % seeds.len()].to_vec();
        for _ in 0..1 + case % 4 {
            random = random.wrapping_mul(1664525).wrapping_add(1013904223);
            let index = random as usize % (bytes.len() + 1);
            match (random >> 16) % 3 {
                0 if index < bytes.len() => {
                    bytes.remove(index);
                }
                1 if index < bytes.len() => {
                    bytes[index] = (random >> 24) as u8;
                }
                _ => bytes.insert(index, (random >> 24) as u8),
            }
        }
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let mut parser = syntax(bytes.clone(), mode);
            for _ in 0..128 {
                if parser.is_eof() {
                    break;
                }
                let before = parser.position();
                match parser.get_object() {
                    Ok(_) => assert!(
                        parser.position() > before,
                        "non-advancing syntax case {case}: {bytes:?}"
                    ),
                    Err(_) => break,
                }
            }
            let mut parser = ContentParser::new(syntax(bytes.clone(), mode));
            for _ in 0..128 {
                match parser.read_operator() {
                    Ok(Some(_)) => {}
                    Ok(None) | Err(_) => break,
                }
            }
        }
    }
}

#[test]
fn document_entry_handles_truncations_and_byte_mutations_without_panics() {
    use pdf::document::PdfDocument;
    let source = include_bytes!("../../tests/path-test.pdf");
    let limits = Limits {
        max_depth: 16,
        max_objects: 256,
        max_token_bytes: 4096,
        max_raw_stream_bytes: 4096,
        max_decoded_bytes: 4096,
        max_cached_stream_bytes: 8192,
        ..Default::default()
    };
    let mut random = 0x70646631u32;
    for case in 0..256 {
        let mut bytes = source.to_vec();
        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
        if case % 2 == 0 {
            bytes.truncate(random as usize % bytes.len());
        } else {
            let index = random as usize % bytes.len();
            bytes[index] = (random >> 24) as u8;
        }
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            if let Ok(document) =
                PdfDocument::from_bytes_with_options(bytes.clone(), ParseOptions { mode, limits })
                && let Ok(count) = document.try_page_count()
            {
                for index in 0..count.min(8) {
                    if let Ok(mut page) = document.get_page(index as u32) {
                        let _ = page.display_list();
                    }
                }
            }
        }
    }
}

#[test]
fn stream_decoder_mutations_stay_within_output_budget() {
    use pdf::filter::{FilterContext, apply_filter_with_context};
    let seeds: &[(&str, &[u8])] = &[
        (
            "FlateDecode",
            &[0x78, 0x9c, 0x4b, 0xcb, 0xcf, 0x07, 0, 2, 0x82, 1, 0x45],
        ),
        (
            "LZWDecode",
            &[0x80, 0x0b, 0x60, 0x50, 0x22, 0x0c, 0x0c, 0x85, 1],
        ),
        ("ASCIIHexDecode", b"0001FF80>"),
        ("ASCII85Decode", b"zz!!!!!~>"),
        ("RunLengthDecode", &[129, b'A', 128]),
    ];
    let mut random = 0x66696c74u32;
    for case in 0..2048 {
        let (name, seed) = seeds[case % seeds.len()];
        let mut bytes = seed.to_vec();
        random = random.wrapping_mul(1664525).wrapping_add(1013904223);
        let index = random as usize % bytes.len();
        match case % 3 {
            0 => bytes.truncate(index),
            1 => bytes[index] = (random >> 24) as u8,
            _ => bytes.insert(index, (random >> 24) as u8),
        }
        let limit = case % 65;
        let context = FilterContext {
            max_decoded_bytes: limit,
            max_image_pixels: 64,
            ..Default::default()
        };
        if let Ok(output) = apply_filter_with_context(name, &bytes, None, context) {
            assert!(
                output.len() <= limit,
                "{name} exceeded budget in case {case}"
            );
        }
    }
}

#[test]
fn malformed_predictor_parameters_are_not_silently_defaulted() {
    use pdf::filter::predictor::apply_predictor;
    for source in [
        "<< /Predictor /bad >>",
        "<< /Predictor 12 /Columns /bad >>",
        "<< /Predictor 12 /Columns 0 >>",
        "<< /Predictor 2 /Colors 0 >>",
        "<< /Predictor 12 /BitsPerComponent 3 >>",
        "<< /Predictor 12 /Columns 1.5 >>",
    ] {
        let obj = syntax(source.as_bytes().to_vec(), ParseMode::Strict)
            .get_object()
            .unwrap();
        assert!(
            apply_predictor(vec![0, 1, 2], obj.as_dict(), "test").is_err(),
            "{source}"
        );
    }
}
