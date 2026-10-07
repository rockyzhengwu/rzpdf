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

use skia_safe::{
    paint::Cap, surfaces, Color, EncodedImageFormat, FontMgr, Paint, PaintStyle, PathBuilder,
    PathEffect, PathFillType, Surface, RGB,
};

pub struct SkiaRender {
    surface: Surface,
    pathbuilder: PathBuilder,
    paint: Paint,
    matrix: Matrix,
    scale_x: f32,
    scale_y: f32,
    page_width: i32,
    page_height: i32,
    limits: Limits,
}

impl SkiaRender {
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
            limits,
        })
    }
}
impl SkiaRender {
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

    fn set_clip(&mut self, clip_path: &pdf::page::clip_path::ClipPath) -> PdfResult<()> {
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
                self.surface.canvas().clip_path(
                    &clip_builder.snapshot(),
                    skia_safe::ClipOp::Intersect,
                    true,
                );
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

    fn set_paint_alpha(&mut self, alpha: f32) -> PdfResult<()> {
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(PdfError::PageOperatorError(
                "paint alpha must be finite and in [0,1]".into(),
            ));
        }
        self.paint.set_alpha_f(alpha);
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
    fn stroke_path(&mut self, graphic_state: &GraphicState) -> PdfResult<()> {
        let rgb = graphic_state
            .color_state
            .stroke_color_space
            .rgb(&graphic_state.color_state.stroke_color_value)?;
        self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
        self.set_paint_alpha(graphic_state.general_state.stroke_alpha)?;
        self.paint.set_blend_mode(Self::skia_blend_mode(
            graphic_state.general_state.blend_mode,
        ));
        self.paint
            .set_stroke_width(graphic_state.graph_state.line_width);
        let dash_phrase = graphic_state.graph_state.dash_phrase;
        let dash_array = &graphic_state.graph_state.dash_array;
        let stroke_miter = graphic_state.graph_state.miter_limit;
        self.paint.set_stroke_miter(stroke_miter);
        self.paint
            .set_path_effect(PathEffect::dash(dash_array, dash_phrase));
        self.paint.set_style(PaintStyle::Stroke);
        self.surface
            .canvas()
            .draw_path(&self.pathbuilder.snapshot(), &self.paint);
        Ok(())
    }
    fn fill_path(&mut self, graphic_state: &GraphicState) -> PdfResult<()> {
        let rgb = graphic_state
            .color_state
            .fill_color_space
            .rgb(&graphic_state.color_state.fill_color_value)?;
        self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
        self.set_paint_alpha(graphic_state.general_state.fill_alpha)?;
        self.paint.set_blend_mode(Self::skia_blend_mode(
            graphic_state.general_state.blend_mode,
        ));
        self.paint.set_style(PaintStyle::Fill);
        self.surface
            .canvas()
            .draw_path(&self.pathbuilder.snapshot(), &self.paint);
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

    fn draw_path(&mut self, pathobject: PathObject, paintstyle: PaintStyle) -> PdfResult<()> {
        let ctm = pathobject.matrix().checked_mul(&self.matrix)?;
        Self::validate_path_coordinates(pathobject.pdf_path(), &ctm)?;
        self.surface.canvas().save();
        self.pathbuilder.reset();
        self.paint.reset();
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
                    self.fill_path(graphic_state)?;
                }
                PaintStyle::Stroke => {
                    self.stroke_path(graphic_state)?;
                }
                PaintStyle::StrokeAndFill => {
                    self.fill_path(graphic_state)?;
                    self.stroke_path(graphic_state)?;
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
    fn start_page(&mut self, page_width: f32, page_height: f32) -> PdfResult<()> {
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
        let width = pixel_width as i32;
        let height = pixel_height as i32;
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
        self.page_width = width;
        self.page_height = height;
        self.surface.canvas().clear(Color::WHITE);
        Ok(())
    }

    fn show_text(&mut self, textobject: TextObject) -> PdfResult<()> {
        self.paint.reset();
        self.paint.set_blend_mode(Self::skia_blend_mode(
            textobject.state.general_state.blend_mode,
        ));
        let fill_stroke = matches!(
            textobject.state.text_state.text_rendering_mode,
            TextRenderingMode::FillStroke
        );
        match textobject.state.text_state.text_rendering_mode {
            TextRenderingMode::Fill => {
                let rgb = textobject
                    .state
                    .color_state
                    .fill_color_space
                    .rgb(&textobject.state.color_state.fill_color_value)?;
                self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
                self.set_paint_alpha(textobject.state.general_state.fill_alpha)?;
                self.paint.set_style(PaintStyle::Fill);
            }
            TextRenderingMode::Stroke => {
                let rgb = textobject
                    .state
                    .color_state
                    .stroke_color_space
                    .rgb(&textobject.state.color_state.stroke_color_value)?;
                self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
                self.set_paint_alpha(textobject.state.general_state.stroke_alpha)?;
                self.set_path_paint_state(&textobject.state);
                self.paint
                    .set_stroke_width(textobject.state.graph_state.line_width);
                self.paint
                    .set_stroke_miter(textobject.state.graph_state.miter_limit);
                self.paint.set_style(PaintStyle::Stroke);
            }
            TextRenderingMode::FillStroke => {
                let rgb = textobject
                    .state
                    .color_state
                    .fill_color_space
                    .rgb(&textobject.state.color_state.fill_color_value)?;
                self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
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
        if let Some(font) = &textobject.state.text_state.font {
            let objmatrix = textobject.matrix.checked_mul(&self.matrix)?;
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

            let fontmgr = FontMgr::new();
            let data = font
                .fontfile()
                .ok_or_else(|| PdfError::FontError("font has no renderable program".into()))?;
            let typeface = fontmgr
                .new_from_data(data, 0)
                .ok_or_else(|| PdfError::FontError("Skia rejected font program".into()))?;
            let mut skia_font = skia_safe::Font::new(typeface, size.abs());
            skia_font.set_edging(skia_safe::font::Edging::SubpixelAntiAlias);
            skia_font.set_subpixel(true);
            skia_font.set_scale_x(textobject.horizontal_scale);
            let mut glyphs = Vec::new();
            let mut positions = Vec::new();

            for char_item in textobject.items.iter() {
                let item_pos = self.matrix.checked_transform(char_item.pos())?;
                let dx = item_pos.x() as f64 - objmatrix.e() as f64;
                let dy = item_pos.y() as f64 - objmatrix.f() as f64;
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
                if let Some(glyph) = font.get_glyph(char_item.charcode()) {
                    match glyph {
                        GlyphDesc::Gid(gid) => {
                            glyphs.push(gid as u16);
                            positions.push(skia_safe::Point::new(x, y));
                        }
                        GlyphDesc::Name(name) => {
                            // TODO
                            println!("{:?}", name);
                        }
                    }
                }
            }
            let transform = self.pdf_to_skia_m44(a, b, c, d, objmatrix.e(), objmatrix.f());
            self.surface.canvas().save();
            if let Err(error) = self.set_clip(&textobject.state.clip_path) {
                self.surface.canvas().restore();
                return Err(error);
            }
            self.surface.canvas().set_matrix(&transform);
            self.surface.canvas().draw_glyphs_at(
                &glyphs,
                positions.as_slice(),
                (0, 0),
                &skia_font,
                &self.paint,
            );
            if fill_stroke {
                let rgb = textobject
                    .state
                    .color_state
                    .stroke_color_space
                    .rgb(&textobject.state.color_state.stroke_color_value);
                // Restore the canvas even if color conversion or alpha validation fails.
                let result = rgb.and_then(|rgb| {
                    self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
                    self.set_paint_alpha(textobject.state.general_state.stroke_alpha)?;
                    self.set_path_paint_state(&textobject.state);
                    self.paint
                        .set_stroke_width(textobject.state.graph_state.line_width);
                    self.paint
                        .set_stroke_miter(textobject.state.graph_state.miter_limit);
                    self.paint.set_style(PaintStyle::Stroke);
                    self.surface.canvas().draw_glyphs_at(
                        &glyphs,
                        positions.as_slice(),
                        (0, 0),
                        &skia_font,
                        &self.paint,
                    );
                    Ok(())
                });
                if let Err(error) = result {
                    self.surface.canvas().restore();
                    return Err(error);
                }
            }
            self.surface.canvas().restore();
        }
        Ok(())
    }

    fn do_image(&mut self, imageobject: ImageObject) -> PdfResult<()> {
        let matrix = &imageobject.matrix;
        let ctm = matrix.checked_mul(&self.matrix)?;
        let m44 = self.pdf_to_skia_m44(ctm.a(), ctm.b(), ctm.c(), ctm.d(), ctm.e(), ctm.f());
        let rgba = imageobject.image.rgba_buffer()?;
        let (width, height) = imageobject.image.pixel_dimensions();
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
        let alpha = imageobject.graphic_state.general_state.fill_alpha;
        if !alpha.is_finite() || !(0.0..=1.0).contains(&alpha) {
            return Err(PdfError::PageOperatorError(
                "image alpha must be finite and in [0,1]".into(),
            ));
        }
        let mut image_paint = skia_safe::paint::Paint::default();
        image_paint.set_alpha_f(alpha);
        image_paint.set_blend_mode(Self::skia_blend_mode(
            imageobject.graphic_state.general_state.blend_mode,
        ));
        self.surface.canvas().save();
        if let Err(error) = self.set_clip(&imageobject.graphic_state.clip_path) {
            self.surface.canvas().restore();
            return Err(error);
        }
        self.surface.canvas().set_matrix(&m44);
        let dest_rect = skia_safe::Rect::from_wh(1.0, 1.0);
        self.surface
            .canvas()
            .draw_image_rect(&image, None, dest_rect, &image_paint);
        self.surface.canvas().restore();
        Ok(())
    }

    fn stroke_path(&mut self, pathobject: PathObject) -> PdfResult<()> {
        self.draw_path(pathobject, PaintStyle::Stroke)?;
        Ok(())
    }

    fn fill_and_stroke_path(&mut self, pathobject: PathObject) -> PdfResult<()> {
        self.draw_path(pathobject, PaintStyle::StrokeAndFill)?;
        Ok(())
    }

    fn fill_path(&mut self, pathobject: PathObject) -> PdfResult<()> {
        self.draw_path(pathobject, PaintStyle::Fill)?;
        Ok(())
    }

    fn end_page(&mut self) -> PdfResult<()> {
        Ok(())
    }

    fn clip_path(&mut self, _pathobject: PathObject) -> PdfResult<()> {
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn nested_multicolor_image_transform_preserves_orientation_and_shared_calls() {
        use pdf::{
            document::{owned::Document, resource_manager::ResourceKind},
            geom::rectangle::Rectangle,
            io::stream_reader::StreamReader,
            objects::{pdf_stream::PdfStream, PdfObject},
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 40 * 4, (0, 0)));
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
            let image = insert(&doc,
                "<< /Subtype /Image /Width 2 /Height 2 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Interpolate false >>",
                &[255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0]);
            let leaf = insert(&doc, &format!("<< /Subtype /Form /BBox [0 0 8 8] /Resources << /XObject << /Tile {} 0 R >> >> >>", image.obj_num()), b"q 8 0 0 8 0 0 cm /Tile Do Q");
            let outer = insert(&doc, &format!("<< /Subtype /Form /BBox [0 0 40 40] /Resources << /XObject << /Child {} 0 R >> >> >>", leaf.obj_num()), b"q 1 0 0 1 5 5 cm /Child Do Q q 1 0 0 1 20 5 cm /Child Do Q");
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
            let replacement = insert(&doc,
                "<< /Subtype /Image /Width 2 /Height 2 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Interpolate false >>",
                &[0, 255, 255, 255, 0, 255, 0, 0, 0, 255, 255, 255]);
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
            objects::{pdf_stream::PdfStream, PdfObject},
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 40 * 4, (0, 0)));
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
            let replacement = insert(&doc,
                "<< /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB >>",
                &[0, 255, 255]);
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
            leaf.obj_num()),
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
            objects::{pdf_stream::PdfStream, PdfObject},
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 40 * 4, (0, 0)));
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
            let outer = insert(&doc,&format!("<< /Subtype /Form /BBox [0 0 40 40] /Resources << /XObject << /Child {} 0 R >> >> >>",leaf.obj_num()),b"q 1 0 0 1 5 5 cm /Child Do Q q 1 0 0 1 20 5 cm /Child Do Q");
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
            objects::{pdf_stream::PdfStream, PdfObject},
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 40 * 4, (0, 0)));
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
            objects::{pdf_stream::PdfStream, PdfObject},
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 60 * 4, (0, 0)));
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
                assert!(expected.chunks_exact(4).any(|pixel| pixel == color));
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
            assert!(removed
                .chunks_exact(4)
                .any(|pixel| pixel == [0, 255, 0, 255]));
            assert!(!removed
                .chunks_exact(4)
                .any(|pixel| pixel == [255, 0, 0, 255] || pixel == [0, 0, 255, 255]));
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
            objects::{pdf_stream::PdfStream, PdfObject},
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 60 * 4, (0, 0)));
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
                    actual.chunks_exact(4).any(|p| p == color),
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
            assert!(expected
                .chunks_exact(4)
                .any(|pixel| pixel == [0, 0, 0, 255]));
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
                assert!(replacement_pixels
                    .chunks_exact(4)
                    .any(|pixel| pixel == [0, 0, 0, 255]));
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
            objects::{pdf_stream::PdfStream, PdfObject},
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, w as usize * 4, (0, 0)));
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
                    actual.2.chunks_exact(4).any(|p| p == color),
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
        assert!(renderer
            .draw_path(PathObject::default(), PaintStyle::Fill)
            .is_ok());
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
        assert!(renderer
            .draw_path(PathObject::default(), PaintStyle::Fill)
            .is_ok());
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
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream, PdfObject},
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
        assert!(renderer
            .do_image(ImageObject::new(
                Matrix::new(2.0, 0.0, 0.0, 2.0, 1.0, 1.0),
                make_image(vec![1, 2]),
                GraphicState::default()
            ))
            .is_err());
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 30 * 4, (0, 0)));
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, width * 4, (0, 0)));
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
            assert!(originals[0].chunks_exact(4).any(|pixel| pixel == color));
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 160 * 4, (0, 0)));
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, width as usize * 4, (0, 0)));
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
                            actual.2.chunks_exact(4).any(|pixel| pixel == color),
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, width * 4, (0, 0)));
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
                assert!(original.chunks_exact(4).any(|pixel| pixel == color));
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, width * 4, (0, 0)));
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 30 * 4, (0, 0)));
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 40 * 4, (0, 0)));
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 30 * 4, (0, 0)));
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 30 * 4, (0, 0)));
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
        assert!(SkiaRender::with_limits(
            72.0,
            72.0,
            Limits {
                max_render_bytes: 3,
                ..Default::default()
            }
        )
        .is_err());
    }

    #[test]
    fn text_prefix_pixels_do_not_change_when_a_character_is_appended() {
        use pdf::{
            document::{resource_manager::StandardFont, PdfDocument},
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut pixels, 160 * 4, (0, 0)));
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
                pdf_array::PdfArray, pdf_dict::PdfDict, pdf_number::PdfNumber,
                pdf_reference::PdfReference, PdfObject,
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
                assert!(renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 160 * 4, (0, 0)));
                assert!(pixels
                    .chunks_exact(4)
                    .any(|pixel| pixel[..3] != [255, 255, 255]));
                pages.push(pixels);
            }
            assert_eq!(pages[0], pages[1]);
        }
    }

    #[test]
    fn alpha_constants_composite_paths_images_and_transparent_text() {
        use pdf::{
            document::{owned::Document, resource_manager::StandardFont},
            objects::{pdf_dict::PdfDict, pdf_number::PdfNumber, PdfObject},
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
            assert!(renderer
                .surface
                .read_pixels(&info, &mut output, 40 * 4, (0, 0)));
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
                assert!(pixels(&doc)
                    .chunks_exact(4)
                    .all(|pixel| pixel == [255, 255, 255, 255]));
            }
        }
    }

    #[test]
    fn standard_blend_modes_match_rgb_compositing_and_restore_normal() {
        use pdf::{
            document::owned::Document,
            objects::{pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber, PdfObject},
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
                    assert!(renderer
                        .surface
                        .read_pixels(&info, &mut pixels, 40 * 4, (0, 0)));
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
            objects::{pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber, PdfObject},
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
                assert!(renderer
                    .surface
                    .read_pixels(&info, &mut pixels, 40 * 4, (0, 0)));
                assert!(
                    pixels.chunks_exact(4).all(|pixel| pixel[0] <= 1),
                    "Multiply cannot introduce red over cyan"
                );
                assert!(
                    pixels
                        .chunks_exact(4)
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
                    .chunks_exact(4)
                    .any(|p| p[0] > 240 && p[1] < 15 && p[2] < 15)
            );
            assert!(
                expected
                    .chunks_exact(4)
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
    fn invisible_text_is_extractable_but_does_not_paint() {
        use pdf::{
            document::{resource_manager::StandardFont, PdfDocument},
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
        assert!(renderer
            .surface
            .read_pixels(&info, &mut pixels, 100 * 4, (0, 0)));
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
        assert!(renderer
            .save(std::env::temp_dir().to_str().unwrap())
            .is_err());
    }
}
