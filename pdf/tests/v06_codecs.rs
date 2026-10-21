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
            page.extract_image_source("Im").unwrap().stream.raw_data(),
            document
                .get_page(i)
                .unwrap()
                .extract_image_source("Im")
                .unwrap()
                .stream
                .raw_data()
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
