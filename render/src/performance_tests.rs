use super::*;

#[test]
fn dense_mixed_alpha_prefers_simd_and_sparse_background_prefers_integer() {
    let mut renderer = SkiaRender::new(72., 72.).unwrap();
    renderer.start_page(32., 32.).unwrap();
    assert!(renderer.prefer_integer_white_background());
    let mut paint = Paint::default();
    paint.set_color(Color::BLACK).set_alpha(127);
    renderer.surface.canvas().draw_paint(&paint);
    assert!(!renderer.prefer_integer_white_background());
}

#[test]
fn integer_white_substrate_matches_skia_for_all_alphas_and_keeps_snapshots() {
    let mut renderer = SkiaRender::new(72., 72.).unwrap();
    renderer.start_page(256., 4.).unwrap();
    renderer
        .surface
        .notify_content_will_change(skia_safe::surface::ContentChangeMode::Retain);
    {
        let mut pixmap = renderer.surface.peek_pixels().unwrap();
        let stride = pixmap.row_bytes();
        let pixels = pixmap.bytes_mut().unwrap();
        for y in 0..4 {
            for alpha in 0..256 {
                let a = alpha as u8;
                pixels[y * stride + alpha * 4..y * stride + alpha * 4 + 4].copy_from_slice(&[
                    a / 2,
                    a / 3,
                    a.saturating_sub(y as u8 * 31),
                    a,
                ]);
            }
        }
    }
    let original = renderer.surface.image_snapshot();
    let mut reference = surfaces::raster_n32_premul((256, 4)).unwrap();
    reference.canvas().draw_image(&original, (0, 0), None);
    let mut paint = Paint::default();
    paint
        .set_color(Color::WHITE)
        .set_blend_mode(skia_safe::BlendMode::DstOver);
    reference.canvas().draw_paint(&paint);
    let before = original.peek_pixels().unwrap().bytes().unwrap().to_vec();
    assert!(renderer.finish_white_background());
    assert_eq!(before, original.peek_pixels().unwrap().bytes().unwrap());
    assert_eq!(
        renderer.surface.peek_pixels().unwrap().bytes().unwrap(),
        reference.peek_pixels().unwrap().bytes().unwrap()
    );
}

#[test]
fn white_substrate_keeps_partial_clip_and_nonwhite_fallbacks() {
    let mut renderer = SkiaRender::new(72., 72.).unwrap();
    renderer.start_page(10., 10.).unwrap();
    renderer
        .surface
        .canvas()
        .clip_rect(skia_safe::Rect::from_wh(5., 5.), None, false);
    assert!(!renderer.finish_white_background());
    renderer.end_page().unwrap();
    let image = renderer.to_rgba().unwrap();
    assert_eq!(&image.pixels[..4], &[255; 4]);
    assert_eq!(&image.pixels[9 * 4..10 * 4], &[0; 4]);
    renderer.start_page(10., 10.).unwrap();
    renderer.clear_color = Color::TRANSPARENT;
    assert!(!renderer.finish_white_background());
}
