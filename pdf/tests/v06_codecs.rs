use pdf::{
    budget::OperationBudget,
    document::Document,
    error::PdfError,
    filter::{FilterContext, jbig2, jpx},
    objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName},
    parser::syntax::ParseMode,
};
const RGB: &[u8] = include_bytes!("fixtures/v06/rgb-lossless.jp2");
const GRAY: &[u8] = include_bytes!("fixtures/v06/gray-lossless.jp2");
const GENERIC: &[u8] = include_bytes!("fixtures/v06/generic-mmr.jb2");
const GLOBALS: &[u8] = include_bytes!("fixtures/v06/patterns-global.jb2");
const HALF: &[u8] = include_bytes!("fixtures/v06/halftone-0.jb2");
const EXTERNAL: &[u8] = include_bytes!("fixtures/v06/external-codecs.pdf");

#[test]
fn jpeg2000_lossless_container_raw_and_second_producer_match_samples() {
    for (encoded, expected, components) in [
        (
            RGB,
            include_bytes!("fixtures/v06/rgb.samples").as_slice(),
            3,
        ),
        (
            GRAY,
            include_bytes!("fixtures/v06/gray.samples").as_slice(),
            1,
        ),
        (
            include_bytes!("fixtures/v06/rgb-raw.j2k").as_slice(),
            include_bytes!("fixtures/v06/rgb.samples").as_slice(),
            3,
        ),
        (
            include_bytes!("fixtures/v06/openjpeg-gray.j2k").as_slice(),
            include_bytes!("fixtures/v06/gray.samples").as_slice(),
            1,
        ),
    ] {
        let pool = OperationBudget::new(8 * 1024 * 1024);
        let (info, decoded) =
            jpx::decode_with_budget(encoded, FilterContext::default(), &pool).unwrap();
        assert_eq!(
            (
                info.width,
                info.height,
                info.components,
                info.bits_per_component
            ),
            (16, 16, components, 8)
        );
        assert_eq!(decoded.as_slice(), expected);
        assert_eq!(pool.live_bytes(), decoded.capacity());
        drop(decoded);
        assert_eq!(pool.live_bytes(), 0);
    }
}

#[test]
fn jbig2_generic_mmr_and_independent_arithmetic_producer_have_pdf_polarity() {
    let expected = include_bytes!("fixtures/v06/mono.samples");
    for encoded in [
        GENERIC,
        include_bytes!("fixtures/v06/jbig2enc-generic.jb2").as_slice(),
    ] {
        let decoded = jbig2::jbig2_decode(encoded, None, FilterContext::default()).unwrap();
        assert_eq!(decoded.len(), 128);
        for y in 0..32 {
            for x in 0..32 {
                assert_eq!(
                    if decoded[y * 4 + x / 8] & (128 >> (x % 8)) != 0 {
                        255
                    } else {
                        0
                    },
                    expected[y * 32 + x]
                );
            }
        }
    }
}

#[test]
fn genuine_shared_globals_decode_and_missing_reference_fails() {
    assert!(jbig2::jbig2_decode(HALF, None, FilterContext::default()).is_err());
    let context = FilterContext {
        jbig2_globals: Some(GLOBALS),
        ..Default::default()
    };
    let first = jbig2::jbig2_decode(HALF, None, context).unwrap();
    let second =
        jbig2::jbig2_decode(include_bytes!("fixtures/v06/halftone-1.jb2"), None, context).unwrap();
    assert_eq!(first.len(), 128);
    assert_eq!(second.len(), 128);
    assert_ne!(first, second);
    for (a, b) in first.iter().zip(second) {
        assert_eq!(*a, !b);
    }
}

#[test]
fn external_pages_metadata_save_preserves_encoding_and_globals_import_outlives_source() {
    let document = Document::from_bytes_with_mode(EXTERNAL.to_vec(), ParseMode::Strict).unwrap();
    assert_eq!(document.try_page_count().unwrap(), 9);
    let mut expected = Vec::new();
    for i in 0..9 {
        let mut page = document.get_page(i).unwrap();
        let images = page.extract_images_paint_order_report().unwrap();
        assert!(images.diagnostics.is_empty());
        assert_eq!(images.images.len(), 1);
        expected.push(images.images[0].image.rgba_buffer().unwrap());
        let source = page.extract_image_source("Im").unwrap();
        let filter = source
            .stream
            .dict()
            .get("Filter")
            .unwrap()
            .as_name()
            .unwrap()
            .name();
        assert_eq!(filter, if i < 5 { "JPXDecode" } else { "JBIG2Decode" });
    }
    let saved =
        Document::from_bytes_with_mode(document.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    for i in 0..9 {
        let mut page = saved.get_page(i).unwrap();
        assert_eq!(
            page.extract_images_paint_order().unwrap()[0]
                .image
                .rgba_buffer()
                .unwrap(),
            expected[i as usize]
        );
        assert_eq!(
            page.extract_image_source("Im")
                .unwrap()
                .stream
                .raw_data()
                .unwrap(),
            document
                .get_page(i)
                .unwrap()
                .extract_image_source("Im")
                .unwrap()
                .stream
                .raw_data()
                .unwrap()
        );
    }
    let target = Document::new_with_version(pdf::document::save_options::PdfVersion::V1_5).unwrap();
    target.import_pages_from(&document, &[6, 7], 0).unwrap();
    drop(document);
    let imported = Document::from_bytes(target.save_to_bytes().unwrap()).unwrap();
    for i in 0..2 {
        assert_eq!(
            imported
                .get_page(i)
                .unwrap()
                .extract_images_paint_order()
                .unwrap()[0]
                .image
                .rgba_buffer()
                .unwrap(),
            expected[i as usize + 6]
        );
    }
    let a = imported
        .get_page(0)
        .unwrap()
        .extract_image_source("Im")
        .unwrap();
    let b = imported
        .get_page(1)
        .unwrap()
        .extract_image_source("Im")
        .unwrap();
    assert_eq!(
        a.stream
            .dict()
            .get("DecodeParms")
            .unwrap()
            .as_dict()
            .unwrap()
            .get("JBIG2Globals")
            .unwrap()
            .as_object_id(),
        b.stream
            .dict()
            .get("DecodeParms")
            .unwrap()
            .as_dict()
            .unwrap()
            .get("JBIG2Globals")
            .unwrap()
            .as_object_id()
    );
}

#[test]
fn budgets_fail_before_decode_release_neighbors_and_allow_retry() {
    for (encoded, globals, jpx_codec) in [
        (RGB, None, true),
        (GENERIC, None, false),
        (HALF, Some(GLOBALS), false),
    ] {
        let pool = OperationBudget::new(8 * 1024 * 1024);
        let neighbor = pool.reserve(8 * 1024 * 1024 - 1024).unwrap();
        let context = FilterContext {
            jbig2_globals: globals,
            ..Default::default()
        };
        let decode = || {
            if jpx_codec {
                jpx::decode_with_budget(encoded, context, &pool).map(|(_, b)| b)
            } else {
                jbig2::decode_with_budget(encoded, context, &pool)
            }
        };
        assert!(matches!(decode(), Err(PdfError::ResourceLimit(_))));
        assert_eq!(pool.live_bytes(), neighbor.bytes());
        drop(neighbor);
        let result = decode().unwrap();
        assert_eq!(pool.live_bytes(), result.capacity());
        drop(result);
        assert_eq!(pool.live_bytes(), 0);
    }
    let context = FilterContext {
        max_image_pixels: 1,
        ..Default::default()
    };
    assert!(matches!(
        jpx::jpx_decode(RGB, None, context),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(matches!(
        jbig2::jbig2_decode(GENERIC, None, context),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn truncated_or_unsupported_codec_envelopes_are_errors_not_blank_images() {
    for length in [0, 1, 20, RGB.len() - 1] {
        assert!(jpx::jpx_decode(&RGB[..length], None, Default::default()).is_err());
    }
    for length in [0, 1, 20, GENERIC.len() - 1] {
        assert!(jbig2::jbig2_decode(&GENERIC[..length], None, Default::default()).is_err());
    }
    let mut symbol = GENERIC.to_vec();
    symbol[4] = 0;
    // Changing a page-information segment into a symbol dictionary does not
    // produce a valid symbol stream, even though real dictionaries now decode.
    assert!(jbig2::jbig2_decode(&symbol, None, Default::default()).is_err());
    let mut image_dict = PdfDict::default();
    image_dict.insert(
        "Width".into(),
        PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(999.)),
    );
    image_dict.insert(
        "Filter".into(),
        PdfObject::PdfName(PdfName::new("JBIG2Decode".into())),
    );
    assert!(
        jbig2::jbig2_decode(
            GENERIC,
            None,
            FilterContext {
                stream_dict: Some(&image_dict),
                ..Default::default()
            }
        )
        .is_err()
    );
}

#[test]
fn jpx_ignores_dictionary_bit_depth_and_decode_and_resolves_indirect_filter() {
    use pdf::{
        document::{resource_manager::ResourceKind, save_options::PdfVersion},
        objects::{
            pdf_array::PdfArray, pdf_number::PdfNumber, pdf_reference::PdfReference,
            pdf_stream::PdfStream,
        },
    };
    let doc = Document::new_with_version(PdfVersion::V1_5).unwrap();
    doc.add_page(0).unwrap();
    let filter = doc
        .try_insert_object(PdfObject::PdfName(PdfName::new("JPXDecode".into())))
        .unwrap();
    let mut filters = PdfArray::default();
    filters.add_obj(PdfObject::PdfReference(PdfReference::from_id(filter)));
    let mut dict = PdfDict::default();
    dict.insert(
        "Subtype".into(),
        PdfObject::PdfName(PdfName::new("Image".into())),
    );
    for key in ["Width", "Height"] {
        let id = doc
            .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(16.)))
            .unwrap();
        dict.insert(
            key.into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        );
    }
    dict.insert("Filter".into(), PdfObject::PdfArray(filters));
    dict.insert(
        "BitsPerComponent".into(),
        PdfObject::PdfNumber(PdfNumber::new(1.)),
    );
    let mut decode = PdfArray::default();
    for value in [1., 0., 1., 0., 1., 0.] {
        decode.add_obj(PdfObject::PdfNumber(PdfNumber::new(value)));
    }
    dict.insert("Decode".into(), PdfObject::PdfArray(decode));
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, RGB.to_vec())))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Image, id)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
        .unwrap();
    let samples = doc
        .get_page(0)
        .unwrap()
        .extract_images_paint_order()
        .unwrap();
    assert_eq!(
        samples[0].image.data(),
        include_bytes!("fixtures/v06/rgb.samples")
    );
    let reopened = Document::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()[0]
            .image
            .data(),
        samples[0].image.data()
    );
}

#[test]
fn jbig2_indirect_metadata_is_checked_against_the_compressed_page() {
    use pdf::{
        document::{resource_manager::ResourceKind, save_options::PdfVersion},
        objects::{pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream},
    };
    for (width, bits, allowed) in [(32., 1., true), (64., 1., false), (32., 8., false)] {
        let doc = Document::new_with_version(PdfVersion::V1_5).unwrap();
        doc.add_page(0).unwrap();
        let mut dict = PdfDict::default();
        for (key, n) in [
            ("Width", width),
            ("Height", 32.),
            ("BitsPerComponent", bits),
        ] {
            let id = doc
                .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(n)))
                .unwrap();
            dict.insert(
                key.into(),
                PdfObject::PdfReference(PdfReference::from_id(id)),
            );
        }
        for (key, value) in [
            ("Subtype", "Image"),
            ("ColorSpace", "DeviceGray"),
            ("Filter", "JBIG2Decode"),
        ] {
            dict.insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())));
        }
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, GENERIC.to_vec())))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Image, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
            .unwrap();
        let result = doc.get_page(0).unwrap().extract_images_paint_order();
        assert_eq!(result.is_ok(), allowed);
        if allowed {
            let rgba = result.unwrap()[0].image.rgba_buffer().unwrap();
            let expected = include_bytes!("fixtures/v06/mono.samples");
            for (pixel, sample) in rgba.as_chunks::<4>().0.iter().zip(expected) {
                assert_eq!(pixel, &[*sample, *sample, *sample, 255]);
            }
        }
    }
}

#[test]
fn jpx_coding_envelope_rejects_precision_tiles_and_header_pixel_limits() {
    let raw = include_bytes!("fixtures/v06/rgb-raw.j2k");
    let mut high_depth = raw.to_vec();
    high_depth[42] = 15;
    assert!(matches!(
        jpx::jpx_decode(&high_depth, None, Default::default()),
        Err(PdfError::UnsupportedFeature(_))
    ));
    let mut tiles = raw.to_vec();
    tiles[24..28].copy_from_slice(&8u32.to_be_bytes());
    assert!(matches!(
        jpx::jpx_decode(&tiles, None, Default::default()),
        Err(PdfError::UnsupportedFeature(_))
    ));
    let mut huge = raw.to_vec();
    for p in [8, 12, 24, 28] {
        huge[p..p + 4].copy_from_slice(&u32::MAX.to_be_bytes());
    }
    assert!(matches!(
        jpx::jpx_decode(&huge, None, Default::default()),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn symbol_dictionary_text_and_globals_decode_with_exact_budget_release() {
    use sha2::{Digest, Sha256};
    let bytes = include_bytes!("fixtures/jbig2-annex-h-page1.bin");
    let expected = "d806f2e73907d03b1defbb7afadcee5962d80a009a844d3170d8b3f8f446a2fd";
    // Segment 0 (35 bytes) is the shared symbol dictionary. Same semantics
    // whether delivered inline or separately through JBIG2Globals.
    for (input, globals) in [(bytes.as_slice(), None), (&bytes[35..], Some(&bytes[..35]))] {
        let pool = OperationBudget::new(8 * 1024 * 1024);
        let context = FilterContext {
            jbig2_globals: globals,
            ..Default::default()
        };
        let decoded = jbig2::decode_with_budget(input, context, &pool).unwrap();
        assert_eq!(decoded.as_slice().len(), 448);
        assert_eq!(
            format!("{:x}", Sha256::digest(decoded.as_slice())),
            expected
        );
        assert_eq!(pool.live_bytes(), decoded.capacity());
        let peak = pool.peak_bytes();
        drop(decoded);
        assert_eq!(pool.live_bytes(), 0);
        let neighbor = pool.reserve(pool.maximum_bytes() - 32 * 1024).unwrap();
        assert!(matches!(
            jbig2::decode_with_budget(input, context, &pool),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), neighbor.bytes());
        drop(neighbor);
        assert!(peak > 448);
        assert!(jbig2::decode_with_budget(input, context, &pool).is_ok());
    }
}

#[test]
fn wrapped_jpx_image_headers_and_samples_use_the_decoded_codestream() {
    use pdf::{
        io::stream_reader::StreamReader, objects::pdf_stream::PdfStream, page::pdf_image::PdfImage,
        parser::syntax::SyntaxParser, pdf_context::PDFContext,
    };
    let ctx = PDFContext::try_new(
        pdf::parser::parser::PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap(),
        )),
        pdf::document::cross_ref_table::CrossRefTable::default(),
    )
    .unwrap();
    let parse = |text: &str| {
        SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(text.as_bytes().to_vec())).unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap()
    };
    let encoded: Vec<u8> = RGB
        .iter()
        .flat_map(|b| format!("{b:02X}").into_bytes())
        .chain(*b">")
        .collect();
    let dict = parse(
        "<< /Subtype /Image /Width 16 /Height 16 /Filter [/ASCIIHexDecode /JPXDecode] /DecodeParms [null null] >>",
    );
    let object = PdfObject::PdfStream(PdfStream::new(dict, encoded.clone()));
    let image = PdfImage::try_new("wrapped".into(), &object, &ctx).unwrap();
    assert_eq!(image.data(), include_bytes!("fixtures/v06/rgb.samples"));
    for fields in [
        "/Filter [/JPXDecode /ASCIIHexDecode]",
        "/Filter [/ASCIIHexDecode /JPXDecode] /DecodeParms [null]",
        "/Filter [/DCTDecode /JPXDecode]",
    ] {
        let dict = parse(&format!(
            "<< /Subtype /Image /Width 16 /Height 16 {fields} >>"
        ));
        assert!(
            PdfImage::try_new(
                "invalid".into(),
                &PdfObject::PdfStream(PdfStream::new(dict, encoded.clone())),
                &ctx
            )
            .is_err()
        );
    }
}

#[test]
fn jpx_palette_expands_bounded_eight_bit_indices_before_color_conversion() {
    fn boxed(tag: &[u8; 4], data: &[u8]) -> Vec<u8> {
        let mut result = ((data.len() + 8) as u32).to_be_bytes().to_vec();
        result.extend_from_slice(tag);
        result.extend_from_slice(data);
        result
    }
    let mut palette = vec![1, 0, 3, 7, 7, 7];
    for value in 0..=255u8 {
        palette.extend_from_slice(&[value, 255 - value, 0]);
    }
    let mapping = [0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 2];
    let make = |mapping: &[u8]| {
        let mut result = Vec::new();
        let mut offset = 0;
        while offset < GRAY.len() {
            let length = u32::from_be_bytes(GRAY[offset..offset + 4].try_into().unwrap()) as usize;
            let tag = &GRAY[offset + 4..offset + 8];
            if tag == b"jp2h" {
                let mut header = GRAY[offset + 8..offset + length].to_vec();
                let color = header.windows(4).position(|b| b == b"colr").unwrap();
                header[color + 7..color + 11].copy_from_slice(&16u32.to_be_bytes());
                header.extend_from_slice(&boxed(b"pclr", &palette));
                header.extend_from_slice(&boxed(b"cmap", mapping));
                result.extend_from_slice(&boxed(b"jp2h", &header));
            } else {
                result.extend_from_slice(&GRAY[offset..offset + length]);
            }
            offset += length;
        }
        result
    };
    let input = make(&mapping);
    let pool = OperationBudget::new(8 * 1024 * 1024);
    let (info, decoded) = jpx::decode_with_budget(&input, Default::default(), &pool).unwrap();
    assert_eq!(info.components, 3);
    let expected: Vec<u8> = include_bytes!("fixtures/v06/gray.samples")
        .iter()
        .flat_map(|value| [*value, 255 - *value, 0])
        .collect();
    assert_eq!(decoded.as_slice(), expected);
    drop(decoded);
    assert_eq!(pool.live_bytes(), 0);
    let mut invalid = mapping;
    invalid[7] = 0;
    assert!(jpx::decode_with_budget(&make(&invalid), Default::default(), &pool).is_err());
    assert_eq!(pool.live_bytes(), 0);
}

#[test]
fn jpx_declared_opacity_and_cmyk_channels_match_original_samples() {
    for (input, components, has_alpha, color) in [
        (
            include_bytes!("fixtures/v06/gray-alpha.jp2").as_slice(),
            2,
            true,
            jpx::JpxColorSpace::Gray,
        ),
        (
            include_bytes!("fixtures/v06/rgb-alpha.jp2").as_slice(),
            4,
            true,
            jpx::JpxColorSpace::Rgb,
        ),
        (
            include_bytes!("fixtures/v06/cmyk-lossless.jp2").as_slice(),
            4,
            false,
            jpx::JpxColorSpace::Cmyk,
        ),
    ] {
        let pool = OperationBudget::new(8 * 1024 * 1024);
        let (info, decoded) = jpx::decode_with_budget(input, Default::default(), &pool).unwrap();
        assert_eq!(
            (
                info.width,
                info.height,
                info.components,
                info.has_alpha,
                info.color_space
            ),
            (4, 4, components, has_alpha, color)
        );
        let mut expected = Vec::new();
        for y in 0..4u8 {
            for x in 0..4u8 {
                match color {
                    jpx::JpxColorSpace::Gray => expected.extend_from_slice(&[x * 85, y * 85]),
                    jpx::JpxColorSpace::Rgb => {
                        expected.extend_from_slice(&[x * 85, y * 85, 64, x * 85])
                    }
                    jpx::JpxColorSpace::Cmyk => expected.extend_from_slice(&[x * 85, y * 85, 0, 0]),
                }
            }
        }
        assert_eq!(decoded.as_slice(), expected);
        drop(decoded);
        assert_eq!(pool.live_bytes(), 0);
    }
}

#[test]
fn jpx_image_alpha_is_opt_in_and_invalid_or_conflicting_masks_fail() {
    use pdf::{
        io::stream_reader::StreamReader, objects::pdf_stream::PdfStream, page::pdf_image::PdfImage,
        parser::syntax::SyntaxParser, pdf_context::PDFContext,
    };
    let ctx = PDFContext::try_new(
        pdf::parser::parser::PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap(),
        )),
        pdf::document::cross_ref_table::CrossRefTable::default(),
    )
    .unwrap();
    let make = |fields: &str, encoded: &[u8]| {
        let dict = SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(
                format!("<< /Subtype /Image /Width 4 /Height 4 /Filter /JPXDecode {fields} >>")
                    .into_bytes(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        PdfImage::try_new(
            "rgba".into(),
            &PdfObject::PdfStream(PdfStream::new(dict, encoded.to_vec())),
            &ctx,
        )
    };
    for (fields, use_alpha) in [
        ("", false),
        ("/SMaskInData 0", false),
        ("/SMaskInData 1", true),
    ] {
        let image = make(fields, include_bytes!("fixtures/v06/rgb-alpha.jp2")).unwrap();
        assert_eq!(image.data().len(), 4 * 4 * 3);
        let pixels = image.rgba_buffer().unwrap();
        for y in 0..4usize {
            for x in 0..4usize {
                assert_eq!(
                    &pixels[(y * 4 + x) * 4..(y * 4 + x + 1) * 4],
                    &[
                        x as u8 * 85,
                        y as u8 * 85,
                        64,
                        if use_alpha { x as u8 * 85 } else { 255 }
                    ]
                );
            }
        }
    }
    for fields in [
        "/SMaskInData 2",
        "/SMaskInData -1",
        "/SMaskInData /Invalid",
        "/SMaskInData 1 /Mask [0 0 0 0 0 0]",
        "/SMaskInData 1 /SMask << >>",
    ] {
        assert!(
            make(fields, include_bytes!("fixtures/v06/rgb-alpha.jp2")).is_err(),
            "{fields}"
        );
    }
    assert!(
        make(
            "/SMaskInData 1 /SMask null /Mask null",
            include_bytes!("fixtures/v06/rgb-alpha.jp2")
        )
        .is_ok()
    );
    let gray = make(
        "/SMaskInData 1",
        include_bytes!("fixtures/v06/gray-alpha.jp2"),
    )
    .unwrap();
    let pixels = gray.rgba_buffer().unwrap();
    for y in 0..4usize {
        for x in 0..4usize {
            assert_eq!(
                &pixels[(y * 4 + x) * 4..(y * 4 + x + 1) * 4],
                &[x as u8 * 85, x as u8 * 85, x as u8 * 85, y as u8 * 85]
            );
        }
    }
    assert!(
        make(
            "/SMaskInData 1",
            include_bytes!("fixtures/v06/cmyk-lossless.jp2")
        )
        .is_err()
    );
}

#[test]
fn jpx_channel_definition_errors_and_small_budgets_reject_without_leaks() {
    let encoded = include_bytes!("fixtures/v06/rgb-alpha.jp2");
    let start = encoded.windows(4).position(|b| b == b"cdef").unwrap();
    let mut variants = Vec::new();
    let mut duplicate = encoded.to_vec();
    duplicate[start + 12..start + 14].copy_from_slice(&0u16.to_be_bytes());
    variants.push(duplicate);
    let mut premultiplied = encoded.to_vec();
    premultiplied[start + 26..start + 28].copy_from_slice(&2u16.to_be_bytes());
    variants.push(premultiplied);
    let mut huge_count = encoded.to_vec();
    huge_count[start + 4..start + 6].copy_from_slice(&u16::MAX.to_be_bytes());
    variants.push(huge_count);
    let mut wrong_color = encoded.to_vec();
    wrong_color[start + 26..start + 28].copy_from_slice(&0u16.to_be_bytes());
    wrong_color[start + 28..start + 30].copy_from_slice(&4u16.to_be_bytes());
    variants.push(wrong_color);
    for encoded in variants {
        let pool = OperationBudget::new(8 * 1024 * 1024);
        let neighbor = pool.reserve(7).unwrap();
        assert!(jpx::decode_with_budget(&encoded, Default::default(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
    }
    let pool = OperationBudget::new(32);
    assert!(matches!(
        jpx::decode_with_budget(encoded, Default::default(), &pool),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(pool.live_bytes(), 0);
    let large = OperationBudget::new(8 * 1024 * 1024);
    drop(jpx::decode_with_budget(encoded, Default::default(), &large).unwrap());
    assert_eq!(large.live_bytes(), 0);
}

#[test]
fn jpx_single_tile_can_extend_beyond_image_without_expanding_budget() {
    let input = include_bytes!("fixtures/v06/rgb-raw.j2k");
    let expected = include_bytes!("fixtures/v06/rgb.samples");
    // SIZ tile extent may exceed the image. The image area and coded pixels
    // are unchanged; include asymmetric and maximal extents as overflow guards.
    for (tw, th) in [(128u32, 128u32), (16, 128), (u32::MAX, u32::MAX)] {
        let mut larger = input.to_vec();
        larger[24..28].copy_from_slice(&tw.to_be_bytes());
        larger[28..32].copy_from_slice(&th.to_be_bytes());
        let pool = OperationBudget::new(8 * 1024 * 1024);
        let (info, pixels) = jpx::decode_with_budget(&larger, Default::default(), &pool).unwrap();
        assert_eq!((info.width, info.height), (16, 16));
        assert_eq!(pixels.as_slice(), expected);
        drop(pixels);
        assert_eq!(pool.live_bytes(), 0);
    }
    for (tw, th) in [(0u32, 16u32), (16, 0), (8, 16), (16, 8)] {
        let mut invalid = input.to_vec();
        invalid[24..28].copy_from_slice(&tw.to_be_bytes());
        invalid[28..32].copy_from_slice(&th.to_be_bytes());
        assert!(jpx::inspect(&invalid, Default::default()).is_err());
    }
}

#[test]
fn jpx_equivalent_component_coding_preserves_samples_and_limits() {
    let source = include_bytes!("fixtures/v06/rgb-raw.j2k");
    let cod = source.windows(2).position(|b| b == [0xff, 0x52]).unwrap();
    let length = u16::from_be_bytes([source[cod + 2], source[cod + 3]]) as usize;
    let payload = &source[cod + 4..cod + 2 + length];
    let mut coc = vec![2, payload[0]];
    coc.extend_from_slice(&payload[5..]);
    let make = |body: &[u8]| {
        let mut result = source[..cod + 2 + length].to_vec();
        result.extend_from_slice(&[0xff, 0x53]);
        result.extend_from_slice(&((body.len() + 2) as u16).to_be_bytes());
        result.extend_from_slice(body);
        result.extend_from_slice(&source[cod + 2 + length..]);
        result
    };
    let pool = OperationBudget::new(8 * 1024 * 1024);
    let (_, decoded) = jpx::decode_with_budget(&make(&coc), Default::default(), &pool).unwrap();
    assert_eq!(
        decoded.as_slice(),
        include_bytes!("fixtures/v06/rgb.samples")
    );
    drop(decoded);
    assert_eq!(pool.live_bytes(), 0);
    let mut changed = coc.clone();
    changed[2] += 1;
    let mut invalid_index = coc.clone();
    invalid_index[0] = 3;
    for body in [&changed[..], &invalid_index[..], &coc[..1]] {
        assert!(jpx::decode_with_budget(&make(body), Default::default(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }
}

#[test]
fn jpx_pdf_indexed_samples_bypass_embedded_palette_and_keep_low_bit_indices() {
    use pdf::{
        io::stream_reader::StreamReader, objects::pdf_stream::PdfStream, page::pdf_image::PdfImage,
        parser::syntax::SyntaxParser, pdf_context::PDFContext,
    };
    let ctx = PDFContext::try_new(
        pdf::parser::parser::PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap(),
        )),
        pdf::document::cross_ref_table::CrossRefTable::default(),
    )
    .unwrap();
    let input = include_bytes!("fixtures/v06/gray-indices-2bit.jp2");
    let context = FilterContext {
        jpx_preserve_indices: true,
        ..Default::default()
    };
    let pool = OperationBudget::new(8 * 1024 * 1024);
    let (info, indices) = jpx::decode_with_budget(input, context, &pool).unwrap();
    assert_eq!(info.bits_per_component, 2);
    assert_eq!(
        indices.as_slice(),
        &[0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3, 0, 1, 2, 3]
    );
    drop(indices);
    assert_eq!(pool.live_bytes(), 0);
    assert!(jpx::decode_with_budget(input, Default::default(), &pool).is_err());
    let make = |input: &[u8], size: u32, palette: &str| {
        let dict=SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(format!("<< /Subtype /Image /Width {size} /Height {size} /Filter /JPXDecode /ColorSpace [/Indexed /DeviceRGB 255 <{palette}>] /Decode [255 0] >>").into_bytes())).unwrap()).get_object().unwrap().into_dict().unwrap();
        PdfImage::try_new(
            "indices".into(),
            &PdfObject::PdfStream(PdfStream::new(dict, input.to_vec())),
            &ctx,
        )
        .unwrap()
    };
    let palette = (0..256)
        .map(|i| format!("{i:02X}00{:02X}", 255 - i))
        .collect::<String>();
    let image = make(input, 4, &palette);
    let rgba = image.rgba_buffer().unwrap();
    for (i, pixel) in rgba.as_chunks::<4>().0.iter().enumerate() {
        let index = (i % 4) as u8;
        assert_eq!(pixel, &[index, 0, 255 - index, 255]);
    }
    let boxed = |tag: &[u8], body: &[u8]| {
        let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        b.extend_from_slice(tag);
        b.extend_from_slice(body);
        b
    };
    let mut embedded = Vec::new();
    let mut offset = 0;
    while offset < GRAY.len() {
        let length = u32::from_be_bytes(GRAY[offset..offset + 4].try_into().unwrap()) as usize;
        let tag = &GRAY[offset + 4..offset + 8];
        let mut body = GRAY[offset + 8..offset + length].to_vec();
        if tag == b"jp2h" {
            let c = body.windows(4).position(|b| b == b"colr").unwrap();
            body[c + 7..c + 11].copy_from_slice(&16u32.to_be_bytes());
            let mut values = vec![1, 0, 3, 7, 7, 7];
            for _ in 0..256 {
                values.extend_from_slice(&[0, 255, 0]);
            }
            body.extend_from_slice(&boxed(b"pclr", &values));
            body.extend_from_slice(&boxed(b"cmap", &[0, 0, 1, 0, 0, 0, 1, 1, 0, 0, 1, 2]));
        }
        embedded.extend_from_slice(&boxed(tag, &body));
        offset += length;
    }
    let image = make(&embedded, 16, &palette);
    for (pixel, &index) in image
        .rgba_buffer()
        .unwrap()
        .as_chunks::<4>()
        .0
        .iter()
        .zip(include_bytes!("fixtures/v06/gray.samples"))
    {
        assert_eq!(pixel, &[index, 0, 255 - index, 255]);
    }
}

#[test]
fn direct_inline_group4_uses_coded_boundary_and_preserves_following_operators() {
    use pdf::page::content_program::ContentProgram;
    use pdf::parser::options::ParseOptions;
    let mut offset = 0;
    let mut mmr = &[][..];
    while offset < GENERIC.len() {
        let length =
            u32::from_be_bytes(GENERIC[offset + 7..offset + 11].try_into().unwrap()) as usize;
        if GENERIC[offset + 4] & 63 == 38 {
            mmr = &GENERIC[offset + 11 + 18..offset + 11 + length];
        }
        offset += 11 + length;
    }
    assert!(!mmr.is_empty());
    let make = |data: &[u8], extra: &str, suffix: &[u8]| {
        let mut source =
            format!("BI /W 32 /H 32 /IM true /F /CCF /DP << /K -1 /Columns 32 {extra} >> ID ")
                .into_bytes();
        source.extend_from_slice(data);
        source.extend_from_slice(suffix);
        source
    };
    let source = make(mmr, "", b"\nEI q Q");
    let program =
        ContentProgram::from_decoded_bytes(source.clone(), ParseOptions::default()).unwrap();
    assert_eq!(program.operations().len(), 3);
    assert_eq!(
        program.operations()[0]
            .operator
            .inline_image()
            .unwrap()
            .encoded_data(),
        mmr
    );
    assert_eq!(program.operations()[1].operator.name(), "q");
    assert_eq!(program.operations()[2].operator.name(), "Q");
    for broken in [
        make(&mmr[..mmr.len() / 2], "", b"\nEI q Q"),
        make(mmr, "/Rows 31", b"\nEI"),
        make(mmr, "", b"\nNotEI"),
        make(mmr, "/K 0", b"\nEI"),
    ] {
        assert!(ContentProgram::from_decoded_bytes(broken, ParseOptions::default()).is_err());
    }
    let pool = OperationBudget::new(source.len() + 64);
    let source = pdf::budget::BudgetedBytes::copy_from_slice(&pool, &source).unwrap();
    assert!(
        ContentProgram::from_decoded_bytes_with_budget(source, ParseOptions::default(), &pool)
            .is_err()
    );
    assert_eq!(pool.live_bytes(), 0);
}
