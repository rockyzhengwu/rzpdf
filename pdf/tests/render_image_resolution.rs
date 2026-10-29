use pdf::{
    device::Device,
    document::{PdfDocument, prepared_page::PreparedPage},
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
    page::{image_object::ImageObject, page_object::PageObject},
    parser::options::{Limits, ParseOptions},
};
use std::{io::Cursor, rc::Rc};
type Doc = PdfDocument<Cursor<Vec<u8>>>;

fn scan(width: usize, height: usize, content: &str) -> Doc {
    scan_with_process(width, height, content, false)
}

fn scan_with_process(width: usize, height: usize, content: &str, progressive: bool) -> Doc {
    let mut encoder = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
    encoder.set_fastest_defaults();
    encoder.set_size(width, height);
    if progressive {
        encoder.set_progressive_mode();
    }
    let mut encoder = encoder.start_compress(Vec::new()).unwrap();
    let row = vec![120; width * 3];
    for _ in 0..height {
        encoder.write_scanlines(&row).unwrap();
    }
    let jpeg = encoder.finish().unwrap();
    let doc = Doc::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc.resources().embed_jpeg_image(0, jpeg).unwrap();
    doc.replace_page_contents(0, content.replace("IMAGE", &name).into_bytes())
        .unwrap();
    Doc::from_bytes_with_options(
        doc.save_to_bytes().unwrap(),
        ParseOptions {
            limits: Limits {
                max_image_pixels: 4_000_000,
                max_decoded_bytes: 16 * 1024 * 1024,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap()
}

#[derive(Default)]
struct Images(Vec<(u32, u32)>, Vec<usize>);
impl Device for Images {
    fn start_page(&mut self, _: f32, _: f32) -> PdfResult<()> {
        Ok(())
    }
    fn end_page(&mut self) -> PdfResult<()> {
        Ok(())
    }
    fn do_image(&mut self, image: ImageObject) -> PdfResult<()> {
        self.1.push(Rc::as_ptr(&image.image) as usize);
        self.0.push(image.image.pixel_dimensions());
        Ok(())
    }
    fn stroke_path(&mut self, _: pdf::page::path_object::PathObject) -> PdfResult<()> {
        Ok(())
    }
    fn fill_path(&mut self, _: pdf::page::path_object::PathObject) -> PdfResult<()> {
        Ok(())
    }
    fn fill_and_stroke_path(&mut self, _: pdf::page::path_object::PathObject) -> PdfResult<()> {
        Ok(())
    }
    fn clip_path(&mut self, _: pdf::page::path_object::PathObject) -> PdfResult<()> {
        Ok(())
    }
    fn show_text(&mut self, _: pdf::page::text_object::TextObject) -> PdfResult<()> {
        Ok(())
    }
}
fn dimensions(doc: &Doc, page: &PreparedPage) -> Vec<(u32, u32)> {
    let mut sink = Images::default();
    page.replay(doc, &mut sink).unwrap();
    sink.0
}

#[test]
fn scan_above_source_pixel_limit_decodes_for_render_without_changing_extraction_or_save() {
    let doc = scan(2500, 3500, "q 350 0 0 490 0 0 cm /IMAGE Do Q");
    let saved = doc.save_to_bytes().unwrap();
    let low_bytes = Doc::from_bytes_with_options(
        saved.clone(),
        ParseOptions {
            limits: Limits {
                max_image_pixels: 4_000_000,
                max_decoded_bytes: 4_000_000,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        low_bytes
            .prepare_page_for_render(0, None, 144.)
            .as_ref()
            .map_err(PdfError::root_cause),
        Err(PdfError::ResourceLimitExceeded {
            reason: "decoded_bytes",
            limit: 4_000_000,
            actual: 8_750_000,
            ..
        })
    ));
    let reduced = doc.prepare_page_for_render(0, None, 144.).unwrap();
    assert_eq!(dimensions(&doc, &reduced), [(1250, 1750)]);
    assert!(Rc::ptr_eq(
        &reduced,
        &doc.prepare_page_for_render(0, None, 144.).unwrap()
    ));
    assert!(matches!(
        doc.prepare_page_for_render(0, None, 576.)
            .as_ref()
            .map_err(PdfError::root_cause),
        Err(PdfError::ResourceLimitExceeded {
            reason: "image_pixels",
            ..
        })
    ));
    // Failed render scope and its cache cannot leak into full-fidelity APIs.
    assert!(
        doc.get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), saved);
    assert!(
        doc.extract_characters_for_display(0)
            .unwrap()
            .characters
            .is_empty()
    );
    doc.clear_render_caches();
    assert_eq!(doc.render_cache_statistics().page_entries, 0);
    assert_eq!(
        dimensions(&doc, &doc.prepare_page_for_render(0, None, 72.).unwrap()),
        [(625, 875)]
    );
}

#[test]
fn repeated_image_draws_with_different_rotated_or_sheared_scales_do_not_alias() {
    let doc = scan(
        2500,
        3500,
        "q 350 0 0 490 0 0 cm /IMAGE Do Q q 0 700 -980 0 200 0 cm /IMAGE Do Q q 700 400 0 980 0 0 cm /IMAGE Do Q",
    );
    let page = doc.prepare_page_for_render(0, None, 72.).unwrap();
    assert_eq!(
        dimensions(&doc, &page),
        [(625, 875), (1250, 1750), (1250, 1750)]
    );
    assert!(!Rc::ptr_eq(
        &page,
        &doc.prepare_page_for_render(0, None, 36.).unwrap()
    ));
}

#[test]
fn fitting_jpegs_keep_original_pixels_and_progressive_oversize_is_not_relaxed() {
    let small = scan(
        256,
        256,
        "q 16 0 0 16 0 0 cm /IMAGE Do Q q 128 0 0 128 0 0 cm /IMAGE Do Q",
    );
    assert_eq!(
        dimensions(
            &small,
            &small.prepare_page_for_render(0, None, 72.).unwrap()
        ),
        [(256, 256), (256, 256)]
    );
    let mut collected = Images::default();
    small
        .prepare_page_for_render(0, None, 72.)
        .unwrap()
        .replay(&small, &mut collected)
        .unwrap();
    assert_eq!(
        collected.1[0], collected.1[1],
        "full-size image cache should remain shared"
    );
    let progressive = scan_with_process(4001, 1000, "q 16 0 0 16 0 0 cm /IMAGE Do Q", true);
    assert!(matches!(
        progressive
            .prepare_page_for_render(0, None, 72.)
            .as_ref()
            .map_err(PdfError::root_cause),
        Err(PdfError::ResourceLimitExceeded {
            reason: "image_pixels",
            ..
        })
    ));
    let oversized_source = scan(4001, 4000, "q 16 0 0 16 0 0 cm /IMAGE Do Q");
    assert!(matches!(
        oversized_source
            .prepare_page_for_render(0, None, 72.)
            .as_ref()
            .map_err(PdfError::root_cause),
        Err(PdfError::ResourceLimitExceeded {
            reason: "image_source_pixels",
            limit: 16_000_000,
            actual: 16_004_000,
            ..
        })
    ));
}

#[test]
fn masked_jpeg_retains_full_precision_and_matte_is_not_dropped() {
    let doc = scan(256, 256, "q 16 0 0 16 0 0 cm /IMAGE Do Q");
    let images = doc
        .get_page(0)
        .unwrap()
        .extract_images_paint_order()
        .unwrap();
    assert_eq!(images.len(), 1);
    let resource = doc.get_page(0).unwrap().resource().unwrap();
    let objects = resource.get("XObject").unwrap().as_dict().unwrap();
    let id = objects
        .get(images[0].image.name())
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let object = doc.resolve_object(id).unwrap();
    let stream = object.as_stream().unwrap();
    let mut mask = PdfDict::default();
    for (key, value) in [("Subtype", "Image"), ("ColorSpace", "DeviceGray")] {
        mask.try_insert(
            key.into(),
            PdfObject::PdfName(pdf::objects::pdf_name::PdfName::new(value.into())),
        )
        .unwrap();
    }
    for (key, value) in [("Width", 256.), ("Height", 256.), ("BitsPerComponent", 8.)] {
        mask.try_insert(
            key.into(),
            PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(value)),
        )
        .unwrap();
    }
    let mut matte = pdf::objects::pdf_array::PdfArray::default();
    for _ in 0..3 {
        matte
            .try_add_obj(PdfObject::PdfNumber(
                pdf::objects::pdf_number::PdfNumber::new(0.),
            ))
            .unwrap();
    }
    mask.try_insert("Matte".into(), PdfObject::PdfArray(matte))
        .unwrap();
    let mask = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            mask,
            vec![128; 256 * 256],
        )))
        .unwrap();
    let mut dict = stream.dict().clone();
    dict.try_insert(
        "SMask".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(mask)),
    )
    .unwrap();
    doc.update_object(
        id,
        PdfObject::PdfStream(PdfStream::new(
            dict,
            doc.read_stream_data(stream).unwrap().to_vec(),
        )),
    )
    .unwrap();
    let page = doc.prepare_page_for_render(0, None, 72.).unwrap();
    assert_eq!(dimensions(&doc, &page), [(256, 256)]);
    let target = doc
        .prepare_page_for_render_with_policy(
            0,
            None,
            72.,
            pdf::document::prepared_page::ImageDecodePolicy::TargetResolution,
        )
        .unwrap();
    assert_eq!(dimensions(&doc, &target), [(256, 256)]);
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Image(image) = &list.objects()[0] else {
        panic!("expected image");
    };
    assert_eq!(image.image.rgba_buffer().unwrap()[3], 128);
}

#[test]
fn target_policy_is_opt_in_and_cache_identity_includes_policy_and_dpi() {
    use pdf::document::prepared_page::ImageDecodePolicy::TargetResolution;
    let doc = scan(1600, 1200, "q 200 0 0 150 0 0 cm /IMAGE Do Q");
    let saved = doc.save_to_bytes().unwrap();
    let full = doc.prepare_page_for_render(0, None, 72.).unwrap();
    assert_eq!(dimensions(&doc, &full), [(1600, 1200)]);
    let target = doc
        .prepare_page_for_render_with_policy(0, None, 72., TargetResolution)
        .unwrap();
    assert_eq!(dimensions(&doc, &target), [(200, 150)]);
    assert!(!Rc::ptr_eq(&target, &full));
    assert!(Rc::ptr_eq(
        &target,
        &doc.prepare_page_for_render_with_policy(0, None, 72., TargetResolution)
            .unwrap()
    ));
    assert!(Rc::ptr_eq(
        &full,
        &doc.prepare_page_for_render(0, None, 72.).unwrap()
    ));
    let high = doc
        .prepare_page_for_render_with_policy(0, None, 144., TargetResolution)
        .unwrap();
    assert_eq!(dimensions(&doc, &high), [(400, 300)]);
    let native = doc
        .prepare_page_for_render_with_policy(0, None, 576., TargetResolution)
        .unwrap();
    assert_eq!(dimensions(&doc, &native), [(1600, 1200)]);
    assert!(
        doc.prepare_page_for_render_with_policy(99, None, 72., TargetResolution)
            .is_err()
    );
    assert_eq!(
        dimensions(&doc, &doc.prepare_page(0, None).unwrap()),
        [(1600, 1200)]
    );
    assert_eq!(saved, doc.save_to_bytes().unwrap());
}

#[test]
fn target_policy_observes_each_draw_transform_and_progressive_fallback() {
    use pdf::document::prepared_page::ImageDecodePolicy::TargetResolution;
    let doc = scan(
        1600,
        1200,
        "q 200 0 0 150 0 0 cm /IMAGE Do Q q 0 800 -600 0 600 0 cm /IMAGE Do Q q 1200 0 0 900 0 0 cm /IMAGE Do Q q 200 0 0 150 0 0 cm /IMAGE Do Q q 200 0 0 150 20 30 cm /IMAGE Do Q",
    );
    let target = doc
        .prepare_page_for_render_with_policy(0, None, 72., TargetResolution)
        .unwrap();
    assert_eq!(
        dimensions(&doc, &target),
        [(200, 150), (800, 600), (1600, 1200), (200, 150), (200, 150)]
    );
    let mut draws = Images::default();
    target.replay(&doc, &mut draws).unwrap();
    assert_eq!(
        draws.1[3], draws.1[4],
        "translation should reuse reduced pixels"
    );
    let progressive = scan_with_process(256, 256, "q 16 0 0 16 0 0 cm /IMAGE Do Q", true);
    let target = progressive
        .prepare_page_for_render_with_policy(0, None, 72., TargetResolution)
        .unwrap();
    assert_eq!(dimensions(&progressive, &target), [(256, 256)]);
}
