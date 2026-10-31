//! Device-space targets for local transparency buffers. Page geometry stays in
//! global raster coordinates; every allocation has an explicit origin/extent.
use super::*;

#[derive(Clone)]
pub(super) struct RasterTarget {
    pub width: i32,
    pub height: i32,
    pub region: Option<RasterRegion>,
    matrix: Matrix,
}
impl RasterTarget {
    pub fn origin(&self) -> (i32, i32) {
        self.region.map_or((0, 0), |r| (r.x as i32, r.y as i32))
    }
    pub fn image_target(&self, width: i32, height: i32, offset: (i32, i32)) -> Self {
        let origin = self.origin();
        Self {
            width,
            height,
            region: Some(RasterRegion {
                x: (origin.0 + offset.0) as u32,
                y: (origin.1 + offset.1) as u32,
                width: width as u32,
                height: height as u32,
            }),
            matrix: Matrix::new(
                self.matrix.a(),
                self.matrix.b(),
                self.matrix.c(),
                self.matrix.d(),
                self.matrix.e() - offset.0 as f32,
                self.matrix.f() - offset.1 as f32,
            ),
        }
    }
    pub fn pixels(&self) -> u64 {
        self.width as u64 * self.height as u64
    }
}
impl SkiaRender {
    pub(super) fn raster_target(&self) -> RasterTarget {
        RasterTarget {
            width: self.page_width,
            height: self.page_height,
            region: self.region,
            matrix: self.matrix.clone(),
        }
    }
    pub(super) fn use_target(&mut self, target: &RasterTarget) {
        if self.region != target.region
            || self.page_width != target.width
            || self.page_height != target.height
        {
            // Cached canonical masks are clipped to the active target. Their
            // live backing images remain in the ledger until canvases release them.
            self.clip_cache.clear();
        }
        self.page_width = target.width;
        self.page_height = target.height;
        self.region = target.region;
        self.matrix = target.matrix.clone();
    }
    pub(super) fn group_target(
        &mut self,
        group: &pdf::page::transparency::GroupPaint,
    ) -> PdfResult<RasterTarget> {
        // Shader clips do not tighten Skia's device_clip_bounds(). Derive a
        // conservative geometric intersection before rasterizing any clip;
        // otherwise a rotated small Form still allocates a whole-page group.
        let mut bounds = self
            .surface
            .canvas()
            .device_clip_bounds()
            .map_or([0., 0., 0., 0.], |r| {
                [r.left as f64, r.top as f64, r.right as f64, r.bottom as f64]
            });
        for element in group.clip.elements() {
            let transform = element.matrix.checked_mul(&self.matrix)?;
            let mut clip = [
                f64::INFINITY,
                f64::INFINITY,
                f64::NEG_INFINITY,
                f64::NEG_INFINITY,
            ];
            let mut include = |point: &pdf::geom::point::Point| -> PdfResult<()> {
                let p = transform.checked_transform(point)?;
                clip[0] = clip[0].min(f64::from(p.x()));
                clip[1] = clip[1].min(f64::from(p.y()));
                clip[2] = clip[2].max(f64::from(p.x()));
                clip[3] = clip[3].max(f64::from(p.y()));
                Ok(())
            };
            for path in element.path.subpaths() {
                for segment in path.segments() {
                    match segment {
                        Segment::MoveTo(p) | Segment::LineTo(p) => include(p)?,
                        Segment::CurveTo(curve) => {
                            include(curve.p1())?;
                            include(curve.p2())?;
                            include(curve.p3())?;
                        }
                        Segment::Close => {}
                    }
                }
            }
            if !clip.iter().all(|v| v.is_finite()) {
                bounds = [0.; 4];
                break;
            }
            bounds = [
                bounds[0].max(clip[0]),
                bounds[1].max(clip[1]),
                bounds[2].min(clip[2]),
                bounds[3].min(clip[3]),
            ];
        }
        // Cubic control hulls may overestimate, never underestimate. Preserve
        // a two-pixel halo for the same antialias/filter rules as region draws.
        let (left, top, right, bottom) = if bounds[2] <= bounds[0] || bounds[3] <= bounds[1] {
            (0, 0, 1, 1)
        } else {
            (
                (bounds[0].floor() - 2.).max(0.) as i32,
                (bounds[1].floor() - 2.).max(0.) as i32,
                (bounds[2].ceil() + 2.).min(f64::from(self.page_width)) as i32,
                (bounds[3].ceil() + 2.).min(f64::from(self.page_height)) as i32,
            )
        };
        let (width, height) = ((right - left).max(1), (bottom - top).max(1));
        let parent = self.raster_target();
        let origin = parent.origin();
        Ok(RasterTarget {
            width,
            height,
            region: Some(RasterRegion {
                x: (origin.0 + left) as u32,
                y: (origin.1 + top) as u32,
                width: width as u32,
                height: height as u32,
            }),
            matrix: Matrix::new(
                self.matrix.a(),
                self.matrix.b(),
                self.matrix.c(),
                self.matrix.d(),
                self.matrix.e() - left as f32,
                self.matrix.f() - top as f32,
            ),
        })
    }
    /// Byte limits govern simultaneously live buffers. Pixel limits govern
    /// each raster's geometry, rather than counting a pixel once per copy.
    pub(super) fn check_group_peak(
        &mut self,
        group_pixels: u64,
        stage: &'static str,
    ) -> PdfResult<()> {
        let (clips, _) = self.clip_image_usage()?;
        let actual = self
            .raster_target()
            .pixels()
            .saturating_add(group_pixels)
            .saturating_mul(4)
            .saturating_add(clips as u64);
        if actual > self.limits.max_render_bytes as u64 {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "render_live_bytes",
                stage,
                limit: self.limits.max_render_bytes as u64,
                actual,
            });
        }
        Ok(())
    }
}
