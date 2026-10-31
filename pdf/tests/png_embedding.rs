use pdf::{document::PdfDocument, objects::PdfObject, parser::syntax::ParseMode};

fn encode(color: png::ColorType, pixels: &[u8]) -> Vec<u8> {
    let mut bytes = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut bytes, 2, 1);
        encoder.set_color(color);
        encoder.set_depth(png::BitDepth::Eight);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(pixels).unwrap();
        writer.finish().unwrap();
    }
    bytes
}

#[test]
fn png_rgb_rgba_and_gray_variants_embed_and_interpret_after_save() {
    for (color, pixels) in [
        (png::ColorType::Rgb, vec![10, 20, 30, 40, 50, 60]),
        (png::ColorType::Rgba, vec![10, 20, 30, 0, 40, 50, 60, 128]),
        (png::ColorType::Grayscale, vec![10, 40]),
        (png::ColorType::GrayscaleAlpha, vec![10, 0, 40, 128]),
    ] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .embed_png_image(0, encode(color, &pixels))
            .unwrap();
        let mut content = pdf::page::content_builder::ContentBuilder::new();
        content
            .draw_image(&name, [100.0, 0.0, 0.0, 50.0, 0.0, 0.0])
            .unwrap();
        doc.replace_page_content_builder(0, &content).unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        assert!(reopened.get_page(0).unwrap().display_list().is_ok());
        let resources = reopened.get_page(0).unwrap().resource().unwrap();
        let id = resources
            .get("XObject")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(&name)
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let image = reopened.resolve_object(id).unwrap();
        let image = image.as_stream().unwrap();
        assert_eq!(
            reopened.read_stream_data(image).unwrap().as_ref(),
            match color {
                png::ColorType::Rgb => vec![10, 20, 30, 40, 50, 60],
                png::ColorType::Rgba => vec![10, 20, 30, 40, 50, 60],
                _ => vec![10, 10, 10, 40, 40, 40],
            }
        );
        if matches!(color, png::ColorType::Rgba | png::ColorType::GrayscaleAlpha) {
            let id = image
                .dict()
                .get("SMask")
                .unwrap()
                .as_reference()
                .unwrap()
                .id();
            assert_eq!(
                reopened
                    .read_stream_data(reopened.resolve_object(id).unwrap().as_stream().unwrap())
                    .unwrap()
                    .as_ref(),
                &[0, 128]
            );
        }
    }
}

#[test]
fn malformed_png_does_not_change_document_objects() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let valid = encode(png::ColorType::Rgb, &[10, 20, 30, 40, 50, 60]);
    for input in [
        vec![],
        valid[..20].to_vec(),
        valid[..valid.len() - 6].to_vec(),
    ] {
        assert!(doc.resources().embed_png_image(0, input).is_err());
    }
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        5
    );
}

#[test]
fn indexed_png_palette_transparency_expands_to_rgb_and_soft_mask() {
    let mut encoded = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut encoded, 2, 1);
        encoder.set_color(png::ColorType::Indexed);
        encoder.set_depth(png::BitDepth::Eight);
        encoder.set_palette(vec![10, 20, 30, 40, 50, 60]);
        encoder.set_trns(vec![0, 128]);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0, 1]).unwrap();
        writer.finish().unwrap();
    }
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc.resources().embed_png_image(0, encoded).unwrap();
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let id = resources
        .get("XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(&name)
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let image = doc.resolve_object(id).unwrap();
    let image = image.as_stream().unwrap();
    assert_eq!(image.raw_data().unwrap(), &[10, 20, 30, 40, 50, 60]);
    let id = image
        .dict()
        .get("SMask")
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    assert_eq!(
        doc.read_stream_data(doc.resolve_object(id).unwrap().as_stream().unwrap())
            .unwrap()
            .as_ref(),
        &[0, 128]
    );
}

#[test]
fn png_bad_crc_and_invalid_page_do_not_mutate_document() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let mut encoded = encode(png::ColorType::Rgb, &[10, 20, 30, 40, 50, 60]);
    encoded[29] ^= 1; // Corrupt IHDR CRC, retaining a readable signature and dimensions.
    assert!(doc.resources().embed_png_image(0, encoded).is_err());
    assert!(doc.resources().embed_png_image(1, vec![]).is_err());
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        5
    );
}

#[test]
fn png_input_pixels_output_and_object_budgets_reject_without_edits() {
    use pdf::{
        error::PdfError,
        parser::options::{Limits, ParseOptions},
    };
    let input = encode(png::ColorType::Rgba, &[10, 20, 30, 0, 40, 50, 60, 128]);
    for limits in [
        Limits {
            max_raw_stream_bytes: input.len() - 1,
            ..Default::default()
        },
        Limits {
            max_image_pixels: 1,
            ..Default::default()
        },
        Limits {
            max_decoded_bytes: 7,
            ..Default::default()
        },
        Limits {
            max_objects: 5,
            ..Default::default()
        },
    ] {
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits,
            ..Default::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        let before = doc.get_page(0).unwrap().resource().unwrap();
        assert!(matches!(
            doc.resources().embed_png_image(0, input.clone()),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), before);
        assert_eq!(
            doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
            5
        );
    }
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_raw_stream_bytes: input.len(),
            max_image_pixels: 2,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    assert_eq!(doc.resources().embed_png_image(0, input).unwrap(), "Im1");
}

#[test]
fn sixteen_bit_png_is_rejected_without_silent_precision_loss() {
    use pdf::error::PdfError;
    let mut input = Vec::new();
    {
        let mut encoder = png::Encoder::new(&mut input, 1, 1);
        encoder.set_color(png::ColorType::Grayscale);
        encoder.set_depth(png::BitDepth::Sixteen);
        let mut writer = encoder.write_header().unwrap();
        writer.write_image_data(&[0x12, 0x34]).unwrap();
        writer.finish().unwrap();
    }
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    assert!(matches!(
        doc.resources().embed_png_image(0, input),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        5
    );
}
