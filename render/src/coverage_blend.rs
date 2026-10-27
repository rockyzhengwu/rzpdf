//! Solid canonical coverage on an untagged N32 raster target. This is the
//! integer SrcOver rule used by Skia's solid rectangle blitter; complex clips,
//! effects and other target formats stay on the canvas path.
use super::coverage_spans::CoverageRect;
use skia_safe::{AlphaType, BlendMode, ColorType, IRect, Paint, PaintStyle, Surface};

pub(super) struct Blitter<'a> {
    pixels: &'a mut [u8],
    stride: usize,
    clip: IRect,
    origin: (i32, i32),
    color: [u8; 4],
}

/// Calls `emit` only when integer blending is supported. The caller supplies
/// the canonical cell coordinates and uses the canvas fallback on `false`.
pub(super) fn draw(
    surface: &mut Surface,
    paint: &Paint,
    origin: (i32, i32),
    emit: impl FnOnce(&mut Blitter<'_>),
) -> bool {
    if paint.style() != PaintStyle::Fill
        || paint.as_blend_mode() != Some(BlendMode::SrcOver)
        || paint.shader().is_some()
        || paint.color_filter().is_some()
        || paint.mask_filter().is_some()
        || paint.image_filter().is_some()
        || paint.path_effect().is_some()
        || paint.is_dither()
        || !surface.canvas().is_clip_rect()
        || surface.canvas().local_to_device_as_3x3()
            != skia_safe::Matrix::translate((-(origin.0 as f32), -(origin.1 as f32)))
    {
        return false;
    }
    let Some(clip) = surface.canvas().device_clip_bounds() else {
        return false;
    };
    // All engine targets are root raster surfaces (groups use separate
    // surfaces, not saveLayer). Notify before borrowing to preserve snapshots.
    surface.notify_content_will_change(skia_safe::surface::ContentChangeMode::Retain);
    let Some(mut pixmap) = surface.peek_pixels() else {
        return false;
    };
    if pixmap.color_type() != ColorType::N32
        || pixmap.alpha_type() != AlphaType::Premul
        || pixmap.color_space().is_some()
        // The canvas fallback positions these rectangles with f32. Keep all
        // global cell coordinates exactly representable before replacing it
        // with integer addressing, even for caller-configured huge pages.
        || i64::from(origin.0).abs() + i64::from(pixmap.width()) + 68 > (1 << 24)
        || i64::from(origin.1).abs() + i64::from(pixmap.height()) + 68 > (1 << 24)
    {
        return false;
    }
    // Paint::color() in skia-safe 0.93 truncates floats, while Skia's native
    // solid blitter rounds. Use the native RGBA conversion for the same bytes.
    let [r, g, b, _] = paint.color4f().to_bytes().to_ne_bytes();
    let color = match pixmap.color_type() {
        ColorType::RGBA8888 => [r, g, b, paint.alpha()],
        ColorType::BGRA8888 => [b, g, r, paint.alpha()],
        _ => return false,
    };
    let stride = pixmap.row_bytes();
    let Some(pixels) = pixmap.bytes_mut() else {
        return false;
    };
    emit(&mut Blitter {
        pixels,
        stride,
        clip,
        origin,
        color,
    });
    true
}

impl Blitter<'_> {
    pub(super) fn rect(&mut self, x: i32, y: i32, rect: CoverageRect) {
        let left = (x - 2 + i32::from(rect.left) - self.origin.0).max(self.clip.left);
        let right = (x - 2 + i32::from(rect.right) - self.origin.0).min(self.clip.right);
        let top = (y - 2 + i32::from(rect.top) - self.origin.1).max(self.clip.top);
        let bottom = (y - 2 + i32::from(rect.bottom) - self.origin.1).min(self.clip.bottom);
        let alpha = (u16::from(self.color[3]) * u16::from(rect.coverage) + 127) / 255;
        if left >= right || top >= bottom || alpha == 0 {
            return;
        }
        let mut source = [0; 4];
        for (dst, color) in source[..3].iter_mut().zip(self.color) {
            *dst = ((u16::from(color) * alpha + 127) / 255) as u8;
        }
        source[3] = alpha as u8;
        let scale = 256 - alpha;
        for row in top..bottom {
            let start = row as usize * self.stride + left as usize * 4;
            let end = start + (right - left) as usize * 4;
            for pixel in self.pixels[start..end].as_chunks_mut::<4>().0 {
                if alpha == 255 {
                    *pixel = source;
                } else {
                    for (channel, src) in pixel.iter_mut().zip(source) {
                        *channel = (u16::from(src) + ((u16::from(*channel) * scale) >> 8)) as u8;
                    }
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use skia_safe::{Color, ImageInfo, Rect, surfaces};

    fn surface(kind: ColorType) -> Surface {
        let info = ImageInfo::new((768, 256), kind, AlphaType::Premul, None);
        let mut surface = surfaces::raster(&info, None, None).unwrap();
        let mut pixels = surface.peek_pixels().unwrap();
        let stride = pixels.row_bytes();
        for (y, row) in pixels.bytes_mut().unwrap().chunks_mut(stride).enumerate() {
            for (x, pixel) in row.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let a = ((x + y * 19) % 256) as u8;
                *pixel = [a / 3, a / 2, a, a];
            }
        }
        drop(pixels);
        surface
    }

    #[test]
    fn every_alpha_and_coverage_match_native_skia_with_snapshots_and_other_order_falls_back() {
        const WIDTHS: [u8; 8] = [1, 2, 3, 4, 5, 16, 63, 64];
        for kind in [ColorType::RGBA8888, ColorType::BGRA8888] {
            let mut actual = surface(kind);
            if kind != ColorType::N32 {
                assert!(!draw(&mut actual, &Paint::default(), (0, 0), |_| panic!(
                    "non-native order must use Skia"
                )));
                continue;
            }
            let mut expected = surface(kind);
            let snapshot = actual.image_snapshot();
            let original = snapshot.peek_pixels().unwrap().bytes().unwrap().to_vec();
            let mut paint = Paint::default();
            paint.set_color4f(skia_safe::Color4f::new(0.157, 0.203, 0.601, 1.), None);
            for alpha in 0..=255 {
                paint.set_alpha(alpha);
                assert!(draw(&mut actual, &paint, (0, 0), |blitter| {
                    for coverage in 0..=255 {
                        blitter.rect(
                            i32::from(coverage) * 3,
                            i32::from(alpha),
                            CoverageRect {
                                left: 2,
                                right: 2 + WIDTHS[coverage as usize % WIDTHS.len()],
                                top: 2,
                                bottom: 3,
                                coverage,
                            },
                        );
                    }
                }));
                for coverage in 0..=255u8 {
                    let mut span = paint.clone();
                    span.set_alpha(((u16::from(alpha) * u16::from(coverage) + 127) / 255) as u8);
                    expected.canvas().draw_rect(
                        Rect::from_xywh(
                            f32::from(coverage) * 3.,
                            f32::from(alpha),
                            f32::from(WIDTHS[coverage as usize % WIDTHS.len()]),
                            1.,
                        ),
                        &span,
                    );
                }
            }
            let a = actual.peek_pixels().unwrap();
            let b = expected.peek_pixels().unwrap();
            let mismatch = a
                .bytes()
                .unwrap()
                .iter()
                .zip(b.bytes().unwrap())
                .enumerate()
                .find(|(_, (a, b))| a != b);
            assert!(mismatch.is_none(), "{kind:?}: first mismatch {mismatch:?}");
            assert_eq!(snapshot.peek_pixels().unwrap().bytes().unwrap(), original);
        }
    }

    #[test]
    fn translated_cells_respect_hard_clip_and_reject_fractional_aa_clip() {
        let mut actual = surface(ColorType::N32);
        let mut expected = surface(ColorType::N32);
        let origin = (37, 51);
        let clip = Rect::from_xywh(4., 5., 20., 30.);
        let mut paint = Paint::default();
        paint.set_color(Color::from_argb(151, 20, 83, 211));
        for surface in [&mut actual, &mut expected] {
            surface.canvas().clip_rect(clip, None, false);
            surface.canvas().translate((-37., -51.));
        }
        let r = CoverageRect {
            left: 2,
            right: 66,
            top: 2,
            bottom: 66,
            coverage: 117,
        };
        assert!(draw(&mut actual, &paint, origin, |blitter| blitter.rect(0, 0, r)));
        paint.set_alpha(((151u16 * 117 + 127) / 255) as u8);
        expected.canvas().draw_rect(Rect::from_wh(64., 64.), &paint);
        assert_eq!(
            actual.peek_pixels().unwrap().bytes().unwrap(),
            expected.peek_pixels().unwrap().bytes().unwrap()
        );

        actual.canvas().reset_matrix();
        actual
            .canvas()
            .clip_rect(Rect::from_xywh(4.5, 5.5, 8., 9.), None, true);
        actual.canvas().translate((-37., -51.));
        assert!(!draw(&mut actual, &paint, origin, |_| panic!(
            "AA clip must use Skia"
        )));
    }
}
