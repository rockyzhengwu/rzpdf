//! PDF knockout compositing. Shape is collected separately from opacity (AIS is
//! not supported); every member sees the group's initial, not running backdrop.
use super::*;
impl SkiaRender {
    pub(super) fn needs_combined_group(&self, state: &GraphicState) -> bool {
        !self.shape_mode
            && (state.general_state.fill_alpha != 1.
                || state.general_state.stroke_alpha != 1.
                || state.general_state.blend_mode != pdf::page::general_state::BlendMode::Normal
                || state.general_state.soft_mask.is_some())
    }
    pub(super) fn combined_group(
        &mut self,
        state: &GraphicState,
        draw: impl FnOnce(&mut Self) -> PdfResult<()>,
    ) -> PdfResult<()> {
        self.begin_group(pdf::page::transparency::GroupPaint {
            isolated: false,
            knockout: true,
            alpha: 1.,
            blend_mode: pdf::page::general_state::BlendMode::Normal,
            clip: state.clip_path.clone(),
            soft_mask: None,
        })?;
        let dispatch = self.object_dispatch;
        self.object_dispatch = false;
        let result = draw(self);
        self.object_dispatch = dispatch;
        let end = self.end_group(result.is_ok());
        result.and(end)
    }
    pub(super) fn effective_blend_mode(
        &self,
        mode: pdf::page::general_state::BlendMode,
    ) -> skia_safe::BlendMode {
        if self.shape_mode {
            skia_safe::BlendMode::SrcOver
        } else {
            Self::skia_blend_mode(mode)
        }
    }
    pub(super) fn empty_object_surface(&self) -> PdfResult<Surface> {
        let mut surface = surfaces::raster_n32_premul((self.page_width, self.page_height))
            .ok_or_else(|| PdfError::ResourceLimit("knockout object surface".into()))?;
        surface.canvas().clear(Color::TRANSPARENT);
        Ok(surface)
    }
    pub(super) fn dispatch_group_object(
        &mut self,
        draw: impl Fn(&mut Self) -> PdfResult<()>,
    ) -> PdfResult<bool> {
        if self.object_dispatch
            || self.shape_mode
            || !self.group_stack.last().is_some_and(|g| g.shape.is_some())
        {
            return Ok(false);
        }
        let knockout = self.group_stack.last().unwrap().paint.knockout;
        let bytes = (self.page_width as usize)
            .checked_mul(self.page_height as usize)
            .and_then(|v| v.checked_mul(4))
            .ok_or_else(|| PdfError::ResourceLimit("knockout bytes".into()))?;
        let old_limits = self.limits;
        // Two member rasters, possible independent opacity raster, output Vec
        // and native copy; accumulated group rasters were charged at begin.
        let count = if knockout { 5usize } else { 1 };
        let reserve = bytes
            .checked_mul(count)
            .ok_or_else(|| PdfError::ResourceLimit("knockout peak bytes".into()))?;
        let mut limits = old_limits;
        limits.max_render_bytes = limits
            .max_render_bytes
            .checked_sub(reserve)
            .filter(|v| *v >= bytes)
            .ok_or_else(|| PdfError::ResourceLimit("knockout peak bytes".into()))?;
        let pixels = self.page_width as u64 * self.page_height as u64;
        limits.max_render_pixels = limits
            .max_render_pixels
            .checked_sub(pixels * count as u64)
            .filter(|v| *v >= pixels)
            .ok_or_else(|| PdfError::ResourceLimit("knockout peak pixels".into()))?;
        let shape = self.empty_object_surface()?;
        let previous = std::mem::replace(&mut self.surface, shape);
        self.shape_mode = true;
        self.object_dispatch = true;
        self.limits = limits;
        let result = draw(self);
        self.shape_mode = false;
        self.object_dispatch = false;
        self.limits = old_limits;
        let mut shape = std::mem::replace(&mut self.surface, previous);
        result?;
        if knockout {
            let mut member = self.empty_object_surface()?;
            let group = self.group_stack.last_mut().unwrap();
            if let Some(initial) = &group.initial {
                member.canvas().draw_image(initial, (0., 0.), None);
            }
            let opacity = if group.coverage.is_some() {
                Some(self.empty_object_surface()?)
            } else {
                None
            };
            let old_opacity =
                std::mem::replace(&mut self.group_stack.last_mut().unwrap().coverage, opacity);
            let previous = std::mem::replace(&mut self.surface, member);
            self.object_dispatch = true;
            self.limits = limits;
            let result = draw(self);
            self.object_dispatch = false;
            self.limits = old_limits;
            let mut member = std::mem::replace(&mut self.surface, previous);
            let mut member_opacity = std::mem::replace(
                &mut self.group_stack.last_mut().unwrap().coverage,
                old_opacity,
            );
            result?;
            let group = self.group_stack.last_mut().unwrap();
            let initial = group.initial.as_ref();
            Self::merge_knockout(&mut self.surface, &mut member, &mut shape, initial)?;
            if let (Some(destination), Some(source)) =
                (group.coverage.as_mut(), member_opacity.as_mut())
            {
                Self::merge_knockout(destination, source, &mut shape, None)?;
            }
        } else {
            self.object_dispatch = true;
            self.limits = limits;
            let result = draw(self);
            self.object_dispatch = false;
            self.limits = old_limits;
            result?;
        }
        let image = shape.image_snapshot();
        self.group_stack
            .last_mut()
            .unwrap()
            .shape
            .as_mut()
            .unwrap()
            .canvas()
            .draw_image(&image, (0., 0.), None);
        Ok(true)
    }

    fn merge_knockout(
        destination: &mut Surface,
        source: &mut Surface,
        shape: &mut Surface,
        initial: Option<&skia_safe::Image>,
    ) -> PdfResult<()> {
        if cfg!(target_endian = "big") {
            return Err(PdfError::UnsupportedFeature(
                "knockout native big-endian pixels".into(),
            ));
        }
        let info = destination.image_info();
        let mut output = Vec::new();
        let length = info
            .min_row_bytes()
            .checked_mul(info.height() as usize)
            .ok_or_else(|| PdfError::ResourceLimit("knockout output extent".into()))?;
        output
            .try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("knockout output bytes".into()))?;
        output.resize(length, 0);
        {
            let dst = destination
                .peek_pixels()
                .ok_or_else(|| PdfError::ResourceLimit("knockout destination pixels".into()))?;
            let src = source
                .peek_pixels()
                .ok_or_else(|| PdfError::ResourceLimit("knockout source pixels".into()))?;
            let mask = shape
                .peek_pixels()
                .ok_or_else(|| PdfError::ResourceLimit("knockout shape pixels".into()))?;
            let bg = initial
                .map(|s| {
                    s.peek_pixels()
                        .ok_or_else(|| PdfError::ResourceLimit("knockout initial pixels".into()))
                })
                .transpose()?;
            // Borrow native arrays directly; no full-page readback copies.
            let d = dst
                .bytes()
                .ok_or_else(|| PdfError::ResourceLimit("knockout destination bytes".into()))?;
            let s = src
                .bytes()
                .ok_or_else(|| PdfError::ResourceLimit("knockout source bytes".into()))?;
            let m = mask
                .bytes()
                .ok_or_else(|| PdfError::ResourceLimit("knockout shape bytes".into()))?;
            let b = bg
                .as_ref()
                .map(|p| {
                    p.bytes()
                        .ok_or_else(|| PdfError::ResourceLimit("knockout backdrop bytes".into()))
                })
                .transpose()?;
            for y in 0..info.height() as usize {
                for x in 0..info.width() as usize {
                    let inverse = 1. - f64::from(m[y * mask.row_bytes() + x * 4 + 3]) / 255.;
                    for channel in 0..4 {
                        let initial = b.map_or(0, |b| {
                            b[y * bg.as_ref().unwrap().row_bytes() + x * 4 + channel]
                        });
                        let value = f64::from(s[y * src.row_bytes() + x * 4 + channel])
                            + inverse
                                * (f64::from(d[y * dst.row_bytes() + x * 4 + channel])
                                    - f64::from(initial));
                        output[y * info.min_row_bytes() + x * 4 + channel] =
                            value.round().clamp(0., 255.) as u8;
                    }
                    let offset = y * info.min_row_bytes() + x * 4;
                    let alpha = output[offset + 3];
                    for channel in 0..3 {
                        output[offset + channel] = output[offset + channel].min(alpha);
                    }
                }
            }
        }
        let image = skia_safe::images::raster_from_data(
            &info,
            skia_safe::Data::new_copy(&output),
            info.min_row_bytes(),
        )
        .ok_or_else(|| PdfError::ResourceLimit("knockout merged image".into()))?;
        let mut paint = Paint::default();
        paint.set_blend_mode(skia_safe::BlendMode::Src);
        destination
            .canvas()
            .draw_image(&image, (0., 0.), Some(&paint));
        Ok(())
    }
}
