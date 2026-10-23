use pdf::{
    device::Device,
    error::{PdfError, PdfResult},
    font::GlyphDesc,
    geom::matrix::Matrix,
    page::{
        graph_state::{LineCap, LineJoin},
        graphic_state::{FillType, GraphicState},
        image_object::ImageObject,
        path_object::PathObject,
        text_object::TextObject,
        text_state::TextRenderingMode,
    },
    parser::options::Limits,
    path::pdf_path::Segment,
};
use std::{fs::File, io::Write};
#[cfg(test)]
#[path = "annotation_tests.rs"]
mod annotation_tests;
#[cfg(test)]
#[path = "group_tests.rs"]
mod group_tests;
#[path = "knockout.rs"]
mod knockout;
#[cfg(test)]
#[path = "pattern_tests.rs"]
mod pattern_tests;
#[path = "raster_path.rs"]
mod raster_path;
#[cfg(test)]
#[path = "region_tests.rs"]
mod region_tests;
#[path = "soft_mask.rs"]
mod soft_mask;
#[cfg(test)]
#[path = "soft_mask_tests.rs"]
mod soft_mask_tests;
#[cfg(test)]
#[path = "text_clip_tests.rs"]
mod text_clip_tests;

use skia_safe::{
    Color, EncodedImageFormat, FontMgr, Paint, PaintStyle, PathBuilder, PathEffect, PathFillType,
    RGB, Surface, paint::Cap, surfaces,
};

/// Owned, tightly packed sRGB RGBA8 pixels with straight (unpremultiplied) alpha.
#[derive(Debug)]
pub struct RgbaImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
}

/// Validated, owned substitute program that can be reused across page renders.
#[derive(Clone)]
pub struct SubstituteFont {
    pub(crate) face: skia_safe::Typeface,
    bytes: usize,
}
impl SubstituteFont {
    pub fn from_bytes(data: &[u8], collection_index: usize) -> PdfResult<Self> {
        if collection_index > i32::MAX as usize {
            return Err(PdfError::FontError(
                "font collection index exceeds backend range".into(),
            ));
        }
        Ok(Self {
            face: FontMgr::new()
                .new_from_data(data, collection_index)
                .ok_or_else(|| PdfError::FontError("invalid substitute font program".into()))?,
            bytes: data.len(),
        })
    }
    pub fn byte_len(&self) -> usize {
        self.bytes
    }
}

/// Integer pixels in the rotated/cropped display raster at this renderer's DPI.
/// Only this area plus a bounded two-pixel edge halo is allocated; page
/// interpretation, offscreen allocations and image limits still apply.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RasterRegion {
    pub x: u32,
    pub y: u32,
    pub width: u32,
    pub height: u32,
}
impl RasterRegion {
    fn validate(self) -> PdfResult<()> {
        if self.width == 0
            || self.height == 0
            || self
                .x
                .checked_add(self.width)
                .is_none_or(|v| v > i32::MAX as u32)
            || self
                .y
                .checked_add(self.height)
                .is_none_or(|v| v > i32::MAX as u32)
        {
            return Err(PdfError::DocumentError("invalid raster region".into()));
        }
        Ok(())
    }
    fn scaled(self, factor: u32) -> PdfResult<Self> {
        let mul = |n: u32| {
            n.checked_mul(factor)
                .ok_or_else(|| PdfError::ResourceLimit("scaled raster region".into()))
        };
        let result = Self {
            x: mul(self.x)?,
            y: mul(self.y)?,
            width: mul(self.width)?,
            height: mul(self.height)?,
        };
        result.validate()?;
        Ok(result)
    }
}

#[derive(Clone)]
struct ClipMask {
    image: skia_safe::Image,
    origin: (i32, i32),
}
impl std::ops::Deref for ClipMask {
    type Target = skia_safe::Image;
    fn deref(&self) -> &Self::Target {
        &self.image
    }
}

pub struct SkiaRender {
    surface: Surface,
    pathbuilder: PathBuilder,
    paint: Paint,
    matrix: Matrix,
    scale_x: f32,
    scale_y: f32,
    page_width: i32,
    page_height: i32,
    region: Option<RasterRegion>,
    requested_region: Option<RasterRegion>,
    region_padding: u32,
    raster_path_budget: usize,
    curved_raster_paths: bool,
    clip_images: Vec<skia_safe::Image>,
    clip_cache: Vec<(skia_safe::Path, ClipMask, usize)>,
    limits: Limits,
    clear_color: Color,
    held_pattern_surfaces: u64,
    group_stack: Vec<RenderGroup>,
    active_intent: pdf::color::conversion::RenderingIntent,
    shape_mode: bool,
    object_dispatch: bool,
    initial_override: Option<skia_safe::Image>,
    substitute_font: Option<skia_safe::Typeface>,
    font_provider: Option<std::rc::Rc<crate::font_provider::MemoryFontProvider>>,
    typeface_cache: std::collections::HashMap<
        usize,
        (
            std::rc::Weak<pdf::font::pdf_font::Font>,
            skia_safe::Typeface,
            usize,
        ),
    >,
    typeface_cache_bytes: usize,
}

struct RenderGroup {
    parent: Surface,
    limits: Limits,
    paint: pdf::page::transparency::GroupPaint,
    coverage: Option<Surface>,
    shape: Option<Surface>,
    initial: Option<skia_safe::Image>,
}

impl SkiaRender {
    /// Explicit substitute for fonts without a program. PDF advances remain
    /// authoritative; glyphs are resolved through proven Unicode mappings.
    pub fn set_substitute_font(&mut self, data: &[u8], collection_index: usize) -> PdfResult<()> {
        let program = SubstituteFont::from_bytes(data, collection_index)?;
        self.set_substitute_program(&program);
        Ok(())
    }

    pub fn set_substitute_program(&mut self, program: &SubstituteFont) {
        self.substitute_font = Some(program.face.clone());
    }

    pub fn set_font_provider(
        &mut self,
        provider: std::rc::Rc<crate::font_provider::MemoryFontProvider>,
    ) {
        self.font_provider = Some(provider);
    }

    /// Shared selection entry for preflight and drawing; GIDs belong to the returned identity.
    pub fn resolve_substitute(
        &self,
        font: &pdf::font::pdf_font::Font,
        character: char,
    ) -> PdfResult<Option<pdf::font::resolver::ResolvedGlyph>> {
        self.font_provider.as_ref().map_or(Ok(None), |p| {
            pdf::font::resolver::resolve(p.as_ref(), font.substitution_request(character))
        })
    }

    fn embedded_typeface(
        &mut self,
        font: &std::rc::Rc<pdf::font::pdf_font::Font>,
    ) -> PdfResult<skia_safe::Typeface> {
        let key = std::rc::Rc::as_ptr(font) as usize;
        if let Some((source, face, _)) = self.typeface_cache.get(&key) {
            if source
                .upgrade()
                .is_some_and(|source| std::rc::Rc::ptr_eq(&source, font))
            {
                return Ok(face.clone());
            }
        }
        let data = font
            .fontfile()
            .ok_or_else(|| PdfError::FontError("missing embedded program".into()))?;
        let maximum = self.limits.max_decoded_bytes;
        if data.len() > maximum {
            return Err(PdfError::ResourceLimit("render font program bytes".into()));
        }
        if self.typeface_cache.len() >= 128
            || self.typeface_cache_bytes.saturating_add(data.len()) > maximum
        {
            self.typeface_cache.clear();
            self.typeface_cache_bytes = 0;
        }
        let face = FontMgr::new()
            .new_from_data(data, 0)
            .ok_or_else(|| PdfError::FontError("Skia rejected font program".into()))?;
        if let Some((_, _, bytes)) = self.typeface_cache.remove(&key) {
            self.typeface_cache_bytes -= bytes;
        }
        self.typeface_cache_bytes += data.len();
        self.typeface_cache.insert(
            key,
            (std::rc::Rc::downgrade(font), face.clone(), data.len()),
        );
        Ok(face)
    }

    fn capture_group_alpha(
        &mut self,
        paint: impl FnOnce(&mut Self) -> PdfResult<()>,
    ) -> PdfResult<()> {
        if self.shape_mode {
            return Ok(());
        }
        let Some(coverage) = self
            .group_stack
            .last_mut()
            .and_then(|group| group.coverage.take())
        else {
            return Ok(());
        };
        let color = std::mem::replace(&mut self.surface, coverage);
        // Taking the coverage surface is also a reentrancy guard: recursive
        // primitive dispatch sees no collector and does not capture itself.
        let result = paint(self);
        let coverage = std::mem::replace(&mut self.surface, color);
        self.group_stack
            .last_mut()
            .ok_or_else(|| {
                PdfError::PageOperatorError("group alpha stack changed during primitive".into())
            })?
            .coverage = Some(coverage);
        result
    }

    fn composite_group_image(
        &mut self,
        image: &skia_safe::Image,
        group: &pdf::page::transparency::GroupPaint,
    ) -> PdfResult<()> {
        let mut paint = Paint::default();
        paint.set_alpha(Self::opacity_byte(group.alpha, self.shape_mode));
        paint.set_blend_mode(self.effective_blend_mode(group.blend_mode));
        if let Some(mask) = group.soft_mask.as_deref().filter(|_| !self.shape_mode) {
            self.paint = paint;
            let image_shader = image
                .to_shader(
                    (skia_safe::TileMode::Decal, skia_safe::TileMode::Decal),
                    skia_safe::SamplingOptions::default(),
                    None,
                )
                .ok_or_else(|| PdfError::ResourceLimit("group image shader".into()))?;
            self.paint.set_shader(image_shader);
            self.apply_soft_mask(Some(mask), &Matrix::identity())?;
            paint = self.paint.clone();
            self.paint.set_shader(None);
        }
        let save = self.surface.canvas().save_count();
        self.surface.canvas().save();
        let result = (|| -> PdfResult<()> {
            self.set_clip(&group.clip)?;
            self.surface.canvas().reset_matrix();
            if group.soft_mask.is_some() && !self.shape_mode {
                self.surface.canvas().draw_rect(
                    skia_safe::Rect::from_wh(self.page_width as f32, self.page_height as f32),
                    &paint,
                );
            } else {
                self.surface
                    .canvas()
                    .draw_image(image, (0., 0.), Some(&paint));
            }
            Ok(())
        })();
        self.surface.canvas().restore_to_count(save);
        result
    }
    fn vertical_glyph_offset(
        code: &pdf::font::CharCode,
        font_size: f32,
        horizontal_scale: f32,
    ) -> PdfResult<(f32, f32)> {
        // W2 v points from horizontal origin 0 to vertical origin 1.
        // Draw the horizontal outline at origin 1 minus v; Skia y is downward.
        let x = -(code.origin_x() as f64) * font_size as f64 / 1000.0 * horizontal_scale as f64;
        let y = code.origin_y() as f64 * font_size as f64 / 1000.0;
        let offset = (x as f32, y as f32);
        if !offset.0.is_finite() || !offset.1.is_finite() {
            return Err(PdfError::PageOperatorError(
                "vertical glyph origin overflows".into(),
            ));
        }
        Ok(offset)
    }

    pub fn new(xdpi: f32, ydpi: f32) -> PdfResult<Self> {
        Self::with_limits(xdpi, ydpi, Limits::default())
    }

    pub fn with_limits(xdpi: f32, ydpi: f32, limits: Limits) -> PdfResult<Self> {
        if !xdpi.is_finite() || !ydpi.is_finite() || xdpi <= 0.0 || ydpi <= 0.0 {
            return Err(PdfError::DocumentError(
                "render DPI must be finite and positive".into(),
            ));
        }
        if limits.max_render_pixels == 0 || limits.max_render_bytes < 4 {
            return Err(PdfError::ResourceLimit(
                "render budget cannot hold a pixel".into(),
            ));
        }
        let surface = surfaces::raster_n32_premul((1, 1))
            .ok_or_else(|| PdfError::ResourceLimit("unable to allocate render surface".into()))?;
        let pathbuilder = PathBuilder::new();
        let paint = Paint::default();
        let sx = xdpi / 72.0;
        let sy = ydpi / 72.0;
        let matrix = Matrix::new(sx, 0.0, 0.0, sy, 0.0, 0.0);
        Ok(Self {
            surface,
            pathbuilder,
            paint,
            matrix,
            scale_x: sx,
            scale_y: sy,
            page_width: 0,
            page_height: 0,
            region: None,
            requested_region: None,
            region_padding: 2,
            raster_path_budget: limits.max_operation_bytes / 16,
            curved_raster_paths: false,
            clip_images: Vec::new(),
            clip_cache: Vec::new(),
            limits,
            clear_color: Color::WHITE,
            substitute_font: None,
            font_provider: None,
            typeface_cache: std::collections::HashMap::new(),
            typeface_cache_bytes: 0,
            held_pattern_surfaces: 0,
            group_stack: Vec::new(),
            active_intent: Default::default(),
            shape_mode: false,
            object_dispatch: false,
            initial_override: None,
        })
    }
}
impl SkiaRender {
    /// Select the pixel allocation for subsequent page replays. None restores
    /// full-page rendering. Invalid updates preserve the preceding selection.
    pub fn set_region(&mut self, region: Option<RasterRegion>) -> PdfResult<()> {
        if let Some(value) = region {
            value.validate()?;
        }
        if !self.group_stack.is_empty() {
            return Err(PdfError::DocumentError(
                "cannot change region within a group".into(),
            ));
        }
        self.clip_cache.clear();
        self.region = region;
        self.requested_region = region;
        self.region_padding = 2;
        self.matrix = Matrix::new(
            self.scale_x,
            0.,
            0.,
            self.scale_y,
            -(region.map_or(0, |r| r.x) as f32),
            -(region.map_or(0, |r| r.y) as f32),
        );
        Ok(())
    }
    // Page-space offscreen pixels share the same translated CTM as the parent.
    fn raster_shader_matrix(&self, ctm: &Matrix) -> PdfResult<skia_safe::Matrix> {
        let matrix = ctm.checked_inverse()?;
        Ok(skia_safe::Matrix::new_all(
            matrix.a(),
            matrix.c(),
            matrix.e(),
            matrix.b(),
            matrix.d(),
            matrix.f(),
            0.,
            0.,
            1.,
        ))
    }
    fn inherit_region(&self, child: &mut Self, factor: u32) -> PdfResult<()> {
        child.set_region(
            self.region
                .map(|region| region.scaled(factor))
                .transpose()?,
        )?;
        // The parent already supplies its bounded edge halo. Keep child pixels
        // aligned one-to-one (or factor-to-one) with the parent's allocation.
        child.region_padding = 0;
        Ok(())
    }

    fn set_color_source(
        &mut self,
        space: &pdf::color::colorspace::ColorSpace,
        value: &pdf::color::value::ColorValue,
        ctm: &Matrix,
    ) -> PdfResult<()> {
        // Release the preceding shader before charging the next offscreen surface.
        self.paint.set_shader(None);
        if let pdf::color::colorspace::ColorSpace::Pattern(pattern) = space {
            let pattern = pattern.paint().ok_or_else(|| {
                PdfError::ColorError("Pattern colour has no selected pattern".into())
            })?;
            if let Some(placement) = &pattern.tile_matrix {
                let transform = placement.checked_mul(&self.matrix)?;
                // Resolve one cell at device resolution; enforce ordinary surface
                // limits and keep at least one pixel for subpixel-sized cells.
                let sx = transform
                    .a()
                    .hypot(transform.b())
                    .max(1. / pattern.list.page_width());
                let sy = transform
                    .c()
                    .hypot(transform.d())
                    .max(1. / pattern.list.page_height());
                // Match the cell transform to its integer raster dimensions;
                // otherwise rounding leaves a transparent strip on each repeat.
                let sx = (sx * pattern.list.page_width()).ceil() / pattern.list.page_width();
                let sy = (sy * pattern.list.page_height()).ceil() / pattern.list.page_height();
                let mut limits = self.child_limits_with_clips()?;
                let parent_pixels = (self.page_width as u64 * self.page_height as u64)
                    .checked_mul(1 + self.held_pattern_surfaces)
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("pattern aggregate surface pixels".into())
                    })?;
                limits.max_render_pixels = limits.max_render_pixels.saturating_sub(parent_pixels);
                limits.max_render_bytes = limits
                    .max_render_bytes
                    .saturating_sub(parent_pixels as usize * 4);
                let mut renderer = Self::with_limits(sx * 72., sy * 72., limits)?;
                renderer.clear_color = Color::TRANSPARENT;
                renderer.shape_mode = self.shape_mode;
                renderer.substitute_font = self.substitute_font.clone();
                renderer.font_provider = self.font_provider.clone();
                pattern.list.replay(&mut renderer)?;
                self.curved_raster_paths |= renderer.curved_raster_paths;
                let image = renderer.surface.image_snapshot();
                let local = Matrix::new(
                    pattern.list.page_width() / image.width() as f32,
                    0.,
                    0.,
                    pattern.list.page_height() / image.height() as f32,
                    0.,
                    0.,
                )
                .checked_mul(&transform)?
                .checked_mul(&ctm.checked_inverse()?)?;
                let matrix = skia_safe::Matrix::new_all(
                    local.a(),
                    local.c(),
                    local.e(),
                    local.b(),
                    local.d(),
                    local.f(),
                    0.,
                    0.,
                    1.,
                );
                let shader = image
                    .to_shader(
                        (skia_safe::TileMode::Repeat, skia_safe::TileMode::Repeat),
                        skia_safe::SamplingOptions::from(skia_safe::FilterMode::Linear),
                        &matrix,
                    )
                    .ok_or_else(|| PdfError::ResourceLimit("pattern cell shader".into()))?;
                self.paint.set_color(Color::WHITE);
                self.paint.set_shader(shader);
                return Ok(());
            }
            let pixels = u64::from(self.page_width as u32) * u64::from(self.page_height as u32);
            let bytes = usize::try_from(pixels)
                .ok()
                .and_then(|v| v.checked_mul(4))
                .ok_or_else(|| PdfError::ResourceLimit("pattern render bytes overflow".into()))?;
            let mut limits = self.child_limits_with_clips()?;
            let factor = if pattern.is_tiling()
                && (self.scale_x.fract() != 0. || self.scale_y.fract() != 0.)
            {
                3usize
            } else {
                1
            };
            let needs_backdrop = pattern.list.objects().iter().any(|object| {
                use pdf::page::page_object::PageObject;
                let mode = match object {
                    PageObject::Path(p) => p.graphic_state().general_state.blend_mode,
                    PageObject::Text(t) => t.state.general_state.blend_mode,
                    PageObject::Image(i) => i.graphic_state.general_state.blend_mode,
                    PageObject::Shading(s) => s.graphic_state.general_state.blend_mode,
                    PageObject::GroupBegin(g) => {
                        return !g.isolated
                            || g.blend_mode != pdf::page::general_state::BlendMode::Normal;
                    }
                    _ => return false,
                };
                mode != pdf::page::general_state::BlendMode::Normal
            });
            // Parent, sibling shader, and both downsampling byte buffers.
            let surfaces = self
                .held_pattern_surfaces
                .checked_add(
                    if factor == 1 { 1 } else { 3 }
                        + if needs_backdrop && factor > 1 {
                            (factor * factor) as u64
                        } else {
                            0
                        },
                )
                .ok_or_else(|| PdfError::ResourceLimit("pattern surface count".into()))?;
            let reserved = bytes
                .checked_mul(surfaces as usize)
                .ok_or_else(|| PdfError::ResourceLimit("pattern aggregate render bytes".into()))?;
            limits.max_render_bytes = limits
                .max_render_bytes
                .checked_sub(reserved)
                .ok_or_else(|| PdfError::ResourceLimit("pattern aggregate render bytes".into()))?;
            limits.max_render_pixels = limits
                .max_render_pixels
                .checked_sub(pixels.checked_mul(surfaces).ok_or_else(|| {
                    PdfError::ResourceLimit("pattern aggregate render pixels".into())
                })?)
                .ok_or_else(|| PdfError::ResourceLimit("pattern aggregate render pixels".into()))?;
            // Oversampling reduces seams from independent antialiased cell edges.
            // All additional pixels are charged before allocating the surface.
            let mut renderer = Self::with_limits(
                self.scale_x * 72. * factor as f32,
                self.scale_y * 72. * factor as f32,
                limits,
            )?;
            self.inherit_region(&mut renderer, factor as u32)?;
            renderer.clear_color = Color::TRANSPARENT;
            renderer.shape_mode = self.shape_mode;
            renderer.substitute_font = self.substitute_font.clone();
            renderer.font_provider = self.font_provider.clone();
            if needs_backdrop && !self.shape_mode {
                renderer.start_page(pattern.list.page_width(), pattern.list.page_height())?;
                let image = self.surface.image_snapshot();
                let backdrop = if factor == 1 {
                    image
                } else {
                    let mut scaled =
                        surfaces::raster_n32_premul((renderer.page_width, renderer.page_height))
                            .ok_or_else(|| {
                                PdfError::ResourceLimit("pattern backdrop surface".into())
                            })?;
                    scaled.canvas().scale((factor as f32, factor as f32));
                    scaled.canvas().draw_image(&image, (0., 0.), None);
                    scaled.image_snapshot()
                };
                renderer.initial_override = Some(backdrop);
                renderer.begin_group(pdf::page::transparency::GroupPaint {
                    isolated: false,
                    knockout: !pattern.is_tiling(),
                    alpha: 1.,
                    blend_mode: pdf::page::general_state::BlendMode::Normal,
                    clip: Default::default(),
                    soft_mask: None,
                })?;
                let result = pattern.list.replay_contents(&mut renderer);
                let end = renderer.end_group(result.is_ok());
                result.and(end)?;
            } else {
                pattern.list.replay(&mut renderer)?;
            }
            self.curved_raster_paths |= renderer.curved_raster_paths;
            let image = if factor == 1 {
                renderer.surface.image_snapshot()
            } else {
                // Average premultiplied native channels, not unpremultiplied
                // colours. Bilinear minification is not an area average and
                // introduces seams at abutting, independently painted cells.
                let source = renderer
                    .surface
                    .peek_pixels()
                    .ok_or_else(|| PdfError::ResourceLimit("pattern raster access".into()))?;
                let stride = source.row_bytes();
                let input = source
                    .bytes()
                    .ok_or_else(|| PdfError::ResourceLimit("pattern raster bytes".into()))?;
                let mut reduced = Vec::new();
                reduced
                    .try_reserve_exact(bytes)
                    .map_err(|_| PdfError::ResourceLimit("pattern reduction buffer".into()))?;
                reduced.resize(bytes, 0u8);
                for y in 0..self.page_height as usize {
                    for x in 0..self.page_width as usize {
                        for channel in 0..4 {
                            let mut sum = 0u32;
                            for sy in y * factor..(y + 1) * factor {
                                for sx in x * factor..(x + 1) * factor {
                                    if sx < renderer.page_width as usize
                                        && sy < renderer.page_height as usize
                                    {
                                        sum += u32::from(input[sy * stride + sx * 4 + channel]);
                                    }
                                }
                            }
                            reduced[(y * self.page_width as usize + x) * 4 + channel] =
                                ((sum + 4) / 9) as u8;
                        }
                    }
                }
                skia_safe::images::raster_from_data(
                    &self.surface.image_info(),
                    skia_safe::Data::new_copy(&reduced),
                    self.page_width as usize * 4,
                )
                .ok_or_else(|| PdfError::ResourceLimit("pattern reduced surface".into()))?
            };
            let matrix = self.raster_shader_matrix(ctm)?;
            let shader = image
                .to_shader(
                    (skia_safe::TileMode::Decal, skia_safe::TileMode::Decal),
                    skia_safe::SamplingOptions::default(),
                    &matrix,
                )
                .ok_or_else(|| PdfError::ResourceLimit("pattern image shader".into()))?;
            self.paint.set_color(Color::WHITE);
            self.paint.set_shader(shader);
        } else {
            let context = pdf::color::conversion::ColorConversionContext::default()
                .with_intent(self.active_intent);
            let rgb = space.rgb_with_context(value, &context)?;
            self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
        }
        Ok(())
    }
    fn validate_path_coordinates(
        path: &pdf::path::pdf_path::PdfPath,
        matrix: &Matrix,
    ) -> PdfResult<()> {
        for subpath in path.subpaths() {
            for segment in subpath.segments() {
                match segment {
                    Segment::Close => {}
                    Segment::MoveTo(point) | Segment::LineTo(point) => {
                        matrix.checked_transform(point)?;
                    }
                    Segment::CurveTo(curve) => {
                        for point in [curve.p1(), curve.p2(), curve.p3()] {
                            matrix.checked_transform(point)?;
                        }
                    }
                }
            }
        }
        Ok(())
    }

    fn clip_image_usage(&mut self) -> PdfResult<(usize, u64)> {
        use skia_safe::ConditionallySend;
        // can_send() is defined by Skia's RCHandle as reference count == 1.
        // Our bookkeeping clone is then the sole owner: canvas restore has
        // released the shader, so this backing allocation can be dropped.
        self.clip_images.retain(|image| !image.can_send());
        let pixels = self.clip_images.iter().try_fold(0u64, |total, image| {
            total
                .checked_add(image.width() as u64 * image.height() as u64)
                .ok_or_else(|| PdfError::ResourceLimit("live clip pixels".into()))
        })?;
        let bytes = usize::try_from(pixels)
            .map_err(|_| PdfError::ResourceLimit("live clip bytes".into()))?;
        Ok((bytes, pixels)) // Every retained image is Alpha8.
    }

    fn child_limits_with_clips(&mut self) -> PdfResult<Limits> {
        let (bytes, pixels) = self.clip_image_usage()?;
        let mut limits = self.limits;
        limits.max_render_bytes = limits
            .max_render_bytes
            .checked_sub(bytes)
            .ok_or_else(|| PdfError::ResourceLimit("child live clip bytes".into()))?;
        limits.max_render_pixels = limits
            .max_render_pixels
            .checked_sub(pixels)
            .ok_or_else(|| PdfError::ResourceLimit("child live clip pixels".into()))?;
        Ok(limits)
    }

    // Repeated text/paint objects commonly share an identical clip. Keep a
    // small page-local LRU of the exact canonical mask, including its global
    // path/fill rule. Cache references stay in the live-image ledger, so child
    // surfaces and allocation peaks continue to account for their backing bytes.
    fn cached_clip_image(&mut self, path: &skia_safe::Path) -> PdfResult<ClipMask> {
        let comparison_work = path
            .points()
            .len()
            .max(1)
            .saturating_mul(self.clip_cache.len() + 1);
        self.raster_path_budget = self
            .raster_path_budget
            .checked_sub(comparison_work)
            .ok_or_else(|| PdfError::ResourceLimit("clip cache comparison work".into()))?;
        if let Some(index) = self.clip_cache.iter().position(|(key, _, _)| key == path) {
            let (held_bytes, held_pixels) = self.clip_image_usage()?;
            let parent = (self.page_width as u64 * self.page_height as u64)
                .saturating_mul(self.held_pattern_surfaces + 1);
            if parent.saturating_add(held_pixels) > self.limits.max_render_pixels
                || parent.saturating_mul(4).saturating_add(held_bytes as u64)
                    > self.limits.max_render_bytes as u64
            {
                return Err(PdfError::ResourceLimit("cached clip peak".into()));
            }
            let entry = self.clip_cache.remove(index);
            let image = entry.1.clone();
            self.clip_cache.push(entry);
            return Ok(image);
        }
        let (_, _, width, height) = self.clip_mask_bounds(path, true);
        let bytes = (width as usize)
            .saturating_mul(height as usize)
            .saturating_add(
                path.points()
                    .len()
                    .saturating_mul(std::mem::size_of::<skia_safe::Point>()),
            )
            .saturating_add(path.verbs().len());
        let maximum = (2 * 1024 * 1024)
            .min(self.limits.max_render_bytes / 8)
            .min(usize::try_from(self.limits.max_render_pixels / 2).unwrap_or(usize::MAX));
        while !self.clip_cache.is_empty()
            && (self.clip_cache.len() >= 4
                || self
                    .clip_cache
                    .iter()
                    .map(|v| v.2)
                    .sum::<usize>()
                    .saturating_add(bytes)
                    > maximum)
        {
            self.clip_cache.remove(0);
        }
        let image = self.canonical_clip_mask(path, true)?;
        // canonical_clip_mask reserved the next ledger slot before allocating.
        self.clip_images.push(image.image.clone());
        if bytes <= maximum && self.clip_cache.try_reserve(1).is_ok() {
            self.clip_cache.push((path.clone(), image.clone(), bytes));
        }
        Ok(image)
    }

    fn clip_mask_bounds(&self, path: &skia_safe::Path, crop: bool) -> (i32, i32, i32, i32) {
        let origin = self.region.map_or((0, 0), |r| (r.x as i32, r.y as i32));
        if !crop {
            return (origin.0, origin.1, self.page_width, self.page_height);
        }
        let b = path.bounds();
        let left = (b.left.floor() as i32).max(origin.0);
        let top = (b.top.floor() as i32).max(origin.1);
        let right = (b.right.ceil() as i32).min(origin.0 + self.page_width);
        let bottom = (b.bottom.ceil() as i32).min(origin.1 + self.page_height);
        if right <= left || bottom <= top {
            (origin.0, origin.1, 1, 1)
        } else {
            (left, top, right - left, bottom - top)
        }
    }
    #[cfg(test)]
    fn canonical_clip_image(&mut self, path: &skia_safe::Path) -> PdfResult<skia_safe::Image> {
        self.canonical_clip_mask(path, false).map(|mask| mask.image)
    }
    fn canonical_clip_mask(&mut self, path: &skia_safe::Path, crop: bool) -> PdfResult<ClipMask> {
        const CELL: i32 = 64;
        const HALO: i32 = 2;
        const SIDE: i32 = CELL + 2 * HALO;
        let (held_bytes, held_pixels) = self.clip_image_usage()?;
        let (mask_left, mask_top, mask_width, mask_height) = self.clip_mask_bounds(path, crop);
        let pixels = mask_width as u64 * mask_height as u64;
        let length = usize::try_from(pixels)
            .map_err(|_| PdfError::ResourceLimit("clip backing length".into()))?;
        let parent = (self.page_width as u64 * self.page_height as u64)
            .checked_mul(self.held_pattern_surfaces + 1)
            .ok_or_else(|| PdfError::ResourceLimit("clip parent surfaces".into()))?;
        let scratch = (SIDE * SIDE) as u64;
        if parent
            .checked_add(held_pixels)
            .and_then(|n| n.checked_add(pixels * 2 + scratch * 2))
            .is_none_or(|n| n > self.limits.max_render_pixels)
            || parent
                .checked_mul(4)
                .and_then(|n| n.checked_add(held_bytes as u64))
                .and_then(|n| n.checked_add(pixels * 2 + scratch * 8))
                .is_none_or(|n| n > self.limits.max_render_bytes as u64)
        {
            return Err(PdfError::ResourceLimit("canonical clip peak".into()));
        }
        let origin = self.region.map_or((0, 0), |r| (r.x as i32, r.y as i32));
        let bounds = path.bounds();
        let left = (bounds.left.floor() as i32).max(origin.0);
        let top = (bounds.top.floor() as i32).max(origin.1);
        let right = (bounds.right.ceil() as i32).min(origin.0 + self.page_width);
        let bottom = (bounds.bottom.ceil() as i32).min(origin.1 + self.page_height);
        let columns = if right > left {
            (right - 1).div_euclid(CELL) - left.div_euclid(CELL) + 1
        } else {
            0
        };
        let rows = if bottom > top {
            (bottom - 1).div_euclid(CELL) - top.div_euclid(CELL) + 1
        } else {
            0
        };
        let work = (columns as usize)
            .checked_mul(rows as usize)
            .and_then(|n| {
                n.checked_mul(
                    path.points()
                        .len()
                        .max(1)
                        .checked_add((CELL * CELL) as usize)?,
                )
            })
            .and_then(|n| n.checked_add(length))
            .ok_or_else(|| PdfError::ResourceLimit("canonical clip work".into()))?;
        self.raster_path_budget = self
            .raster_path_budget
            .checked_sub(work)
            .ok_or_else(|| PdfError::ResourceLimit("canonical clip work".into()))?;
        self.clip_images
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("clip image ledger".into()))?;
        let mut output = Vec::new();
        output
            .try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("clip backing allocation".into()))?;
        output.resize(length, 0u8);
        let info = skia_safe::ImageInfo::new(
            (SIDE, SIDE),
            skia_safe::ColorType::Alpha8,
            skia_safe::AlphaType::Premul,
            None,
        );
        let mut mask = surfaces::raster(&info, None, None)
            .ok_or_else(|| PdfError::ResourceLimit("clip cell allocation".into()))?;
        let mut paint = Paint::default();
        paint.set_color(Color::WHITE).set_anti_alias(true);
        if right > left && bottom > top {
            for y in (top.div_euclid(CELL) * CELL..bottom).step_by(CELL as usize) {
                for x in (left.div_euclid(CELL) * CELL..right).step_by(CELL as usize) {
                    mask.canvas().clear(Color::TRANSPARENT);
                    mask.canvas().reset_matrix();
                    mask.canvas()
                        .translate((-(x - HALO) as f32, -(y - HALO) as f32));
                    mask.canvas().draw_path(path, &paint);
                    let source = mask
                        .peek_pixels()
                        .ok_or_else(|| PdfError::ResourceLimit("clip cell pixels".into()))?;
                    let bytes = source
                        .bytes()
                        .ok_or_else(|| PdfError::ResourceLimit("clip cell bytes".into()))?;
                    let x0 = x.max(mask_left);
                    let x1 = (x + CELL).min(mask_left + mask_width);
                    for yy in y.max(mask_top)..(y + CELL).min(mask_top + mask_height) {
                        let start = (yy - y + HALO) as usize * source.row_bytes()
                            + (x0 - x + HALO) as usize;
                        let target = (yy - mask_top) as usize * mask_width as usize
                            + (x0 - mask_left) as usize;
                        output[target..target + (x1 - x0) as usize]
                            .copy_from_slice(&bytes[start..start + (x1 - x0) as usize]);
                    }
                }
            }
        }
        let info = skia_safe::ImageInfo::new(
            (mask_width, mask_height),
            skia_safe::ColorType::Alpha8,
            skia_safe::AlphaType::Premul,
            None,
        );
        skia_safe::images::raster_from_data(
            &info,
            skia_safe::Data::new_copy(&output),
            mask_width as usize,
        )
        .map(|image| ClipMask {
            image,
            origin: (mask_left, mask_top),
        })
        .ok_or_else(|| PdfError::ResourceLimit("clip image allocation".into()))
    }

    fn set_clip(&mut self, clip_path: &pdf::page::clip_path::ClipPath) -> PdfResult<()> {
        self.clip_image_usage()?;
        if !clip_path.is_empty() {
            for clip_ele in clip_path.elements() {
                let matrix = &clip_ele.matrix;
                let ctm = matrix.checked_mul(&self.matrix)?;
                Self::validate_path_coordinates(&clip_ele.path, &ctm)?;
                let m44 =
                    self.pdf_to_skia_m44(ctm.a(), ctm.b(), ctm.c(), ctm.d(), ctm.e(), ctm.f());
                self.surface.canvas().set_matrix(&m44);
                let mut clip_builder = PathBuilder::new();
                for sub_path in clip_ele.path.subpaths() {
                    for seg in sub_path.segments() {
                        match seg {
                            Segment::Close => {
                                clip_builder.close();
                            }
                            Segment::MoveTo(p) => {
                                clip_builder.move_to((p.x(), p.y()));
                            }
                            Segment::LineTo(p) => {
                                clip_builder.line_to((p.x(), p.y()));
                            }
                            Segment::CurveTo(bezier) => {
                                let p1 = bezier.p1();
                                let p2 = bezier.p2();
                                let p3 = bezier.p3();
                                clip_builder.cubic_to(
                                    (p1.x(), p1.y()),
                                    (p2.x(), p2.y()),
                                    (p3.x(), p3.y()),
                                );
                            }
                        }
                    }
                }
                match clip_ele.fill_type {
                    FillType::NoFill => {
                        clip_builder.set_fill_type(PathFillType::Winding);
                    }
                    FillType::EvenOdd => {
                        clip_builder.set_fill_type(PathFillType::EvenOdd);
                    }
                    FillType::Winding => {
                        clip_builder.set_fill_type(PathFillType::Winding);
                    }
                }
                let path = clip_builder.snapshot();
                self.curved_raster_paths |= raster_path::has_curves(&path);
                if path.bounds().is_empty() {
                    self.surface
                        .canvas()
                        .clip_path(&path, skia_safe::ClipOp::Intersect, true);
                    continue;
                }
                let axis_aligned =
                    (ctm.b() == 0. && ctm.c() == 0.) || (ctm.a() == 0. && ctm.d() == 0.);
                let global = clip_ele.matrix.checked_mul(&Matrix::new(
                    self.scale_x,
                    0.,
                    0.,
                    self.scale_y,
                    0.,
                    0.,
                ))?;
                let matrix = skia_safe::Matrix::new_all(
                    global.a(),
                    global.c(),
                    global.e(),
                    global.b(),
                    global.d(),
                    global.f(),
                    0.,
                    0.,
                    1.,
                );
                let path = raster_path::normalized(
                    &path.with_transform(&matrix),
                    &mut self.raster_path_budget,
                )?;
                let origin = self.region.map_or((0, 0), |r| (r.x, r.y));
                let transform =
                    self.pdf_to_skia_m44(1., 0., 0., 1., -(origin.0 as f32), -(origin.1 as f32));
                self.surface.canvas().set_matrix(&transform);
                if !axis_aligned || path.is_rect().is_none() {
                    let image = self.cached_clip_image(&path)?;
                    let local = skia_safe::Matrix::translate((
                        image.origin.0 as f32,
                        image.origin.1 as f32,
                    ));
                    let shader = image
                        .to_shader(
                            (skia_safe::TileMode::Decal, skia_safe::TileMode::Decal),
                            skia_safe::SamplingOptions::default(),
                            Some(&local),
                        )
                        .ok_or_else(|| PdfError::ResourceLimit("canonical clip shader".into()))?;
                    self.surface.canvas().clip_shader(shader, None);
                } else {
                    self.surface
                        .canvas()
                        .clip_path(&path, skia_safe::ClipOp::Intersect, true);
                }
            }
        }
        Ok(())
    }
    fn build_path(&mut self, pathobject: &PathObject) {
        let pdf_path = pathobject.pdf_path();
        match pathobject.fill_type() {
            FillType::NoFill => {}
            FillType::EvenOdd => {
                self.pathbuilder.set_fill_type(PathFillType::EvenOdd);
            }
            FillType::Winding => {
                self.pathbuilder.set_fill_type(PathFillType::Winding);
            }
        };

        for subpath in pdf_path.subpaths() {
            for segment in subpath.segments() {
                match segment {
                    Segment::Close => {
                        self.pathbuilder.close();
                    }
                    Segment::MoveTo(p) => {
                        self.pathbuilder.move_to((p.x(), p.y()));
                    }
                    Segment::LineTo(p) => {
                        self.pathbuilder.line_to((p.x(), p.y()));
                    }
                    Segment::CurveTo(bezier) => {
                        let p1 = bezier.p1();
                        let p2 = bezier.p2();
                        let p3 = bezier.p3();
                        self.pathbuilder.cubic_to(
                            (p1.x(), p1.y()),
                            (p2.x(), p2.y()),
                            (p3.x(), p3.y()),
                        );
                    }
                }
            }
        }
    }
    /// Copies the current page into an owned buffer; no PNG or filesystem round trip.
    pub fn to_rgba(&mut self) -> PdfResult<RgbaImage> {
        if self.page_width <= 0 || self.page_height <= 0 {
            return Err(PdfError::DocumentError("no rendered page".into()));
        }
        let (width, height, origin) =
            if let (Some(request), Some(actual)) = (self.requested_region, self.region) {
                (
                    request.width as i32,
                    request.height as i32,
                    ((request.x - actual.x) as i32, (request.y - actual.y) as i32),
                )
            } else {
                (self.page_width, self.page_height, (0, 0))
            };
        let row_bytes = (width as usize)
            .checked_mul(4)
            .ok_or_else(|| PdfError::ResourceLimit("RGBA stride overflow".into()))?;
        let size = row_bytes
            .checked_mul(height as usize)
            .filter(|n| *n <= self.limits.max_render_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("RGBA output exceeds budget".into()))?;
        let mut pixels = Vec::new();
        pixels
            .try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("RGBA allocation failed".into()))?;
        pixels.resize(size, 0);
        let info = skia_safe::ImageInfo::new(
            (width, height),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            skia_safe::ColorSpace::new_srgb(),
        );
        if !self
            .surface
            .read_pixels(&info, &mut pixels, row_bytes, origin)
        {
            return Err(PdfError::DocumentError("RGBA readback failed".into()));
        }
        Ok(RgbaImage {
            width: width as u32,
            height: height as u32,
            pixels,
        })
    }

    pub fn save(&mut self, path: &str) -> PdfResult<()> {
        let image = self.surface.image_snapshot();
        let mut context = self.surface.direct_context();
        let d = image
            .encode(context.as_mut(), EncodedImageFormat::PNG, None)
            .ok_or_else(|| PdfError::IOError("PNG encoding failed".into()))?;
        let mut file = File::create(path)?;
        let bytes = d.as_bytes();
        file.write_all(bytes)?;
        Ok(())
    }

    fn opacity_byte(alpha: f32, shape_mode: bool) -> u8 {
        // Raster surfaces and mask samples use eight-bit premultiplied alpha.
        // Quantize constants at the same boundary rather than leaving half-way
        // floating colour conversions to platform-specific SIMD rounding.
        if shape_mode {
            255
        } else {
            (alpha * 255.).round() as u8
        }
    }

    fn set_paint_alpha(&mut self, alpha: f32) -> PdfResult<()> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(PdfError::PageOperatorError(
                "paint alpha must be finite and in [0,1]".into(),
            ));
        }
        self.paint
            .set_alpha(Self::opacity_byte(alpha, self.shape_mode));
        Ok(())
    }

    fn skia_blend_mode(mode: pdf::page::general_state::BlendMode) -> skia_safe::BlendMode {
        use pdf::page::general_state::BlendMode as Pdf;
        use skia_safe::BlendMode as Skia;
        match mode {
            Pdf::Normal => Skia::SrcOver,
            Pdf::Multiply => Skia::Multiply,
            Pdf::Screen => Skia::Screen,
            Pdf::Overlay => Skia::Overlay,
            Pdf::Darken => Skia::Darken,
            Pdf::Lighten => Skia::Lighten,
            Pdf::ColorDodge => Skia::ColorDodge,
            Pdf::ColorBurn => Skia::ColorBurn,
            Pdf::HardLight => Skia::HardLight,
            Pdf::SoftLight => Skia::SoftLight,
            Pdf::Difference => Skia::Difference,
            Pdf::Exclusion => Skia::Exclusion,
            Pdf::Hue => Skia::Hue,
            Pdf::Saturation => Skia::Saturation,
            Pdf::Color => Skia::Color,
            Pdf::Luminosity => Skia::Luminosity,
        }
    }

    fn device_stroke_width(width: f32, ctm: &Matrix) -> f32 {
        // Preserve subpixel hairlines instead of quantizing their coverage to
        // zero. Only collapse to a device hairline when every principal scale
        // is below one pixel; anisotropic wide strokes keep their user width.
        let a = f64::from(ctm.a());
        let b = f64::from(ctm.b());
        let c = f64::from(ctm.c());
        let d = f64::from(ctm.d());
        let trace = a * a + b * b + c * c + d * d;
        let det = a * d - b * c;
        let largest = ((trace + (trace * trace - 4. * det * det).max(0.).sqrt()) * 0.5).sqrt();
        if width > 0. && largest > 0. && f64::from(width) * largest < 1. {
            0.
        } else {
            width
        }
    }

    fn set_path_paint_state(&mut self, graphic_state: &GraphicState) {
        match graphic_state.graph_state.line_cap {
            LineCap::Butt => self.paint.set_stroke_cap(Cap::Butt),
            LineCap::Round => self.paint.set_stroke_cap(Cap::Round),
            LineCap::Square => self.paint.set_stroke_cap(Cap::Square),
        };

        match graphic_state.graph_state.line_join {
            LineJoin::Miter => self.paint.set_stroke_join(skia_safe::PaintJoin::Miter),
            LineJoin::Round => self.paint.set_stroke_join(skia_safe::PaintJoin::Round),
            LineJoin::Bevel => self.paint.set_stroke_join(skia_safe::PaintJoin::Bevel),
        };
    }
    fn stroke_path(&mut self, graphic_state: &GraphicState, ctm: &Matrix) -> PdfResult<()> {
        self.set_color_source(
            &graphic_state.color_state.stroke_color_space,
            &graphic_state.color_state.stroke_color_value,
            ctm,
        )?;
        self.set_paint_alpha(graphic_state.general_state.stroke_alpha)?;
        self.paint
            .set_blend_mode(self.effective_blend_mode(graphic_state.general_state.blend_mode));
        self.paint.set_stroke_width(Self::device_stroke_width(
            graphic_state.graph_state.line_width,
            ctm,
        ));
        let dash_phrase = graphic_state.graph_state.dash_phrase;
        let dash_array = &graphic_state.graph_state.dash_array;
        let stroke_miter = graphic_state.graph_state.miter_limit;
        self.paint.set_stroke_miter(stroke_miter);
        self.paint
            .set_path_effect(PathEffect::dash(dash_array, dash_phrase));
        self.paint.set_style(PaintStyle::Stroke);
        self.apply_soft_mask(graphic_state.general_state.soft_mask.as_deref(), ctm)?;
        Ok(())
    }
    fn fill_path(&mut self, graphic_state: &GraphicState, ctm: &Matrix) -> PdfResult<()> {
        self.set_color_source(
            &graphic_state.color_state.fill_color_space,
            &graphic_state.color_state.fill_color_value,
            ctm,
        )?;
        self.set_paint_alpha(graphic_state.general_state.fill_alpha)?;
        self.paint
            .set_blend_mode(self.effective_blend_mode(graphic_state.general_state.blend_mode));
        self.paint.set_style(PaintStyle::Fill);
        self.apply_soft_mask(graphic_state.general_state.soft_mask.as_deref(), ctm)?;
        Ok(())
    }

    fn pdf_to_skia_m44(&self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> skia_safe::M44 {
        // M44::new() takes 16 values in row-major order:
        // [ m00, m01, m02, m03,
        //   m10, m11, m12, m13,
        //   m20, m21, m22, m23,
        //   m30, m31, m32, m33 ]
        skia_safe::M44::new(
            a, c, 0.0, e, // Row 0: X output
            b, d, 0.0, f, // Row 1: Y output
            0.0, 0.0, 1.0, 0.0, // Row 2: Z output (Identity)
            0.0, 0.0, 0.0, 1.0, // Row 3: W output (Homogeneous)
        )
    }

    fn draw_filled_glyph(
        &mut self,
        font: &skia_safe::Font,
        glyph: u16,
        position: skia_safe::Point,
        paint: &Paint,
        global: &Matrix,
    ) -> PdfResult<()> {
        let (a, b, c, d) = (global.a(), global.b(), global.c(), global.d());
        let x = f64::from(a) * f64::from(position.x)
            + f64::from(c) * f64::from(position.y)
            + f64::from(global.e());
        let y = f64::from(b) * f64::from(position.x)
            + f64::from(d) * f64::from(position.y)
            + f64::from(global.f());
        let x = ((x * 256.).round() / 256.) as f32;
        let y = ((y * 256.).round() / 256.) as f32;
        if !x.is_finite() || !y.is_finite() {
            return Err(PdfError::PageOperatorError(
                "glyph raster origin overflow".into(),
            ));
        }
        let origin = self.region.map_or((0, 0), |r| (r.x, r.y));
        let transform = self.pdf_to_skia_m44(a, b, c, d, x - origin.0 as f32, y - origin.1 as f32);
        self.surface.canvas().set_matrix(&transform);
        let mut fill = paint.clone();
        if let Some(shader) = paint.shader() {
            // Changing the glyph origin must not move a page-space Pattern
            // or soft mask. The linear part is identical, so only translate
            // the shader in face coordinates; avoid a noisy inverse product.
            let determinant = f64::from(a) * f64::from(d) - f64::from(b) * f64::from(c);
            let dx = f64::from(global.e()) - f64::from(x);
            let dy = f64::from(global.f()) - f64::from(y);
            let sx = ((f64::from(d) * dx - f64::from(c) * dy) / determinant) as f32;
            let sy = ((f64::from(a) * dy - f64::from(b) * dx) / determinant) as f32;
            if !sx.is_finite() || !sy.is_finite() {
                return Err(PdfError::PageOperatorError(
                    "glyph shader origin overflow".into(),
                ));
            }
            fill.set_shader(shader.with_local_matrix(&skia_safe::Matrix::translate((sx, sy))));
        }
        // Match SkStrikeSpec::ShouldDrawAsPath's affine 256-pixel cache cap.
        // Keep hinted bitmap rendering below it; large vector glyphs need the
        // same global coverage windows as ordinary paths.
        let size = font.size();
        let xx = a * size * font.scale_x();
        let yx = b * size * font.scale_x();
        let xy = a * size * font.skew_x() + c * size;
        let yy = b * size * font.skew_x() + d * size;
        if xx * xx + yx * yx > 256. * 256. || xy * xy + yy * yy > 256. * 256. {
            if let Some(path) = font.get_path(glyph) {
                return self.draw_prepared_path(&path, &fill, &Matrix::new(a, b, c, d, x, y));
            }
        }
        // Bitmap-only/empty glyphs retain Skia's existing fallback as well.
        self.surface.canvas().draw_glyphs_at(
            &[glyph],
            &[skia_safe::Point::new(0., 0.)][..],
            (0, 0),
            font,
            &fill,
        );
        Ok(())
    }

    fn draw_prepared_path(
        &mut self,
        path: &skia_safe::Path,
        paint: &Paint,
        global: &Matrix,
    ) -> PdfResult<()> {
        if paint.style() == PaintStyle::Fill && path.bounds().is_empty() {
            return Ok(());
        }
        let axis_aligned =
            (global.b() == 0. && global.c() == 0.) || (global.a() == 0. && global.d() == 0.);
        let direct_rect =
            paint.style() == PaintStyle::Fill && path.is_rect().is_some() && axis_aligned;
        let matrix = skia_safe::Matrix::new_all(
            global.a(),
            global.c(),
            global.e(),
            global.b(),
            global.d(),
            global.f(),
            0.,
            0.,
            1.,
        );
        let mut outline = path.clone();
        let mut fill = paint.clone();
        if paint.style() == PaintStyle::Stroke {
            let filled = skia_safe::path_utils::fill_path_with_paint(
                path,
                paint,
                &mut outline,
                None,
                Some(matrix.clone()),
            );
            outline = outline.with_transform(&matrix);
            if !filled {
                // A device hairline has one-pixel width independent of its
                // source affine transform. Expand it once before flattening.
                let mut hairline = paint.clone();
                hairline.set_stroke_width(1.);
                hairline.set_path_effect(None);
                let mut expanded = skia_safe::Path::new();
                skia_safe::path_utils::fill_path_with_paint(
                    &outline,
                    &hairline,
                    &mut expanded,
                    None,
                    None,
                );
                outline = expanded;
            }
            fill.set_style(PaintStyle::Fill);
            fill.set_path_effect(None);
        } else {
            outline = outline.with_transform(&matrix);
        }
        self.curved_raster_paths |= raster_path::has_curves(&outline);
        let outline = raster_path::normalized(&outline, &mut self.raster_path_budget)?;
        if let Some(shader) = paint.shader() {
            fill.set_shader(shader.with_local_matrix(&matrix));
        }
        let origin = self.region.map_or((0, 0), |r| (r.x, r.y));
        let bounds = outline.bounds();
        let left = (bounds.left.floor() as i32).max(origin.0 as i32);
        let top = (bounds.top.floor() as i32).max(origin.1 as i32);
        let right = (bounds.right.ceil() as i32).min(origin.0 as i32 + self.page_width);
        let bottom = (bounds.bottom.ceil() as i32).min(origin.1 as i32 + self.page_height);
        if right <= left || bottom <= top {
            return Ok(());
        }
        let transform =
            self.pdf_to_skia_m44(1., 0., 0., 1., -(origin.0 as f32), -(origin.1 as f32));
        if direct_rect {
            self.surface.canvas().save();
            self.surface.canvas().set_matrix(&transform);
            self.surface.canvas().draw_path(&outline, &fill);
            self.surface.canvas().restore();
            return Ok(());
        }
        // Rasterize coverage in canonical global cells. Skia's analytic edge
        // clipping depends on the destination viewport even for identical
        // global outlines; a fixed coverage viewport removes that dependency.
        const CELL: i32 = 64;
        const HALO: i32 = 2;
        const SIDE: i32 = CELL + 2 * HALO;
        let (clip_bytes, clip_pixels) = self.clip_image_usage()?;
        let pixels = (self.page_width as u64 * self.page_height as u64)
            .checked_mul(self.held_pattern_surfaces + 1)
            .ok_or_else(|| PdfError::ResourceLimit("canonical curve aggregate surfaces".into()))?;
        let scratch = (SIDE * SIDE) as u64;
        if pixels
            .checked_add(clip_pixels)
            .and_then(|n| n.checked_add(scratch * 2))
            .is_none_or(|n| n > self.limits.max_render_pixels)
            || pixels
                .checked_mul(4)
                .and_then(|n| n.checked_add(clip_bytes as u64))
                .and_then(|n| n.checked_add(scratch * 8))
                .is_none_or(|n| n > self.limits.max_render_bytes as u64)
        {
            return Err(PdfError::ResourceLimit(
                "canonical curve coverage scratch".into(),
            ));
        }
        let columns = (right - 1).div_euclid(CELL) - left.div_euclid(CELL) + 1;
        let rows = (bottom - 1).div_euclid(CELL) - top.div_euclid(CELL) + 1;
        let integer_spans =
            fill.shader().is_none() && fill.as_blend_mode() == Some(skia_safe::BlendMode::SrcOver);
        let cell_work = outline
            .points()
            .len()
            .max(1)
            .checked_add(if integer_spans {
                (CELL * CELL) as usize
            } else {
                0
            })
            .ok_or_else(|| PdfError::ResourceLimit("canonical coverage cell work".into()))?;
        let work = (columns as usize)
            .checked_mul(rows as usize)
            .and_then(|n| n.checked_mul(cell_work))
            .ok_or_else(|| PdfError::ResourceLimit("canonical curve coverage work".into()))?;
        self.raster_path_budget = self
            .raster_path_budget
            .checked_sub(work)
            .ok_or_else(|| PdfError::ResourceLimit("canonical curve coverage work".into()))?;
        let mask_info = skia_safe::ImageInfo::new(
            (SIDE, SIDE),
            skia_safe::ColorType::Alpha8,
            skia_safe::AlphaType::Premul,
            None,
        );
        let mut mask = surfaces::raster(&mask_info, None, None)
            .ok_or_else(|| PdfError::ResourceLimit("canonical curve coverage allocation".into()))?;
        let mut coverage = Paint::default();
        coverage
            .set_color(Color::WHITE)
            .set_anti_alias(paint.is_anti_alias());
        self.surface.canvas().save();
        self.surface.canvas().set_matrix(&transform);
        for y in (top.div_euclid(CELL) * CELL..bottom).step_by(CELL as usize) {
            for x in (left.div_euclid(CELL) * CELL..right).step_by(CELL as usize) {
                mask.canvas().clear(Color::TRANSPARENT);
                mask.canvas().reset_matrix();
                mask.canvas()
                    .translate((-(x - HALO) as f32, -(y - HALO) as f32));
                mask.canvas().draw_path(&outline, &coverage);
                let image = mask.image_snapshot();
                self.surface.canvas().save();
                self.surface.canvas().clip_rect(
                    skia_safe::Rect::from_xywh(x as f32, y as f32, CELL as f32, CELL as f32),
                    None,
                    false,
                );
                if fill.shader().is_some() {
                    let local =
                        skia_safe::Matrix::translate(((x - HALO) as f32, (y - HALO) as f32));
                    let shader =
                        image.to_shader(None, skia_safe::SamplingOptions::default(), Some(&local));
                    let Some(shader) = shader else {
                        self.surface.canvas().restore();
                        self.surface.canvas().restore();
                        return Err(PdfError::ResourceLimit("canonical coverage shader".into()));
                    };
                    self.surface.canvas().clip_shader(shader, None);
                    self.surface.canvas().draw_paint(&fill);
                } else if integer_spans {
                    // A8 image draws use the floating raster pipeline, whose
                    // vector/scalar colour conversion differs by one level
                    // between native and Wasm. Solid SrcOver spans use Skia's
                    // integer blitter and retain the current canvas clip.
                    let Some(pixels) = mask.peek_pixels() else {
                        self.surface.canvas().restore();
                        self.surface.canvas().restore();
                        return Err(PdfError::ResourceLimit("coverage pixels".into()));
                    };
                    let Some(bytes) = pixels.bytes() else {
                        self.surface.canvas().restore();
                        self.surface.canvas().restore();
                        return Err(PdfError::ResourceLimit("coverage bytes".into()));
                    };
                    let mut span = fill.clone();
                    span.set_anti_alias(false);
                    for row in 0..CELL as usize {
                        let row_start = (row + HALO as usize) * pixels.row_bytes() + HALO as usize;
                        let values = &bytes[row_start..row_start + CELL as usize];
                        let mut first = 0;
                        while first < values.len() {
                            let coverage = values[first];
                            let mut end = first + 1;
                            while end < values.len() && values[end] == coverage {
                                end += 1;
                            }
                            if coverage != 0 {
                                let alpha =
                                    (u16::from(fill.alpha()) * u16::from(coverage) + 127) / 255;
                                span.set_alpha(alpha as u8);
                                self.surface.canvas().draw_rect(
                                    skia_safe::Rect::from_xywh(
                                        x as f32 + first as f32,
                                        y as f32 + row as f32,
                                        (end - first) as f32,
                                        1.,
                                    ),
                                    &span,
                                );
                            }
                            first = end;
                        }
                    }
                } else {
                    self.surface.canvas().draw_image(
                        &image,
                        ((x - HALO) as f32, (y - HALO) as f32),
                        Some(&fill),
                    );
                }
                self.surface.canvas().restore();
            }
        }
        self.surface.canvas().restore();
        Ok(())
    }

    fn draw_path(&mut self, pathobject: PathObject, paintstyle: PaintStyle) -> PdfResult<()> {
        self.active_intent = pathobject.graphic_state().general_state.rendering_intent;
        let ctm = pathobject.matrix().checked_mul(&self.matrix)?;
        let global = pathobject.matrix().checked_mul(&Matrix::new(
            self.scale_x,
            0.,
            0.,
            self.scale_y,
            0.,
            0.,
        ))?;
        Self::validate_path_coordinates(pathobject.pdf_path(), &ctm)?;
        self.surface.canvas().save();
        self.pathbuilder.reset();
        self.paint.reset();
        self.paint.set_anti_alias(true);
        let result = (|| -> PdfResult<()> {
            self.set_clip(&pathobject.graphic_state().clip_path)?;
            let path_m44 =
                self.pdf_to_skia_m44(ctm.a(), ctm.b(), ctm.c(), ctm.d(), ctm.e(), ctm.f());
            self.surface.canvas().set_matrix(&path_m44);
            let graphic_state = pathobject.graphic_state();
            self.set_path_paint_state(graphic_state);
            self.build_path(&pathobject);
            match paintstyle {
                PaintStyle::Fill => {
                    self.fill_path(graphic_state, &ctm)?;
                    self.draw_prepared_path(
                        &self.pathbuilder.snapshot(),
                        &self.paint.clone(),
                        &global,
                    )?;
                }
                PaintStyle::Stroke => {
                    self.stroke_path(graphic_state, &ctm)?;
                    self.draw_prepared_path(
                        &self.pathbuilder.snapshot(),
                        &self.paint.clone(),
                        &global,
                    )?;
                }
                PaintStyle::StrokeAndFill => {
                    self.fill_path(graphic_state, &ctm)?;
                    let fill = self.paint.clone();
                    self.held_pattern_surfaces =
                        u64::from(matches!(
                            graphic_state.color_state.fill_color_space,
                            pdf::color::colorspace::ColorSpace::Pattern(_)
                        )) + u64::from(graphic_state.general_state.soft_mask.is_some());
                    let stroke_result = self.stroke_path(graphic_state, &ctm);
                    self.held_pattern_surfaces = 0;
                    stroke_result?;
                    // Neither side paints until both colour sources are prepared.
                    self.draw_prepared_path(&self.pathbuilder.snapshot(), &fill, &global)?;
                    self.draw_prepared_path(
                        &self.pathbuilder.snapshot(),
                        &self.paint.clone(),
                        &global,
                    )?;
                }
            }
            Ok(())
        })();
        self.surface.canvas().restore();
        result?;
        Ok(())
    }
}

impl Device for SkiaRender {
    fn begin_group(&mut self, group: pdf::page::transparency::GroupPaint) -> PdfResult<()> {
        self.paint.set_shader(None);
        if !group.alpha.is_finite() || !(0.0..=1.).contains(&group.alpha) {
            return Err(PdfError::PageOperatorError("invalid group alpha".into()));
        }
        if self.group_stack.len() >= self.limits.max_depth || self.group_stack.len() >= 128 {
            return Err(PdfError::ResourceLimit("render group depth".into()));
        }
        let pixels = self.page_width as u64 * self.page_height as u64;
        let bytes = usize::try_from(pixels)
            .ok()
            .and_then(|v| v.checked_mul(4))
            .ok_or_else(|| PdfError::ResourceLimit("render group bytes".into()))?;
        let (clip_bytes, clip_pixels) = self.clip_image_usage()?;
        let mut limits = self.limits;
        let needs_shape =
            group.knockout || self.group_stack.last().is_some_and(|g| g.shape.is_some());
        // A retained initial backdrop may trigger copy-on-write when nested
        // inside an isolated knockout group, so reserve that page as well.
        let count = (if group.isolated { 1 } else { 3 }) + usize::from(needs_shape);
        let reserved = bytes
            .checked_mul(count)
            .ok_or_else(|| PdfError::ResourceLimit("aggregate group surface bytes".into()))?;
        limits.max_render_bytes = limits
            .max_render_bytes
            .checked_sub(reserved)
            .filter(|v| v.checked_sub(clip_bytes).is_some_and(|n| n >= bytes))
            .ok_or_else(|| PdfError::ResourceLimit("aggregate group surface bytes".into()))?;
        limits.max_render_pixels = limits
            .max_render_pixels
            .checked_sub(pixels * count as u64)
            .filter(|v| v.checked_sub(clip_pixels).is_some_and(|n| n >= pixels))
            .ok_or_else(|| PdfError::ResourceLimit("aggregate group surface pixels".into()))?;
        self.group_stack
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("render group stack".into()))?;
        let mut surface = surfaces::raster_n32_premul((self.page_width, self.page_height))
            .ok_or_else(|| PdfError::ResourceLimit("group offscreen surface".into()))?;
        surface.canvas().clear(Color::TRANSPARENT);
        let initial = if group.isolated {
            None
        } else {
            let image = if let Some(image) = self.initial_override.take() {
                image
            } else if let Some(parent) = self.group_stack.last_mut().filter(|g| g.paint.knockout) {
                if parent.paint.isolated {
                    surface.image_snapshot()
                } else {
                    parent.initial.as_ref().unwrap().clone()
                }
            } else {
                self.surface.image_snapshot()
            };
            surface.canvas().draw_image(&image, (0., 0.), None);
            Some(image)
        };
        let coverage = if group.isolated {
            None
        } else {
            let mut coverage = surfaces::raster_n32_premul((self.page_width, self.page_height))
                .ok_or_else(|| PdfError::ResourceLimit("group alpha surface".into()))?;
            coverage.canvas().clear(Color::TRANSPARENT);
            Some(coverage)
        };
        let shape = if needs_shape {
            Some(self.empty_object_surface()?)
        } else {
            None
        };
        let parent = std::mem::replace(&mut self.surface, surface);
        self.group_stack.push(RenderGroup {
            parent,
            limits: self.limits,
            paint: group,
            coverage,
            shape,
            initial,
        });
        self.limits = limits;
        Ok(())
    }

    fn end_group(&mut self, commit: bool) -> PdfResult<()> {
        self.paint.set_shader(None);
        let mut group = self
            .group_stack
            .pop()
            .ok_or_else(|| PdfError::PageOperatorError("unmatched render group end".into()))?;
        let mut surface = std::mem::replace(&mut self.surface, group.parent);
        self.limits = group.limits;
        if !commit {
            return Ok(());
        }
        let image = if let Some(mut coverage) = group.coverage.take() {
            let bytes = (self.page_width as usize)
                .checked_mul(self.page_height as usize)
                .and_then(|v| v.checked_mul(4))
                .ok_or_else(|| PdfError::ResourceLimit("group uncompositing bytes".into()))?;
            // Parent, initial backdrop, colour, opacity, optional shape, plus
            // output Vec/native copy. Initial may alias but is counted in full.
            let peak = 6 + usize::from(group.shape.is_some());
            if bytes
                .checked_mul(peak)
                .is_none_or(|v| v > self.limits.max_render_bytes)
                || (self.page_width as u64 * self.page_height as u64)
                    .checked_mul(peak as u64)
                    .is_none_or(|v| v > self.limits.max_render_pixels)
            {
                return Err(PdfError::ResourceLimit("group uncompositing peak".into()));
            }
            let source = surface
                .peek_pixels()
                .ok_or_else(|| PdfError::ResourceLimit("group colour pixels".into()))?;
            let backdrop = group
                .initial
                .as_ref()
                .and_then(|image| image.peek_pixels())
                .ok_or_else(|| PdfError::ResourceLimit("group backdrop pixels".into()))?;
            let alpha = coverage
                .peek_pixels()
                .ok_or_else(|| PdfError::ResourceLimit("group alpha pixels".into()))?;
            let (src, bg, mask) = (
                source
                    .bytes()
                    .ok_or_else(|| PdfError::ResourceLimit("group colour bytes".into()))?,
                backdrop
                    .bytes()
                    .ok_or_else(|| PdfError::ResourceLimit("group backdrop bytes".into()))?,
                alpha
                    .bytes()
                    .ok_or_else(|| PdfError::ResourceLimit("group alpha bytes".into()))?,
            );
            let mut output = Vec::new();
            output
                .try_reserve_exact(bytes)
                .map_err(|_| PdfError::ResourceLimit("group uncompositing buffer".into()))?;
            output.resize(bytes, 0u8);
            // Native N32 on the supported little-endian backends stores alpha
            // last; RGB byte order is immaterial because all three agree.
            if cfg!(target_endian = "big") {
                return Err(PdfError::UnsupportedFeature(
                    "nonisolated group native big-endian pixels".into(),
                ));
            }
            for y in 0..self.page_height as usize {
                for x in 0..self.page_width as usize {
                    let a = mask[y * alpha.row_bytes() + x * 4 + 3];
                    let inverse = 1. - f64::from(a) / 255.;
                    for channel in 0..3 {
                        let v = f64::from(src[y * source.row_bytes() + x * 4 + channel])
                            - inverse * f64::from(bg[y * backdrop.row_bytes() + x * 4 + channel]);
                        output[(y * self.page_width as usize + x) * 4 + channel] =
                            v.round().clamp(0., f64::from(a)) as u8;
                    }
                    output[(y * self.page_width as usize + x) * 4 + 3] = a;
                }
            }
            skia_safe::images::raster_from_data(
                &self.surface.image_info(),
                skia_safe::Data::new_copy(&output),
                self.page_width as usize * 4,
            )
            .ok_or_else(|| PdfError::ResourceLimit("group corrected image".into()))?
        } else {
            surface.image_snapshot()
        };
        let shape = group.shape.as_mut().map(Surface::image_snapshot);
        if self.dispatch_group_object(|device| {
            let source = if device.shape_mode {
                shape.as_ref().unwrap_or(&image)
            } else {
                &image
            };
            device
                .capture_group_alpha(|device| device.composite_group_image(source, &group.paint))?;
            device.composite_group_image(source, &group.paint)
        })? {
            return Ok(());
        }
        self.capture_group_alpha(|device| device.composite_group_image(&image, &group.paint))?;
        self.composite_group_image(&image, &group.paint)
    }

    fn start_page(&mut self, page_width: f32, page_height: f32) -> PdfResult<()> {
        self.raster_path_budget = self.limits.max_operation_bytes / 16;
        self.curved_raster_paths = false;
        self.typeface_cache
            .retain(|_, (font, _, _)| font.strong_count() != 0);
        self.typeface_cache_bytes = self
            .typeface_cache
            .values()
            .map(|(_, _, bytes)| *bytes)
            .sum();
        while !self.group_stack.is_empty() {
            self.end_group(false)?;
        }
        let pixel_width = (f64::from(self.scale_x) * f64::from(page_width)).round();
        let pixel_height = (f64::from(self.scale_y) * f64::from(page_height)).round();
        if !page_width.is_finite()
            || !page_height.is_finite()
            || pixel_width <= 0.0
            || pixel_height <= 0.0
        {
            return Err(PdfError::DocumentError(
                "invalid render page dimensions".into(),
            ));
        }
        if !pixel_width.is_finite()
            || !pixel_height.is_finite()
            || pixel_width > f64::from(i32::MAX)
            || pixel_height > f64::from(i32::MAX)
        {
            return Err(PdfError::ResourceLimit(
                "render dimensions exceed backend range".into(),
            ));
        }
        let effective = if let Some(region) = self.requested_region {
            if self.region_padding != 0
                && (f64::from(region.x + region.width) > pixel_width
                    || f64::from(region.y + region.height) > pixel_height)
            {
                return Err(PdfError::DocumentError("raster region exceeds page".into()));
            }
            let pad = self.region_padding;
            let x = region.x.saturating_sub(pad);
            let y = region.y.saturating_sub(pad);
            let right = if pad == 0 {
                region.x + region.width
            } else {
                (region.x + region.width + pad).min(pixel_width as u32)
            };
            let bottom = if pad == 0 {
                region.y + region.height
            } else {
                (region.y + region.height + pad).min(pixel_height as u32)
            };
            Some(RasterRegion {
                x,
                y,
                width: right - x,
                height: bottom - y,
            })
        } else {
            None
        };
        let (width, height) = effective.map_or((pixel_width as i32, pixel_height as i32), |r| {
            (r.width as i32, r.height as i32)
        });
        let pixels = width as u64 * height as u64;
        if pixels > self.limits.max_render_pixels
            || pixels
                .checked_mul(4)
                .is_none_or(|bytes| bytes > self.limits.max_render_bytes as u64)
        {
            return Err(PdfError::ResourceLimit(
                "render surface exceeds pixel/byte budget".into(),
            ));
        }
        let surface = surfaces::raster_n32_premul((width, height))
            .ok_or_else(|| PdfError::ResourceLimit("unable to allocate render surface".into()))?;
        self.surface = surface;
        self.clip_cache.clear();
        self.clip_images.clear();
        self.region = effective;
        if self.region_padding == 0 {
            if let Some(region) = effective {
                if f64::from(region.x + region.width) > pixel_width
                    || f64::from(region.y + region.height) > pixel_height
                {
                    // A rounded-up parent pixel may exceed a supersampled
                    // child's last integer row/column. Preserve the old zero
                    // contribution without allocating the full child page.
                    let w = (pixel_width - f64::from(region.x))
                        .max(0.)
                        .min(f64::from(width)) as f32;
                    let h = (pixel_height - f64::from(region.y))
                        .max(0.)
                        .min(f64::from(height)) as f32;
                    self.surface.canvas().clip_rect(
                        skia_safe::Rect::from_wh(w, h),
                        skia_safe::ClipOp::Intersect,
                        false,
                    );
                }
            }
        }
        self.matrix = Matrix::new(
            self.scale_x,
            0.,
            0.,
            self.scale_y,
            -(effective.map_or(0, |r| r.x) as f32),
            -(effective.map_or(0, |r| r.y) as f32),
        );
        self.page_width = width;
        self.page_height = height;
        // The page group blends over a transparent backdrop; white is the
        // output substrate, added only after page compositing.
        self.surface.canvas().clear(Color::TRANSPARENT);
        Ok(())
    }

    fn show_text(&mut self, textobject: TextObject) -> PdfResult<()> {
        // Default PDF text knockout (TK=true) treats each glyph as a member,
        // not a whole Tj run. TK=false is rejected by ExtGState validation.
        if !self.object_dispatch
            && !self.shape_mode
            && self.group_stack.last().is_some_and(|g| g.paint.knockout)
            && textobject.items.len() > 1
        {
            for item in &textobject.items {
                let mut glyph = textobject.clone();
                glyph.items = vec![item.clone()];
                self.show_text(glyph)?;
            }
            return Ok(());
        }
        if self.dispatch_group_object(|device| device.show_text(textobject.clone()))? {
            return Ok(());
        }
        if matches!(
            textobject.state.text_state.text_rendering_mode,
            TextRenderingMode::FillStroke | TextRenderingMode::FillStrokeClip
        ) && self.needs_combined_group(&textobject.state)
        {
            let state = textobject.state.clone();
            return self.combined_group(&state, |device| {
                let mut fill = textobject.clone();
                fill.state.text_state.text_rendering_mode = TextRenderingMode::Fill;
                device.show_text(fill)?;
                let mut stroke = textobject;
                stroke.state.text_state.text_rendering_mode = TextRenderingMode::Stroke;
                device.show_text(stroke)
            });
        }
        self.active_intent = textobject.state.general_state.rendering_intent;
        self.capture_group_alpha(|device| device.show_text(textobject.clone()))?;
        // Type3 shapes were executed as ordinary graphics by the interpreter.
        // Keep its text event for extraction, never ask Skia for a font file.
        if textobject.is_type3_metadata() {
            return Ok(());
        }
        if matches!(
            textobject.state.text_state.font.as_deref(),
            Some(pdf::font::pdf_font::Font::Type3(_))
        ) {
            return Err(PdfError::UnsupportedFeature(
                "Type3 raw text requires interpreted glyph graphics".into(),
            ));
        }
        self.paint.reset();
        self.paint.set_anti_alias(true);
        self.paint
            .set_blend_mode(self.effective_blend_mode(textobject.state.general_state.blend_mode));
        let text_matrix = textobject.matrix.checked_mul(&self.matrix)?;
        let text_sign = if textobject.state.text_state.font_size < 0. {
            -1.
        } else {
            1.
        };
        let text_paint_ctm = Matrix::new(
            text_matrix.a() * text_sign,
            text_matrix.b() * text_sign,
            -text_matrix.c() * text_sign,
            -text_matrix.d() * text_sign,
            text_matrix.e(),
            text_matrix.f(),
        );
        let mode = match textobject.state.text_state.text_rendering_mode {
            TextRenderingMode::FillClip
            | TextRenderingMode::StrokeClip
            | TextRenderingMode::FillStrokeClip
            | TextRenderingMode::Clip
                if !textobject.has_interpreted_clipping() =>
            {
                return Err(PdfError::UnsupportedFeature(
                    "raw text clipping requires interpreted ET geometry".into(),
                ));
            }
            TextRenderingMode::FillClip => TextRenderingMode::Fill,
            TextRenderingMode::StrokeClip => TextRenderingMode::Stroke,
            TextRenderingMode::FillStrokeClip => TextRenderingMode::FillStroke,
            TextRenderingMode::Clip => return Ok(()),
            ref mode => mode.clone(),
        };
        let fill_stroke = matches!(mode, TextRenderingMode::FillStroke);
        let stroke_only = matches!(mode, TextRenderingMode::Stroke);
        let stroke_ctm = textobject.paint_matrix().checked_mul(&self.matrix)?;
        match mode {
            TextRenderingMode::Fill => {
                self.set_color_source(
                    &textobject.state.color_state.fill_color_space,
                    &textobject.state.color_state.fill_color_value,
                    &text_paint_ctm,
                )?;
                self.set_paint_alpha(textobject.state.general_state.fill_alpha)?;
                self.paint.set_style(PaintStyle::Fill);
            }
            TextRenderingMode::Stroke => {
                self.set_color_source(
                    &textobject.state.color_state.stroke_color_space,
                    &textobject.state.color_state.stroke_color_value,
                    &stroke_ctm,
                )?;
                self.set_paint_alpha(textobject.state.general_state.stroke_alpha)?;
                self.set_path_paint_state(&textobject.state);
                self.paint.set_path_effect(PathEffect::dash(
                    &textobject.state.graph_state.dash_array,
                    textobject.state.graph_state.dash_phrase,
                ));
                self.paint.set_stroke_width(Self::device_stroke_width(
                    textobject.state.graph_state.line_width,
                    &stroke_ctm,
                ));
                self.paint
                    .set_stroke_miter(textobject.state.graph_state.miter_limit);
                self.paint.set_style(PaintStyle::Stroke);
            }
            TextRenderingMode::FillStroke => {
                self.set_color_source(
                    &textobject.state.color_state.fill_color_space,
                    &textobject.state.color_state.fill_color_value,
                    &text_paint_ctm,
                )?;
                self.set_paint_alpha(textobject.state.general_state.fill_alpha)?;
                self.paint.set_style(PaintStyle::Fill);
            }
            TextRenderingMode::INVisible => return Ok(()),
            TextRenderingMode::FillClip
            | TextRenderingMode::StrokeClip
            | TextRenderingMode::FillStrokeClip
            | TextRenderingMode::Clip => {
                return Err(PdfError::UnsupportedFeature(
                    "text clipping rendering modes".into(),
                ));
            }
        }
        self.apply_soft_mask(
            textobject.state.general_state.soft_mask.as_deref(),
            if stroke_only {
                &stroke_ctm
            } else {
                &text_paint_ctm
            },
        )?;
        // Prepare both paints before touching the canvas. Keep sibling shaders
        // charged while a stroke Pattern or soft mask allocates offscreen data.
        let fill_paint = self.paint.clone();
        let stroke_paint = if fill_stroke {
            self.held_pattern_surfaces =
                u64::from(matches!(
                    textobject.state.color_state.fill_color_space,
                    pdf::color::colorspace::ColorSpace::Pattern(_)
                )) + u64::from(textobject.state.general_state.soft_mask.is_some());
            let result = (|| -> PdfResult<Paint> {
                self.set_color_source(
                    &textobject.state.color_state.stroke_color_space,
                    &textobject.state.color_state.stroke_color_value,
                    &stroke_ctm,
                )?;
                self.set_paint_alpha(textobject.state.general_state.stroke_alpha)?;
                self.set_path_paint_state(&textobject.state);
                self.paint.set_path_effect(PathEffect::dash(
                    &textobject.state.graph_state.dash_array,
                    textobject.state.graph_state.dash_phrase,
                ));
                self.paint.set_stroke_width(Self::device_stroke_width(
                    textobject.state.graph_state.line_width,
                    &stroke_ctm,
                ));
                self.paint
                    .set_stroke_miter(textobject.state.graph_state.miter_limit);
                self.paint.set_style(PaintStyle::Stroke);
                self.apply_soft_mask(
                    textobject.state.general_state.soft_mask.as_deref(),
                    &stroke_ctm,
                )?;
                Ok(self.paint.clone())
            })();
            self.held_pattern_surfaces = 0;
            Some(result?)
        } else {
            None
        };
        if let Some(font) = &textobject.state.text_state.font {
            let objmatrix = textobject.matrix.checked_mul(&self.matrix)?;
            // Derive face-local positions before applying a tile translation.
            // Subtracting a distant tile origin in f32 can change glyph phase.
            let position_scale = Matrix::new(self.scale_x, 0., 0., self.scale_y, 0., 0.);
            let position_matrix = textobject.matrix.checked_mul(&position_scale)?;
            // Skia glyph outlines use downward y; PDF text outlines use upward y.
            let size = textobject.state.text_state.font_size;
            if !size.is_finite() || !textobject.horizontal_scale.is_finite() {
                return Err(PdfError::PageOperatorError("non-finite text scale".into()));
            }
            let sign = if size < 0.0 { -1.0 } else { 1.0 };
            let (a, b, c, d) = (
                objmatrix.a() * sign,
                objmatrix.b() * sign,
                -objmatrix.c() * sign,
                -objmatrix.d() * sign,
            );
            let determinant = a as f64 * d as f64 - b as f64 * c as f64;
            if determinant == 0.0 {
                return Ok(()); // A collapsed text transform has no painted area.
            }

            let substituted = font.fontfile().is_none();
            let typeface = if !substituted {
                Some(self.embedded_typeface(font)?)
            } else {
                self.substitute_font.clone()
            };
            let make_font = |face| {
                let mut value = skia_safe::Font::new(face, size.abs());
                value.set_edging(skia_safe::font::Edging::SubpixelAntiAlias);
                value.set_subpixel(true);
                value.set_scale_x(textobject.horizontal_scale);
                value
            };
            let mut current_font = typeface.map(&make_font);
            let mut current_face = None;
            let mut runs: Vec<(usize, skia_safe::Font)> = Vec::new();
            let mut glyphs = Vec::new();
            let mut positions = Vec::new();

            for char_item in textobject.items.iter() {
                let item_pos = position_scale.checked_transform(char_item.pos())?;
                let dx = item_pos.x() as f64 - position_matrix.e() as f64;
                let dy = item_pos.y() as f64 - position_matrix.f() as f64;
                let mut x = ((d as f64 * dx - c as f64 * dy) / determinant) as f32;
                let mut y = ((a as f64 * dy - b as f64 * dx) / determinant) as f32;
                if font.writting_mode() == pdf::font::WritingMode::Vertical {
                    let offset = Self::vertical_glyph_offset(
                        char_item.charcode(),
                        size.abs(),
                        textobject.horizontal_scale,
                    )?;
                    x += offset.0;
                    y += offset.1;
                }
                if !x.is_finite() || !y.is_finite() {
                    return Err(PdfError::PageOperatorError(
                        "text position transform overflows".into(),
                    ));
                }
                let glyph = if substituted {
                    use crate::font_provider::{GlyphChoice, choose_glyph_with_language};
                    let (face_index, gid) = match choose_glyph_with_language(
                        self.font_provider.as_deref(),
                        self.substitute_font.as_ref(),
                        font,
                        char_item.charcode(),
                        textobject.language(),
                    )? {
                        GlyphChoice::Registered(resolved) => {
                            if current_face != Some(resolved.provider_index + 1) {
                                current_font = Some(make_font(
                                    self.font_provider
                                        .as_ref()
                                        .unwrap()
                                        .face(resolved.provider_index),
                                ));
                            }
                            (resolved.provider_index + 1, resolved.glyph)
                        }
                        GlyphChoice::Legacy(gid) => {
                            if current_face != Some(0) {
                                current_font =
                                    Some(make_font(self.substitute_font.as_ref().unwrap().clone()));
                            }
                            (0, gid)
                        }
                        GlyphChoice::MissingUnicode => {
                            return Err(PdfError::FontError(
                                "substitution requires a Unicode mapping".into(),
                            ));
                        }
                        GlyphChoice::NeedsShaping => {
                            return Err(PdfError::UnsupportedFeature(
                                "substitute glyph needs text shaping for multi-character mapping"
                                    .into(),
                            ));
                        }
                        GlyphChoice::MissingGlyph(ch) => {
                            return Err(PdfError::FontError(format!(
                                "font '{}' needs a substitute covering U+{:04X}",
                                font.name(),
                                ch as u32
                            )));
                        }
                    };
                    if current_face != Some(face_index) {
                        runs.push((glyphs.len(), current_font.as_ref().unwrap().clone()));
                        current_face = Some(face_index);
                    }
                    Some(GlyphDesc::Gid(gid as u32))
                } else {
                    font.raw_cff_cid(char_item.charcode())
                        .map(GlyphDesc::Gid)
                        .or_else(|| font.get_glyph(char_item.charcode()))
                };
                match glyph {
                    // Embedded glyph zero is the font's defined .notdef glyph;
                    // PDF producers also use an empty .notdef for spacing.
                    Some(GlyphDesc::Gid(gid))
                        if gid <= u16::MAX as u32 && (!substituted || gid != 0) =>
                    {
                        glyphs.push(gid as u16);
                        positions.push(skia_safe::Point::new(x, y));
                    }
                    Some(GlyphDesc::Name(name)) => {
                        return Err(PdfError::UnsupportedFeature(format!(
                            "font '{}' uses unresolved glyph name '{}'",
                            font.name(),
                            name
                        )));
                    }
                    _ => {
                        return Err(PdfError::FontError(format!(
                            "font '{}' has no drawable glyph for character code {}",
                            font.name(),
                            char_item.charcode().code()
                        )));
                    }
                }
            }
            if !substituted {
                runs.push((0, current_font.unwrap()));
            }
            let global_stroke_ctm = textobject.paint_matrix().checked_mul(&position_scale)?;
            let stroke_path_matrix = if stroke_only || stroke_paint.is_some() {
                let relative = Matrix::new(a, b, c, d, position_matrix.e(), position_matrix.f())
                    .checked_mul(&global_stroke_ctm.checked_inverse()?)?;
                let path_matrix = skia_safe::Matrix::new_all(
                    relative.a(),
                    relative.c(),
                    relative.e(),
                    relative.b(),
                    relative.d(),
                    relative.f(),
                    0.,
                    0.,
                    1.,
                );
                Some(path_matrix)
            } else {
                None
            };
            let transform = self.pdf_to_skia_m44(a, b, c, d, objmatrix.e(), objmatrix.f());
            self.surface.canvas().save();
            if let Err(error) = self.set_clip(&textobject.state.clip_path) {
                self.surface.canvas().restore();
                return Err(error);
            }
            self.surface.canvas().set_matrix(&transform);
            if !stroke_only {
                let global = Matrix::new(a, b, c, d, position_matrix.e(), position_matrix.f());
                for (i, (start, run_font)) in runs.iter().enumerate() {
                    let end = runs.get(i + 1).map_or(glyphs.len(), |r| r.0);
                    for (glyph, position) in glyphs[*start..end].iter().zip(&positions[*start..end])
                    {
                        if let Err(error) = self.draw_filled_glyph(
                            run_font,
                            *glyph,
                            *position,
                            &fill_paint,
                            &global,
                        ) {
                            self.surface.canvas().restore();
                            return Err(error);
                        }
                    }
                }
            }
            if let Some(path_matrix) = &stroke_path_matrix {
                // Transform outlines into PDF user space before stroking. A Tm
                // scale changes glyph geometry, not the graphics-state width.
                let paint = stroke_paint.as_ref().unwrap_or(&fill_paint);
                let transform = self.pdf_to_skia_m44(
                    stroke_ctm.a(),
                    stroke_ctm.b(),
                    stroke_ctm.c(),
                    stroke_ctm.d(),
                    stroke_ctm.e(),
                    stroke_ctm.f(),
                );
                self.surface.canvas().set_matrix(&transform);
                for (i, (start, run_font)) in runs.iter().enumerate() {
                    let end = runs.get(i + 1).map_or(glyphs.len(), |r| r.0);
                    for (glyph, position) in glyphs[*start..end].iter().zip(&positions[*start..end])
                    {
                        if let Some(path) = run_font.get_path(*glyph) {
                            let path = path.with_offset(*position).with_transform(path_matrix);
                            if let Err(error) =
                                self.draw_prepared_path(&path, paint, &global_stroke_ctm)
                            {
                                self.surface.canvas().restore();
                                return Err(error);
                            }
                        }
                    }
                }
            }
            self.surface.canvas().restore();
        }
        Ok(())
    }

    fn do_shading(&mut self, object: pdf::page::shading_object::ShadingObject) -> PdfResult<()> {
        if self.dispatch_group_object(|device| device.do_shading(object.clone()))? {
            return Ok(());
        }
        self.active_intent = object.graphic_state.general_state.rendering_intent;
        self.capture_group_alpha(|device| device.do_shading(object.clone()))?;
        if object.shading.colorspace().is_nonpainting() {
            return Ok(());
        }
        let alpha = object.graphic_state.general_state.fill_alpha;
        let color_context = pdf::color::conversion::ColorConversionContext::default()
            .with_intent(self.active_intent);
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(PdfError::PageOperatorError("invalid shading alpha".into()));
        }
        let global = Matrix::new(self.scale_x, 0., 0., self.scale_y, 0., 0.);
        let matrix = object.matrix.checked_mul(&global)?;
        let origin = self.region.map_or([0, 0], |r| [r.x, r.y]);
        let [a, b, c, d, e, f] = [
            matrix.a(),
            matrix.b(),
            matrix.c(),
            matrix.d(),
            matrix.e(),
            matrix.f(),
        ]
        .map(f64::from);
        let determinant = a * d - b * c;
        if determinant == 0.0 {
            return Ok(());
        }
        if self.page_width <= 0 || self.page_height <= 0 {
            return Err(PdfError::DocumentError(
                "shading requires an active page".into(),
            ));
        }
        let bytes = (self.page_width as usize)
            .checked_mul(self.page_height as usize)
            .and_then(|pixels| pixels.checked_mul(4))
            .ok_or_else(|| PdfError::ResourceLimit("shading raster size overflow".into()))?;
        // Surface, Rust raster, and Skia's owned copy coexist. Native bookkeeping
        // and temporary allocations inside color/function evaluation are excluded.
        if bytes
            .checked_mul(3)
            .is_none_or(|peak| peak > self.limits.max_render_bytes)
        {
            return Err(PdfError::ResourceLimit(
                "shading raster exceeds byte budget".into(),
            ));
        }
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(bytes)
            .map_err(|_| PdfError::ResourceLimit("unable to allocate shading raster".into()))?;
        rgba.resize(bytes, 0);
        // A viewport must not decide between exact point sampling and forward
        // approximation based on which pixels happen to be visible. Patch
        // geometry always uses the same bounded forward display rasterizer;
        // the public certified point-sampling API is unchanged.
        let patch_raster = matches!(
            object.shading.geometry(),
            pdf::color::shading::ShadingGeometry::CoonsMesh
                | pdf::color::shading::ShadingGeometry::TensorMesh
        );
        if !patch_raster {
            for y in 0..self.page_height {
                for x in 0..self.page_width {
                    let px = f64::from(x) + f64::from(origin[0]) + 0.5 - e;
                    let py = f64::from(y) + f64::from(origin[1]) + 0.5 - f;
                    let ux = (d * px - c * py) / determinant;
                    let uy = (-b * px + a * py) / determinant;
                    let color =
                        object
                            .shading
                            .color_at_point_with_context(ux, uy, &color_context)?;
                    let color = if color.is_none()
                        && object.uses_pattern_background()
                        && object.shading.bbox().is_none_or(|b| {
                            ux >= f64::from(b[0])
                                && uy >= f64::from(b[1])
                                && ux <= f64::from(b[2])
                                && uy <= f64::from(b[3])
                        }) {
                        object
                            .shading
                            .background()
                            .map(|value| {
                                object
                                    .shading
                                    .colorspace()
                                    .rgb_with_context(value, &color_context)
                            })
                            .transpose()?
                    } else {
                        color
                    };
                    if let Some(color) = color {
                        let offset = (y as usize * self.page_width as usize + x as usize) * 4;
                        rgba[offset..offset + 4].copy_from_slice(&[
                            color.r(),
                            color.g(),
                            color.b(),
                            255,
                        ]);
                    }
                }
            }
        }
        if patch_raster {
            object.shading.rasterize_patch_region_into(
                pdf::color::shading::PatchRasterOptions {
                    width: self.page_width as u32,
                    height: self.page_height as u32,
                    matrix: [a, b, c, d, e, f],
                    max_scratch_bytes: self.limits.max_render_bytes.saturating_sub(bytes * 2),
                    paint_background: object.uses_pattern_background(),
                },
                origin,
                &color_context,
                &mut rgba,
            )?;
        }
        let info = skia_safe::ImageInfo::new(
            (self.page_width, self.page_height),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        let image = skia_safe::images::raster_from_data(
            &info,
            skia_safe::Data::new_copy(&rgba),
            info.min_row_bytes(),
        )
        .ok_or_else(|| PdfError::ResourceLimit("Skia rejected shading raster".into()))?;
        drop(rgba);
        let mut paint = Paint::default();
        paint.set_alpha(Self::opacity_byte(alpha, self.shape_mode));
        paint.set_blend_mode(
            self.effective_blend_mode(object.graphic_state.general_state.blend_mode),
        );
        let masked = object.graphic_state.general_state.soft_mask.is_some();
        if masked {
            self.paint = paint;
            self.paint.set_shader(
                image
                    .to_shader(
                        (skia_safe::TileMode::Decal, skia_safe::TileMode::Decal),
                        skia_safe::SamplingOptions::default(),
                        None,
                    )
                    .ok_or_else(|| PdfError::ResourceLimit("masked shading shader".into()))?,
            );
            self.apply_soft_mask(
                object.graphic_state.general_state.soft_mask.as_deref(),
                &Matrix::identity(),
            )?;
            paint = self.paint.clone();
            self.paint.set_shader(None);
        }
        self.surface.canvas().save();
        let result = self.set_clip(&object.graphic_state.clip_path);
        if result.is_ok() {
            self.surface.canvas().reset_matrix();
            if masked {
                self.surface.canvas().draw_rect(
                    skia_safe::Rect::from_wh(self.page_width as f32, self.page_height as f32),
                    &paint,
                );
            } else {
                self.surface
                    .canvas()
                    .draw_image(&image, (0.0, 0.0), Some(&paint));
            }
        }
        self.surface.canvas().restore();
        result
    }

    fn do_image(&mut self, imageobject: ImageObject) -> PdfResult<()> {
        if self.dispatch_group_object(|device| device.do_image(imageobject.clone()))? {
            return Ok(());
        }
        self.active_intent = imageobject
            .image
            .rendering_intent()
            .unwrap_or(imageobject.graphic_state.general_state.rendering_intent);
        self.capture_group_alpha(|device| device.do_image(imageobject.clone()))?;
        let matrix = &imageobject.matrix;
        let ctm = matrix.checked_mul(&self.matrix)?;
        let m44 = self.pdf_to_skia_m44(ctm.a(), ctm.b(), ctm.c(), ctm.d(), ctm.e(), ctm.f());
        let pattern_stencil = imageobject.image.is_mask()
            && matches!(
                imageobject.graphic_state.color_state.fill_color_space,
                pdf::color::colorspace::ColorSpace::Pattern(_)
            );
        let (raster_width, raster_height, mut rgba) = if imageobject.image.is_mask() {
            let state = &imageobject.graphic_state.color_state;
            let color = if pattern_stencil {
                pdf::color::value::ColorRgb::new(1., 1., 1.)
            } else {
                state.fill_color_space.rgb_with_context(
                    &state.fill_color_value,
                    &pdf::color::conversion::ColorConversionContext::default()
                        .with_intent(self.active_intent),
                )?
            };
            let (width, height) = imageobject.image.pixel_dimensions();
            (
                width,
                height,
                imageobject.image.rgba_buffer_with_stencil_color(&color)?,
            )
        } else {
            imageobject.image.rendering_rgba_with_context(
                &pdf::color::conversion::ColorConversionContext::default()
                    .with_intent(self.active_intent),
            )?
        };
        if self.shape_mode && !imageobject.image.is_mask() && !imageobject.image.has_explicit_mask()
        {
            for pixel in rgba.as_chunks_mut::<4>().0 {
                pixel[3] = 255;
            }
        }
        let (width, height) = (raster_width, raster_height);
        let width = i32::try_from(width)
            .map_err(|_| PdfError::ResourceLimit("image width exceeds backend range".into()))?;
        let height = i32::try_from(height)
            .map_err(|_| PdfError::ResourceLimit("image height exceeds backend range".into()))?;
        let info = skia_safe::ImageInfo::new(
            (width, height),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        let row_bytes = info.min_row_bytes();
        let pixels = skia_safe::Data::new_copy(&rgba);
        let image = skia_safe::images::raster_from_data(&info, pixels, row_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("Skia rejected image pixel buffer".into()))?;
        // Mipmaps integrate source pixels when scans and stencils shrink. A
        // nearest-neighbour sample can erase entire thin strokes at low DPI.
        let shrinking = imageobject.image.interpolate()
            && (ctm.a().hypot(ctm.b()) < width as f32 || ctm.c().hypot(ctm.d()) < height as f32);
        let sampling = if shrinking {
            skia_safe::SamplingOptions::new(
                skia_safe::FilterMode::Linear,
                skia_safe::MipmapMode::Linear,
            )
        } else if imageobject.image.interpolate() {
            skia_safe::SamplingOptions::from(skia_safe::FilterMode::Linear)
        } else {
            skia_safe::SamplingOptions::default()
        };
        let image = if shrinking {
            let mut mip_pixels = 0u64;
            let (mut w, mut h) = (width as u64, height as u64);
            while w > 1 || h > 1 {
                w = (w / 2).max(1);
                h = (h / 2).max(1);
                mip_pixels = mip_pixels
                    .checked_add(w * h)
                    .ok_or_else(|| PdfError::ResourceLimit("image mipmap pixels".into()))?;
            }
            if mip_pixels > self.limits.max_render_pixels
                || mip_pixels > (self.limits.max_render_bytes / 4) as u64
            {
                return Err(PdfError::ResourceLimit("image mipmap storage".into()));
            }
            image
                .with_default_mipmaps()
                .ok_or_else(|| PdfError::ResourceLimit("image mipmap allocation".into()))?
        } else {
            image
        };
        let alpha = imageobject.graphic_state.general_state.fill_alpha;
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(PdfError::PageOperatorError(
                "image alpha must be finite and in [0,1]".into(),
            ));
        }
        let mut image_paint = skia_safe::paint::Paint::default();
        image_paint.set_alpha(Self::opacity_byte(alpha, self.shape_mode));
        image_paint.set_blend_mode(
            self.effective_blend_mode(imageobject.graphic_state.general_state.blend_mode),
        );
        if pattern_stencil {
            let color = &imageobject.graphic_state.color_state;
            self.set_color_source(&color.fill_color_space, &color.fill_color_value, &ctm)?;
            let shader = self
                .paint
                .shader()
                .ok_or_else(|| PdfError::ColorError("missing pattern stencil shader".into()))?;
            let matrix = skia_safe::Matrix::scale((1. / width as f32, 1. / height as f32));
            let mask = image
                .to_shader(
                    (skia_safe::TileMode::Decal, skia_safe::TileMode::Decal),
                    sampling,
                    &matrix,
                )
                .ok_or_else(|| PdfError::ResourceLimit("pattern stencil mask shader".into()))?;
            image_paint.set_shader(skia_safe::shaders::blend(
                skia_safe::BlendMode::DstIn,
                shader,
                mask,
            ));
            // image_paint now owns the shader; do not retain an extra copy.
            self.paint.set_shader(None);
        }
        let masked =
            imageobject.graphic_state.general_state.soft_mask.is_some() && !self.shape_mode;
        if masked {
            self.paint = image_paint;
            if self.paint.shader().is_none() {
                let image_matrix =
                    skia_safe::Matrix::scale((1. / width as f32, 1. / height as f32));
                self.paint.set_shader(
                    image
                        .to_shader(
                            (skia_safe::TileMode::Decal, skia_safe::TileMode::Decal),
                            sampling,
                            &image_matrix,
                        )
                        .ok_or_else(|| PdfError::ResourceLimit("masked image shader".into()))?,
                );
            }
            self.apply_soft_mask(
                imageobject.graphic_state.general_state.soft_mask.as_deref(),
                &ctm,
            )?;
            image_paint = self.paint.clone();
            self.paint.set_shader(None);
        }
        self.surface.canvas().save();
        if let Err(error) = self.set_clip(&imageobject.graphic_state.clip_path) {
            self.surface.canvas().restore();
            return Err(error);
        }
        self.surface.canvas().set_matrix(&m44);
        let dest_rect = skia_safe::Rect::from_wh(1.0, 1.0);
        if pattern_stencil || masked {
            self.surface.canvas().draw_rect(dest_rect, &image_paint);
        } else {
            self.surface.canvas().draw_image_rect_with_sampling_options(
                &image,
                None,
                dest_rect,
                sampling,
                &image_paint,
            );
        }
        self.surface.canvas().restore();
        Ok(())
    }

    fn stroke_path(&mut self, pathobject: PathObject) -> PdfResult<()> {
        if self.dispatch_group_object(|device| Device::stroke_path(device, pathobject.clone()))? {
            return Ok(());
        }
        self.capture_group_alpha(|device| {
            device.draw_path(pathobject.clone(), PaintStyle::Stroke)
        })?;
        self.draw_path(pathobject, PaintStyle::Stroke)?;
        Ok(())
    }

    fn fill_and_stroke_path(&mut self, pathobject: PathObject) -> PdfResult<()> {
        if self.dispatch_group_object(|device| device.fill_and_stroke_path(pathobject.clone()))? {
            return Ok(());
        }
        if self.needs_combined_group(pathobject.graphic_state()) {
            let state = pathobject.graphic_state().clone();
            return self.combined_group(&state, |device| {
                Device::fill_path(device, pathobject.clone())?;
                Device::stroke_path(device, pathobject)
            });
        }
        self.capture_group_alpha(|device| {
            device.draw_path(pathobject.clone(), PaintStyle::StrokeAndFill)
        })?;
        self.draw_path(pathobject, PaintStyle::StrokeAndFill)?;
        Ok(())
    }

    fn fill_path(&mut self, pathobject: PathObject) -> PdfResult<()> {
        if self.dispatch_group_object(|device| Device::fill_path(device, pathobject.clone()))? {
            return Ok(());
        }
        self.capture_group_alpha(|device| device.draw_path(pathobject.clone(), PaintStyle::Fill))?;
        self.draw_path(pathobject, PaintStyle::Fill)?;
        Ok(())
    }

    fn end_page(&mut self) -> PdfResult<()> {
        if self.clear_color != Color::TRANSPARENT {
            let mut background = Paint::default();
            background.set_color(self.clear_color);
            background.set_blend_mode(skia_safe::BlendMode::DstOver);
            self.surface.canvas().draw_paint(&background);
        }
        Ok(())
    }

    fn clip_path(&mut self, _pathobject: PathObject) -> PdfResult<()> {
        Ok(())
    }
}

#[cfg(test)]
fn white_test_output(mut pixels: Vec<u8>) -> Vec<u8> {
    for pixel in pixels.chunks_exact_mut(4) {
        let alpha = u32::from(pixel[3]);
        for channel in &mut pixel[..3] {
            *channel = ((u32::from(*channel) * alpha + 255 * (255 - alpha) + 127) / 255) as u8;
        }
        pixel[3] = 255;
    }
    pixels
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn subpixel_nonzero_strokes_remain_visible_without_collapsing_wide_anisotropic_strokes() {
        use pdf::{document::Document, page::page_builder::PageBuilder};
        let doc = Document::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(20., 20.))
            .unwrap();
        doc.replace_page_contents(
            0,
            b"0.001 w 2 5 m 18 5 l S 0.01 w 2 10 m 18 10 l S 0.1 w 2 15 m 18 15 l S".to_vec(),
        )
        .unwrap();
        let mut renderer = SkiaRender::new(72., 72.).unwrap();
        doc.get_page(0).unwrap().display(&mut renderer).unwrap();
        let pixels = renderer.to_rgba().unwrap().pixels;
        for y in [5usize, 10, 15] {
            assert!((y - 1..=y).any(|row| pixels[(row * 20 + 10) * 4] < 240));
        }
        assert_eq!(
            SkiaRender::device_stroke_width(0.1, &Matrix::new(20., 0., 0., 0.01, 0., 0.)),
            0.1
        );
        assert_eq!(
            SkiaRender::device_stroke_width(1., &Matrix::new(0., 0., 0., 0., 0., 0.)),
            1.
        );
    }

    #[test]
    fn page_blend_modes_use_transparent_initial_backdrop_before_white_output() {
        use pdf::{
            document::Document,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_dict::PdfDict},
            page::page_builder::PageBuilder,
            parser::syntax::SyntaxParser,
        };
        for (mode, color, expected) in [
            ("Screen", "0 1 1", [0, 255, 255, 255]),
            ("SoftLight", "0 0 0", [0, 0, 0, 255]),
        ] {
            let doc = Document::new().unwrap();
            let state = SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(
                    format!("<< /BM /{mode} >>").into_bytes(),
                ))
                .unwrap(),
            )
            .get_object()
            .unwrap();
            let mut states = PdfDict::default();
            states.insert("G".into(), state);
            let mut resources = PdfDict::default();
            resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
            doc.add_page_with_builder(0, PageBuilder::new().size(4., 4.).resources(resources))
                .unwrap();
            let name = "G";
            doc.replace_page_contents(
                0,
                format!("/{name} gs {color} rg 1 1 2 2 re f").into_bytes(),
            )
            .unwrap();
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(0).unwrap().display(&mut renderer).unwrap();
            let image = renderer.to_rgba().unwrap();
            assert_eq!(&image.pixels[20..24], &expected, "{mode}");
            assert_eq!(&image.pixels[..4], &[255, 255, 255, 255]);
        }
    }
    #[test]
    fn nonuniform_coons_boundary_pixels_survive_function_and_strict_save() {
        use pdf::{
            document::{
                PdfDocument,
                resource_manager::ResourceKind,
                save_options::{SaveOptions, StreamCompression},
            },
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::page_builder::PageBuilder,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        for nonlinear in [false, true] {
            let doc = PdfDocument::new().unwrap();
            doc.add_page_with_builder(0, PageBuilder::new().size(50.0, 50.0))
                .unwrap();
            let function = if nonlinear {
                "/Function << /FunctionType 2 /Domain [0 1] /N 2 >>"
            } else {
                ""
            };
            let source = format!(
                "<< /ShadingType 6 /ColorSpace /DeviceGray /BitsPerFlag 8 /BitsPerCoordinate 16 /BitsPerComponent 8 /Decode [0 65535 0 65535 0 1] {function} >>"
            );
            let dict = SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(source.into_bytes())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            let mut data = vec![0];
            for point in [
                [0_u16, 0],
                [0, 512],
                [0, 1024],
                [0, 1536],
                [576, 1536],
                [1088, 1536],
                [1536, 1536],
                [1536, 1024],
                [1536, 512],
                [1536, 0],
                [1088, 0],
                [576, 0],
            ] {
                for coordinate in point {
                    data.extend_from_slice(&coordinate.to_be_bytes());
                }
            }
            data.extend([0, 0, 255, 255]);
            let id = doc
                .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, data)))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Shading, id)
                .unwrap();
            // Exact dyadic transform places x(1/4)=420 at pixel x=13,
            // and both y boundaries at pixel centers (display y=.5/48.5).
            doc.replace_page_contents(
                0,
                format!("q 0.03125 0 0 0.03125 0.375 0.5 cm /{name} sh Q").into_bytes(),
            )
            .unwrap();
            let render = |target: &PdfDocument<std::io::Cursor<Vec<u8>>>| {
                let list = target.get_page(0).unwrap().display_list().unwrap();
                let [pdf::page::page_object::PageObject::Shading(object)] = list.objects() else {
                    panic!("expected Coons shading")
                };
                for y in 0..50 {
                    for x in 0..50 {
                        let ux = (f64::from(x) + 0.125) * 32.0;
                        let uy = f64::from(49 - y) * 32.0;
                        object
                            .shading
                            .color_at_point(ux, uy)
                            .unwrap_or_else(|error| {
                                panic!("pixel ({x},{y}), shading ({ux},{uy}): {error:?}")
                            });
                    }
                }
                let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
                target.get_page(0).unwrap().display(&mut renderer).unwrap();
                let info = skia_safe::ImageInfo::new(
                    (50, 50),
                    skia_safe::ColorType::RGBA8888,
                    skia_safe::AlphaType::Unpremul,
                    None,
                );
                let mut pixels = vec![0; 10000];
                assert!(
                    renderer
                        .surface
                        .read_pixels(&info, &mut pixels, 200, (0, 0))
                );
                let expected = if nonlinear { 16 } else { 64 };
                for y in [1_usize, 49] {
                    let offset = (y * 50 + 13) * 4;
                    assert_eq!(
                        &pixels[offset..offset + 4],
                        &[expected, expected, expected, 255]
                    );
                }
                pixels
            };
            let original = render(&doc);
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let saved = doc
                    .save_to_bytes_with_options(SaveOptions {
                        compression,
                        ..Default::default()
                    })
                    .unwrap();
                let reopened = PdfDocument::from_bytes_with_mode(saved, ParseMode::Strict).unwrap();
                assert_eq!(render(&reopened), original);
            }
        }
    }
    #[test]
    fn triangle_and_coons_mesh_pixels_follow_parameter_interpolation() {
        use pdf::{
            document::{
                PdfDocument,
                resource_manager::ResourceKind,
                save_options::{SaveOptions, StreamCompression},
            },
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::page_builder::PageBuilder,
            parser::syntax::SyntaxParser,
        };
        for (nonlinear, mirrored, kind) in [
            (false, false, 4),
            (true, false, 4),
            (false, true, 4),
            (true, true, 4),
            (false, false, 5),
            (true, false, 5),
            (false, true, 5),
            (true, true, 5),
            (false, false, 6),
            (true, false, 6),
            (false, true, 6),
            (true, true, 6),
        ] {
            let doc = PdfDocument::new().unwrap();
            doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 20.0))
                .unwrap();
            let function = if nonlinear {
                "/Function << /FunctionType 2 /Domain [0 1] /N 2 >>"
            } else {
                ""
            };
            let geometry = match kind {
                5 => "/ShadingType 5 /VerticesPerRow 2",
                6 => "/ShadingType 6 /BitsPerFlag 8",
                _ => "/ShadingType 4 /BitsPerFlag 8",
            };
            let source = format!(
                "<< {geometry} /ColorSpace /DeviceGray /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 40 0 20 0 1] /BBox [4 4 36 16] {function} >>"
            );
            let dict = SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(source.into_bytes())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            let data = if kind == 6 {
                let mut bytes = vec![0];
                for point in [
                    [0, 0],
                    [0, 85],
                    [0, 170],
                    [0, 255],
                    [85, 255],
                    [170, 255],
                    [255, 255],
                    [255, 170],
                    [255, 85],
                    [255, 0],
                    [170, 0],
                    [85, 0],
                ] {
                    bytes.extend(point);
                }
                bytes.extend([0, 0, 255, 255]);
                bytes
            } else if kind == 5 {
                vec![0, 0, 0, 255, 0, 255, 0, 255, 0, 255, 255, 255]
            } else {
                vec![0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 0, 1, 255, 255, 255]
            };
            let id = doc
                .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, data)))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Shading, id)
                .unwrap();
            let transform = if mirrored { "-1 0 0 1 40 0 cm " } else { "" };
            doc.replace_page_contents(0, format!("q {transform}/{name} sh Q").into_bytes())
                .unwrap();
            let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
            doc.get_page(0).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (40, 20),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut bytes = vec![0; 3200];
            assert!(renderer.surface.read_pixels(&info, &mut bytes, 160, (0, 0)));
            for (x, y) in [(10usize, 5usize), (30, 15), (20, 10)] {
                let coordinate = (x as f64 + 0.5) / 40.0;
                let t = if mirrored {
                    1.0 - coordinate
                } else {
                    coordinate
                };
                let expected = (255.0 * if nonlinear { t * t } else { t }).round() as u8;
                assert_eq!(
                    &bytes[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4],
                    &[expected, expected, expected, 255]
                );
            }
            assert_eq!(&bytes[(10 * 40 + 2) * 4..(10 * 40 + 2) * 4 + 4], &[255; 4]);
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let saved = doc
                    .save_to_bytes_with_options(SaveOptions {
                        compression,
                        ..Default::default()
                    })
                    .unwrap();
                let reopened = PdfDocument::from_bytes_with_mode(
                    saved,
                    pdf::parser::syntax::ParseMode::Strict,
                )
                .unwrap();
                let mut readback_renderer = SkiaRender::new(72.0, 72.0).unwrap();
                reopened
                    .get_page(0)
                    .unwrap()
                    .display(&mut readback_renderer)
                    .unwrap();
                let mut readback_pixels = vec![0; 3200];
                assert!(readback_renderer.surface.read_pixels(
                    &info,
                    &mut readback_pixels,
                    160,
                    (0, 0)
                ));
                assert_eq!(readback_pixels, bytes);
            }
        }
    }

    #[test]
    fn type1_shading_pixels_and_bbox_survive_strict_save() {
        use pdf::{
            document::{
                PdfDocument,
                resource_manager::ResourceKind,
                save_options::{SaveOptions, StreamCompression},
            },
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::page_builder::PageBuilder,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        for sampled in [false, true] {
            let doc = PdfDocument::new().unwrap();
            doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 20.0))
                .unwrap();
            let (dict, data) = if sampled {
                (
                    "<< /FunctionType 0 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] /Size [2 2] /BitsPerSample 8 >>",
                    vec![0, 0, 0, 255, 0, 128, 0, 255, 128, 255, 255, 255],
                )
            } else {
                (
                    "<< /FunctionType 4 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] >>",
                    b"{ 2 copy add 2 div }".to_vec(),
                )
            };
            let function = doc
                .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                    parse(dict).into_dict().unwrap(),
                    data,
                )))
                .unwrap();
            let id = doc.try_insert_object(parse(&format!(
                    "<< /ShadingType 1 /ColorSpace /DeviceRGB /Matrix [40 0 0 20 0 0] /BBox [4 4 36 16] /Background [1 0 0] /Function {} {} R >>",
                    function.obj_num(), function.gen_num()
                ))).unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Shading, id)
                .unwrap();
            doc.replace_page_contents(0, format!("/{name} sh").into_bytes())
                .unwrap();
            let raster = |document: &PdfDocument<std::io::Cursor<Vec<u8>>>| {
                let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
                document
                    .get_page(0)
                    .unwrap()
                    .display(&mut renderer)
                    .unwrap();
                let info = skia_safe::ImageInfo::new(
                    (40, 20),
                    skia_safe::ColorType::RGBA8888,
                    skia_safe::AlphaType::Unpremul,
                    None,
                );
                let mut bytes = vec![0; 3200];
                assert!(renderer.surface.read_pixels(&info, &mut bytes, 160, (0, 0)));
                bytes
            };
            let expected = raster(&doc);
            let pixel = |x: usize, y: usize| &expected[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
            assert_eq!(pixel(10, 5), &[67, 185, 126, 255]);
            assert_eq!(pixel(10, 15), &[67, 57, 62, 255]);
            assert_eq!(pixel(2, 10), &[255; 4]);
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let bytes = doc
                    .save_to_bytes_with_options(SaveOptions {
                        compression,
                        ..Default::default()
                    })
                    .unwrap();
                assert_eq!(
                    raster(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap()),
                    expected
                );
            }
        }
    }

    #[test]
    fn shading_pixels_use_functions_geometry_and_strict_save() {
        use pdf::{
            document::{
                PdfDocument,
                resource_manager::ResourceKind,
                save_options::{SaveOptions, StreamCompression},
            },
            io::stream_reader::StreamReader,
            page::page_builder::PageBuilder,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        for radial in [false, true] {
            let doc = PdfDocument::new().unwrap();
            doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 20.0))
                .unwrap();
            let source = if radial {
                "<< /ShadingType 3 /ColorSpace /DeviceGray /Coords [20 10 0 20 10 10] /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 1 >> >>"
            } else {
                "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 40 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 2 >> >>"
            };
            let id = doc.try_insert_object(parse(source)).unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Shading, id)
                .unwrap();
            doc.replace_page_contents(0, format!("/{name} sh").into_bytes())
                .unwrap();
            let render = |document: &PdfDocument<std::io::Cursor<Vec<u8>>>| {
                let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
                document
                    .get_page(0)
                    .unwrap()
                    .display(&mut renderer)
                    .unwrap();
                let info = skia_safe::ImageInfo::new(
                    (40, 20),
                    skia_safe::ColorType::RGBA8888,
                    skia_safe::AlphaType::Unpremul,
                    None,
                );
                let mut bytes = vec![0; 40 * 20 * 4];
                assert!(renderer.surface.read_pixels(&info, &mut bytes, 160, (0, 0)));
                bytes
            };
            let expected = render(&doc);
            let pixel = |x: usize, y: usize| &expected[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
            if radial {
                assert_eq!(pixel(20, 10), &[18, 18, 18, 255]);
                assert_eq!(pixel(0, 10), &[255; 4]);
            } else {
                assert_eq!(pixel(10, 10), &[18, 18, 18, 255]);
                assert_eq!(pixel(30, 10), &[148, 148, 148, 255]);
            }
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let bytes = doc
                    .save_to_bytes_with_options(SaveOptions {
                        compression,
                        ..SaveOptions::default()
                    })
                    .unwrap();
                let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
                assert_eq!(render(&reopened), expected);
            }
        }
    }

    #[test]
    fn shading_clip_alpha_and_budget_recover_without_partial_paint() {
        use pdf::{
            document::{PdfDocument, resource_manager::ResourceKind},
            io::stream_reader::StreamReader,
            page::{page_builder::PageBuilder, page_object::PageObject},
            parser::syntax::SyntaxParser,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 20.0))
            .unwrap();
        let source = b"<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 40 0] /Extend [true true] /BBox [0 0 30 20] /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [0] /N 1 >> >>";
        let object = SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(source.to_vec())).unwrap(),
        )
        .get_object()
        .unwrap();
        let id = doc.try_insert_object(object).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Shading, id)
            .unwrap();
        doc.replace_page_contents(0, format!("5 0 35 20 re W n /{name} sh").into_bytes())
            .unwrap();
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let PageObject::Shading(mut shading) = list.objects()[0].clone() else {
            panic!()
        };
        shading.graphic_state.general_state.fill_alpha = 0.5;
        let pixels = |renderer: &mut SkiaRender| {
            let info = skia_safe::ImageInfo::new(
                (40, 20),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut bytes = vec![0; 3200];
            assert!(renderer.surface.read_pixels(&info, &mut bytes, 160, (0, 0)));
            white_test_output(bytes)
        };
        let mut renderer = SkiaRender::with_limits(
            72.0,
            72.0,
            Limits {
                max_render_bytes: 9599,
                ..Limits::default()
            },
        )
        .unwrap();
        renderer.start_page(40.0, 20.0).unwrap();
        assert!(matches!(
            renderer.do_shading(shading.clone()),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(pixels(&mut renderer).iter().all(|byte| *byte == 255));
        renderer.limits.max_render_bytes = 9600;
        renderer.do_shading(shading.clone()).unwrap();
        let bytes = pixels(&mut renderer);
        assert_eq!(&bytes[(10 * 40 + 2) * 4..(10 * 40 + 2) * 4 + 4], &[255; 4]);
        assert_eq!(
            &bytes[(10 * 40 + 10) * 4..(10 * 40 + 10) * 4 + 4],
            &[127, 127, 127, 255]
        );
        assert_eq!(
            &bytes[(10 * 40 + 35) * 4..(10 * 40 + 35) * 4 + 4],
            &[255; 4]
        );
        shading.graphic_state.general_state.fill_alpha = f32::NAN;
        assert!(renderer.do_shading(shading.clone()).is_err());
        assert_eq!(pixels(&mut renderer), bytes);
        shading.graphic_state.general_state.fill_alpha = 1.0;
        shading.graphic_state.clip_path.elements.clear();
        renderer.do_shading(shading).unwrap();
        assert_eq!(
            &pixels(&mut renderer)[(10 * 40 + 2) * 4..(10 * 40 + 2) * 4 + 4],
            &[0, 0, 0, 255]
        );
    }

    #[test]
    fn nested_multicolor_image_transform_preserves_orientation_and_shared_calls() {
        use pdf::{
            document::{owned::Document, resource_manager::ResourceKind},
            geom::rectangle::Rectangle,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::page_object::PageObject,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        fn insert(doc: &Document, dict: &str, data: &[u8]) -> pdf::objects::object_id::ObjectId {
            let dict = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(dict.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, data.to_vec()).unwrap(),
            ))
            .unwrap()
        }
        fn raster(doc: &Document, page: u32) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (40, 40),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 40 * 40 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 40 * 4, (0, 0))
            );
            pixels
        }
        // Expected quadrants are bottom-left, bottom-right, top-left, top-right.
        for (matrix, colors, origin, size) in [
            (
                [0., 1., -1., 0., 1., 0.],
                ["1 0 0", "0 0 1", "0 1 0", "1 1 0"],
                5.,
                8.,
            ),
            (
                [-1., 0., 0., 1., 1., 0.],
                ["1 1 0", "0 0 1", "0 1 0", "1 0 0"],
                5.,
                8.,
            ),
            (
                [0.5, 0., 0., 0.5, 0.25, 0.25],
                ["0 0 1", "1 1 0", "1 0 0", "0 1 0"],
                7.,
                4.,
            ),
        ] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_crop_box(0, Some(Rectangle::new(0., 0., 40., 40.)))
                .unwrap();
            let image = insert(
                &doc,
                "<< /Subtype /Image /Width 2 /Height 2 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Interpolate false >>",
                &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
            );
            let leaf = insert(
                &doc,
                &format!(
                    "<< /Subtype /Form /BBox [0 0 8 8] /Resources << /XObject << /Tile {} 0 R >> >> >>",
                    image.obj_num()
                ),
                b"q 8 0 0 8 0 0 cm /Tile Do Q",
            );
            let outer = insert(
                &doc,
                &format!(
                    "<< /Subtype /Form /BBox [0 0 40 40] /Resources << /XObject << /Child {} 0 R >> >> >>",
                    leaf.obj_num()
                ),
                b"q 1 0 0 1 5 5 cm /Child Do Q q 1 0 0 1 20 5 cm /Child Do Q",
            );
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, outer)
                .unwrap();
            doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
                .unwrap();
            doc.copy_page(0, 1).unwrap();
            let original = raster(&doc, 1);
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            let PageObject::Image(image) = &list.objects()[0] else {
                panic!("missing image")
            };
            doc.transform_nested_image_draw(0, &root, image.nested_source().unwrap(), matrix)
                .unwrap();
            doc.add_page(2).unwrap();
            doc.set_page_crop_box(2, Some(Rectangle::new(0., 0., 40., 40.)))
                .unwrap();
            let half = size / 2.;
            let mut reference = String::new();
            for (i, color) in colors.iter().enumerate() {
                let x = origin + (i % 2) as f32 * half;
                let y = origin + (i / 2) as f32 * half;
                reference.push_str(&format!("{color} rg {x} {y} {half} {half} re f "));
            }
            reference.push_str("0 0 1 rg 20 5 4 4 re f 1 1 0 rg 24 5 4 4 re f 1 0 0 rg 20 9 4 4 re f 0 1 0 rg 24 9 4 4 re f");
            doc.replace_page_contents(2, reference.into_bytes())
                .unwrap();
            let expected = raster(&doc, 2);
            assert_ne!(expected, original);
            assert_eq!(raster(&doc, 0), expected, "matrix {matrix:?}");
            assert_eq!(raster(&doc, 1), original);
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(raster(&reopened, 0), expected);
            assert_eq!(raster(&reopened, 1), original);
            // Replace the transformed first call with a different four-color tile.
            // The reference uses rectangles, not another image draw or Form.
            let replacement = insert(
                &doc,
                "<< /Subtype /Image /Width 2 /Height 2 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Interpolate false >>",
                &[0, 255, 255, 255, 0, 255, 0, 0, 0, 255, 255, 255],
            );
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            let PageObject::Image(image) = &list.objects()[0] else {
                panic!("missing transformed image")
            };
            doc.replace_nested_image_draw(
                0,
                &root,
                image.nested_source().unwrap(),
                &doc.object_handle(replacement).unwrap(),
            )
            .unwrap();
            let mut reference = String::new();
            for (i, color) in colors.iter().enumerate() {
                let replacement_color = match *color {
                    "1 0 0" => "0 1 1",
                    "0 1 0" => "1 0 1",
                    "0 0 1" => "0 0 0",
                    "1 1 0" => "1 1 1",
                    _ => unreachable!(),
                };
                let x = origin + (i % 2) as f32 * half;
                let y = origin + (i / 2) as f32 * half;
                reference.push_str(&format!(
                    "{replacement_color} rg {x} {y} {half} {half} re f "
                ));
            }
            reference.push_str("0 0 1 rg 20 5 4 4 re f 1 1 0 rg 24 5 4 4 re f 1 0 0 rg 20 9 4 4 re f 0 1 0 rg 24 9 4 4 re f");
            doc.replace_page_contents(2, reference.into_bytes())
                .unwrap();
            let replaced_expected = raster(&doc, 2);
            assert_ne!(replaced_expected, expected);
            assert_eq!(
                raster(&doc, 0),
                replaced_expected,
                "replacement matrix {matrix:?}"
            );
            assert_eq!(raster(&doc, 1), original);
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(raster(&reopened, 0), replaced_expected);
            assert_eq!(raster(&reopened, 1), original);
        }
    }

    #[test]
    fn nested_image_removal_pixels_isolate_call_and_preserve_form_clip() {
        use pdf::{
            document::{owned::Document, resource_manager::ResourceKind},
            geom::rectangle::Rectangle,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::page_object::PageObject,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        fn insert(
            doc: &Document,
            dictionary: &str,
            body: &[u8],
        ) -> pdf::objects::object_id::ObjectId {
            let dictionary = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(dictionary.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dictionary, body.to_vec()).unwrap(),
            ))
            .unwrap()
        }
        fn raster(doc: &Document, page: u32) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (40, 40),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 40 * 40 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 40 * 4, (0, 0))
            );
            pixels
        }
        for (transform, first_image) in [
            (None, ""),
            (None, "0 1 1 rg 5 5 4 8 re f"),
            (
                Some([0.5, 0., 0., 0.5, 0.25, 0.25]),
                "1 0 0 rg 7 7 2 4 re f",
            ),
            (Some([0., 0.5, -0.25, 0., 0.5, 0.]), "1 0 0 rg 7 5 2 4 re f"),
            (
                Some([-0.5, 0., 0., 0.5, 0.5, 0.25]),
                "1 0 0 rg 5 7 4 4 re f",
            ),
        ] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_crop_box(0, Some(Rectangle::new(0., 0., 40., 40.)))
                .unwrap();
            let image = insert(
                &doc,
                "<< /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB >>",
                &[255, 0, 0],
            );
            let replacement = insert(
                &doc,
                "<< /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB >>",
                &[0, 255, 255],
            );
            let leaf = insert(
                &doc,
                &format!(
                    "<< /Subtype /Form /BBox [0 0 6 8] /Resources << /XObject << /Red {} 0 R >> >> >>",
                    image.obj_num()
                ),
                b"q 8 0 0 8 0 0 cm /Red Do Q 0 0 1 rg 4 0 8 8 re f",
            );
            let outer = insert(
                &doc,
                &format!(
                    "<< /Subtype /Form /BBox [0 0 40 40] /Resources << /XObject << /Child {} 0 R >> >> >>",
                    leaf.obj_num()
                ),
                b"q 1 0 0 1 5 5 cm /Child Do Q q 1 0 0 1 20 5 cm /Child Do Q",
            );
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, outer)
                .unwrap();
            doc.replace_page_contents(
                0,
                format!("/{name} Do 0 1 0 rg 30 30 5 5 re f").into_bytes(),
            )
            .unwrap();
            doc.copy_page(0, 1).unwrap();
            let original = raster(&doc, 1);
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            let PageObject::Image(image) = &list.objects()[0] else {
                panic!("missing image")
            };
            if let Some(transform) = transform {
                doc.transform_nested_image_draw(
                    0,
                    &root,
                    image.nested_source().unwrap(),
                    transform,
                )
                .unwrap();
            } else if !first_image.is_empty() {
                doc.replace_nested_image_draw(
                    0,
                    &root,
                    image.nested_source().unwrap(),
                    &doc.object_handle(replacement).unwrap(),
                )
                .unwrap();
            } else {
                doc.remove_nested_image_draw(0, &root, image.nested_source().unwrap())
                    .unwrap();
            }
            doc.add_page(2).unwrap();
            doc.set_page_crop_box(2, Some(Rectangle::new(0., 0., 40., 40.)))
                .unwrap();
            // Direct rectangles independently describe the surviving clipped pixels.
            doc.replace_page_contents(2,
            format!("{first_image} 0 0 1 rg 9 5 2 8 re f 1 0 0 rg 20 5 4 8 re f 0 0 1 rg 24 5 2 8 re f 0 1 0 rg 30 30 5 5 re f").into_bytes()).unwrap();
            let expected = raster(&doc, 2);
            assert_ne!(expected, original);
            for (x, y, color) in [
                (
                    6usize,
                    34usize,
                    if transform.is_none() && !first_image.is_empty() {
                        [0, 255, 255, 255]
                    } else {
                        [255, 255, 255, 255]
                    },
                ),
                (10, 34, [0, 0, 255, 255]),
                (12, 34, [255, 255, 255, 255]),
                (21, 34, [255, 0, 0, 255]),
                (25, 34, [0, 0, 255, 255]),
                (27, 34, [255, 255, 255, 255]),
                (32, 7, [0, 255, 0, 255]),
            ] {
                assert_eq!(&expected[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4], &color);
            }
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            let color = if transform.is_some() {
                [255, 0, 0, 255]
            } else if !first_image.is_empty() {
                [0, 255, 255, 255]
            } else {
                [255, 255, 255, 255]
            };
            assert_eq!(&expected[(31 * 40 + 8) * 4..(31 * 40 + 8) * 4 + 4], &color);
            assert_eq!(
                &expected[(34 * 40 + 8) * 4..(34 * 40 + 8) * 4 + 4],
                &if transform.is_some_and(|matrix| matrix[1] != 0.) {
                    [255, 0, 0, 255]
                } else if transform.is_none() && !first_image.is_empty() {
                    [0, 255, 255, 255]
                } else {
                    [255, 255, 255, 255]
                }
            );
            for document in [&doc, &reopened] {
                assert_eq!(raster(document, 0), expected);
                assert_eq!(raster(document, 1), original);
            }
        }
    }

    #[test]
    fn nested_path_removal_pixels_preserve_pending_clip_and_other_invocations() {
        use pdf::{
            document::{owned::Document, resource_manager::ResourceKind},
            geom::rectangle::Rectangle,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::page_object::PageObject,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        fn insert(
            doc: &Document,
            dictionary: &str,
            body: &[u8],
        ) -> pdf::objects::object_id::ObjectId {
            let dict = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(dictionary.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, body.to_vec()).unwrap(),
            ))
            .unwrap()
        }
        fn raster(doc: &Document, page: u32) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (40, 40),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 40 * 40 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 40 * 4, (0, 0))
            );
            pixels
        }
        for edit_geometry in [false, true] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_crop_box(0, Some(Rectangle::new(0., 0., 40., 40.)))
                .unwrap();
            let body = "1 0 0 rg 0 0 8 8 re W f 0 0 1 rg 4 0 8 8 re f";
            let leaf = insert(
                &doc,
                "<< /Subtype /Form /BBox [0 0 20 20] /Resources << >> >>",
                body.as_bytes(),
            );
            let outer = insert(
                &doc,
                &format!(
                    "<< /Subtype /Form /BBox [0 0 40 40] /Resources << /XObject << /Child {} 0 R >> >> >>",
                    leaf.obj_num()
                ),
                b"q 1 0 0 1 5 5 cm /Child Do Q q 1 0 0 1 20 5 cm /Child Do Q",
            );
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, outer)
                .unwrap();
            doc.replace_page_contents(
                0,
                format!("/{name} Do 0 1 0 rg 30 30 5 5 re f").into_bytes(),
            )
            .unwrap();
            doc.copy_page(0, 1).unwrap();
            let original = raster(&doc, 1);
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            let PageObject::Path(path) = &list.objects()[0] else {
                panic!("missing nested path")
            };
            if edit_geometry {
                let source = path.nested_source().unwrap();
                doc.replace_nested_path_numbers(
                    0,
                    &root,
                    source,
                    source.geometry_operation_indices[0],
                    &[2., 0., 4., 8.],
                )
                .unwrap();
            } else {
                doc.remove_nested_path_draw(0, &root, path.nested_source().unwrap())
                    .unwrap();
            }
            doc.add_page(2).unwrap();
            doc.set_page_crop_box(2, Some(Rectangle::new(0., 0., 40., 40.)))
                .unwrap();
            let first = if edit_geometry {
                "1 0 0 rg 2 0 4 8 re W f 0 0 1 rg 4 0 8 8 re f"
            } else {
                "0 0 8 8 re W n 0 0 1 rg 4 0 8 8 re f"
            };
            doc.replace_page_contents(
                2,
                format!(
                    "q 1 0 0 1 5 5 cm {first} Q q 1 0 0 1 20 5 cm {body} Q 0 1 0 rg 30 30 5 5 re f"
                )
                .into_bytes(),
            )
            .unwrap();
            let expected = raster(&doc, 2);
            assert_ne!(expected, original);
            for (x, y, color) in [
                (6usize, 34usize, [255, 255, 255, 255]),
                (10, 34, [0, 0, 255, 255]),
                (15, 34, [255, 255, 255, 255]),
                (21, 34, [255, 0, 0, 255]),
                (25, 34, [0, 0, 255, 255]),
                (32, 7, [0, 255, 0, 255]),
            ] {
                assert_eq!(&expected[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4], &color);
            }
            for (x, color) in if edit_geometry {
                [(8usize, [255, 0, 0, 255]), (12, [255, 255, 255, 255])]
            } else {
                [(8usize, [255, 255, 255, 255]), (12, [0, 0, 255, 255])]
            } {
                assert_eq!(&expected[(34 * 40 + x) * 4..(34 * 40 + x) * 4 + 4], &color);
            }
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for document in [&doc, &reopened] {
                assert_eq!(raster(document, 0), expected);
                assert_eq!(raster(document, 1), original);
            }
        }
    }

    #[test]
    fn replaced_form_pixels_change_only_selected_call_and_survive_reopen() {
        use pdf::{
            document::{owned::Document, resource_manager::ResourceKind},
            geom::rectangle::Rectangle,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        fn raster(doc: &Document, page: u32) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (40, 40),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 40 * 40 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 40 * 4, (0, 0))
            );
            pixels
        }
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.set_page_crop_box(0, Some(Rectangle::new(0., 0., 40., 40.)))
            .unwrap();
        let dictionary = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Subtype /Form /BBox [0 0 8 8] /Resources << >> >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let old = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dictionary.clone(), b"1 0 0 rg 0 0 8 8 re f".to_vec())
                    .unwrap(),
            ))
            .unwrap();
        let new = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dictionary, b"0 0 1 rg 0 0 4 4 re f".to_vec())
                    .unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, old)
            .unwrap();
        doc.replace_page_contents(0, format!("q 1 0 0 1 5 5 cm /{name} Do Q q 1 0 0 1 20 5 cm /{name} Do Q 0 1 0 rg 30 30 5 5 re f").into_bytes()).unwrap();
        doc.copy_page(0, 1).unwrap();
        let original = raster(&doc, 1);
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        doc.replace_page_form_draw(0, &snapshot, 6, &doc.object_handle(new).unwrap())
            .unwrap();
        doc.add_page(2).unwrap();
        doc.set_page_crop_box(2, Some(Rectangle::new(0., 0., 40., 40.)))
            .unwrap();
        doc.replace_page_contents(
            2,
            b"1 0 0 rg 5 5 8 8 re f 0 0 1 rg 20 5 4 4 re f 0 1 0 rg 30 30 5 5 re f".to_vec(),
        )
        .unwrap();
        let expected = raster(&doc, 2);
        assert_ne!(expected, original);
        for (x, y, color) in [
            (6usize, 34usize, [255, 0, 0, 255]),
            (21, 34, [0, 0, 255, 255]),
            (26, 34, [255, 255, 255, 255]),
            (32, 7, [0, 255, 0, 255]),
        ] {
            assert_eq!(&expected[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4], &color);
        }
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            assert_eq!(raster(document, 0), expected);
            assert_eq!(raster(document, 1), original);
        }
    }

    #[test]
    fn edited_form_affine_pixels_match_direct_content_and_preserve_shared_page() {
        use pdf::{
            document::{owned::Document, resource_manager::ResourceKind},
            geom::rectangle::Rectangle,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        fn raster(doc: &Document, page: u32) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (60, 60),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 60 * 60 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 60 * 4, (0, 0))
            );
            pixels
        }
        for matrix in [
            [0., 1., -1., 0., 20., 10.],
            [2., 0., 0., 0.5, 10., 10.],
            [-1., 0., 0., 1., 20., 10.],
        ] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_crop_box(0, Some(Rectangle::new(0., 0., 60., 60.)))
                .unwrap();
            let dictionary = SyntaxParser::new(StreamReader::try_new(Cursor::new(b"<< /Subtype /Form /BBox [0 0 10 10] /Matrix [1 0 0 1 1 2] /Resources << >> >>".to_vec())).unwrap()).get_object().unwrap().into_dict().unwrap();
            let body = "1 0 0 rg -5 -5 20 20 re f 0 0 1 rg 0 0 4 4 re f";
            let id = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(dictionary, body.as_bytes().to_vec()).unwrap(),
                ))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, id)
                .unwrap();
            let neighbor = "0 1 0 rg 45 45 5 5 re f";
            doc.replace_page_contents(0, format!("/{name} Do {neighbor}").into_bytes())
                .unwrap();
            doc.copy_page(0, 1).unwrap();
            let original = raster(&doc, 1);
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            doc.transform_page_form_draw(0, &snapshot, 0, matrix)
                .unwrap();
            doc.add_page(2).unwrap();
            doc.set_page_crop_box(2, Some(Rectangle::new(0., 0., 60., 60.)))
                .unwrap();
            let [a, b, c, d, e, f] = matrix;
            doc.replace_page_contents(2, format!("q {a} {b} {c} {d} {e} {f} cm 1 0 0 1 1 2 cm 0 0 10 10 re W n {body} Q {neighbor}").into_bytes()).unwrap();
            let expected = raster(&doc, 2);
            assert_ne!(expected, original);
            for color in [[255, 0, 0, 255], [0, 0, 255, 255], [0, 255, 0, 255]] {
                assert!(expected.as_chunks::<4>().0.contains(&color));
            }
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for document in [&doc, &reopened] {
                assert_eq!(raster(document, 0), expected);
                assert_eq!(raster(document, 1), original);
            }
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            doc.remove_page_form_draw(0, &snapshot, 2).unwrap();
            doc.replace_page_contents(2, neighbor.as_bytes().to_vec())
                .unwrap();
            let removed = raster(&doc, 2);
            assert!(removed.as_chunks::<4>().0.contains(&[0, 255, 0, 255]));
            assert!(
                !removed
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|pixel| *pixel == [255, 0, 0, 255] || *pixel == [0, 0, 255, 255])
            );
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for document in [&doc, &reopened] {
                assert_eq!(raster(document, 0), removed);
                assert_eq!(raster(document, 1), original);
            }
        }
    }

    #[test]
    fn form_images_and_text_respect_bbox_and_restore_canvas_clip() {
        use pdf::{
            document::{
                owned::Document,
                resource_manager::{ResourceKind, StandardFont},
            },
            geom::rectangle::Rectangle,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        fn raster(doc: &Document, page: u32) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            assert_eq!((renderer.page_width, renderer.page_height), (60, 60));
            let info = skia_safe::ImageInfo::new(
                (60, 60),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 60 * 60 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 60 * 4, (0, 0))
            );
            pixels
        }
        for (show, leading) in [
            ("(MMMM) Tj", 0),
            ("(MMMM) '", 0),
            ("0 0 (MMMM) \"", 0),
            ("[-25 (MMMM) 25] TJ", 0),
            ("[-25 (MM) 25 (MM)] TJ", 0),
            ("(MMMM) '", 3),
            ("1 2 (MMMM) \"", 3),
        ] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            doc.add_page(1).unwrap();
            for page in [0, 1] {
                doc.set_page_crop_box(page, Some(Rectangle::new(0., 0., 60., 60.)))
                    .unwrap();
            }
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            let image = doc
                .resources()
                .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
                .unwrap();
            let scope = doc.get_page(0).unwrap().resource().unwrap();
            let make_form = |bbox: &str, body: String| {
                let mut parser = SyntaxParser::new(
                    StreamReader::try_new(Cursor::new(
                        format!("<< /Type /XObject /Subtype /Form /BBox [{bbox}] >>").into_bytes(),
                    ))
                    .unwrap(),
                );
                let mut dict = parser.get_object().unwrap().into_dict().unwrap();
                dict.insert("Resources".into(), PdfObject::PdfDict(scope.clone()));
                let id = doc
                    .try_insert_object(PdfObject::PdfStream(
                        PdfStream::from_decoded_data(dict, body.into_bytes()).unwrap(),
                    ))
                    .unwrap();
                doc.resources()
                    .register_existing(0, ResourceKind::Form, id)
                    .unwrap()
            };
            let image_form = make_form("10 10 20 20", format!("q 40 0 0 40 0 0 cm /{image} Do Q"));
            let initial_y = 5 + leading;
            let text_body =
                format!("BT /{font} 24 Tf {leading} TL 1 0 0 1 25 {initial_y} Tm {show} ET");
            let text_form = make_form("30 10 40 20", text_body.clone());
            let mut outer_dict = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(
                    b"<< /Subtype /Form /BBox [0 0 60 60] >>".to_vec(),
                ))
                .unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            outer_dict.insert(
                "Resources".into(),
                PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
            );
            let outer = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(
                        outer_dict,
                        format!("/{text_form} Do q 1 0 0 1 0 20 cm /{text_form} Do Q").into_bytes(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            let outer_name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, outer)
                .unwrap();
            doc.replace_page_contents(
                0,
                format!("/{image_form} Do /{outer_name} Do 0 1 0 rg 0 0 5 5 re f").into_bytes(),
            )
            .unwrap();
            doc.resources()
                .register_standard_font(1, StandardFont::Helvetica)
                .unwrap();
            let second_text = format!("q 1 0 0 1 0 20 cm 30 10 10 10 re W n {text_body} Q");
            doc.replace_page_contents(1,format!("1 0 0 rg 10 10 10 10 re f 0 0 0 rg q 30 10 10 10 re W n {text_body} Q {second_text} 0 1 0 rg 0 0 5 5 re f").into_bytes()).unwrap();
            let actual = raster(&doc, 0);
            assert_eq!(actual, raster(&doc, 1));
            for color in [[255, 0, 0, 255], [0, 0, 0, 255], [0, 255, 0, 255]] {
                assert!(
                    actual.as_chunks::<4>().0.contains(&color),
                    "missing ink {color:?}"
                );
            }
            // No image or glyph ink may escape either bbox; green proves clip restoration.
            for y in 0..60 {
                for x in 0..60 {
                    let user_y = 59 - y;
                    if !((10..20).contains(&x) && (10..20).contains(&user_y)
                        || (30..40).contains(&x) && (10..20).contains(&user_y)
                        || (30..40).contains(&x) && (30..40).contains(&user_y)
                        || (0..5).contains(&x) && (0..5).contains(&user_y))
                    {
                        assert_eq!(
                            &actual[(y * 60 + x) * 4..(y * 60 + x + 1) * 4],
                            &[255, 255, 255, 255],
                            "escaped ink {x},{y}"
                        );
                    }
                }
            }
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(raster(&reopened, 0), actual);
            // Editing the nested text must remove real glyph ink without changing
            // the clipped image, page neighbor, or a shared copy of the page.
            doc.copy_page(0, 2).unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let characters = doc
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            let source = characters.characters[0].nested_source.as_ref().unwrap();
            assert_eq!(source.invocations.len(), 2);
            doc.remove_nested_text_draw(0, &root, source).unwrap();
            doc.replace_page_contents(
                1,
                format!("1 0 0 rg 10 10 10 10 re f 0 0 0 rg {second_text} 0 1 0 rg 0 0 5 5 re f")
                    .into_bytes(),
            )
            .unwrap();
            let expected = raster(&doc, 1);
            assert_ne!(expected, actual);
            assert!(expected.as_chunks::<4>().0.contains(&[0, 0, 0, 255]));
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for document in [&doc, &reopened] {
                assert_eq!(raster(document, 0), expected);
                assert_eq!(raster(document, 2), actual);
                assert_eq!(
                    document
                        .get_page(0)
                        .unwrap()
                        .extract_characters_paint_order_report()
                        .unwrap()
                        .characters
                        .len(),
                    4
                );
                assert_eq!(
                    document
                        .get_page(2)
                        .unwrap()
                        .extract_characters_paint_order_report()
                        .unwrap()
                        .characters
                        .len(),
                    8
                );
            }
            doc.copy_page(2, 3).unwrap();
            for (text, encoded, partial) in [
                ("WWWW", "(WWWW)", false),
                ("é€", "<E980>", false),
                ("éA", "<E941>", true),
            ] {
                let root = doc.get_page(2).unwrap().content_program().unwrap();
                let characters = doc
                    .get_page(2)
                    .unwrap()
                    .extract_characters_paint_order_report()
                    .unwrap();
                let source = characters.characters[0].nested_source.as_ref().unwrap();
                assert_eq!(source.invocations.len(), 2);
                if partial {
                    doc.replace_nested_text_range(
                        2,
                        &root,
                        source,
                        show.ends_with("TJ").then_some(1),
                        1..2,
                        "A",
                    )
                    .unwrap();
                } else if show == "[-25 (MM) 25 (MM)] TJ" {
                    let leaf = doc
                        .get_page(2)
                        .unwrap()
                        .nested_content_program(&root, source)
                        .unwrap();
                    let end = leaf.operations()[source.operation_index]
                        .operator
                        .get_as_array(0)
                        .unwrap()
                        .get(3)
                        .unwrap()
                        .as_string()
                        .unwrap()
                        .bytes()
                        .len();
                    doc.replace_nested_text_array_range(2, &root, source, (1, 0)..(3, end), text)
                        .unwrap();
                } else if show.ends_with("TJ") {
                    doc.replace_nested_text_array_item(2, &root, source, 1, text)
                        .unwrap();
                } else {
                    doc.replace_nested_text(2, &root, source, text).unwrap();
                }
                let replacement_show = if show == "[-25 (MM) 25 (MM)] TJ" {
                    format!("[-25 {encoded} 25 <>] TJ")
                } else if show.ends_with("TJ") {
                    format!("[-25 {encoded} 25] TJ")
                } else {
                    format!("{encoded} Tj")
                };
                let spacing = if show.starts_with("1 2 ") {
                    "1 Tw 2 Tc"
                } else {
                    ""
                };
                let replacement_body =
                    format!("BT /{font} 24 Tf {spacing} 1 0 0 1 25 5 Tm {replacement_show} ET");
                doc.replace_page_contents(1, format!(
            "1 0 0 rg 10 10 10 10 re f 0 0 0 rg q 30 10 10 10 re W n {replacement_body} Q {second_text} 0 1 0 rg 0 0 5 5 re f"
        ).into_bytes()).unwrap();
                let replacement_pixels = raster(&doc, 1);
                assert_ne!(replacement_pixels, actual);
                assert!(
                    replacement_pixels
                        .as_chunks::<4>()
                        .0
                        .contains(&[0, 0, 0, 255])
                );
                let reopened =
                    Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                        .unwrap();
                for document in [&doc, &reopened] {
                    assert_eq!(raster(document, 2), replacement_pixels);
                    assert_eq!(raster(document, 3), actual);
                    assert_eq!(raster(document, 0), expected);
                    let report = document
                        .get_page(2)
                        .unwrap()
                        .extract_characters_paint_order_report()
                        .unwrap();
                    assert_eq!(
                        report
                            .characters
                            .iter()
                            .filter_map(|ch| ch.unicode.as_deref())
                            .collect::<String>(),
                        format!("{text}MMMM")
                    );
                    if text == "é€" {
                        assert_eq!(
                            report
                                .characters
                                .iter()
                                .take(2)
                                .map(|ch| ch.charcode)
                                .collect::<Vec<_>>(),
                            [0xe9, 0x80]
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn form_bbox_and_inherited_clip_match_direct_content_pixels() {
        use pdf::{
            document::{owned::Document, resource_manager::ResourceKind, save_options::PdfVersion},
            geom::rectangle::Rectangle,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        fn raster(doc: &Document, page: u32) -> (i32, i32, Vec<u8>) {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            let (w, h) = (renderer.page_width, renderer.page_height);
            let info = skia_safe::ImageInfo::new(
                (w, h),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; w as usize * h as usize * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, w as usize * 4, (0, 0))
            );
            (w, h, pixels)
        }
        let doc = Document::new_with_version(PdfVersion::V1_6).unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let mut parser = SyntaxParser::new(StreamReader::try_new(Cursor::new(b"<< /Type /XObject /Subtype /Form /BBox [0 0 20 15] /Matrix [1 0.5 0 1 25 30] /Resources << >> >>".to_vec())).unwrap());
        let dict = parser.get_object().unwrap().into_dict().unwrap();
        let body = "1 0 0 rg -10 -10 50 50 re f 0 0 1 rg 0 0 4 4 re f";
        let form = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, body.as_bytes().to_vec()).unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, form)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("0 1 0 rg q 10 20 55 40 re W n /{name} Do Q 65 25 10 10 re f").into_bytes(),
        )
        .unwrap();
        doc.replace_page_contents(1,format!("0 1 0 rg q 10 20 55 40 re W n q 1 0.5 0 1 25 30 cm 0 0 20 15 re W n {body} Q Q 65 25 10 10 re f").into_bytes()).unwrap();
        for index in [0, 1] {
            doc.set_page_crop_box(index, Some(Rectangle::new(10., 20., 90., 80.)))
                .unwrap();
            doc.set_page_user_unit(index, Some(1.5)).unwrap();
        }
        for rotation in [0, 90, 180, 270] {
            for index in [0, 1] {
                doc.set_page_rotation(index, rotation).unwrap();
            }
            let expected = raster(&doc, 1);
            let actual = raster(&doc, 0);
            assert_eq!(actual, expected, "Form pixels rotation={rotation}");
            for color in [[255, 0, 0, 255], [0, 0, 255, 255], [0, 255, 0, 255]] {
                assert!(
                    actual.2.as_chunks::<4>().0.contains(&color),
                    "missing ink {color:?}"
                );
            }
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(
                raster(&reopened, 0),
                expected,
                "saved Form rotation={rotation}"
            );
        }
    }

    #[test]
    fn vertical_glyph_origins_use_negative_v_and_scale_only_x() {
        let mut code = pdf::font::CharCode::new(1, 2, -1000.0);
        code.set_origin_x(250.0);
        code.set_origin_y(880.0);
        let (x, y) = SkiaRender::vertical_glyph_offset(&code, 10.0, 0.5).unwrap();
        assert_eq!(x, -1.25);
        assert!((y - 8.8).abs() < 0.00001);
        assert_eq!(code.origin_x(), 250.0);
        code.set_origin_y(f32::INFINITY);
        assert!(SkiaRender::vertical_glyph_offset(&code, 10.0, 1.0).is_err());
    }

    #[test]
    fn path_coordinate_overflow_is_rejected_before_canvas_mutation() {
        use pdf::{
            geom::point::Point,
            path::pdf_path::{Bezier, PdfPath, SubPath},
        };
        let mut renderer = SkiaRender::new(144.0, 144.0).unwrap();
        let count = renderer.surface.canvas().save_count();
        let matrix = renderer.surface.canvas().local_to_device();
        let huge = Point::new(f32::MAX, 0.0);
        for segment in [
            Segment::MoveTo(huge.clone()),
            Segment::LineTo(huge.clone()),
            Segment::CurveTo(Bezier::new(
                Point::default(),
                huge.clone(),
                Point::default(),
            )),
            Segment::MoveTo(Point::new(f32::NAN, 0.0)),
        ] {
            let mut path = PdfPath::default();
            path.add_subpath(SubPath::new(vec![segment], false));
            let object = PathObject::new(
                path,
                Matrix::identity(),
                Default::default(),
                FillType::Winding,
                false,
            );
            assert!(renderer.draw_path(object, PaintStyle::Fill).is_err());
            assert_eq!(renderer.surface.canvas().save_count(), count);
            assert_eq!(renderer.surface.canvas().local_to_device(), matrix);
        }
        assert!(
            renderer
                .draw_path(PathObject::default(), PaintStyle::Fill)
                .is_ok()
        );
    }

    #[test]
    fn failed_clip_matrix_restores_canvas_state() {
        use pdf::path::pdf_path::PdfPath;
        let mut renderer = SkiaRender::new(144.0, 144.0).unwrap();
        let count = renderer.surface.canvas().save_count();
        let matrix = renderer.surface.canvas().local_to_device();
        let mut state = GraphicState::default();
        state
            .clip_path
            .add_path(PdfPath::default(), FillType::Winding, Matrix::identity());
        state.clip_path.add_path(
            PdfPath::default(),
            FillType::Winding,
            Matrix::new(f32::MAX, 0.0, 0.0, 1.0, 0.0, 0.0),
        );
        let path = PathObject::new(
            PdfPath::default(),
            Matrix::identity(),
            state,
            FillType::Winding,
            false,
        );
        assert!(renderer.draw_path(path, PaintStyle::Fill).is_err());
        assert_eq!(renderer.surface.canvas().save_count(), count);
        assert_eq!(renderer.surface.canvas().local_to_device(), matrix);
        assert!(
            renderer
                .draw_path(PathObject::default(), PaintStyle::Fill)
                .is_ok()
        );
    }

    #[test]
    fn explicit_close_reaches_skia_at_its_original_position() {
        use pdf::{
            geom::point::Point,
            path::pdf_path::{PdfPath, SubPath},
        };
        let mut subpath = SubPath::new(
            vec![
                Segment::MoveTo(Point::new(10.0, 20.0)),
                Segment::LineTo(Point::new(30.0, 40.0)),
            ],
            false,
        );
        subpath.close();
        let mut path = PdfPath::default();
        path.add_subpath(subpath.clone());
        let object = PathObject::new(
            path,
            Matrix::identity(),
            Default::default(),
            FillType::NoFill,
            true,
        );
        let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
        renderer.build_path(&object);
        assert!(renderer.pathbuilder.snapshot().is_last_contour_closed());
        subpath.add_segment(Segment::LineTo(Point::new(50.0, 60.0)));
        let mut path = PdfPath::default();
        path.add_subpath(subpath);
        renderer.pathbuilder.reset();
        renderer.build_path(&PathObject::new(
            path,
            Matrix::identity(),
            Default::default(),
            FillType::NoFill,
            true,
        ));
        assert!(!renderer.pathbuilder.snapshot().is_last_contour_closed());
    }

    #[test]
    fn image_errors_leave_canvas_state_unchanged() {
        use pdf::{
            document::cross_ref_table::CrossRefTable,
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
            page::pdf_image::PdfImage,
            parser::{parser::PdfParser, syntax::SyntaxParser},
            pdf_context::PDFContext,
        };
        use std::{collections::HashMap, io::Cursor};
        let parser = PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let ctx = PDFContext::try_new(
            parser,
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let dict = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Width 1 /Height 1 /ColorSpace /DeviceRGB /BitsPerComponent 8 >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let make_image = |bytes| {
            PdfImage::try_new(
                "test".into(),
                &PdfObject::PdfStream(PdfStream::new(dict.clone(), bytes)),
                &ctx,
            )
            .unwrap()
        };
        let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
        renderer.start_page(4.0, 4.0).unwrap();
        let before_count = renderer.surface.canvas().save_count();
        let before_matrix = renderer.surface.canvas().local_to_device();
        assert!(
            renderer
                .do_image(ImageObject::new(
                    Matrix::new(2.0, 0.0, 0.0, 2.0, 1.0, 1.0),
                    make_image(vec![1, 2]),
                    GraphicState::default()
                ))
                .is_err()
        );
        assert_eq!(renderer.surface.canvas().save_count(), before_count);
        assert_eq!(renderer.surface.canvas().local_to_device(), before_matrix);
        renderer
            .do_image(ImageObject::new(
                Matrix::new(2.0, 0.0, 0.0, 2.0, 1.0, 1.0),
                make_image(vec![1, 2, 3]),
                GraphicState::default(),
            ))
            .unwrap();
        assert_eq!(renderer.surface.canvas().save_count(), before_count);
        assert_eq!(renderer.surface.canvas().local_to_device(), before_matrix);
    }

    #[test]
    fn rgba_soft_mask_renders_straight_colors_over_white_before_and_after_save() {
        use pdf::{
            document::PdfDocument,
            page::{content_builder::ContentBuilder, page_builder::PageBuilder},
            parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(30.0, 10.0))
            .unwrap();
        let name = doc
            .resources()
            .embed_rgba_image(
                0,
                3,
                1,
                vec![200, 40, 20, 0, 200, 40, 20, 128, 200, 40, 20, 255],
            )
            .unwrap();
        let mut builder = ContentBuilder::new();
        builder
            .draw_image(&name, [30.0, 0.0, 0.0, 10.0, 0.0, 0.0])
            .unwrap();
        doc.replace_page_content_builder(0, &builder).unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [doc, reopened] {
            let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
            document
                .get_page(0)
                .unwrap()
                .display(&mut renderer)
                .unwrap();
            let info = skia_safe::ImageInfo::new(
                (30, 10),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 30 * 10 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 30 * 4, (0, 0))
            );
            for (x, expected) in [
                (5usize, [255u8, 255, 255, 255]),
                (15, [227, 147, 137, 255]),
                (25, [200, 40, 20, 255]),
            ] {
                let pixel = &pixels[(5 * 30 + x) * 4..(5 * 30 + x) * 4 + 4];
                for (&actual, expected) in pixel.iter().zip(expected) {
                    assert!(
                        actual.abs_diff(expected) <= 1,
                        "x={x}, pixel={pixel:?}, expected={expected}"
                    );
                }
            }
        }
    }

    #[test]
    fn merge_and_reordered_extract_preserve_rotated_page_pixels() {
        use pdf::{document::PdfDocument, geom::rectangle::Rectangle, parser::syntax::ParseMode};
        fn raster(
            doc: &PdfDocument<std::io::Cursor<Vec<u8>>>,
            index: u32,
            size: (usize, usize),
        ) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(index).unwrap().display(&mut renderer).unwrap();
            let (width, height) = size;
            let info = skia_safe::ImageInfo::new(
                (width as i32, height as i32),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; width * height * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, width * 4, (0, 0))
            );
            pixels
        }
        let source = PdfDocument::new().unwrap();
        source.add_page(0).unwrap();
        source
            .set_page_crop_box(0, Some(Rectangle::new(10., 20., 90., 70.)))
            .unwrap();
        let image = source
            .resources()
            .embed_rgb_image(0, 2, 1, vec![255, 0, 0, 0, 255, 0])
            .unwrap();
        source
            .replace_page_contents(
                0,
                format!("q 20 0 0 10 25 45 cm /{image} Do Q 0 0 1 rg 60 25 20 15 re f")
                    .into_bytes(),
            )
            .unwrap();
        let mut originals = Vec::new();
        for index in 0..4 {
            if index != 0 {
                source.copy_page(0, index).unwrap();
            }
            source
                .set_page_rotation(index, (index * 90) as i32)
                .unwrap();
            let size = if index % 2 == 0 { (80, 50) } else { (50, 80) };
            originals.push(raster(&source, index, size));
        }
        // Establish that the source includes oriented image colors, a blue path
        // and white background, so an empty or monochrome output cannot pass.
        for color in [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 255, 255],
        ] {
            assert!(originals[0].as_chunks::<4>().0.contains(&color));
        }
        let source_stamp = source.stamp();
        let target = PdfDocument::new().unwrap();
        target.add_page(0).unwrap();
        target.merge_document(&source).unwrap();
        let merged =
            PdfDocument::from_bytes_with_mode(target.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let extracted = merged.extract_pages(&[4, 2, 1, 3]).unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(
            extracted.save_to_bytes().unwrap(),
            ParseMode::Strict,
        )
        .unwrap();
        for (index, original_index) in [3usize, 1, 0, 2].into_iter().enumerate() {
            let size = if original_index % 2 == 0 {
                (80, 50)
            } else {
                (50, 80)
            };
            assert_eq!(
                raster(&merged, original_index as u32 + 1, size),
                originals[original_index]
            );
            assert_eq!(
                raster(&extracted, index as u32, size),
                originals[original_index]
            );
            assert_eq!(
                raster(&reopened, index as u32, size),
                originals[original_index]
            );
            assert_eq!(
                raster(&source, original_index as u32, size),
                originals[original_index]
            );
        }
        assert!(source.is_stamp_current(&source_stamp));
    }

    #[test]
    fn removing_text_draw_clears_ink_without_changing_independent_neighbors() {
        use pdf::{
            document::{owned::Document, resource_manager::StandardFont},
            page::page_builder::PageBuilder,
            parser::syntax::ParseMode,
        };
        fn raster(doc: &Document, page: u32) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (160, 80),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 160 * 80 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 160 * 4, (0, 0))
            );
            pixels
        }
        for (target, baseline) in [
            ("(AB) Tj", 50),
            ("[(A) 25 (B)] TJ", 50),
            ("(AB) '", 68),
            ("2 1 (AB) \"", 68),
        ] {
            let doc = Document::new().unwrap();
            doc.add_page_with_builder(0, PageBuilder::new().size(160., 80.))
                .unwrap();
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            doc.replace_page_contents(0, format!("BT /{font} 18 Tf 18 TL 1 0 0 1 10 {baseline} Tm {target} 1 0 0 1 95 50 Tm (C) Tj ET 0 0 1 rg 10 10 10 10 re f").into_bytes()).unwrap();
            doc.copy_page(0, 1).unwrap();
            let original = raster(&doc, 1);
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let operation = snapshot
                .operations()
                .iter()
                .position(|op| matches!(op.operator.name(), "Tj" | "TJ" | "'" | "\""))
                .unwrap();
            doc.remove_page_text_draw(0, &snapshot, operation).unwrap();
            let edited = raster(&doc, 0);
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(edited, raster(&reopened, 0));
            assert_eq!(original, raster(&doc, 1));
            assert_eq!(original, raster(&reopened, 1));
            let mut target_ink = 0;
            let mut neighbor_ink = 0;
            for y in 0..80 {
                for x in 0..160 {
                    let offset = (y * 160 + x) * 4;
                    let before = &original[offset..offset + 4];
                    let after = &edited[offset..offset + 4];
                    if (5..65).contains(&x) && (5..45).contains(&y) {
                        target_ink += usize::from(before != [255, 255, 255, 255]);
                        assert_eq!(after, [255, 255, 255, 255], "target={target}, ({x},{y})");
                    } else {
                        assert_eq!(
                            before, after,
                            "neighbor changed: target={target}, ({x},{y})"
                        );
                        if (90..140).contains(&x) && (5..45).contains(&y) {
                            neighbor_ink += usize::from(after != [255, 255, 255, 255]);
                        }
                    }
                }
            }
            assert!(
                target_ink > 0 && neighbor_ink > 0,
                "text must actually render"
            );
            let offset = (65 * 160 + 15) * 4;
            assert_eq!(&edited[offset..offset + 4], &[0, 0, 255, 255]);
        }
    }

    #[test]
    fn fractional_user_unit_matches_dpi_scaling_for_text_and_images() {
        use pdf::{
            document::{owned::Document, resource_manager::StandardFont, save_options::PdfVersion},
            geom::rectangle::Rectangle,
            parser::syntax::ParseMode,
        };
        fn raster(doc: &Document, page: u32, dpi: (f32, f32)) -> (i32, i32, Vec<u8>) {
            let mut renderer = SkiaRender::new(dpi.0, dpi.1).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            let (width, height) = (renderer.page_width, renderer.page_height);
            let info = skia_safe::ImageInfo::new(
                (width, height),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; width as usize * height as usize * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, width as usize * 4, (0, 0))
            );
            (width, height, pixels)
        }
        for rotation in [0, 90, 180, 270] {
            let doc = Document::new_with_version(PdfVersion::V1_6).unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_crop_box(0, Some(Rectangle::new(10., 20., 90., 70.)))
                .unwrap();
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            let image = doc
                .resources()
                .embed_rgb_image(0, 2, 1, vec![255, 0, 0, 0, 255, 0])
                .unwrap();
            doc.replace_page_contents(0, format!("q 20 0 0 10 25 45 cm /{image} Do Q 0 0 1 rg 60 25 20 15 re f 0 0 0 rg BT /{font} 24 Tf 1 0 0 1 15 25 Tm (AB) Tj ET").into_bytes()).unwrap();
            doc.set_page_rotation(0, rotation).unwrap();
            doc.copy_page(0, 1).unwrap();
            // Unit 1.5 at this DPI must equal unit 1 at 1.5x DPI, including
            // anisotropic output. This checks the two independent scale stages.
            for unit in [0.5, 1.5, 2.5] {
                doc.set_page_user_unit(0, Some(unit)).unwrap();
                let reopened =
                    Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                        .unwrap();
                for dpi in [(72., 72.), (72., 144.)] {
                    let expected = raster(&doc, 1, (dpi.0 * unit, dpi.1 * unit));
                    let actual = raster(&doc, 0, dpi);
                    assert_eq!(
                        actual, expected,
                        "rotation={rotation}, unit={unit}, dpi={dpi:?}"
                    );
                    assert_eq!(actual, raster(&reopened, 0, dpi));
                    // Positive evidence of image, path and actual text ink.
                    for color in [
                        [255, 0, 0, 255],
                        [0, 255, 0, 255],
                        [0, 0, 255, 255],
                        [0, 0, 0, 255],
                    ] {
                        assert!(
                            actual.2.as_chunks::<4>().0.contains(&color),
                            "missing color {color:?}, rotation={rotation}, unit={unit}, dpi={dpi:?}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn user_unit_scales_rotated_pixels_and_survives_save() {
        use pdf::{
            document::{owned::Document, save_options::PdfVersion},
            geom::rectangle::Rectangle,
            parser::syntax::ParseMode,
        };
        fn raster(doc: &Document, page: u32, width: usize, height: usize) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            assert_eq!(
                (renderer.page_width, renderer.page_height),
                (width as i32, height as i32)
            );
            let info = skia_safe::ImageInfo::new(
                (width as i32, height as i32),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; width * height * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, width * 4, (0, 0))
            );
            pixels
        }
        for rotation in [0, 90, 180, 270] {
            let doc = Document::new_with_version(PdfVersion::V1_6).unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_crop_box(0, Some(Rectangle::new(10., 20., 90., 70.)))
                .unwrap();
            doc.replace_page_contents(
                0,
                b"1 0 0 rg 15 55 20 10 re f 0 1 0 rg 55 25 30 15 re f 0 0 1 rg 10 20 5 50 re f"
                    .to_vec(),
            )
            .unwrap();
            doc.set_page_rotation(0, rotation).unwrap();
            doc.copy_page(0, 1).unwrap();
            let (width, height) = if rotation % 180 == 0 {
                (80, 50)
            } else {
                (50, 80)
            };
            let original = raster(&doc, 1, width, height);
            for color in [
                [255, 0, 0, 255],
                [0, 255, 0, 255],
                [0, 0, 255, 255],
                [255, 255, 255, 255],
            ] {
                assert!(original.as_chunks::<4>().0.contains(&color));
            }
            doc.set_page_user_unit(0, Some(2.)).unwrap();
            let scaled = raster(&doc, 0, width * 2, height * 2);
            // Integer-aligned solid rectangles give an exact, independent 2x oracle.
            for y in 0..height * 2 {
                for x in 0..width * 2 {
                    let old = ((y / 2) * width + x / 2) * 4;
                    let new = (y * width * 2 + x) * 4;
                    assert_eq!(
                        &scaled[new..new + 4],
                        &original[old..old + 4],
                        "rotation={rotation}, pixel=({x},{y})"
                    );
                }
            }
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(scaled, raster(&reopened, 0, width * 2, height * 2));
            assert_eq!(original, raster(&doc, 1, width, height));
            assert_eq!(original, raster(&reopened, 1, width, height));
            doc.set_page_user_unit(0, None).unwrap();
            assert_eq!(original, raster(&doc, 0, width, height));
        }
    }

    #[test]
    fn relative_page_rotation_is_a_pixel_rotation_not_a_reflection() {
        use pdf::{
            document::owned::Document, geom::rectangle::Rectangle, parser::syntax::ParseMode,
        };
        fn raster(doc: &Document, page: u32, width: usize, height: usize) -> Vec<u8> {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            doc.get_page(page).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (width as i32, height as i32),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; width * height * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, width * 4, (0, 0))
            );
            pixels
        }
        for rotation in [0, 90, 180, 270] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_crop_box(0, Some(Rectangle::new(10., 20., 90., 70.)))
                .unwrap();
            doc.replace_page_contents(
                0,
                b"1 0 0 rg 15 55 20 10 re f 0 1 0 rg 55 25 30 15 re f 0 0 1 rg 10 20 5 50 re f"
                    .to_vec(),
            )
            .unwrap();
            doc.copy_page(0, 1).unwrap();
            let original = raster(&doc, 1, 80, 50);
            assert_eq!(
                &original[(25 * 80 + 2) * 4..(25 * 80 + 2) * 4 + 4],
                &[0, 0, 255, 255]
            );
            for (x, y, expected) in [
                (15, 10, [255, 0, 0, 255]),
                (60, 35, [0, 255, 0, 255]),
                (35, 25, [255, 255, 255, 255]),
            ] {
                let offset = (y * 80 + x) * 4;
                assert_eq!(&original[offset..offset + 4], &expected);
            }
            doc.rotate_page(0, rotation).unwrap();
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            let (width, height) = if rotation % 180 == 0 {
                (80, 50)
            } else {
                (50, 80)
            };
            let edited = raster(&doc, 0, width, height);
            assert_eq!(edited, raster(&reopened, 0, width, height));
            assert_eq!(original, raster(&doc, 1, 80, 50));
            assert_eq!(original, raster(&reopened, 1, 80, 50));
            for y in 0..50 {
                for x in 0..80 {
                    let (new_x, new_y) = match rotation {
                        0 => (x, y),
                        90 => (49 - y, x),
                        180 => (79 - x, 49 - y),
                        270 => (y, 79 - x),
                        _ => unreachable!(),
                    };
                    let old = (y * 80 + x) * 4;
                    let new = (new_y * width + new_x) * 4;
                    assert_eq!(
                        &original[old..old + 4],
                        &edited[new..new + 4],
                        "rotation={rotation}, pixel=({x},{y})"
                    );
                }
            }
        }
    }

    #[test]
    fn removing_path_paint_keeps_pending_clip_for_later_draws() {
        use pdf::{
            document::PdfDocument, page::page_builder::PageBuilder, parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(30., 10.))
            .unwrap();
        doc.replace_page_contents(
            0,
            b"1 0 0 rg 0 0 20 10 re W f 0 0 1 rg 10 0 20 10 re f".to_vec(),
        )
        .unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        doc.remove_page_path_draw(0, &snapshot, 3).unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let mut rendered = Vec::new();
        for document in [doc, reopened] {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            document
                .get_page(0)
                .unwrap()
                .display(&mut renderer)
                .unwrap();
            let info = skia_safe::ImageInfo::new(
                (30, 10),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 30 * 10 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 30 * 4, (0, 0))
            );
            for (x, expected) in [
                (5usize, [255u8, 255, 255, 255]),
                (15, [0, 0, 255, 255]),
                (25, [255, 255, 255, 255]),
            ] {
                let offset = (5 * 30 + x) * 4;
                assert_eq!(&pixels[offset..offset + 4], &expected);
            }
            rendered.push(pixels);
        }
        assert_eq!(rendered[0], rendered[1]);
    }

    #[test]
    fn display_space_image_rotation_preserves_texture_orientation_and_neighbor() {
        use pdf::{
            document::PdfDocument, page::page_builder::PageBuilder, parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(40., 40.))
            .unwrap();
        let texture = doc
            .resources()
            .embed_rgb_image(0, 2, 1, vec![255, 0, 0, 0, 255, 0])
            .unwrap();
        let blue = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![0, 0, 255])
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("q 20 0 0 10 0 0 cm /{texture} Do Q q 10 0 0 10 30 0 cm /{blue} Do Q")
                .into_bytes(),
        )
        .unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        // In display space: (x,y) -> (40-y,x). Bottom-left horizontal
        // red/green texture becomes a top-left vertical red/green texture.
        doc.transform_page_image_draw_display(0, &snapshot, 2, [0., 1., -1., 0., 40., 0.])
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let mut rendered = Vec::new();
        for document in [doc, reopened] {
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            document
                .get_page(0)
                .unwrap()
                .display(&mut renderer)
                .unwrap();
            let info = skia_safe::ImageInfo::new(
                (40, 40),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 40 * 40 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 40 * 4, (0, 0))
            );
            for (x, y, expected) in [
                (5usize, 5usize, [255u8, 0, 0, 255]),
                (5, 15, [0, 255, 0, 255]),
                (15, 35, [255, 255, 255, 255]),
                (35, 35, [0, 0, 255, 255]),
                (15, 15, [255, 255, 255, 255]),
            ] {
                let offset = (y * 40 + x) * 4;
                assert_eq!(&pixels[offset..offset + 4], &expected);
            }
            rendered.push(pixels);
        }
        assert_eq!(rendered[0], rendered[1]);
    }

    #[test]
    fn edited_image_translation_renders_in_current_user_space_without_leaking() {
        use pdf::{
            document::PdfDocument, page::page_builder::PageBuilder, parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(30.0, 10.0))
            .unwrap();
        let red = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        let blue = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![0, 0, 255])
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("q 10 0 0 10 0 0 cm /{red} Do Q q 10 0 0 10 20 0 cm /{blue} Do Q").into_bytes(),
        )
        .unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        // One local user-space unit becomes ten page units under the existing cm.
        doc.transform_page_image_draw(0, &snapshot, 2, [1.0, 0.0, 0.0, 1.0, 1.0, 0.0])
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [doc, reopened] {
            let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
            document
                .get_page(0)
                .unwrap()
                .display(&mut renderer)
                .unwrap();
            let info = skia_safe::ImageInfo::new(
                (30, 10),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 30 * 10 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 30 * 4, (0, 0))
            );
            for (x, expected) in [
                (5usize, [255u8, 255, 255, 255]),
                (15, [255, 0, 0, 255]),
                (25, [0, 0, 255, 255]),
            ] {
                assert_eq!(&pixels[(5 * 30 + x) * 4..(5 * 30 + x) * 4 + 4], &expected);
            }
        }
    }

    #[test]
    fn edited_rectangle_renders_at_new_position_without_changing_adjacent_path() {
        use pdf::{
            document::PdfDocument, page::page_builder::PageBuilder, parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(30.0, 10.0))
            .unwrap();
        doc.replace_page_contents(
            0,
            b"1 0 0 rg 0 0 10 10 re f 0 0 1 rg 20 0 10 10 re f".to_vec(),
        )
        .unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        doc.replace_page_path_numbers(0, &snapshot, 1, &[10.0, 0.0, 10.0, 10.0])
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [doc, reopened] {
            let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
            document
                .get_page(0)
                .unwrap()
                .display(&mut renderer)
                .unwrap();
            let info = skia_safe::ImageInfo::new(
                (30, 10),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 30 * 10 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 30 * 4, (0, 0))
            );
            for (x, expected) in [
                (5usize, [255u8, 255, 255, 255]),
                (15, [255, 0, 0, 255]),
                (25, [0, 0, 255, 255]),
            ] {
                assert_eq!(&pixels[(5 * 30 + x) * 4..(5 * 30 + x) * 4 + 4], &expected);
            }
        }
    }

    #[test]
    fn custom_surface_budgets_check_boundaries_and_preserve_previous_page() {
        let mut renderer = SkiaRender::with_limits(
            72.0,
            72.0,
            Limits {
                max_render_pixels: 16,
                max_render_bytes: 64,
                ..Default::default()
            },
        )
        .unwrap();
        renderer.start_page(4.0, 4.0).unwrap();
        assert!(matches!(
            renderer.start_page(5.0, 4.0),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!((renderer.page_width, renderer.page_height), (4, 4));
        assert_eq!(
            (renderer.surface.width(), renderer.surface.height()),
            (4, 4)
        );
        renderer.start_page(2.0, 2.0).unwrap();
        let mut renderer = SkiaRender::with_limits(
            144.0,
            72.0,
            Limits {
                max_render_pixels: 100,
                max_render_bytes: 31,
                ..Default::default()
            },
        )
        .unwrap();
        assert!(renderer.start_page(2.0, 2.0).is_err());
        renderer.start_page(1.0, 2.0).unwrap();
        assert!(
            SkiaRender::with_limits(
                72.0,
                72.0,
                Limits {
                    max_render_bytes: 3,
                    ..Default::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn text_prefix_pixels_do_not_change_when_a_character_is_appended() {
        use pdf::{
            document::{PdfDocument, resource_manager::StandardFont},
            page::{content_builder::ContentBuilder, page_builder::PageBuilder},
        };
        let render = |text: &str, scale: u32, matrix: [f32; 6], size: f32| {
            let doc = PdfDocument::new().unwrap();
            doc.add_page_with_builder(0, PageBuilder::new().size(160.0, 60.0))
                .unwrap();
            let name = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            let mut builder = ContentBuilder::new();
            builder
                .begin_text()
                .set_font(&name, size)
                .raw_operator(&format!("{scale} Tz"))
                .set_text_matrix(
                    matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5],
                )
                .show_text(text)
                .end_text();
            doc.replace_page_content_builder(0, &builder).unwrap();
            let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
            doc.get_page(0).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (160, 60),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut pixels = vec![0; 160 * 60 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 160 * 4, (0, 0))
            );
            pixels
        };
        let matrix = [1.0, 0.0, 0.0, 1.0, 10.25, 15.25];
        let single = render("I", 100, matrix, 36.0);
        let pair = render("I I", 100, matrix, 36.0);
        let normal = render("M", 100, matrix, 36.0);
        let compressed = render("M", 50, matrix, 36.0);
        let bounds = |pixels: &[u8]| {
            let mut min_x = 160;
            let mut max_x = 0;
            for y in 0..60 {
                for x in 0..160 {
                    if pixels[(y * 160 + x) * 4] < 240 {
                        min_x = min_x.min(x);
                        max_x = max_x.max(x);
                    }
                }
            }
            assert!(min_x <= max_x, "glyph must produce visible pixels");
            max_x - min_x + 1
        };
        let normal_width = bounds(&normal);
        let compressed_width = bounds(&compressed);
        assert!(compressed_width < normal_width);
        assert!((compressed_width * 2).abs_diff(normal_width) <= 2);
        // A 90-degree matrix has a=0; it must still paint a full-height glyph.
        let rotated = render("I", 100, [0.0, 1.0, -1.0, 0.0, 60.25, 15.25], 36.0);
        assert!(bounds(&rotated) > bounds(&single) * 2);
        let stretched = render("M", 100, [2.0, 0.0, 0.0, 1.0, 10.25, 15.25], 36.0);
        assert!(bounds(&stretched).abs_diff(normal_width * 2) <= 2);
        let negative = render("M", 100, [1.0, 0.0, 0.0, 1.0, 80.25, 45.25], -36.0);
        let mirrored = render("M", 100, [-1.0, 0.0, 0.0, -1.0, 80.25, 45.25], 36.0);
        assert_eq!(negative, mirrored);
        assert!(bounds(&negative) > 0);
        let collapsed = render("M", 100, [0.0, 0.0, 0.0, 1.0, 10.25, 15.25], 36.0);
        assert!(collapsed.iter().all(|byte| *byte == 255));
        let mut antialiased = false;
        for y in 0..60 {
            for x in 0..20 {
                let offset = (y * 160 + x) * 4;
                let pixel = &single[offset..offset + 4];
                antialiased |= pixel[0] > 0 && pixel[0] < 255;
                assert_eq!(pixel, &pair[offset..offset + 4], "prefix pixel {x},{y}");
            }
        }
        assert!(
            antialiased,
            "fixture must exercise partially covered glyph pixels"
        );
    }

    #[test]
    fn extgstate_font_and_dash_render_like_explicit_operators() {
        use pdf::{
            document::{owned::Document, resource_manager::StandardFont},
            objects::{
                PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_number::PdfNumber,
                pdf_reference::PdfReference,
            },
            page::page_builder::PageBuilder,
            parser::syntax::ParseMode,
        };
        let doc = Document::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(160.0, 60.0))
            .unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let font = doc
            .try_insert_object(
                doc.get_page(0)
                    .unwrap()
                    .get_resource_font(&name)
                    .unwrap()
                    .unwrap(),
            )
            .unwrap();
        let mut font_value = PdfArray::default();
        font_value
            .try_add_obj(PdfObject::PdfReference(PdfReference::from_id(font)))
            .unwrap();
        font_value
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(24.0)))
            .unwrap();
        let mut dash = PdfArray::default();
        for number in [4.0, 2.0] {
            dash.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(number)))
                .unwrap();
        }
        let mut pattern = PdfArray::default();
        pattern.try_add_obj(PdfObject::PdfArray(dash)).unwrap();
        pattern
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(1.0)))
            .unwrap();
        let mut state = PdfDict::default();
        state.insert("Font".into(), PdfObject::PdfArray(font_value));
        state.insert("LW".into(), PdfObject::PdfNumber(PdfNumber::new(3.0)));
        state.insert("D".into(), PdfObject::PdfArray(pattern));
        let mut states = PdfDict::default();
        states.insert("G".into(), PdfObject::PdfDict(state));
        let mut resources = PdfDict::default();
        resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
        doc.replace_page_contents(
            0,
            format!(
                "3 w [4 2] 1 d BT /{name} 24 Tf 1 0 0 1 10 20 Tm (MM) Tj ET 10 10 m 120 10 l S"
            )
            .into_bytes(),
        )
        .unwrap();
        doc.add_page_with_builder(
            1,
            PageBuilder::new()
                .size(160.0, 60.0)
                .resources(resources)
                .content(b"/G gs BT 1 0 0 1 10 20 Tm (MM) Tj ET 10 10 m 120 10 l S".to_vec()),
        )
        .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [doc, reopened] {
            let mut pages = Vec::new();
            for index in [0, 1] {
                let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
                doc.get_page(index).unwrap().display(&mut renderer).unwrap();
                let info = skia_safe::ImageInfo::new(
                    (160, 60),
                    skia_safe::ColorType::RGBA8888,
                    skia_safe::AlphaType::Unpremul,
                    None,
                );
                let mut pixels = vec![0; 160 * 60 * 4];
                assert!(
                    renderer
                        .surface
                        .read_pixels(&info, &mut pixels, 160 * 4, (0, 0))
                );
                assert!(
                    pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|pixel| pixel[..3] != [255, 255, 255])
                );
                pages.push(pixels);
            }
            assert_eq!(pages[0], pages[1]);
        }
    }

    #[test]
    fn alpha_constants_composite_paths_images_and_transparent_text() {
        use pdf::{
            document::{owned::Document, resource_manager::StandardFont},
            objects::{PdfObject, pdf_dict::PdfDict, pdf_number::PdfNumber},
            page::page_builder::PageBuilder,
            parser::syntax::ParseMode,
        };
        let make = |fill: f32, stroke: f32| {
            let doc = Document::new().unwrap();
            let mut state = PdfDict::default();
            state.insert("ca".into(), PdfObject::PdfNumber(PdfNumber::new(fill)));
            state.insert("CA".into(), PdfObject::PdfNumber(PdfNumber::new(stroke)));
            let mut states = PdfDict::default();
            states.insert("G".into(), PdfObject::PdfDict(state));
            let mut resources = PdfDict::default();
            resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
            doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 40.0).resources(resources))
                .unwrap();
            doc
        };
        let pixels = |doc: &Document| {
            let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
            doc.get_page(0).unwrap().display(&mut renderer).unwrap();
            let info = skia_safe::ImageInfo::new(
                (40, 40),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut output = vec![0; 40 * 40 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut output, 40 * 4, (0, 0))
            );
            output
        };
        let check = |data: &[u8], x: usize, y: usize, expected: [u8; 3]| {
            let pixel = &data[(y * 40 + x) * 4..(y * 40 + x) * 4 + 4];
            for channel in 0..3 {
                assert!(
                    pixel[channel].abs_diff(expected[channel]) <= 1,
                    "pixel {x},{y}: {pixel:?}"
                );
            }
            assert_eq!(pixel[3], 255);
        };
        let doc = make(0.5, 0.25);
        doc.replace_page_contents(
            0,
            b"q /G gs 1 0 0 rg 0 0 10 10 re f 0 0 0 RG 4 w 0 20 m 20 20 l S Q 20 0 10 10 re f"
                .to_vec(),
        )
        .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [doc, reopened] {
            let data = pixels(&doc);
            check(&data, 5, 35, [255, 128, 128]);
            check(&data, 5, 20, [191, 191, 191]);
            check(&data, 25, 35, [0, 0, 0]);
        }
        let doc = make(0.5, 1.0);
        let name = doc
            .resources()
            .embed_rgba_image(0, 1, 1, vec![255, 0, 0, 128])
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("q /G gs 10 0 0 10 0 0 cm /{name} Do Q q 10 0 0 10 20 0 cm /{name} Do Q")
                .into_bytes(),
        )
        .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [doc, reopened] {
            let data = pixels(&doc);
            check(&data, 5, 35, [255, 191, 191]);
            check(&data, 25, 35, [255, 127, 127]);
        }
        for (mode, fill, stroke) in [(0, 0.0, 1.0), (1, 1.0, 0.0), (2, 0.0, 0.0)] {
            let doc = make(fill, stroke);
            let name = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            doc.replace_page_contents(
                0,
                format!("/G gs BT /{name} 12 Tf {mode} Tr 1 0 0 1 2 15 Tm (ABC) Tj ET")
                    .into_bytes(),
            )
            .unwrap();
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for doc in [doc, reopened] {
                let report = doc
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order_report()
                    .unwrap();
                assert!(report.diagnostics.is_empty());
                assert_eq!(
                    report
                        .characters
                        .iter()
                        .map(|character| character.unicode.as_deref().unwrap())
                        .collect::<String>(),
                    "ABC"
                );
                assert!(
                    pixels(&doc)
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|pixel| *pixel == [255, 255, 255, 255])
                );
            }
        }
    }

    #[test]
    fn standard_blend_modes_match_rgb_compositing_and_restore_normal() {
        use pdf::{
            document::owned::Document,
            objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber},
            page::page_builder::PageBuilder,
            parser::syntax::ParseMode,
        };
        for mode in [
            "Normal",
            "Multiply",
            "Screen",
            "Darken",
            "Lighten",
            "Difference",
            "Exclusion",
            "Overlay",
            "HardLight",
        ] {
            for alpha in [1.0, 0.5] {
                let doc = Document::new().unwrap();
                let mut state = PdfDict::default();
                state.insert("BM".into(), PdfObject::PdfName(PdfName::new(mode.into())));
                state.insert("ca".into(), PdfObject::PdfNumber(PdfNumber::new(alpha)));
                let mut states = PdfDict::default();
                states.insert("G".into(), PdfObject::PdfDict(state));
                let mut resources = PdfDict::default();
                resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
                doc.add_page_with_builder(0,PageBuilder::new().size(40.0,20.0).resources(resources).content(b"0.25 0.5 0.75 rg 0 0 40 20 re f q /G gs 0.75 0.25 0.5 rg 0 0 20 20 re f Q 0.75 0.25 0.5 rg 20 0 20 20 re f".to_vec())).unwrap();
                let reopened =
                    Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                        .unwrap();
                for doc in [doc, reopened] {
                    let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
                    doc.get_page(0).unwrap().display(&mut renderer).unwrap();
                    let info = skia_safe::ImageInfo::new(
                        (40, 20),
                        skia_safe::ColorType::RGBA8888,
                        skia_safe::AlphaType::Unpremul,
                        None,
                    );
                    let mut pixels = vec![0; 40 * 20 * 4];
                    assert!(
                        renderer
                            .surface
                            .read_pixels(&info, &mut pixels, 40 * 4, (0, 0))
                    );
                    for (channel, (backdrop, source)) in
                        [(0.25f32, 0.75f32), (0.5, 0.25), (0.75, 0.5)]
                            .iter()
                            .enumerate()
                    {
                        let (b, s) = (*backdrop, *source);
                        let blend = match mode {
                            "Normal" => s,
                            "Multiply" => b * s,
                            "Screen" => b + s - b * s,
                            "Darken" => b.min(s),
                            "Lighten" => b.max(s),
                            "Difference" => (b - s).abs(),
                            "Exclusion" => b + s - 2.0 * b * s,
                            "Overlay" => {
                                if b <= 0.5 {
                                    2.0 * b * s
                                } else {
                                    1.0 - 2.0 * (1.0 - b) * (1.0 - s)
                                }
                            }
                            "HardLight" => {
                                if s <= 0.5 {
                                    2.0 * b * s
                                } else {
                                    1.0 - 2.0 * (1.0 - b) * (1.0 - s)
                                }
                            }
                            _ => unreachable!(),
                        };
                        let expected = ((alpha * blend + (1.0 - alpha) * b) * 255.0).round() as u8;
                        let actual = pixels[(10 * 40 + 10) * 4 + channel];
                        assert!(
                            actual.abs_diff(expected) <= 2,
                            "{mode} alpha {alpha} channel {channel}: {actual} != {expected}"
                        );
                        let normal = pixels[(10 * 40 + 30) * 4 + channel];
                        assert!(
                            normal.abs_diff((s * 255.0).round() as u8) <= 1,
                            "q/Q normal restore"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn multiply_blending_applies_to_images_and_text_not_only_paths() {
        use pdf::{
            document::{owned::Document, resource_manager::StandardFont},
            objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber},
            page::page_builder::PageBuilder,
            parser::syntax::ParseMode,
        };
        for image in [false, true] {
            let doc = Document::new().unwrap();
            let mut state = PdfDict::default();
            state.insert(
                "BM".into(),
                PdfObject::PdfName(PdfName::new("Multiply".into())),
            );
            state.insert("ca".into(), PdfObject::PdfNumber(PdfNumber::new(0.5)));
            let mut states = PdfDict::default();
            states.insert("G".into(), PdfObject::PdfDict(state));
            let mut resources = PdfDict::default();
            resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
            doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 20.0).resources(resources))
                .unwrap();
            let content = if image {
                let name = doc
                    .resources()
                    .embed_rgba_image(0, 1, 1, vec![255, 0, 0, 128])
                    .unwrap();
                format!("0 1 1 rg 0 0 40 20 re f /G gs q 40 0 0 20 0 0 cm /{name} Do Q")
            } else {
                let name = doc
                    .resources()
                    .register_standard_font(0, StandardFont::Helvetica)
                    .unwrap();
                format!(
                    "0 1 1 rg 0 0 40 20 re f /G gs 1 0 0 rg BT /{name} 16 Tf 1 0 0 1 2 4 Tm (MM) Tj ET"
                )
            };
            doc.replace_page_contents(0, content.into_bytes()).unwrap();
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for doc in [doc, reopened] {
                let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
                doc.get_page(0).unwrap().display(&mut renderer).unwrap();
                let info = skia_safe::ImageInfo::new(
                    (40, 20),
                    skia_safe::ColorType::RGBA8888,
                    skia_safe::AlphaType::Unpremul,
                    None,
                );
                let mut pixels = vec![0; 40 * 20 * 4];
                assert!(
                    renderer
                        .surface
                        .read_pixels(&info, &mut pixels, 40 * 4, (0, 0))
                );
                assert!(
                    pixels.as_chunks::<4>().0.iter().all(|pixel| pixel[0] <= 1),
                    "Multiply cannot introduce red over cyan"
                );
                assert!(
                    pixels
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .any(|pixel| pixel[1] < 200 && pixel[2] < 200),
                    "fixture must paint foreground"
                );
                if image {
                    let pixel = &pixels[(10 * 40 + 20) * 4..(10 * 40 + 20) * 4 + 4];
                    assert!(pixel[1].abs_diff(191) <= 1 && pixel[2].abs_diff(191) <= 1);
                }
            }
        }
    }

    #[test]
    fn none_colorants_do_not_paint_text_or_paths_and_preserve_visible_combined_sides() {
        use pdf::{
            document::{
                PdfDocument,
                resource_manager::StandardFont,
                save_options::{SaveOptions, StreamCompression},
            },
            io::stream_reader::StreamReader,
            page::page_builder::PageBuilder,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        let pixels = |renderer: &mut SkiaRender| {
            let info = skia_safe::ImageInfo::new(
                (40, 40),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            let mut bytes = vec![0; 40 * 40 * 4];
            assert!(
                renderer
                    .surface
                    .read_pixels(&info, &mut bytes, 40 * 4, (0, 0))
            );
            bytes
        };
        for source in [
            "[/Separation /None /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 /C0 [0] /C1 [0] >>]",
            "[/DeviceN [/None] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 /C0 [0] /C1 [0] >>]",
            "[/Indexed [/Separation /None /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 /C0 [0] /C1 [0] >>] 1 <00FF>]",
            "[/Indexed [/DeviceN [/None] /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 /C0 [0] /C1 [0] >>] 1 <00FF>]",
        ] {
            let doc = PdfDocument::new().unwrap();
            doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 40.0))
                .unwrap();
            let color = SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap();
            let name = doc.resources().register_color_space(0, color).unwrap();
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            doc.replace_page_contents(0, format!("/{name} cs /{name} CS 0 0 40 40 re B BT /{font} 10 Tf 1 0 0 1 5 15 Tm (Hidden) Tj ET").into_bytes()).unwrap();
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap()
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "Hidden"
            );
            let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
            doc.get_page(0).unwrap().display(&mut renderer).unwrap();
            assert!(pixels(&mut renderer).iter().all(|byte| *byte == 255));
            doc.replace_page_contents(
                0,
                format!(
                    "/{name} cs 1 0 0 RG 2 w 5 5 10 10 re B 0 1 0 rg /{name} CS 25 5 10 10 re B"
                )
                .into_bytes(),
            )
            .unwrap();
            doc.get_page(0).unwrap().display(&mut renderer).unwrap();
            let expected = pixels(&mut renderer);
            assert!(
                expected
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[0] > 240 && p[1] < 15 && p[2] < 15)
            );
            assert!(
                expected
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .any(|p| p[0] < 15 && p[1] > 240 && p[2] < 15)
            );
            let center = &expected[(30 * 40 + 10) * 4..(30 * 40 + 10) * 4 + 4];
            assert_eq!(center, &[255, 255, 255, 255]);
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let reopened = PdfDocument::from_bytes_with_mode(
                    doc.save_to_bytes_with_options(SaveOptions {
                        compression,
                        ..Default::default()
                    })
                    .unwrap(),
                    ParseMode::Strict,
                )
                .unwrap();
                reopened
                    .get_page(0)
                    .unwrap()
                    .display(&mut renderer)
                    .unwrap();
                assert_eq!(pixels(&mut renderer), expected);
            }
        }
    }

    #[test]
    fn stencil_pixels_use_current_fill_decode_and_none_after_save() {
        use pdf::{
            document::{
                PdfDocument,
                resource_manager::ResourceKind,
                save_options::{SaveOptions, StreamCompression},
            },
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::page_builder::PageBuilder,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        for reversed in [false, true] {
            let doc = PdfDocument::new().unwrap();
            doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 40.0))
                .unwrap();
            let dict = parse(&format!(
                "<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /ImageMask true {} >>",
                if reversed { "/Decode [1 0]" } else { "" }
            ))
            .into_dict()
            .unwrap();
            let object = doc
                .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![0x40])))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Image, object)
                .unwrap();
            let none = doc
                .resources()
                .register_color_space(
                    0,
                    parse(
                        "[/Separation /None /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
                    ),
                )
                .unwrap();
            doc.replace_page_contents(0, format!("0 0 1 rg 0 0 40 40 re f q 20 0 0 10 5 5 cm 1 0 0 rg /{name} Do Q q 20 0 0 10 5 20 cm 0 1 0 rg /{name} Do Q /{none} cs q 40 0 0 40 0 0 cm /{name} Do Q").into_bytes()).unwrap();
            let render = |doc: &PdfDocument<std::io::Cursor<Vec<u8>>>| {
                let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
                doc.get_page(0).unwrap().display(&mut renderer).unwrap();
                let info = skia_safe::ImageInfo::new(
                    (40, 40),
                    skia_safe::ColorType::RGBA8888,
                    skia_safe::AlphaType::Unpremul,
                    None,
                );
                let mut bytes = vec![0; 40 * 40 * 4];
                assert!(
                    renderer
                        .surface
                        .read_pixels(&info, &mut bytes, 40 * 4, (0, 0))
                );
                bytes
            };
            let expected = render(&doc);
            for (y, painted) in [(30, [255, 0, 0, 255]), (15, [0, 255, 0, 255])] {
                for (x, left) in [(10, true), (20, false)] {
                    let offset = (y * 40 + x) * 4;
                    assert_eq!(
                        &expected[offset..offset + 4],
                        if left != reversed {
                            &painted
                        } else {
                            &[0, 0, 255, 255]
                        }
                    );
                }
            }
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let bytes = doc
                    .save_to_bytes_with_options(SaveOptions {
                        compression,
                        ..Default::default()
                    })
                    .unwrap();
                let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
                assert_eq!(render(&reopened), expected);
            }
        }
    }

    #[test]
    fn calibrated_image_pixels_and_reverse_decode_survive_save() {
        use pdf::{
            document::{
                PdfDocument,
                resource_manager::ResourceKind,
                save_options::{SaveOptions, StreamCompression},
            },
            io::stream_reader::StreamReader,
            objects::{PdfObject, pdf_stream::PdfStream},
            page::page_builder::PageBuilder,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        for kind in 0..8 {
            let icc = kind == 1;
            for reversed in [false, true] {
                let doc = PdfDocument::new().unwrap();
                doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 20.0))
                    .unwrap();
                let (space, samples, low, high) = if icc {
                    let dict = parse("<< /N 1 /Range [0.25 0.75] /Alternate /DeviceGray >>")
                        .into_dict()
                        .unwrap();
                    let id = doc
                        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                            dict,
                            pdf::color::icc_profile::ICCProfile::gray_srgb()
                                .unwrap()
                                .to_bytes()
                                .unwrap(),
                        )))
                        .unwrap();
                    (
                        format!("[/ICCBased {} {} R]", id.obj_num(), id.gen_num()),
                        vec![0, 0, 255, 255],
                        64,
                        191,
                    )
                } else if kind == 2 || kind == 5 || kind == 6 {
                    let colorant = if kind == 5 { "None" } else { "Spot" };
                    let space = if kind == 6 {
                        let dict = parse("<< /FunctionType 4 /Domain [0 1] /Range [0 1] >>")
                            .into_dict()
                            .unwrap();
                        let id = doc
                            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                                dict,
                                b"{.5 gt {1} {0} ifelse}".to_vec(),
                            )))
                            .unwrap();
                        format!(
                            "[/Separation /Spot /DeviceGray {} {} R]",
                            id.obj_num(),
                            id.gen_num()
                        )
                    } else {
                        format!(
                            "[/Separation /{colorant} /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]"
                        )
                    };
                    (
                        space,
                        vec![0, 0, 255, 255],
                        if kind == 5 { 255 } else { 0 },
                        255,
                    )
                } else if kind == 3 || kind == 4 || kind == 7 {
                    let (function, bytes) = if kind == 7 {
                        (
                            parse(
                                "<< /FunctionType 4 /Domain [0 1 0 1 0 1 0 1 0 1 0 1] /Range [0 1] >>",
                            )
                            .into_dict()
                            .unwrap(),
                            b"{pop pop pop pop pop .5 gt {1} {0} ifelse}".to_vec(),
                        )
                    } else {
                        (parse("<< /FunctionType 0 /Domain [0 1 0 1 0 1 0 1 0 1 0 1] /Range [0 1] /Size [2 1 1 1 1 1] /BitsPerSample 8 >>").into_dict().unwrap(), vec![0, 255])
                    };
                    let id = doc
                        .try_insert_object(PdfObject::PdfStream(PdfStream::new(function, bytes)))
                        .unwrap();
                    let names = if kind == 4 {
                        "/None /None /None /None /None /None"
                    } else {
                        "/A /B /None /C /D /E"
                    };
                    let mut samples = vec![0; 24];
                    samples[12] = 255;
                    samples[13] = 255;
                    (
                        format!(
                            "[/DeviceN [{names}] /DeviceGray {} {} R]",
                            id.obj_num(),
                            id.gen_num()
                        ),
                        samples,
                        if kind == 4 { 255 } else { 0 },
                        255,
                    )
                } else {
                    (
                        "[/Lab << /WhitePoint [0.95047 1 1.08883] /Range [0 0 0 0] >>]".into(),
                        vec![0, 0, 0, 0, 0, 0, 255, 255, 0, 0, 0, 0],
                        0,
                        255,
                    )
                };
                let decode = if reversed {
                    if icc {
                        "/Decode [0.75 0.25]"
                    } else if kind == 2 || kind == 5 || kind == 6 {
                        "/Decode [1 0]"
                    } else if kind == 3 || kind == 4 || kind == 7 {
                        "/Decode [1 0 0 1 0 1 0 1 0 1 0 1]"
                    } else {
                        "/Decode [100 0 0 0 0 0]"
                    }
                } else {
                    ""
                };
                let dict = parse(&format!("<< /Type /XObject /Subtype /Image /Width 2 /Height 1 /BitsPerComponent 16 /ColorSpace {space} {decode} >>")).into_dict().unwrap();
                let id = doc
                    .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, samples)))
                    .unwrap();
                let name = doc
                    .resources()
                    .register_existing(0, ResourceKind::Image, id)
                    .unwrap();
                doc.replace_page_contents(
                    0,
                    format!("q 40 0 0 20 0 0 cm /{name} Do Q").into_bytes(),
                )
                .unwrap();
                let render = |doc: &PdfDocument<std::io::Cursor<Vec<u8>>>| {
                    assert_eq!(
                        doc.get_page(0)
                            .unwrap()
                            .display_list()
                            .unwrap()
                            .objects()
                            .len(),
                        1
                    );
                    let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
                    doc.get_page(0).unwrap().display(&mut renderer).unwrap();
                    let info = skia_safe::ImageInfo::new(
                        (40, 20),
                        skia_safe::ColorType::RGBA8888,
                        skia_safe::AlphaType::Unpremul,
                        None,
                    );
                    let mut pixels = vec![0; 40 * 20 * 4];
                    assert!(
                        renderer
                            .surface
                            .read_pixels(&info, &mut pixels, 40 * 4, (0, 0))
                    );
                    pixels
                };
                let expected = render(&doc);
                if kind == 4 || kind == 5 {
                    assert!(expected.iter().all(|byte| *byte == 255));
                }
                for (x, left) in [(5, true), (35, false)] {
                    let gray = if left != reversed { low } else { high };
                    let offset = (10 * 40 + x) * 4;
                    assert_eq!(&expected[offset..offset + 4], &[gray, gray, gray, 255]);
                }
                for compression in [
                    StreamCompression::Preserve,
                    StreamCompression::FlateUnencoded,
                ] {
                    let bytes = doc
                        .save_to_bytes_with_options(SaveOptions {
                            compression,
                            ..Default::default()
                        })
                        .unwrap();
                    let reopened =
                        PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
                    assert_eq!(render(&reopened), expected);
                }
            }
        }
    }

    #[test]
    fn invisible_text_is_extractable_but_does_not_paint() {
        use pdf::{
            document::{PdfDocument, resource_manager::StandardFont},
            page::{content_builder::ContentBuilder, page_builder::PageBuilder},
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(100.0, 60.0))
            .unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let mut builder = ContentBuilder::new();
        builder
            .begin_text()
            .set_font(&name, 30.0)
            .set_text_rendering_mode(TextRenderingMode::INVisible)
            .set_text_matrix(1.0, 0.0, 0.0, 1.0, 10.0, 20.0)
            .show_text("Hidden")
            .end_text();
        doc.replace_page_content_builder(0, &builder).unwrap();
        let chars = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap();
        assert_eq!(
            chars
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "Hidden"
        );
        let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
        doc.get_page(0).unwrap().display(&mut renderer).unwrap();
        let info = skia_safe::ImageInfo::new(
            (100, 60),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        let mut pixels = vec![0; 100 * 60 * 4];
        assert!(
            renderer
                .surface
                .read_pixels(&info, &mut pixels, 100 * 4, (0, 0))
        );
        assert!(pixels.iter().all(|byte| *byte == 255));
        for mode in [
            TextRenderingMode::FillClip,
            TextRenderingMode::StrokeClip,
            TextRenderingMode::FillStrokeClip,
            TextRenderingMode::Clip,
        ] {
            let mut object = TextObject::new(GraphicState::default());
            object.state.text_state.text_rendering_mode = mode;
            assert!(matches!(
                renderer.show_text(object),
                Err(PdfError::UnsupportedFeature(_))
            ));
        }
    }

    #[test]
    fn invalid_dimensions_and_dpi_return_errors() {
        assert!(SkiaRender::new(f32::NAN, 72.0).is_err());
        assert!(SkiaRender::new(0.0, 72.0).is_err());
        let mut renderer = SkiaRender::new(72.0, 72.0).unwrap();
        assert!(renderer.start_page(-1.0, 100.0).is_err());
        assert!(renderer.start_page(f32::INFINITY, 100.0).is_err());
        assert!(matches!(
            renderer.start_page(100_000.0, 100_000.0),
            Err(PdfError::ResourceLimit(_))
        ));
        renderer.start_page(16.0, 16.0).unwrap();
        assert!(
            renderer
                .save(std::env::temp_dir().to_str().unwrap())
                .is_err()
        );
    }
}

#[cfg(test)]
mod font_program_regression_tests {
    use super::*;
    use pdf::{
        document::{Document, resource_manager::ResourceKind},
        io::stream_reader::StreamReader,
        objects::{PdfObject, pdf_stream::PdfStream},
        page::page_builder::PageBuilder,
        parser::syntax::SyntaxParser,
    };
    fn document(embedded: bool) -> Document {
        document_with_mapping(embedded, false)
    }
    fn document_with_mapping(embedded: bool, mixed: bool) -> Document {
        document_custom(embedded, mixed, None)
    }
    fn document_custom(embedded: bool, mixed: bool, cmap: Option<&[u8]>) -> Document {
        let doc = Document::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(40., 40.))
            .unwrap();
        let parse = |text: &str| {
            SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(text.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap()
        };
        let mut descriptor = parse("<< /Flags 6 >>");
        if embedded {
            descriptor.insert(
                "FontFile2".into(),
                PdfObject::PdfStream(PdfStream::new(
                    Default::default(),
                    include_bytes!("../../pdf/tests/fixtures/synthetic-symbol.ttf").to_vec(),
                )),
            );
        }
        let mut font = parse(&format!(
            "<< /Type /Font /Subtype /TrueType /BaseFont /RzpdfSyntheticSymbol /FirstChar 0 /LastChar 32 /Widths [{}] >>",
            std::iter::repeat_n("1000", 33)
                .collect::<Vec<_>>()
                .join(" ")
        ));
        font.insert("FontDescriptor".into(), PdfObject::PdfDict(descriptor));
        font.insert("ToUnicode".into(), PdfObject::PdfStream(PdfStream::new(Default::default(), if mixed { b"1 begincodespacerange <00> <ff> endcodespacerange 2 beginbfchar <20> <65b0> <1f> <00d7> endbfchar".to_vec() } else { b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <20> <00d7> endbfchar".to_vec() })));
        if let Some(cmap) = cmap {
            font.insert(
                "ToUnicode".into(),
                PdfObject::PdfStream(PdfStream::new(Default::default(), cmap.to_vec())),
            );
        }
        let id = doc.try_insert_object(PdfObject::PdfDict(font)).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Font, id)
            .unwrap();
        let bytes = if mixed {
            "201f20"
        } else if embedded {
            "0020"
        } else {
            "2020"
        };
        doc.replace_page_contents(
            0,
            format!("BT /{name} 10 Tf 1 0 0 1 0 20 Tm <{bytes}> Tj ET").into_bytes(),
        )
        .unwrap();
        doc
    }
    #[test]
    fn language_routing_matches_preflight_and_independent_face_pixels() {
        use crate::{font_preflight::FontPreflight, font_provider::MemoryFontProvider};
        use pdf::{
            font::resolver::FontMetadata,
            objects::{object_id::ObjectId, pdf_string::PdfString},
        };
        let base = include_bytes!("../../pdf/tests/fixtures/synthetic-symbol.ttf");
        let mut variant = base.to_vec();
        let tables = u16::from_be_bytes([variant[4], variant[5]]) as usize;
        let record = (0..tables)
            .map(|i| 12 + i * 16)
            .find(|i| &variant[*i..*i + 4] == b"head")
            .unwrap();
        let offset =
            u32::from_be_bytes(variant[record + 8..record + 12].try_into().unwrap()) as usize;
        let len =
            u32::from_be_bytes(variant[record + 12..record + 16].try_into().unwrap()) as usize;
        variant[offset + 18..offset + 20].copy_from_slice(&2000u16.to_be_bytes());
        variant[offset + 8..offset + 12].fill(0);
        let sum = |b: &[u8]| {
            b.chunks(4).fold(0u32, |s, p| {
                let mut x = [0; 4];
                x[..p.len()].copy_from_slice(p);
                s.wrapping_add(u32::from_be_bytes(x))
            })
        };
        let checksum = sum(&variant[offset..offset + len]);
        variant[record + 4..record + 8].copy_from_slice(&checksum.to_be_bytes());
        let adjustment = 0xB1B0_AFBAu32.wrapping_sub(sum(&variant));
        variant[offset + 8..offset + 12].copy_from_slice(&adjustment.to_be_bytes());
        let d = document(false);
        let root = ObjectId::new(2, 0);
        let mut catalog = d.resolve_object(root).unwrap().into_dict().unwrap();
        catalog
            .try_insert(
                "Lang".into(),
                PdfObject::PdfString(PdfString::from_text("ja-JP").unwrap()),
            )
            .unwrap();
        d.update_object(root, PdfObject::PdfDict(catalog)).unwrap();
        let resources = d.get_page(0).unwrap().resource().unwrap();
        let name = resources
            .get("Font")
            .unwrap()
            .as_dict()
            .unwrap()
            .into_iter()
            .next()
            .unwrap()
            .0;
        d.replace_page_contents(0,format!("BT /{name} 10 Tf 0 20 Td <20> Tj ET /Span << /Lang (zh-Hant-HK) >> BDC BT /{name} 10 Tf 14 20 Td <20> Tj ET EMC /Span << /Lang () >> BDC BT /{name} 10 Tf 26 20 Td <20> Tj ET EMC").into_bytes()).unwrap();
        let list = d.get_page(0).unwrap().display_list().unwrap();
        let metadata = |language: &str| FontMetadata {
            family: "RzpdfSyntheticSymbol".into(),
            aliases: vec![],
            weight: 400,
            italic: false,
            serif: false,
            languages: vec![language.into()],
        };
        let mut provider = MemoryFontProvider::default();
        let zh = provider.register(base, 0, metadata("zh-Hant")).unwrap();
        let ja = provider.register(&variant, 0, metadata("ja-JP")).unwrap();
        let mut preflight = FontPreflight::new(&provider, None, Limits::default()).unwrap();
        list.replay(&mut preflight).unwrap();
        let report = preflight.finish();
        assert_eq!(report.len(), 3);
        assert_eq!(report[0].selected[0].0, Some(ja));
        assert_eq!(report[1].selected[0].0, Some(zh));
        assert_eq!(report[2].selected[0].0, Some(zh));
        let mut actual = SkiaRender::new(72., 72.).unwrap();
        actual.set_font_provider(std::rc::Rc::new(provider));
        list.replay(&mut actual).unwrap();
        let mut expected = SkiaRender::new(72., 72.).unwrap();
        expected.start_page(40., 40.).unwrap();
        let mut i = 0;
        for object in list.objects() {
            if let pdf::page::page_object::PageObject::Text(text) = object {
                expected
                    .set_substitute_font(if i == 0 { &variant } else { base }, 0)
                    .unwrap();
                expected.show_text(text.clone()).unwrap();
                i += 1;
            }
        }
        expected.end_page().unwrap();
        let a = actual.to_rgba().unwrap().pixels;
        let b = expected.to_rgba().unwrap().pixels;
        assert!(a.chunks_exact(4).any(|p| p[0] < 200));
        if a != b {
            let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .parent()
                .unwrap()
                .join("output");
            std::fs::write(root.join("language-actual.rgba"), &a).unwrap();
            std::fs::write(root.join("language-expected.rgba"), &b).unwrap();
        }
        assert!(
            a == b,
            "language pixel mismatch: {} channels, dimensions {}x{}",
            a.iter().zip(&b).filter(|(a, b)| a != b).count(),
            list.page_width(),
            list.page_height()
        );
    }

    #[test]
    fn preflight_agrees_with_painting_coverage_and_reports_unmapped_or_shaping_without_guessing() {
        use crate::{font_preflight::FontPreflight, font_provider::MemoryFontProvider};
        use pdf::font::resolver::FontMetadata;
        let empty = MemoryFontProvider::default();
        let doc = document_with_mapping(false, true);
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let inspect = |p: &MemoryFontProvider| {
            let mut c = FontPreflight::new(p, None, Limits::default()).unwrap();
            list.replay(&mut c).unwrap();
            c.finish()
        };
        let missing = inspect(&empty);
        assert_eq!(missing.len(), 1);
        assert_eq!(
            missing[0].missing.iter().copied().collect::<Vec<_>>(),
            vec![0xd7, 0x65b0]
        );
        let mut provider = empty;
        let meta = |family: &str| FontMetadata {
            family: family.into(),
            aliases: vec![],
            weight: 400,
            italic: false,
            serif: false,
            languages: vec![],
        };
        provider
            .register(
                include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf"),
                0,
                meta("CJK"),
            )
            .unwrap();
        assert_eq!(
            inspect(&provider)[0]
                .missing
                .iter()
                .copied()
                .collect::<Vec<_>>(),
            vec![0xd7]
        );
        provider
            .register(
                include_bytes!("../../pdf/tests/fixtures/synthetic-symbol.ttf"),
                0,
                meta("Symbol"),
            )
            .unwrap();
        let ready = inspect(&provider);
        assert!(ready[0].missing.is_empty());
        assert_eq!(ready[0].selected.len(), 2);
        assert_eq!(ready[0].selected.iter().map(|v| v.1).sum::<usize>(), 3);
        let mut renderer = SkiaRender::new(72., 72.).unwrap();
        renderer.set_font_provider(std::rc::Rc::new(provider.clone()));
        list.replay(&mut renderer).unwrap();
        for (cmap,shaping) in [(b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <01> <0041> endbfchar".as_slice(),false),(b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <20> <00660069> endbfchar".as_slice(),true)] {
            let d=document_custom(false,false,Some(cmap));let list=d.get_page(0).unwrap().display_list().unwrap();
            let mut c=FontPreflight::new(&provider,None,Limits::default()).unwrap();list.replay(&mut c).unwrap();let report=c.finish();
            assert!(report[0].missing.is_empty());assert_eq!(report[0].needs_shaping,if shaping{2}else{0});assert_eq!(report[0].missing_unicode,if shaping{0}else{2});
            let mut renderer=SkiaRender::new(72.,72.).unwrap();renderer.set_font_provider(std::rc::Rc::new(provider.clone()));assert!(list.replay(&mut renderer).is_err());
        }
        let mut limited = FontPreflight::new(
            &provider,
            None,
            Limits {
                max_text_chars: 1,
                ..Limits::default()
            },
        )
        .unwrap();
        assert!(matches!(
            list.replay(&mut limited),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(
            FontPreflight::new(
                &provider,
                None,
                Limits {
                    max_operation_bytes: 1,
                    ..Limits::default()
                }
            )
            .is_err()
        );
    }

    #[test]
    fn text_matrix_scale_does_not_scale_user_space_stroke_width() {
        let doc = document(true);
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let pdf::page::page_object::PageObject::Text(text) = &list.objects()[0] else {
            panic!("text");
        };
        let raster = |scale: f32, size: f32| {
            let mut text = text.clone();
            text.state.text_state.font_size = size;
            text.state.text_state.text_rendering_mode = TextRenderingMode::Stroke;
            text.state.graph_state.line_width = 1.;
            text.matrix = Matrix::new(scale, 0., 0., scale, 0., 0.);
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            renderer.start_page(40., 40.).unwrap();
            renderer.show_text(text).unwrap();
            renderer.to_rgba().unwrap().pixels
        };
        let normal = raster(1., 10.);
        let scaled = raster(2., 5.);
        assert!(normal.chunks_exact(4).any(|p| p[0] < 200));
        assert_eq!(normal, scaled);
    }
    #[test]
    fn embedded_empty_notdef_is_valid_and_does_not_abort_following_glyph() {
        let doc = document(true);
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let mut renderer = SkiaRender::new(72., 72.).unwrap();
        list.replay(&mut renderer).unwrap();
        assert_eq!(renderer.typeface_cache.len(), 1);
        list.replay(&mut renderer).unwrap();
        assert_eq!(renderer.typeface_cache.len(), 1);
        let info = skia_safe::ImageInfo::new(
            (40, 40),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        let mut pixels = vec![0; 40 * 40 * 4];
        assert!(
            renderer
                .surface
                .read_pixels(&info, &mut pixels, 160, (0, 0))
        );
        assert!(pixels.as_chunks::<4>().0.iter().any(|p| p[0] < 200));
    }
    #[test]
    fn registered_fonts_mix_faces_with_pdf_positions_and_match_independent_draws() {
        use crate::font_provider::MemoryFontProvider;
        use pdf::font::resolver::{FontMetadata, FontProvider};
        let cjk = include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf");
        let symbol = include_bytes!("../../pdf/tests/fixtures/synthetic-symbol.ttf");
        let metadata = |family: &str| FontMetadata {
            family: family.into(),
            aliases: vec![],
            weight: 400,
            italic: false,
            serif: false,
            languages: vec![],
        };
        let mut provider = MemoryFontProvider::default();
        let one = provider.register(symbol, 0, metadata("SameName")).unwrap();
        let two = provider.register(cjk, 0, metadata("SameName")).unwrap();
        assert_ne!(one, two);
        assert_eq!(
            provider.register(symbol, 0, metadata("SameName")).unwrap(),
            one
        );
        assert!(
            provider
                .register(symbol, 0, metadata("Conflicting"))
                .is_err()
        );
        assert!(
            provider
                .register(b"invalid", 0, metadata("Broken"))
                .is_err()
        );
        assert!(
            provider
                .register(symbol, u32::MAX, metadata("HugeIndex"))
                .is_err()
        );
        assert_eq!(provider.len(), 2);
        assert_eq!(provider.byte_len(), symbol.len() + cjk.len());
        let provider = std::rc::Rc::new(provider);
        let list = document_with_mapping(false, true)
            .get_page(0)
            .unwrap()
            .display_list()
            .unwrap();
        let pdf::page::page_object::PageObject::Text(original) = &list.objects()[0] else {
            panic!("text")
        };
        for mode in [
            TextRenderingMode::Fill,
            TextRenderingMode::Stroke,
            TextRenderingMode::FillStroke,
        ] {
            let mut text = original.clone();
            text.state.text_state.text_rendering_mode = mode.clone();
            let mut actual = SkiaRender::new(72., 72.).unwrap();
            actual.set_font_provider(provider.clone());
            actual.start_page(40., 40.).unwrap();
            let f = text.state.text_state.font.as_ref().unwrap();
            assert_eq!(
                actual
                    .resolve_substitute(f, '新')
                    .unwrap()
                    .unwrap()
                    .identity,
                two
            );
            actual.show_text(text.clone()).unwrap();
            let mut expected = SkiaRender::new(72., 72.).unwrap();
            expected.start_page(40., 40.).unwrap();
            for (i, item) in text.items.iter().enumerate() {
                expected
                    .set_substitute_font(
                        if i == 1 {
                            symbol.as_slice()
                        } else {
                            cjk.as_slice()
                        },
                        0,
                    )
                    .unwrap();
                let mut single = text.clone();
                single.items = vec![item.clone()];
                expected.show_text(single).unwrap();
            }
            let a = actual.to_rgba().unwrap().pixels;
            let b = expected.to_rgba().unwrap().pixels;
            assert_eq!(a, b, "mixed-face mode {mode:?}");
            assert!(a.chunks_exact(4).any(|p| p[0] < 200));
        }
    }

    #[test]
    fn explicit_substitution_uses_unicode_cmap_and_preserves_pdf_positions() {
        let doc = document(false);
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let pdf::page::page_object::PageObject::Text(text) = &list.objects()[0] else {
            panic!("text")
        };
        assert_eq!(text.items[1].pos().x() - text.items[0].pos().x(), 10.);
        let mut renderer = SkiaRender::new(72., 72.).unwrap();
        assert!(list.replay(&mut renderer).is_err());
        renderer
            .set_substitute_font(
                include_bytes!("../../pdf/tests/fixtures/synthetic-symbol.ttf"),
                0,
            )
            .unwrap();
        list.replay(&mut renderer).unwrap();
        assert!(renderer.set_substitute_font(b"invalid", 0).is_err());
    }
}

#[cfg(test)]
mod image_reduction_tests {
    use super::*;
    use pdf::{
        io::stream_reader::StreamReader,
        objects::{PdfObject, pdf_stream::PdfStream},
        page::pdf_image::PdfImage,
        parser::syntax::SyntaxParser,
    };
    #[test]
    fn packed_stripes_match_pixel_center_nearest_sampling_across_dpi() {
        use pdf::{
            document::{Document, resource_manager::ResourceKind},
            page::page_builder::PageBuilder,
        };
        let doc = Document::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(64., 64.))
            .unwrap();
        let dict = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
            b"<< /Subtype /Image /Width 64 /Height 64 /BitsPerComponent 1 /ColorSpace /DeviceGray /Filter /RunLengthDecode /Interpolate false >>".to_vec()
        )).unwrap()).get_object().unwrap().into_dict().unwrap();
        // Independently constructed PDF literal runs: one white then one black
        // row. No reference rasterizer or rzpdf decoder generates the oracle.
        let mut encoded = Vec::new();
        for row in 0..64 {
            encoded.push(7);
            encoded.extend_from_slice(&[if row % 2 == 0 { 255 } else { 0 }; 8]);
        }
        encoded.push(128);
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, encoded)))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Image, id)
            .unwrap();
        doc.replace_page_contents(0, format!("q 64 0 0 64 0 0 cm /{name} Do Q").into_bytes())
            .unwrap();
        for dpi in [36., 48., 72., 96., 144.] {
            let mut renderer = SkiaRender::new(dpi, dpi).unwrap();
            doc.get_page(0).unwrap().display(&mut renderer).unwrap();
            let image = renderer.to_rgba().unwrap();
            for (index, pixel) in image.pixels.chunks_exact(4).enumerate() {
                let row = index / image.width as usize;
                let source = (row as f64 + 0.5) * 72. / f64::from(dpi);
                if source == source.floor() {
                    // At an exact source-pixel boundary both adjacent centers
                    // are equally near. Either source color is valid, not gray.
                    assert!(pixel == [0, 0, 0, 255] || pixel == [255, 255, 255, 255]);
                } else {
                    let source_row = source.floor() as usize;
                    let expected = if source_row % 2 == 0 { 255 } else { 0 };
                    assert_eq!(
                        pixel,
                        [expected, expected, expected, 255],
                        "dpi={dpi}, pixel={index}"
                    );
                }
            }
        }
    }

    #[test]
    fn image_interpolation_is_opt_in_and_nearest_preserves_source_colors() {
        let context = pdf::pdf_context::PDFContext::try_new(
            pdf::parser::parser::PdfParser::new(SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap(),
            )),
            pdf::document::cross_ref_table::CrossRefTable::default(),
        )
        .unwrap();
        for interpolate in [false, true] {
            let dict=SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(format!("<< /Width 8 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Interpolate {interpolate} >>").into_bytes())).unwrap()).get_object().unwrap().into_dict().unwrap();
            let data = (0..8)
                .flat_map(|i| if i % 2 == 0 { [255, 0, 0] } else { [0, 0, 255] })
                .collect();
            let image = PdfImage::try_new(
                "checker".into(),
                &PdfObject::PdfStream(PdfStream::new(dict, data)),
                &context,
            )
            .unwrap();
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            renderer.start_page(1., 1.).unwrap();
            renderer
                .do_image(ImageObject::new(
                    Matrix::identity(),
                    image,
                    GraphicState::default(),
                ))
                .unwrap();
            renderer.end_page().unwrap();
            let pixels = renderer.to_rgba().unwrap().pixels;
            if interpolate {
                assert!((110..=145).contains(&pixels[0]) && (110..=145).contains(&pixels[2]));
            } else {
                assert!(pixels[..3] == [255, 0, 0] || pixels[..3] == [0, 0, 255]);
            }
        }
    }

    #[test]
    fn reducing_thin_scan_strokes_preserves_coverage_for_rgb_and_stencil_images() {
        let context = pdf::pdf_context::PDFContext::try_new(
            pdf::parser::parser::PdfParser::new(SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap(),
            )),
            pdf::document::cross_ref_table::CrossRefTable::default(),
        )
        .unwrap();
        for stencil in [false, true] {
            let header = if stencil {
                "<< /Width 64 /Height 64 /ImageMask true /BitsPerComponent 1 /Interpolate true >>"
            } else {
                "<< /Width 64 /Height 64 /ColorSpace /DeviceRGB /BitsPerComponent 8 /Interpolate true >>"
            };
            let dict = SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(header.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            let bytes = if stencil {
                vec![0x77; 512]
            } else {
                (0..4096)
                    .flat_map(|i| [if i % 4 == 0 { 0 } else { 255 }; 3])
                    .collect()
            };
            let image = PdfImage::try_new(
                "test".into(),
                &PdfObject::PdfStream(PdfStream::new(dict, bytes)),
                &context,
            )
            .unwrap();
            let mut renderer = SkiaRender::new(72., 72.).unwrap();
            renderer.start_page(4., 4.).unwrap();
            renderer
                .do_image(ImageObject::new(
                    Matrix::new(4., 0., 0., 4., 0., 0.),
                    image,
                    GraphicState::default(),
                ))
                .unwrap();
            let info = skia_safe::ImageInfo::new(
                (4, 4),
                skia_safe::ColorType::RGBA8888,
                skia_safe::AlphaType::Unpremul,
                None,
            );
            renderer.end_page().unwrap();
            let mut pixels = [0; 64];
            assert!(renderer.surface.read_pixels(&info, &mut pixels, 16, (0, 0)));
            for pixel in pixels.as_chunks::<4>().0 {
                assert!(
                    (175..=205).contains(&pixel[0]),
                    "stencil={stencil}, pixel={pixel:?}"
                );
                assert_eq!(pixel[3], 255);
            }
        }
    }
}

#[cfg(test)]
mod cid_cff_backend_tests {
    use super::*;
    use pdf::{
        document::{Document, resource_manager::ResourceKind},
        io::stream_reader::StreamReader,
        objects::{PdfObject, pdf_stream::PdfStream},
        page::page_builder::PageBuilder,
        parser::syntax::SyntaxParser,
    };
    #[test]
    fn standalone_sparse_cff_uses_cid_for_backend_and_gid_for_charset_inspection() {
        let doc = Document::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(40., 40.))
            .unwrap();
        let program = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                Default::default(),
                include_bytes!("../../pdf/tests/fixtures/synthetic-cid.cff").to_vec(),
            )))
            .unwrap();
        let source = format!(
            "<< /Type /Font /Subtype /Type0 /BaseFont /RzpdfSyntheticCID /Encoding /Identity-H /DescendantFonts [ << /Subtype /CIDFontType0 /BaseFont /RzpdfSyntheticCID /CIDToGIDMap /Identity /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor << /FontFile3 {} 0 R >> >> ] >>",
            program.obj_num()
        );
        let font = SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(source.into_bytes())).unwrap(),
        )
        .get_object()
        .unwrap();
        let id = doc.try_insert_object(font).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Font, id)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{name} 20 Tf 1 0 0 1 5 10 Tm <002a> Tj ET").into_bytes(),
        )
        .unwrap();
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let pdf::page::page_object::PageObject::Text(text) = &list.objects()[0] else {
            panic!("text")
        };
        let font = text.state.text_state.font.as_ref().unwrap();
        assert!(matches!(
            font.get_glyph(text.items[0].charcode()),
            Some(GlyphDesc::Gid(2))
        ));
        assert_eq!(font.raw_cff_cid(text.items[0].charcode()), Some(42));
        let mut renderer = SkiaRender::new(72., 72.).unwrap();
        list.replay(&mut renderer).unwrap();
        let info = skia_safe::ImageInfo::new(
            (40, 40),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Unpremul,
            None,
        );
        let mut pixels = vec![0; 40 * 40 * 4];
        assert!(
            renderer
                .surface
                .read_pixels(&info, &mut pixels, 160, (0, 0))
        );
        assert!(
            pixels
                .as_chunks::<4>()
                .0
                .iter()
                .filter(|p| p[0] < 128)
                .count()
                > 50
        );
    }
}

#[cfg(test)]
mod rgba_output_tests {
    use super::*;
    #[test]
    fn rgba_output_is_owned_straight_alpha_and_channel_ordered() {
        let mut r = SkiaRender::new(72., 72.).unwrap();
        assert!(r.to_rgba().is_err());
        r.start_page(2., 1.).unwrap();
        r.surface.canvas().clear(Color::from_argb(128, 255, 0, 0));
        let image = r.to_rgba().unwrap();
        assert_eq!((image.width, image.height), (2, 1));
        assert_eq!(image.pixels, [255, 0, 0, 128, 255, 0, 0, 128]);
        r.surface.canvas().clear(Color::BLUE);
        assert_eq!(image.pixels[0..4], [255, 0, 0, 128]);
        r.limits.max_render_bytes = 4;
        assert!(r.to_rgba().is_err());
    }
}
