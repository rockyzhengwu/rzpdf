//! Batch ordinary bitmap glyphs only when their independently snapped origins
//! can be represented exactly under one shared canvas matrix.
use super::*;

impl SkiaRender {
    /// FreeType can return empty hinted metrics after a font's bytecode fails.
    /// Preserve legitimate empty glyphs, and retry only when unhinted metrics
    /// prove that visible geometry was lost. Scratch is bounded independently
    /// of the text run length. CoreText/DirectWrite retain their own policy.
    pub(super) fn recover_empty_hinted_glyphs(
        font: &skia_safe::Font,
        glyphs: &[u16],
        global: &Matrix,
    ) -> Option<skia_safe::Font> {
        if cfg!(any(target_vendor = "apple", target_os = "windows")) {
            return None;
        }
        let scale = global
            .a()
            .hypot(global.b())
            .max(global.c().hypot(global.d()));
        let size = font.size() * scale;
        if !size.is_finite() || size <= 0. || size > 256. {
            return None;
        }
        let mut hinted = font.clone();
        hinted.set_size(size);
        let mut plain = hinted.clone();
        plain.set_hinting(skia_safe::FontHinting::None);
        let mut bounds = [skia_safe::Rect::default(); 128];
        for chunk in glyphs.chunks(bounds.len()) {
            hinted.get_bounds(chunk, &mut bounds[..chunk.len()], None);
            for (&glyph, bounds) in chunk.iter().zip(&bounds) {
                if bounds.is_empty() {
                    let mut retry = [skia_safe::Rect::default()];
                    plain.get_bounds(&[glyph], &mut retry, None);
                    if !retry[0].is_empty() {
                        let mut recovered = font.clone();
                        recovered.set_hinting(skia_safe::FontHinting::None);
                        return Some(recovered);
                    }
                }
            }
        }
        None
    }

    pub(super) fn try_draw_glyph_run(
        &mut self,
        font: &skia_safe::Font,
        glyphs: &[u16],
        positions: &[skia_safe::Point],
        paint: &Paint,
        global: &Matrix,
    ) -> bool {
        fn power_of_two(value: f32) -> bool {
            let bits = value.abs().to_bits();
            bits & 0x7fffff == 0 && bits >> 23 != 0 && bits >> 23 != 255
        }
        let (a, b, c, d) = (global.a(), global.b(), global.c(), global.d());
        if glyphs.len() < 2
            || glyphs.len() != positions.len()
            || b != 0.
            || c != 0.
            || !a.is_finite()
            || !d.is_finite()
            || a == 0.
            || d == 0.
            || paint.shader().is_some()
            || paint.alpha() != 255
            || paint.as_blend_mode() != Some(skia_safe::BlendMode::SrcOver)
            || paint.style() != PaintStyle::Fill
        {
            return false;
        }
        let size = font.size();
        let xx = a * size * font.scale_x();
        let xy = a * size * font.skew_x();
        let yy = d * size;
        // Large glyphs need canonical path coverage; do not bypass that policy.
        if !xx.is_finite()
            || !xy.is_finite()
            || !yy.is_finite()
            || xx * xx > 256. * 256.
            || xy * xy + yy * yy > 256. * 256.
        {
            return false;
        }
        let Some(bytes) = positions
            .len()
            .checked_mul(std::mem::size_of::<skia_safe::Point>())
        else {
            return false;
        };
        // Cover both position arrays and glyph IDs in this optional scratch allowance.
        if bytes > self.limits.max_operation_bytes / 3 {
            return false;
        }
        let origin = self.region.map_or((0, 0), |r| (r.x, r.y));
        if origin.0 > 32768 || origin.1 > 32768 {
            return false;
        }
        let binary_scale = power_of_two(a) && power_of_two(d);
        let base = if binary_scale {
            (0., 0.)
        } else {
            let Ok(base) = Self::glyph_device_origin(global, positions[0]) else {
                return false;
            };
            base
        };
        let translation = (base.0 - origin.0 as f32, base.1 - origin.1 as f32);
        fn local(value: f32, base: f32, origin: u32, scale: f32, binary: bool) -> Option<f32> {
            let position = ((f64::from(value) - f64::from(base)) / f64::from(scale)) as f32;
            if !position.is_finite() {
                return None;
            }
            if binary {
                return (position * scale == value).then_some(position);
            }
            let expected = value - origin as f32;
            let translate = base - origin as f32;
            // Native Skia ports can use separate arithmetic or fused multiply
            // add. Accept a shared origin only when both reconstruct exactly
            // the same device position as the per-glyph path. Neighbor floats
            // handle a division rounding to the other side of a valid preimage.
            [position, position.next_down(), position.next_up()]
                .into_iter()
                .find(|p| {
                    p.is_finite()
                        && scale * *p + translate == expected
                        && scale.mul_add(*p, translate) == expected
                })
        }
        let mut snapped = Vec::new();
        if snapped.try_reserve_exact(positions.len()).is_err() {
            return false;
        }
        for &position in positions {
            let Ok((x, y)) = Self::glyph_device_origin(global, position) else {
                return false;
            };
            if x.abs() > 32768. || y.abs() > 32768. {
                return false;
            }
            let Some(sx) = local(x, base.0, origin.0, a, binary_scale) else {
                return false;
            };
            let Some(sy) = local(y, base.1, origin.1, d, binary_scale) else {
                return false;
            };
            snapped.push(skia_safe::Point::new(sx, sy));
        }
        #[cfg(feature = "render-diagnostics")]
        let _span = self.profile_span(crate::render_profile::RenderPhase::GlyphBatch);
        let transform = self.pdf_to_skia_m44(a, b, c, d, translation.0, translation.1);
        self.surface.canvas().set_matrix(&transform);
        self.surface
            .canvas()
            .draw_glyphs_at(glyphs, &snapped[..], (0, 0), font, paint);
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snapped_runs_match_independent_glyphs_at_fractional_and_overlapping_origins() {
        let face = crate::memory_font_manager::new()
            .new_from_data(
                include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf"),
                0,
            )
            .unwrap();
        let mut font = skia_safe::Font::new(face, 12.);
        font.set_subpixel(true)
            .set_edging(skia_safe::font::Edging::SubpixelAntiAlias);
        let glyphs: Vec<_> = "Hooo"
            .chars()
            .map(|c| font.unichar_to_glyph(c as i32))
            .collect();
        assert!(glyphs.iter().all(|g| *g != 0));
        let positions = [
            skia_safe::Point::new(3.123, 16.126),
            skia_safe::Point::new(3.123, 16.126),
            skia_safe::Point::new(12.999, 16.126),
            skia_safe::Point::new(21.875, 16.126),
        ];
        for (a, d, e, f) in [
            (1., 1., 0.125, 0.375),
            (2., 2., -0.375, 0.125),
            (-1., 1., 64.125, 0.25),
            (1., -1., 0.125, 48.375),
            (0.5, 0.5, 0.75, 0.125),
            (1.5, -1.5, 0.125, 48.375),
            (1.3333334, -1.3333334, 0.125, 48.375),
        ] {
            let matrix = Matrix::new(a, 0., 0., d, e, f);
            let mut batched = SkiaRender::new(72., 72.).unwrap();
            let mut single = SkiaRender::new(72., 72.).unwrap();
            batched.start_page(96., 64.).unwrap();
            single.start_page(96., 64.).unwrap();
            let mut paint = Paint::default();
            paint.set_color(Color::BLACK).set_anti_alias(true);
            assert!(batched.try_draw_glyph_run(&font, &glyphs, &positions, &paint, &matrix));
            for (&glyph, &position) in glyphs.iter().zip(&positions) {
                single
                    .draw_filled_glyph(&font, glyph, position, &paint, &matrix)
                    .unwrap();
            }
            assert_eq!(
                batched.to_rgba().unwrap().pixels,
                single.to_rgba().unwrap().pixels,
                "matrix {a},{d},{e},{f}"
            );
        }
    }
    #[test]
    fn unsupported_runs_fall_back_without_painting() {
        let face = crate::memory_font_manager::new()
            .new_from_data(
                include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf"),
                0,
            )
            .unwrap();
        let font = skia_safe::Font::new(face, 12.);
        let glyphs = [font.unichar_to_glyph('H' as i32); 2];
        let positions = [skia_safe::Point::new(3., 16.); 2];
        let mut renderer = SkiaRender::new(72., 72.).unwrap();
        renderer.start_page(32., 32.).unwrap();
        let before = renderer.to_rgba().unwrap().pixels;
        let mut paint = Paint::default();
        for matrix in [
            Matrix::new(0., 0., 0., 1., 0., 0.),
            Matrix::new(0., 1., -1., 0., 0., 0.),
            Matrix::new(1., 0.2, 0., 1., 0., 0.),
            Matrix::new(1., 0., 0., 1., 40000., 0.),
        ] {
            assert!(!renderer.try_draw_glyph_run(&font, &glyphs, &positions, &paint, &matrix));
        }
        let matrix = Matrix::default();
        paint.set_alpha(127);
        assert!(!renderer.try_draw_glyph_run(&font, &glyphs, &positions, &paint, &matrix));
        paint
            .set_alpha(255)
            .set_blend_mode(skia_safe::BlendMode::Multiply);
        assert!(!renderer.try_draw_glyph_run(&font, &glyphs, &positions, &paint, &matrix));
        paint.set_blend_mode(skia_safe::BlendMode::SrcOver);
        let mut large = font.clone();
        large.set_size(512.);
        assert!(!renderer.try_draw_glyph_run(&large, &glyphs, &positions, &paint, &matrix));
        assert_eq!(before, renderer.to_rgba().unwrap().pixels);
    }
}
