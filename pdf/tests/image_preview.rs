use pdf::{
    device::Device,
    document::{
        Document,
        prepared_page::{ImageDecodePolicy, PreparedPage},
    },
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_name::PdfName, pdf_stream::PdfStream,
    },
    page::image_object::ImageObject,
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};
use std::rc::Rc;

fn options() -> ParseOptions {
    ParseOptions {
        mode: ParseMode::Compatible,
        limits: Limits {
            max_image_pixels: 4_000_000,
            max_decoded_bytes: 16 * 1024 * 1024,
            max_operation_bytes: 64 * 1024 * 1024,
            max_cached_stream_bytes: 16 * 1024 * 1024,
            ..Default::default()
        },
    }
}
fn fixed(codec: &str) -> Document {
    let source: &[u8] = match codec {
        "flate" => include_bytes!("fixtures/image-preview/preview-flate.pdf"),
        "jpeg" => include_bytes!("fixtures/image-preview/preview-jpeg.pdf"),
        _ => include_bytes!("fixtures/image-preview/preview-progressive.pdf"),
    };
    Document::from_bytes_with_options(source.to_vec(), options()).unwrap()
}
#[derive(Default)]
struct Images(Vec<ImageObject>);
impl Device for Images {
    fn start_page(&mut self, _: f32, _: f32) -> PdfResult<()> {
        Ok(())
    }
    fn end_page(&mut self) -> PdfResult<()> {
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
    fn do_image(&mut self, image: ImageObject) -> PdfResult<()> {
        self.0.push(image);
        Ok(())
    }
}
fn images(doc: &Document, page: &PreparedPage) -> Vec<ImageObject> {
    let mut sink = Images::default();
    page.replay(doc, &mut sink).unwrap();
    sink.0
}
fn prepare(doc: &Document, dpi: f32) -> Rc<PreparedPage> {
    doc.prepare_page_for_render_with_policy(1, None, dpi, ImageDecodePolicy::TargetResolution)
        .unwrap()
}
fn pixel(image: &ImageObject, x: u32, y: u32) -> [u8; 4] {
    let rgba = image.image.rgba_buffer().unwrap();
    let w = image.image.pixel_dimensions().0;
    rgba[((y * w + x) * 4) as usize..((y * w + x) * 4 + 4) as usize]
        .try_into()
        .unwrap()
}

#[test]
fn fixed_flate_preview_has_correct_pattern_source_dimensions_and_save_roundtrip() {
    let doc = fixed("flate");
    let saved = doc.save_to_bytes().unwrap();
    let original = doc.resolve_object(ObjectId::new(7, 0)).unwrap();
    let original_bytes = doc
        .read_stream_data(original.as_stream().unwrap())
        .unwrap()
        .to_vec();
    for dpi in [72., 36., 18.] {
        let page = prepare(&doc, dpi);
        let draws = images(&doc, &page);
        let image = &draws[0];
        assert_eq!(image.image.source_pixel_dimensions(), (1916, 2665));
        let shared = image.image.as_ref().clone();
        assert_eq!(shared.data().as_ptr(), image.image.data().as_ptr());
        assert_eq!(shared.source_pixel_dimensions(), (1916, 2665));
        let (w, h) = image.image.pixel_dimensions();
        assert_eq!(
            (w, h),
            (
                (595. * dpi / 72.).ceil() as u32,
                (842. * dpi / 72.).ceil() as u32
            )
        );
        assert_eq!(pixel(image, w / 2, h / 5), [230, 180, 60, 255]);
        assert_eq!(pixel(image, w / 2, h * 3 / 8), [34, 96, 180, 255]);
        assert_eq!(pixel(image, w / 2, h * 3 / 5), [240, 245, 250, 255]);
        assert!(Rc::ptr_eq(&page, &prepare(&doc, dpi)));
    }
    assert_eq!(doc.source_stream_statistics().resident_bytes, 0);
    assert!(doc.render_cache_statistics().page_entries <= 2);
    assert!(matches!(
        doc.get_page(1).unwrap().extract_images_paint_order(),
        Err(PdfError::ResourceLimitExceeded {
            reason: "image_pixels",
            ..
        })
    ));
    assert_eq!(doc.save_to_bytes().unwrap(), saved);
    let source = doc
        .get_page(1)
        .unwrap()
        .extract_image_source("Im1")
        .unwrap();
    assert_eq!(source.stream.raw_data().unwrap(), original_bytes);
    let reopened = Document::from_bytes_with_options(saved, options()).unwrap();
    assert_eq!(
        pixel(&images(&reopened, &prepare(&reopened, 72.))[0], 200, 200),
        [230, 180, 60, 255]
    );
}
#[test]
fn budgeted_fallback_and_transform_dependent_cache_keep_correct_scales() {
    for policy in [
        ImageDecodePolicy::Budgeted,
        ImageDecodePolicy::TargetResolution,
    ] {
        let doc = fixed("flate");
        doc.replace_page_contents(1,b"q 595 0 0 842 0 0 cm /Im1 Do Q q 100 0 0 140 0 0 cm /Im1 Do Q q 100 0 0 140 10 0 cm /Im1 Do Q q 0 100 -140 0 100 0 cm /Im1 Do Q q 100 60 30 140 0 0 cm /Im1 Do Q".to_vec()).unwrap();
        let page = doc
            .prepare_page_for_render_with_policy(1, None, 72., policy)
            .unwrap();
        let draws = images(&doc, &page);
        assert_eq!(
            draws
                .iter()
                .map(|i| i.image.pixel_dimensions())
                .collect::<Vec<_>>(),
            [(595, 842), (100, 140), (100, 140), (100, 140), (117, 144)]
        );
        assert!(Rc::ptr_eq(&draws[1].image, &draws[2].image));
        assert!(!Rc::ptr_eq(&draws[0].image, &draws[1].image));
        for image in draws {
            let (w, h) = image.image.pixel_dimensions();
            assert_eq!(pixel(&image, w / 2, h / 5), [230, 180, 60, 255]);
        }
        doc.set_page_rotation(1, 90).unwrap();
        assert_eq!(
            images(
                &doc,
                &doc.prepare_page_for_render_with_policy(1, None, 72., policy)
                    .unwrap()
            )[0]
            .image
            .pixel_dimensions(),
            (595, 842)
        );
    }
}
fn rejected(doc: &Document) -> PdfError {
    doc.prepare_page_for_render_with_policy(1, None, 72., ImageDecodePolicy::TargetResolution)
        .unwrap_err()
}
#[test]
fn progressive_and_masked_images_keep_typed_negative_results_and_can_recover() {
    let progressive = fixed("progressive");
    let error = rejected(&progressive);
    assert!(matches!(
        error.root_cause(),
        PdfError::ResourceLimitExceeded {
            reason: "image_pixels",
            actual: 5106140,
            limit: 4000000,
            ..
        }
    ));
    let info = error.image_decode_details().unwrap();
    assert_eq!(info.encoding, "jpeg");
    assert_eq!(info.jpeg_process, Some("progressive"));
    assert!(!info.target_decode_supported);
    assert!(progressive.prepare_page_for_render(0, None, 72.).is_ok());
    let doc = fixed("flate");
    let original = doc.resolve_object(ObjectId::new(7, 0)).unwrap();
    let stream = original.as_stream().unwrap();
    let mut dict = stream.dict().clone();
    dict.try_insert("SMask".into(), PdfObject::PdfDict(PdfDict::default()))
        .unwrap();
    doc.update_object(
        ObjectId::new(7, 0),
        PdfObject::PdfStream(PdfStream::new(
            dict,
            doc.read_stream_data(stream).unwrap().to_vec(),
        )),
    )
    .unwrap();
    let error = rejected(&doc);
    assert!(matches!(
        error.root_cause(),
        PdfError::ResourceLimitExceeded {
            reason: "image_pixels",
            ..
        }
    ));
    assert_eq!(
        error.image_decode_details().unwrap().unsupported_reason,
        Some("mask_invalid")
    );
    assert_eq!(doc.render_cache_statistics().page_entries, 0);
    doc.update_object(ObjectId::new(7, 0), original).unwrap();
    assert_eq!(images(&doc, &prepare(&doc, 72.)).len(), 1);
}
#[test]
fn corrupt_stream_failure_does_not_publish_page_or_change_document_and_retry_works() {
    let doc = fixed("flate");
    let id = ObjectId::new(7, 0);
    let original = doc.resolve_object(id).unwrap();
    let stream = original.as_stream().unwrap();
    let mut raw = doc.read_stream_data(stream).unwrap().to_vec();
    raw.pop();
    doc.update_object(
        id,
        PdfObject::PdfStream(PdfStream::new(stream.dict().clone(), raw)),
    )
    .unwrap();
    let stamp = doc.stamp();
    let before = doc.save_to_bytes().unwrap();
    let error = rejected(&doc);
    assert!(matches!(error.root_cause(), PdfError::FilterError(_)));
    assert!(
        error
            .image_decode_details()
            .unwrap()
            .target_decode_supported
    );
    assert_eq!(doc.render_cache_statistics().page_entries, 0);
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    doc.update_object(id, original).unwrap();
    assert_eq!(images(&doc, &prepare(&doc, 72.)).len(), 1);
}
#[test]
fn jpeg_choices_stay_unchanged_and_flate_reports_each_budget_and_complex_space() {
    let jpeg = fixed("jpeg");
    assert_eq!(
        images(&jpeg, &prepare(&jpeg, 72.))[0]
            .image
            .pixel_dimensions(),
        (958, 1333)
    );
    let doc = fixed("flate");
    let error = doc
        .prepare_page_for_render_with_policy(1, None, 288., ImageDecodePolicy::TargetResolution)
        .unwrap_err();
    assert!(matches!(
        error.root_cause(),
        PdfError::ResourceLimitExceeded {
            reason: "image_pixels",
            ..
        }
    ));
    assert_eq!(error.image_decode_details().unwrap().encoding, "flate");
    let id = ObjectId::new(7, 0);
    let original = doc.resolve_object(id).unwrap();
    let stream = original.as_stream().unwrap();
    let mut dict = stream.dict().clone();
    dict.try_insert(
        "ColorSpace".into(),
        PdfObject::PdfName(PdfName::new("DeviceCMYK".into())),
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
    let error = rejected(&doc);
    assert_eq!(
        error.image_decode_details().unwrap().unsupported_reason,
        Some("color_space")
    );
}

#[test]
fn device_default_overrides_do_not_enter_the_simple_flate_preview_path() {
    let doc = fixed("flate");
    let page_id = ObjectId::new(5, 0);
    let mut page = doc.resolve_object(page_id).unwrap().into_dict().unwrap();
    let mut resources = page.get("Resources").unwrap().as_dict().unwrap().clone();
    let mut colors = PdfDict::default();
    colors
        .try_insert(
            "DefaultRGB".into(),
            PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
        )
        .unwrap();
    resources
        .try_insert("ColorSpace".into(), PdfObject::PdfDict(colors))
        .unwrap();
    page.try_insert("Resources".into(), PdfObject::PdfDict(resources))
        .unwrap();
    doc.update_object(page_id, PdfObject::PdfDict(page))
        .unwrap();
    let error = rejected(&doc);
    assert_eq!(
        error.image_decode_details().unwrap().unsupported_reason,
        Some("device_color_override")
    );
    assert!(matches!(
        error.root_cause(),
        PdfError::ResourceLimitExceeded {
            reason: "image_pixels",
            ..
        }
    ));
}

#[test]
fn jpeg_preview_does_not_start_validating_unused_color_resources() {
    let doc = fixed("jpeg");
    let page_id = ObjectId::new(5, 0);
    let mut page = doc.resolve_object(page_id).unwrap().into_dict().unwrap();
    let mut resources = page.get("Resources").unwrap().as_dict().unwrap().clone();
    resources
        .try_insert(
            "ColorSpace".into(),
            PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(42.)),
        )
        .unwrap();
    page.try_insert("Resources".into(), PdfObject::PdfDict(resources))
        .unwrap();
    doc.update_object(page_id, PdfObject::PdfDict(page))
        .unwrap();
    let draws = images(&doc, &prepare(&doc, 72.));
    assert_eq!(draws[0].image.pixel_dimensions(), (958, 1333));
}

fn masked_document(mask_size: (u32, u32), alpha: &[u8], extra: &str) -> Document {
    use std::io::Write;
    let doc = fixed("flate");
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(alpha).unwrap();
    let mut dict = PdfDict::default();
    for (key, value) in [
        ("Subtype", "Image"),
        ("ColorSpace", "DeviceGray"),
        ("Filter", "FlateDecode"),
    ] {
        dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))
            .unwrap();
    }
    for (key, value) in [
        ("Width", mask_size.0),
        ("Height", mask_size.1),
        ("BitsPerComponent", 8),
    ] {
        dict.try_insert(
            key.into(),
            PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(value as f32)),
        )
        .unwrap();
    }
    if extra == "invert" {
        let mut decode = pdf::objects::pdf_array::PdfArray::default();
        for v in [1., 0.] {
            decode.add_obj(PdfObject::PdfNumber(
                pdf::objects::pdf_number::PdfNumber::new(v),
            ));
        }
        dict.try_insert("Decode".into(), PdfObject::PdfArray(decode))
            .unwrap();
    }
    if extra == "interpolate" {
        dict.try_insert(
            "Interpolate".into(),
            PdfObject::PdfBool(pdf::objects::pdf_bool::PdfBool::new(b"true")),
        )
        .unwrap();
    }
    let mask = PdfObject::PdfStream(PdfStream::new(dict, encoder.finish().unwrap()));
    let source = doc.resolve_object(ObjectId::new(7, 0)).unwrap();
    let stream = source.as_stream().unwrap();
    let mut dict = stream.dict().clone();
    let mask_id = doc.insert_object(mask);
    dict.try_insert(
        "SMask".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(mask_id)),
    )
    .unwrap();
    doc.update_object(
        ObjectId::new(7, 0),
        PdfObject::PdfStream(PdfStream::new(
            dict,
            doc.read_stream_data(stream).unwrap().to_vec(),
        )),
    )
    .unwrap();
    doc
}
#[test]
fn soft_mask_preview_preserves_coverage_source_save_and_transform_caches() {
    let doc = masked_document((3, 2), &[255, 128, 0, 255, 128, 0], "");
    let before = doc.save_to_bytes().unwrap();
    let stamp = doc.stamp();
    for policy in [
        ImageDecodePolicy::Budgeted,
        ImageDecodePolicy::TargetResolution,
    ] {
        for dpi in [72., 36., 18.] {
            let page = doc
                .prepare_page_for_render_with_policy(1, None, dpi, policy)
                .unwrap();
            let draws = images(&doc, &page);
            let image = &draws[0];
            let (w, h) = image.image.pixel_dimensions();
            assert_eq!(image.image.source_pixel_dimensions(), (1916, 2665));
            assert_eq!(pixel(image, w / 2, h / 5), [230, 180, 60, 128]);
            assert_eq!(pixel(image, w / 8, h * 3 / 8), [34, 96, 180, 255]);
            assert_eq!(pixel(image, w * 7 / 8, h / 5)[3], 0);
            let clone = image.image.as_ref().clone();
            assert_eq!(clone.data().as_ptr(), image.image.data().as_ptr());
        }
    }
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    let source = doc
        .get_page(1)
        .unwrap()
        .extract_image_source("Im1")
        .unwrap();
    assert_eq!(
        source.stream.dict().get("Width").unwrap().as_u32(),
        Some(1916)
    );
    assert!(source.stream.dict().get("SMask").is_some());
    let reopened = Document::from_bytes_with_options(before, options()).unwrap();
    let draws = images(&reopened, &prepare(&reopened, 72.));
    assert_eq!(pixel(&draws[0], 297, 168), [230, 180, 60, 128]);
    doc.replace_page_contents(1,b"q 595 0 0 842 0 0 cm /Im1 Do Q q 100 0 0 140 0 0 cm /Im1 Do Q q 100 0 0 140 20 0 cm /Im1 Do Q q 0 100 -140 0 200 0 cm /Im1 Do Q q 100 60 30 140 0 0 cm /Im1 Do Q".to_vec()).unwrap();
    let draws = images(&doc, &prepare(&doc, 72.));
    assert!(Rc::ptr_eq(&draws[1].image, &draws[2].image));
    assert!(!Rc::ptr_eq(&draws[0].image, &draws[1].image));
    for image in draws {
        let (w, h) = image.image.pixel_dimensions();
        assert_eq!(pixel(&image, w / 2, h / 5), [230, 180, 60, 128]);
    }
    doc.clear_render_caches();
    assert_eq!(doc.render_cache_statistics().page_entries, 0);
}
#[test]
fn soft_mask_decode_reversal_and_unsupported_interpolation_are_explicit() {
    let doc = masked_document((1, 1), &[0], "invert");
    let draws = images(&doc, &prepare(&doc, 18.));
    assert_eq!(pixel(&draws[0], 50, 50)[3], 255);
    let doc = masked_document((1, 1), &[128], "interpolate");
    let error = rejected(&doc);
    let details = error.image_decode_details().unwrap();
    assert_eq!(details.mask_kind, Some("soft"));
    assert_eq!(
        (details.mask_width, details.mask_height),
        (Some(1), Some(1))
    );
    assert_eq!(details.mask_encoding, Some("flate"));
    assert_eq!(details.mask_bits_per_component, Some(8));
    assert!(!details.matte_present);
    assert_eq!(details.unsupported_reason, Some("mask_interpolate"));
    assert!(!details.target_decode_supported);
    assert!(doc.prepare_page_for_render(0, None, 72.).is_ok());
}
#[test]
fn corrupt_or_cyclic_soft_mask_never_publishes_and_replacement_recovers() {
    let doc = masked_document((3, 2), &[128; 6], "");
    let id = ObjectId::new(7, 0);
    let original = doc.resolve_object(id).unwrap();
    let stream = original.as_stream().unwrap();
    let resolved_mask = doc
        .resolve_object(
            stream
                .dict()
                .get("SMask")
                .unwrap()
                .as_reference()
                .unwrap()
                .id(),
        )
        .unwrap();
    let mask = resolved_mask.as_stream().unwrap();
    let mut raw = doc.read_stream_data(mask).unwrap().to_vec();
    *raw.last_mut().unwrap() ^= 1;
    let bad_id = doc.insert_object(PdfObject::PdfStream(PdfStream::new(
        mask.dict().clone(),
        raw,
    )));
    let mut dict = stream.dict().clone();
    dict.try_insert(
        "SMask".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(bad_id)),
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
    let before = doc.save_to_bytes().unwrap();
    let stamp = doc.stamp();
    let error = rejected(&doc);
    assert!(matches!(error.root_cause(), PdfError::FilterError(_)));
    assert!(
        error
            .image_decode_details()
            .unwrap()
            .target_decode_supported
    );
    assert_eq!(doc.render_cache_statistics().page_entries, 0);
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert!(doc.prepare_page_for_render(0, None, 72.).is_ok());
    let mut dict = stream.dict().clone();
    dict.try_insert(
        "SMask".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(id)),
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
    let error = rejected(&doc);
    assert_eq!(
        error.image_decode_details().unwrap().unsupported_reason,
        Some("mask_nested")
    );
    doc.update_object(id, original).unwrap();
    assert_eq!(images(&doc, &prepare(&doc, 72.)).len(), 1);
}

#[test]
fn fixed_matte_fixture_decodes_without_halo_and_keeps_original_streams() {
    let doc = Document::from_bytes_with_options(
        include_bytes!("fixtures/masked-preview/soft-matte.pdf").to_vec(),
        options(),
    )
    .unwrap();
    let before = doc.save_to_bytes().unwrap();
    for dpi in [72., 36., 18.] {
        let page = doc
            .prepare_page_for_render_with_policy(0, None, dpi, ImageDecodePolicy::TargetResolution)
            .unwrap();
        let draws = images(&doc, &page);
        let image = &draws[0];
        let (w, h) = image.image.pixel_dimensions();
        assert_eq!(image.image.source_pixel_dimensions(), (2665, 1916));
        assert_eq!(pixel(image, w / 4, h / 2), [255, 0, 0, 128]);
        assert_eq!(pixel(image, w * 3 / 4, h / 2)[3], 0);
    }
    assert_eq!(doc.save_to_bytes().unwrap(), before);
}

#[test]
fn a_finer_mask_controls_target_size_and_budgeted_cache_reuse() {
    use std::io::Write;
    let doc = Document::from_bytes_with_options(
        include_bytes!("fixtures/masked-preview/soft-white.pdf").to_vec(),
        options(),
    )
    .unwrap();
    let id = ObjectId::new(5, 0);
    let original = doc.resolve_object(id).unwrap();
    let mut dict = original.as_stream().unwrap().dict().clone();
    for key in ["Width", "Height"] {
        dict.try_insert(
            key.into(),
            PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(1.)),
        )
        .unwrap();
    }
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&[220, 40, 80]).unwrap();
    doc.update_object(
        id,
        PdfObject::PdfStream(PdfStream::new(dict, z.finish().unwrap())),
    )
    .unwrap();
    doc.replace_page_contents(
        0,
        b"q 1 0 0 1 0 0 cm /Im1 Do Q q 767 0 0 551 0 0 cm /Im1 Do Q".to_vec(),
    )
    .unwrap();
    let page = doc
        .prepare_page_for_render_with_policy(0, None, 72., ImageDecodePolicy::Budgeted)
        .unwrap();
    let draws = images(&doc, &page);
    assert_eq!(draws[0].image.pixel_dimensions(), (1, 1));
    assert_eq!(draws[1].image.pixel_dimensions(), (767, 551));
    assert!(!Rc::ptr_eq(&draws[0].image, &draws[1].image));
    assert_eq!(pixel(&draws[1], 100, 200)[3], 0);
    assert_eq!(pixel(&draws[1], 480, 200), [220, 40, 80, 255]);
}

#[test]
fn mask_diagnostics_distinguish_kinds_without_decoding_unsupported_payloads() {
    let valid = fixed("flate");
    let original = valid.resolve_object(ObjectId::new(7, 0)).unwrap();
    let stream = original.as_stream().unwrap();
    let raw = valid.read_stream_data(stream).unwrap().to_vec();
    for (kind, key, value) in [
        (
            "color-key",
            "Mask",
            PdfObject::PdfArray(pdf::objects::pdf_array::PdfArray::default()),
        ),
        ("explicit", "Mask", original.clone()),
        (
            "stencil",
            "ImageMask",
            PdfObject::PdfBool(pdf::objects::pdf_bool::PdfBool::new(b"true")),
        ),
        (
            "embedded-alpha",
            "SMaskInData",
            PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(1.)),
        ),
    ] {
        let doc = fixed("flate");
        let mut dict = stream.dict().clone();
        dict.try_insert(key.into(), value).unwrap();
        doc.update_object(
            ObjectId::new(7, 0),
            PdfObject::PdfStream(PdfStream::new(dict, raw.clone())),
        )
        .unwrap();
        let error = rejected(&doc);
        let info = error.image_decode_details().unwrap();
        assert_eq!(info.mask_kind, Some(kind));
        assert!(!info.target_decode_supported);
        if kind == "explicit" {
            assert_eq!(info.mask_width, Some(1916));
            assert_eq!(info.mask_encoding, Some("flate"));
        }
        assert_eq!(doc.render_cache_statistics().page_entries, 0);
    }
}

#[test]
fn shared_indirect_mask_revision_invalidates_combined_previews() {
    use std::io::Write;
    let doc = masked_document((1, 1), &[128], "");
    let base = doc.resolve_object(ObjectId::new(7, 0)).unwrap();
    let mask_id = base
        .as_stream()
        .unwrap()
        .dict()
        .get("SMask")
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let other_id = doc.insert_object(base.clone());
    let page_id = ObjectId::new(5, 0);
    let mut page = doc.resolve_object(page_id).unwrap().into_dict().unwrap();
    let mut resources = page.get("Resources").unwrap().as_dict().unwrap().clone();
    let mut xobjects = resources.get("XObject").unwrap().as_dict().unwrap().clone();
    xobjects
        .try_insert(
            "Im2".into(),
            PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(other_id)),
        )
        .unwrap();
    resources
        .try_insert("XObject".into(), PdfObject::PdfDict(xobjects))
        .unwrap();
    page.try_insert("Resources".into(), PdfObject::PdfDict(resources))
        .unwrap();
    doc.update_object(page_id, PdfObject::PdfDict(page))
        .unwrap();
    doc.replace_page_contents(
        1,
        b"q 100 0 0 140 0 0 cm /Im1 Do Q q 300 0 0 420 100 0 cm /Im2 Do Q".to_vec(),
    )
    .unwrap();
    let old = prepare(&doc, 72.);
    for image in images(&doc, &old) {
        assert_eq!(pixel(&image, 20, 20)[3], 128);
    }
    let mask = doc.resolve_object(mask_id).unwrap();
    let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    z.write_all(&[255]).unwrap();
    doc.update_object(
        mask_id,
        PdfObject::PdfStream(PdfStream::new(
            mask.as_stream().unwrap().dict().clone(),
            z.finish().unwrap(),
        )),
    )
    .unwrap();
    let new = prepare(&doc, 72.);
    assert!(!Rc::ptr_eq(&old, &new));
    assert!(old.replay(&doc, &mut Images::default()).is_err());
    for image in images(&doc, &new) {
        assert_eq!(pixel(&image, 20, 20)[3], 255);
    }
}
