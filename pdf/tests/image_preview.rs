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
        Some("mask")
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
