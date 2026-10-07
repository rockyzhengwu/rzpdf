use pdf::{document::PdfDocument, objects::PdfObject, parser::syntax::ParseMode};

fn jpeg(gray: bool) -> Vec<u8> {
    let mut compressor = mozjpeg::Compress::new(if gray {
        mozjpeg::ColorSpace::JCS_GRAYSCALE
    } else {
        mozjpeg::ColorSpace::JCS_RGB
    });
    compressor.set_size(2, 1);
    let mut compressor = compressor.start_compress(Vec::new()).unwrap();
    compressor
        .write_scanlines(if gray {
            &[40, 200]
        } else {
            &[200, 40, 20, 200, 40, 20]
        })
        .unwrap();
    compressor.finish().unwrap()
}

#[test]
fn jpeg_encoded_bytes_and_colorspace_survive_save_without_recompression() {
    for gray in [false, true] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let encoded = jpeg(gray);
        let name = doc
            .resources()
            .embed_jpeg_image(0, encoded.clone())
            .unwrap();
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
        let mut content = pdf::page::content_builder::ContentBuilder::new();
        content
            .draw_image(&name, [100.0, 0.0, 0.0, 50.0, 0.0, 0.0])
            .unwrap();
        doc.replace_page_content_builder(0, &content).unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes_with_options(pdf::document::save_options::SaveOptions {
                compression: pdf::document::save_options::StreamCompression::FlateUnencoded,
                ..Default::default()
            })
            .unwrap(),
            ParseMode::Strict,
        )
        .unwrap();
        for document in [doc, reopened] {
            assert!(document.get_page(0).unwrap().display_list().is_ok());
            let image = document.resolve_object(id).unwrap();
            let image = image.as_stream().unwrap();
            assert_eq!(image.raw_data(), encoded);
            assert_eq!(
                image
                    .dict()
                    .get("Filter")
                    .unwrap()
                    .as_name()
                    .unwrap()
                    .name(),
                "DCTDecode"
            );
            assert_eq!(
                image
                    .dict()
                    .get("ColorSpace")
                    .unwrap()
                    .as_name()
                    .unwrap()
                    .name(),
                if gray { "DeviceGray" } else { "DeviceRGB" }
            );
        }
    }
}

#[test]
fn jpeg_budget_boundaries_and_invalid_page_preserve_allocation_state() {
    use pdf::{
        error::PdfError,
        parser::options::{Limits, ParseOptions},
    };
    let encoded = jpeg(false);
    for (raw, decoded, pixels, accepted) in [
        (encoded.len(), 8, 2, true),
        (encoded.len() - 1, 8, 2, false),
        (encoded.len(), 7, 2, false),
        (encoded.len(), 8, 1, false),
    ] {
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_raw_stream_bytes: raw,
                max_decoded_bytes: decoded,
                max_image_pixels: pixels,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        let result = doc.resources().embed_jpeg_image(0, encoded.clone());
        if accepted {
            assert_eq!(result.unwrap(), "Im1");
        } else {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
            assert_eq!(
                doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
                5
            );
        }
    }
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    assert!(doc.resources().embed_jpeg_image(1, encoded).is_err());
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        5
    );
}

#[test]
fn malformed_jpeg_does_not_allocate_objects() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    for bytes in [
        vec![],
        vec![0xff, 0xd8],
        vec![0xff, 0xd8, 0xff, 0xe0, 0, 1],
        vec![0xff, 0xd8, 0xff, 0xc0, 0, 8, 12, 0, 1, 0, 1, 1],
    ] {
        assert!(doc.resources().embed_jpeg_image(0, bytes).is_err());
    }
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        5
    );
}

#[test]
fn jpeg_resource_registration_failure_restores_image_allocation() {
    use pdf::objects::{object_id::ObjectId, pdf_dict::PdfDict};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let page_id = ObjectId::new(4, 0);
    let mut page = doc.resolve_object(page_id).unwrap().into_dict().unwrap();
    let mut resources = PdfDict::default();
    resources
        .try_insert("XObject".into(), PdfObject::PdfNull)
        .unwrap();
    page.try_insert("Resources".into(), PdfObject::PdfDict(resources))
        .unwrap();
    doc.update_object(page_id, PdfObject::PdfDict(page.clone()))
        .unwrap();
    assert!(doc.resources().embed_jpeg_image(0, jpeg(false)).is_err());
    assert_eq!(
        doc.resolve_object(page_id).unwrap(),
        PdfObject::PdfDict(page)
    );
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        5
    );
}
