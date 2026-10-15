use super::*;
use pdf::page::transparency::{SoftMaskKind, SoftMaskPaint};
impl SkiaRender {
    pub(super) fn apply_soft_mask(
        &mut self,
        mask: Option<&SoftMaskPaint>,
        ctm: &Matrix,
    ) -> PdfResult<()> {
        if self.shape_mode {
            return Ok(());
        }
        let Some(mask) = mask else {
            return Ok(());
        };
        let pixels = self.page_width as u64 * self.page_height as u64;
        let bytes = usize::try_from(pixels)
            .ok()
            .and_then(|v| v.checked_mul(4))
            .ok_or_else(|| PdfError::ResourceLimit("soft mask render bytes".into()))?;
        let held = self
            .held_pattern_surfaces
            .checked_add(3 + u64::from(self.paint.shader().is_some()))
            .ok_or_else(|| PdfError::ResourceLimit("soft mask surface count".into()))?;
        let reserve = bytes
            .checked_mul(held as usize)
            .ok_or_else(|| PdfError::ResourceLimit("soft mask aggregate bytes".into()))?;
        let mut limits = self.limits;
        limits.max_render_bytes = limits
            .max_render_bytes
            .checked_sub(reserve)
            .ok_or_else(|| PdfError::ResourceLimit("soft mask aggregate bytes".into()))?;
        limits.max_render_pixels = limits
            .max_render_pixels
            .checked_sub(
                pixels
                    .checked_mul(held)
                    .ok_or_else(|| PdfError::ResourceLimit("soft mask aggregate pixels".into()))?,
            )
            .ok_or_else(|| PdfError::ResourceLimit("soft mask aggregate pixels".into()))?;
        let mut renderer = Self::with_limits(self.scale_x * 72., self.scale_y * 72., limits)?;
        // Supported Luminosity groups declare RGB and are isolated. Evaluate
        // their members on transparent, then composite the group over BC below;
        // BC must not affect a member's internal Multiply/Screen operation.
        renderer.clear_color = Color::TRANSPARENT;
        mask.list.replay(&mut renderer)?;
        let source = renderer
            .surface
            .peek_pixels()
            .ok_or_else(|| PdfError::ResourceLimit("soft mask raster access".into()))?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(bytes)
            .map_err(|_| PdfError::ResourceLimit("soft mask alpha buffer".into()))?;
        output.resize(bytes, 255u8);
        let background =
            0.3 * mask.background[0] + 0.59 * mask.background[1] + 0.11 * mask.background[2];
        for y in 0..self.page_height {
            for x in 0..self.page_width {
                let color = source.get_color((x, y));
                let alpha = f32::from(color.a()) / 255.;
                let mut value = match mask.kind {
                    SoftMaskKind::Alpha => alpha,
                    SoftMaskKind::Luminosity => {
                        let luminosity = (0.3 * f32::from(color.r())
                            + 0.59 * f32::from(color.g())
                            + 0.11 * f32::from(color.b()))
                            / 255.;
                        alpha * luminosity + (1. - alpha) * background
                    }
                };
                if let Some(transfer) = &mask.transfer {
                    let mapped = transfer.eval(&[value])?;
                    value = *mapped
                        .first()
                        .filter(|v| mapped.len() == 1 && v.is_finite())
                        .ok_or_else(|| {
                            PdfError::FunctionError("soft mask transfer result".into())
                        })?;
                }
                output[(y as usize * self.page_width as usize + x as usize) * 4 + 3] =
                    (value.clamp(0., 1.) * 255.).round() as u8;
            }
        }
        let info = skia_safe::ImageInfo::new(
            (self.page_width, self.page_height),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        let image = skia_safe::images::raster_from_data(
            &info,
            skia_safe::Data::new_copy(&output),
            self.page_width as usize * 4,
        )
        .ok_or_else(|| PdfError::ResourceLimit("soft mask image".into()))?;
        let inverse = ctm.checked_inverse()?;
        let matrix = skia_safe::Matrix::new_all(
            inverse.a(),
            inverse.c(),
            inverse.e(),
            inverse.b(),
            inverse.d(),
            inverse.f(),
            0.,
            0.,
            1.,
        );
        let mask = image
            .to_shader(
                (skia_safe::TileMode::Decal, skia_safe::TileMode::Decal),
                skia_safe::SamplingOptions::default(),
                &matrix,
            )
            .ok_or_else(|| PdfError::ResourceLimit("soft mask shader".into()))?;
        let source = self
            .paint
            .shader()
            .unwrap_or_else(|| skia_safe::shaders::color(self.paint.color().with_a(255)));
        let alpha = self.paint.alpha_f();
        self.paint.set_color(Color::WHITE);
        self.paint.set_alpha_f(alpha);
        self.paint.set_shader(skia_safe::shaders::blend(
            skia_safe::BlendMode::DstIn,
            source,
            mask,
        ));
        Ok(())
    }
}
