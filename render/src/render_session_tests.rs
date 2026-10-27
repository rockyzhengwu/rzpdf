use super::*;
use pdf::{
    document::{Document, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::SyntaxParser,
};

fn document() -> Document {
    let doc = Document::new().unwrap();
    doc.add_page_with_builder(
        0,
        PageBuilder::new()
            .size(128., 128.)
            .content(b"0.2 0.5 0.9 rg 4 4 m 118 8 8 118 110 110 c 4 4 l f".to_vec()),
    )
    .unwrap();
    doc
}
fn raster(
    session: &RenderSession,
    doc: &Document,
    dpi: f32,
    region: Option<RasterRegion>,
) -> Vec<u8> {
    let mut target = session.renderer(doc, 0, dpi, dpi).unwrap();
    target.set_region(region).unwrap();
    doc.prepare_page_for_render(0, None, dpi)
        .unwrap()
        .replay(doc, &mut target)
        .unwrap();
    target.to_rgba().unwrap().pixels
}
fn parse(bytes: &[u8]) -> pdf::objects::pdf_dict::PdfDict {
    SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(bytes.to_vec())).unwrap())
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap()
}
fn image_document() -> Document {
    let doc = document();
    let dictionary = parse(b"<< /Type /XObject /Subtype /Image /Width 2 /Height 2 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate true >>");
    let pixels = vec![128, 64, 192, 32, 128, 64, 64, 32, 128, 192, 128, 32];
    let image = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dictionary, pixels)))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Image, image)
        .unwrap();
    doc.replace_page_contents(0, format!("q 100 0 0 100 5 5 cm /{name} Do Q").into_bytes())
        .unwrap();
    doc
}

#[test]
fn fresh_targets_reuse_geometry_and_match_uncached_pixels_at_different_dpi_and_tiles() {
    let doc = document();
    let session = RenderSession::new(doc.resource_limits());
    let disabled = RenderSession::with_cache_limit(doc.resource_limits(), 0);
    for dpi in [72., 144.] {
        for region in [
            None,
            Some(RasterRegion {
                x: 16,
                y: 12,
                width: 70,
                height: 80,
            }),
        ] {
            let expected = raster(&disabled, &doc, dpi, region);
            assert_eq!(raster(&session, &doc, dpi, region), expected);
            let before = session.statistics().geometry_hits;
            assert_eq!(raster(&session, &doc, dpi, region), expected);
            assert!(session.statistics().geometry_hits > before);
        }
    }
    assert_eq!(disabled.statistics().estimated_bytes, 0);
    assert!(session.statistics().estimated_bytes <= session.byte_limit());
}

#[test]
fn image_cache_respects_color_context_shape_and_new_target_lifetime() {
    let doc = image_document();
    let session = RenderSession::new(doc.resource_limits());
    let expected = raster(&session, &doc, 72., None);
    assert_eq!(session.statistics().image_entries, 1);
    assert_eq!(raster(&session, &doc, 72., None), expected);
    assert!(session.statistics().image_hits > 0);
    let page = doc.prepare_page_for_render(0, None, 72.).unwrap();
    let mut linear = moxcms::ColorProfile::new_srgb();
    let trc = moxcms::ColorProfile::new_gray_with_gamma(1.).gray_trc;
    linear.red_trc = trc.clone();
    linear.green_trc = trc.clone();
    linear.blue_trc = trc;
    let output = pdf::color::icc_profile::ICCProfile::try_new(&linear.encode().unwrap()).unwrap();
    for shape in [false, true] {
        for profile in [pdf::color::icc_profile::ICCProfile::srgb(), output.clone()] {
            let context =
                pdf::color::conversion::ColorConversionContext::with_output_profile(profile)
                    .unwrap();
            let disabled = RenderSession::with_cache_limit(doc.resource_limits(), 0);
            let run = |session: &RenderSession| {
                let mut target = session.renderer(&doc, 0, 72., 72.).unwrap();
                target.shape_mode = shape;
                target.color_context = context.clone();
                page.replay(&doc, &mut target).unwrap();
                target.to_rgba().unwrap().pixels
            };
            let reference = run(&disabled);
            assert_eq!(run(&session), reference);
            if !context.output_profile().is_builtin_srgb() {
                assert_ne!(
                    reference, expected,
                    "output profile must change midtone image samples"
                );
            }
            let before = session.statistics().image_hits;
            assert_eq!(run(&session), reference);
            assert!(session.statistics().image_hits > before);
        }
    }
    session.clear();
    assert_eq!(session.statistics().estimated_bytes, 0);
    assert_eq!(session.statistics().image_entries, 0);
    assert!(expected.iter().any(|v| *v != 255));
}

#[test]
fn document_revision_font_epoch_and_release_invalidate_without_affecting_outputs() {
    let doc = document();
    let session = RenderSession::new(doc.resource_limits());
    let original = raster(&session, &doc, 72., None);
    assert!(session.statistics().geometry_entries > 0);
    drop(session.renderer(&doc, 1, 72., 72.).unwrap());
    assert_eq!(session.statistics().estimated_bytes, 0);
    let _ = raster(&session, &doc, 72., None);
    doc.replace_page_contents(0, b"1 0 0 rg 0 0 128 128 re f".to_vec())
        .unwrap();
    let changed = raster(&session, &doc, 72., None);
    assert_ne!(original, changed);
    assert!(
        changed
            .as_chunks::<4>()
            .0
            .iter()
            .all(|p| *p == [255, 0, 0, 255])
    );
    let another = document();
    drop(session.renderer(&another, 0, 72., 72.).unwrap());
    assert_eq!(session.statistics().estimated_bytes, 0);
    assert!(original.iter().any(|v| *v != 255));
}

#[test]
fn optional_cache_eviction_and_zero_budget_do_not_reject_valid_rendering() {
    let doc = document();
    let tiny = RenderSession::with_cache_limit(doc.resource_limits(), 2048);
    let disabled = RenderSession::with_cache_limit(doc.resource_limits(), 0);
    for dpi in [72., 144., 96., 72.] {
        assert_eq!(
            raster(&tiny, &doc, dpi, None),
            raster(&disabled, &doc, dpi, None)
        );
        assert!(tiny.statistics().estimated_bytes <= 2048);
    }
}

#[test]
fn warm_coverage_preserves_work_and_scratch_limit_rejections() {
    let doc = document();
    let session = RenderSession::new(doc.resource_limits());
    let page = doc.prepare_page_for_render(0, None, 72.).unwrap();
    let mut target = session.renderer(&doc, 0, 72., 72.).unwrap();
    page.replay(&doc, &mut target).unwrap();
    let baseline = target.to_rgba().unwrap().pixels;
    let mut builder = PathBuilder::new();
    builder
        .move_to((4., 4.))
        .cubic_to((118., 8.), (8., 118.), (110., 110.))
        .close();
    let path = builder.snapshot();
    let mut paint = Paint::default();
    paint.set_anti_alias(true).set_color(Color::RED);
    target
        .draw_prepared_path(&path, &paint, &Matrix::identity())
        .unwrap();
    let bytes = target.to_rgba().unwrap().pixels;
    target.raster_path_budget = 1;
    assert!(
        target
            .draw_prepared_path(&path, &paint, &Matrix::identity())
            .is_err()
    );
    assert_eq!(target.to_rgba().unwrap().pixels, bytes);
    target.raster_path_budget = usize::MAX;
    target.limits.max_render_bytes = 128 * 128 * 4;
    assert!(
        target
            .draw_prepared_path(&path, &paint, &Matrix::identity())
            .is_err()
    );
    assert_eq!(target.to_rgba().unwrap().pixels, bytes);
    assert_ne!(baseline, bytes);
}

#[test]
fn font_manager_is_lazy_budgeted_shared_and_released_on_clear() {
    let session = RenderSession::new(Limits::default());
    assert_eq!(session.statistics().font_manager_entries, 0);
    let first = session.font_manager();
    let bytes = session.statistics().estimated_bytes;
    assert!(bytes > 0 && bytes <= session.byte_limit());
    let second = session.font_manager();
    assert_eq!(session.statistics().font_manager_entries, 1);
    assert_eq!(session.statistics().font_manager_hits, 1);
    assert_eq!(session.statistics().estimated_bytes, bytes);
    session.clear();
    assert_eq!(session.statistics().font_manager_entries, 0);
    // Active references remain usable after optional cache removal.
    let data = include_bytes!("../../pdf/tests/fixtures/synthetic-symbol.ttf");
    assert!(first.new_from_data(data, 0).is_some());
    assert!(second.new_from_data(data, 0).is_some());
    let disabled = RenderSession::with_cache_limit(Limits::default(), 0);
    drop(disabled.font_manager());
    assert_eq!(disabled.statistics().font_manager_entries, 0);
    assert_eq!(disabled.statistics().estimated_bytes, 0);
}

#[test]
fn backend_face_count_stays_bounded_for_many_tiny_programs() {
    let session = RenderSession::new(Limits::default());
    let manager = session.font_manager();
    let face = manager
        .new_from_data(
            include_bytes!("../../pdf/tests/fixtures/synthetic-symbol.ttf"),
            0,
        )
        .unwrap();
    let mut sources = Vec::new();
    for _ in 0..129 {
        let font = std::rc::Rc::new(pdf::font::pdf_font::Font::Type0(Default::default()));
        session.insert_font(&font, face.clone(), 16);
        sources.push(font);
    }
    assert_eq!(session.statistics().font_entries, 128);
    assert!(session.statistics().evictions > 0);
    assert!(session.font(&sources[0]).is_none());
    assert!(session.font(&sources[128]).is_some());
    session.clear();
    assert_eq!(session.statistics().estimated_bytes, 0);
    assert_eq!(session.statistics().font_entries, 0);
}
