use super::*;
use pdf::{document::Document, page::page_builder::PageBuilder};

trait Replay {
    fn replay(&self, renderer: &mut SkiaRender) -> PdfResult<()>;
}

#[test]
fn canonical_curve_coverage_preserves_alpha_across_cells_and_checks_scratch() {
    let mut path = PathBuilder::new();
    path.move_to((8., 8.));
    path.cubic_to((8., 4.), (180., 4.), (180., 8.));
    path.line_to((180., 140.)).line_to((8., 140.)).close();
    let path = path.snapshot();
    let mut paint = Paint::default();
    paint
        .set_color(Color::RED)
        .set_alpha(128)
        .set_anti_alias(true);
    let mut renderer = SkiaRender::new(72., 72.).unwrap();
    renderer.start_page(192., 160.).unwrap();
    renderer.surface.canvas().clear(Color::WHITE);
    renderer
        .draw_prepared_path(&path, &paint, &Matrix::identity())
        .unwrap();
    let image = renderer.to_rgba().unwrap();
    // Independent interior colour oracle: one red layer at 128/255 over white.
    // Cell boundaries must neither paint twice nor leave an uncovered seam.
    for y in 10..138 {
        for x in 10..178 {
            let i = (y * image.width as usize + x) * 4;
            assert_eq!(&image.pixels[i..i + 4], &[255, 127, 127, 255]);
        }
    }
    let original = image.pixels;
    renderer.limits.max_render_bytes = 192 * 160 * 4;
    assert!(matches!(
        renderer.draw_prepared_path(&path, &paint, &Matrix::identity()),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(renderer.to_rgba().unwrap().pixels, original);
    renderer.limits.max_render_bytes = Limits::default().max_render_bytes;
    renderer.raster_path_budget = 100;
    assert!(matches!(
        renderer.draw_prepared_path(&path, &paint, &Matrix::identity()),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(renderer.to_rgba().unwrap().pixels, original);
}
impl Replay for pdf::page::display_list::DisplayList {
    fn replay(&self, r: &mut SkiaRender) -> PdfResult<()> {
        pdf::page::display_list::DisplayList::replay(self, r)
    }
}
impl Replay for pdf::page::annotations::AnnotatedDisplayList {
    fn replay(&self, r: &mut SkiaRender) -> PdfResult<()> {
        pdf::page::annotations::AnnotatedDisplayList::replay(self, r)
    }
}
fn stitched(list: &impl Replay, page: u32, xdpi: f32, ydpi: f32) {
    stitched_grid(list, page, xdpi, ydpi, 17, 13)
}
fn stitched_grid(
    list: &impl Replay,
    page: u32,
    xdpi: f32,
    ydpi: f32,
    tile_width: usize,
    tile_height: usize,
) {
    let mut full = SkiaRender::new(xdpi, ydpi).unwrap();
    list.replay(&mut full).unwrap();
    let expected = full.to_rgba().unwrap();
    let mut assembled = vec![0u8; expected.pixels.len()];
    let mut tile = SkiaRender::new(xdpi, ydpi).unwrap();
    for y in (0..expected.height).step_by(tile_height) {
        for x in (0..expected.width).step_by(tile_width) {
            let region = RasterRegion {
                x,
                y,
                width: (tile_width as u32).min(expected.width - x),
                height: (tile_height as u32).min(expected.height - y),
            };
            tile.set_region(Some(region)).unwrap();
            list.replay(&mut tile).unwrap();
            let image = tile.to_rgba().unwrap();
            assert_eq!((image.width, image.height), (region.width, region.height));
            assert!(
                tile.page_width as u32 <= region.width + 4
                    && tile.page_height as u32 <= region.height + 4
            );
            for row in 0..region.height as usize {
                let target = ((y as usize + row) * expected.width as usize + x as usize) * 4;
                let source = row * region.width as usize * 4;
                assembled[target..target + region.width as usize * 4]
                    .copy_from_slice(&image.pixels[source..source + region.width as usize * 4]);
            }
        }
    }
    let differences = assembled
        .iter()
        .zip(&expected.pixels)
        .filter(|(a, b)| a != b)
        .count();
    let maximum = assembled
        .iter()
        .zip(&expected.pixels)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0);
    // Rectangles now consume normalization work too; work counters are not a
    // valid proxy for the spec's additional curved-edge flat/alpha checks.
    let curves = full.curved_raster_paths;
    let tolerance = u8::from((xdpi / 72.).fract() != 0. || (ydpi / 72.).fract() != 0.);
    if curves {
        for (i, (a, b)) in assembled
            .chunks_exact(4)
            .zip(expected.pixels.chunks_exact(4))
            .enumerate()
        {
            if a == b {
                continue;
            }
            assert_eq!(a[3], b[3]);
            let x = i % expected.width as usize;
            let y = i / expected.width as usize;
            let mut edge = false;
            for yy in y.saturating_sub(1)..=(y + 1).min(expected.height as usize - 1) {
                for xx in x.saturating_sub(1)..=(x + 1).min(expected.width as usize - 1) {
                    let offset = (yy * expected.width as usize + xx) * 4;
                    edge |= (0..3).any(|c| {
                        assembled[offset + c] != a[c] || expected.pixels[offset + c] != b[c]
                    });
                }
            }
            assert!(
                edge,
                "flat curve-render region changed at {x}/{y}, page {page}, DPI {xdpi}/{ydpi}: {b:?} -> {a:?}"
            );
        }
    }
    if maximum > tolerance {
        let worst: Vec<_> = assembled
            .chunks_exact(4)
            .zip(expected.pixels.chunks_exact(4))
            .enumerate()
            .filter(|(_, (a, b))| a.iter().zip(*b).any(|(a, b)| a.abs_diff(*b) > 1))
            .take(20)
            .map(|(i, (a, b))| {
                (
                    i % expected.width as usize,
                    i / expected.width as usize,
                    a.to_vec(),
                    b.to_vec(),
                )
            })
            .collect();
        eprintln!("worst: {worst:?}");
    }
    assert!(
        maximum <= tolerance,
        "page {page}, DPI {xdpi}/{ydpi}: {differences} channels differ, max {maximum}, tolerance {tolerance}"
    );
    tile.set_region(None).unwrap();
    list.replay(&mut tile).unwrap();
    assert!(
        tile.to_rgba().unwrap().pixels == expected.pixels,
        "clearing region did not restore full-page rendering"
    );
}

#[test]
fn region_stitch_preserves_embedded_fonts_encodings_vertical_and_rotation() {
    let doc =
        Document::from_bytes(include_bytes!("../../pdf/tests/fixtures/v07/fonts.pdf").to_vec())
            .unwrap();
    for page in 0..doc.page_count() as u32 {
        stitched(
            &doc.get_page(page).unwrap().display_list().unwrap(),
            page,
            72.,
            72.,
        );
    }
    stitched(
        &doc.get_page(4).unwrap().display_list().unwrap(),
        4,
        96.,
        144.,
    );
}

#[test]
fn region_stitch_preserves_patterns_masks_and_transparency_at_fractional_dpi() {
    let plain = Document::new().unwrap();
    plain
        .add_page_with_builder(
            0,
            PageBuilder::new()
                .size(30., 20.)
                .content(b"0 1 0 rg 0 0 30 20 re f 1 1 0 rg 25 0 5 20 re f".to_vec()),
        )
        .unwrap();
    stitched(
        &plain.get_page(0).unwrap().display_list().unwrap(),
        0,
        96.,
        96.,
    );
    let thin = Document::new().unwrap();
    thin.add_page_with_builder(
        0,
        PageBuilder::new().size(40., 30.).content(
            b"q 1 0.2 0.1 1 0.3 0.7 cm 0.01 w [2 1] 0 d 3 3 m 3 25 35 25 35 3 c S Q".to_vec(),
        ),
    )
    .unwrap();
    let thin_list = thin.get_page(0).unwrap().display_list().unwrap();
    for dpi in [72., 96., 144.] {
        stitched(&thin_list, 0, dpi, dpi);
    }
    let pattern = super::pattern_tests::document(
        "/Matrix [1 0.2 0.1 1 0.3 0.7]",
        "1 0 0 rg 0 0 4 4 re f",
        "0 0 40 40 re f",
    );
    let backdrop = super::pattern_tests::document(
        "/BBox [0 0 10 10] /Resources << /ExtGState << /M << /BM /Multiply >> >> >>",
        "/M gs 0.5 g 0 0 10 10 re f",
        "0 1 0 rg 0 0 40 40 re f /Pattern cs /P1 scn 0 0 40 40 re f",
    );
    let mask = super::soft_mask_tests::document(
        "Luminosity",
        "/BC [0.2 0.4 0.6]",
        "",
        "/M gs 0 0 40 20 re f",
    );
    let stroke_pattern = super::pattern_tests::document(
        "/Matrix [1 0.2 0.1 1 0.3 0.7]",
        "1 0 0 rg 0 0 4 4 re f",
        "2 w [3 1] 0 d 3 3 30 28 re S",
    );
    let stroke_mask = super::soft_mask_tests::document(
        "Luminosity",
        "/BC [0.2 0.4 0.6]",
        "",
        "/M gs 2 w 2 2 30 12 re S",
    );
    let group = super::group_tests::document(false);
    let rounded = super::pattern_tests::document_sized(
        20.,
        "/BBox [0 0 10 10] /Resources << /ExtGState << /M << /BM /Multiply >> >> >>",
        "/M gs 0.5 g 0 0 10 10 re f",
        "0 1 0 rg 0 0 20 20 re f /Pattern cs /P1 scn 0 0 20 20 re f",
    );
    for (doc, page) in [
        (&pattern, 0),
        (&backdrop, 0),
        (&mask, 1),
        (&group, 0),
        (&rounded, 0),
        (&stroke_pattern, 0),
        (&stroke_mask, 1),
    ] {
        for (x, y) in [(72., 72.), (96., 96.), (144., 96.)] {
            stitched(
                &doc.get_page(page).unwrap().display_list().unwrap(),
                page,
                x,
                y,
            );
        }
    }
}

#[test]
fn region_stitch_preserves_rotated_radio_and_cjk_widgets() {
    use pdf::document::{
        acroform::{FormFieldDefinition, FormFieldKind, FormTextStyle, FormValue, FormWidgetSpec},
        embedded_font::TrueTypeEmbeddingOptions,
    };
    let doc = Document::new().unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(300., 300.))
        .unwrap();
    let font = doc
        .resources()
        .embed_truetype_font(
            0,
            include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf").to_vec(),
            "你好世界Hello PDF，！",
            TrueTypeEmbeddingOptions {
                postscript_name: "NotoSansSC-Thin".into(),
                cap_height: 733.,
                stem_v: 80.,
                serif: false,
            },
        )
        .unwrap();
    let mut field = FormFieldDefinition::new(
        FormFieldKind::Text { multiline: false },
        FormValue::Text("你好".into()),
    );
    field.text_style = FormTextStyle {
        font: Some(font.object_handle().clone()),
        font_size: 14.,
        ..Default::default()
    };
    doc.add_form_field(
        "unicode",
        &field,
        &[FormWidgetSpec::new(0, [20., 135., 180., 160.])],
    )
    .unwrap();
    let radio = FormFieldDefinition::new(
        FormFieldKind::Radio {
            no_toggle_to_off: false,
            in_unison: false,
        },
        FormValue::Radio(Some("A".into())),
    );
    let mut a = FormWidgetSpec::new(0, [200., 240., 220., 260.]);
    a.on_state = Some("A".into());
    let mut b = FormWidgetSpec::new(0, [240., 240., 260., 260.]);
    b.on_state = Some("B".into());
    doc.add_form_field("radio", &radio, &[a, b]).unwrap();
    let mut annotation = pdf::document::annotations::Annotation::new(
        pdf::document::annotations::AnnotationKind::Square,
        [200., 180., 260., 220.],
    );
    annotation.color = [1., 0., 0.];
    annotation.stroke_width = 2.;
    doc.add_annotation(0, &annotation).unwrap();
    doc.set_page_rotation(0, 90).unwrap();
    doc.set_page_user_unit(0, Some(1.5)).unwrap();
    doc.set_page_crop_box(
        0,
        Some(pdf::geom::rectangle::Rectangle::new(10., 10., 290., 290.)),
    )
    .unwrap();
    let list = doc
        .get_page(0)
        .unwrap()
        .display_list_with_annotations(Default::default())
        .unwrap();
    for dpi in [72., 96., 144.] {
        stitched_grid(&list, 0, dpi, dpi, 128, 128);
    }
}

#[test]
fn canonical_clip_masks_track_nested_canvas_lifetimes_and_reject_before_allocation() {
    let mut renderer = SkiaRender::new(72., 72.).unwrap();
    renderer.start_page(160., 140.).unwrap();
    let mut builder = PathBuilder::new();
    builder
        .move_to((10., 50.))
        .cubic_to((10., 5.), (120., 5.), (120., 50.))
        .cubic_to((120., 100.), (10., 100.), (10., 50.))
        .close();
    let path = raster_path::normalized(&builder.snapshot(), &mut 10000).unwrap();
    let install = |r: &mut SkiaRender| {
        let image = r.canonical_clip_image(&path).unwrap();
        let shader = image
            .to_shader(None, skia_safe::SamplingOptions::default(), None)
            .unwrap();
        r.surface.canvas().clip_shader(shader, None);
        r.clip_images.push(image);
    };
    for _ in 0..100 {
        renderer.raster_path_budget = 1_000_000;
        renderer.surface.canvas().save();
        install(&mut renderer);
        assert_eq!(renderer.clip_image_usage().unwrap(), (22400, 22400));
        renderer.surface.canvas().save();
        install(&mut renderer);
        assert_eq!(renderer.clip_image_usage().unwrap(), (44800, 44800));
        renderer.surface.canvas().restore();
        assert_eq!(renderer.clip_image_usage().unwrap(), (22400, 22400));
        renderer.surface.canvas().restore();
        assert_eq!(renderer.clip_image_usage().unwrap(), (0, 0));
    }
    renderer.raster_path_budget = 1_000_000;
    renderer.surface.canvas().save();
    install(&mut renderer);
    renderer
        .begin_group(pdf::page::transparency::GroupPaint {
            isolated: true,
            knockout: false,
            alpha: 1.,
            blend_mode: pdf::page::general_state::BlendMode::Normal,
            clip: Default::default(),
            soft_mask: None,
        })
        .unwrap();
    install(&mut renderer);
    assert_eq!(renderer.clip_image_usage().unwrap(), (44800, 44800));
    renderer.end_group(false).unwrap();
    assert_eq!(renderer.clip_image_usage().unwrap(), (22400, 22400));
    renderer.surface.canvas().restore();
    assert_eq!(renderer.clip_image_usage().unwrap(), (0, 0));
    let original = renderer.to_rgba().unwrap().pixels;
    renderer.limits.max_render_bytes = 160 * 140 * 4;
    assert!(matches!(
        renderer.canonical_clip_image(&path),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(renderer.to_rgba().unwrap().pixels, original);
    assert_eq!(renderer.clip_image_usage().unwrap(), (0, 0));
    renderer.limits = Limits::default();
    renderer.raster_path_budget = 1;
    assert!(matches!(
        renderer.canonical_clip_image(&path),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(renderer.to_rgba().unwrap().pixels, original);
    assert_eq!(renderer.clip_image_usage().unwrap(), (0, 0));
}

#[test]
fn region_stitch_preserves_curved_clip_coverage() {
    let doc = Document::new().unwrap();
    doc.add_page_with_builder(
        0,
        PageBuilder::new().size(160., 140.).content(
            b"q 1 0.2 0.1 1 0.3 0.7 cm 10 50 m 10 5 120 5 120 50 c 120 100 10 100 10 50 c W n 0 0 1 rg 0 0 150 120 re f Q".to_vec(),
        ),
    ).unwrap();
    for (page, content) in [
        (1, b"q 1 0.2 0.1 1 0.3 0.7 cm 10.2 50.7 m 50.3 10.2 l 125.7 50.3 l 70.8 110.1 l h W n 0 0 1 rg 0 0 150 120 re f Q".as_slice()),
        (2, b"q 1 0.2 0.1 1 0.3 0.7 cm 12.2 10.7 100.5 80.4 re W n 0 0 1 rg 0 0 150 120 re f Q".as_slice()),
        (3, b"q 1 0.2 0.1 1 0.3 0.7 cm 0 0 1 rg 10.2 50.7 m 50.3 10.2 l 125.7 50.3 l 70.8 110.1 l h f Q".as_slice()),
        (4, b"q 1 0.2 0.1 1 0.3 0.7 cm 0 0 1 rg 12.2 10.7 100.5 80.4 re f Q".as_slice()),
    ] {
        doc.add_page_with_builder(page, PageBuilder::new().size(160., 140.).content(content.to_vec())).unwrap();
    }
    for page in 0..5 {
        let list = doc.get_page(page).unwrap().display_list().unwrap();
        for dpi in [72., 96., 144.] {
            stitched(&list, page, dpi, dpi);
        }
    }
    let text = super::text_clip_tests::document(
        "q 1 0.2 0.1 1 0.3 0.7 cm BT /FONT 20 Tf 7 Tr 1 0 0 1 5 5 Tm (A) Tj ET 1 0 0 rg 0 0 40 40 re f Q",
    );
    let list = text.get_page(0).unwrap().display_list().unwrap();
    for dpi in [72., 96., 144.] {
        stitched(&list, 5, dpi, dpi);
    }
}

#[test]
fn region_stitch_preserves_glyph_cache_phase_and_large_outline_fallback() {
    let doc = Document::new().unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(160., 220.))
        .unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, pdf::document::resource_manager::StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("BT /{font} 200 Tf 1 0 0 1 0.124999 20 Tm (A) Tj ET").into_bytes(),
    )
    .unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    for dpi in [72., 96., 144.] {
        stitched(&list, 0, dpi, dpi);
    }
}

#[test]
fn region_stitch_preserves_curved_stroke_and_fill_stroke_text() {
    let doc = Document::new().unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(160., 80.))
        .unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, pdf::document::resource_manager::StandardFont::Helvetica)
        .unwrap();
    doc.replace_page_contents(0,format!("q 1 0.2 0.1 1 0.3 0.7 cm 0.5 w BT /{font} 14 Tf 1 Tr 1 0 0 1 25.3 22.7 Tm (OS) Tj ET 0 0 1 RG BT /{font} 14 Tf 2 Tr 1 0 0 1 70.3 22.7 Tm (OS) Tj ET Q").into_bytes()).unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    for dpi in [72., 96., 144.] {
        stitched(&list, 0, dpi, dpi);
    }
}

#[test]
fn region_allocation_fits_budget_when_full_page_cannot_and_invalid_updates_are_atomic() {
    let doc = Document::new().unwrap();
    doc.add_page_with_builder(
        0,
        PageBuilder::new()
            .size(10000., 10000.)
            .content(b"1 0 0 rg 0 0 10000 10000 re f".to_vec()),
    )
    .unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let mut renderer = SkiaRender::with_limits(
        72.,
        72.,
        Limits {
            max_render_pixels: 1200,
            max_render_bytes: 4800,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        list.replay(&mut renderer),
        Err(PdfError::ResourceLimit(_))
    ));
    let region = RasterRegion {
        x: 9000,
        y: 8000,
        width: 32,
        height: 24,
    };
    renderer.set_region(Some(region)).unwrap();
    list.replay(&mut renderer).unwrap();
    let pixels = renderer.to_rgba().unwrap();
    assert_eq!(pixels.pixels.len(), 3072);
    assert!(pixels.pixels.chunks_exact(4).all(|p| p == [255, 0, 0, 255]));
    for bad in [
        RasterRegion { width: 0, ..region },
        RasterRegion {
            x: u32::MAX,
            ..region
        },
    ] {
        assert!(renderer.set_region(Some(bad)).is_err());
        assert_eq!(renderer.requested_region, Some(region));
    }
    renderer
        .set_region(Some(RasterRegion { x: 9999, ..region }))
        .unwrap();
    assert!(list.replay(&mut renderer).is_err());
    renderer.set_region(Some(region)).unwrap();
    list.replay(&mut renderer).unwrap();
    assert_eq!(renderer.to_rgba().unwrap().pixels, pixels.pixels);
}
