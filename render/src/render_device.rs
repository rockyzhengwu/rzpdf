use pdf::{
    device::Device,
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
    path::pdf_path::Segment,
};
use std::{fs::File, io::Write};

use skia_safe::{
    Color, EncodedImageFormat, FontMgr, Paint, PaintStyle, PathBuilder, PathEffect, PathFillType,
    RGB, Surface, paint::Cap, surfaces,
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
}

impl SkiaRender {
    pub fn new(xdpi: f32, ydpi: f32) -> Self {
        let surface = surfaces::raster_n32_premul((100, 100)).expect("create surface failed");
        let pathbuilder = PathBuilder::new();
        let paint = Paint::default();
        let sx = xdpi / 72.0;
        let sy = ydpi / 72.0;
        let matrix = Matrix::new(sx, 0.0, 0.0, sy, 0.0, 0.0);
        Self {
            surface,
            pathbuilder,
            paint,
            matrix,
            scale_x: sx,
            scale_y: sy,
            page_width: 0,
            page_height: 0,
        }
    }
}
impl SkiaRender {
    fn set_clip(&mut self, pathobject: &PathObject) {
        let clip_path = &pathobject.graphic_state().clip_path;
        if !clip_path.is_empty() {
            for clip_ele in clip_path.elements() {
                let matrix = &clip_ele.matrix;
                let ctm = matrix.mul(&self.matrix);
                let m44 =
                    self.pdf_to_skia_m44(ctm.a(), ctm.b(), ctm.c(), ctm.d(), ctm.e(), ctm.f());
                self.surface.canvas().set_matrix(&m44);
                let mut clip_builder = PathBuilder::new();
                for sub_path in clip_ele.path.subpaths() {
                    for seg in sub_path.segments() {
                        match seg {
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
    pub fn save(&mut self, path: &str) {
        let image = self.surface.image_snapshot();
        let mut context = self.surface.direct_context();
        let d = image
            .encode(context.as_mut(), EncodedImageFormat::PNG, None)
            .unwrap();
        let mut file = File::create(path).unwrap();
        let bytes = d.as_bytes();
        file.write_all(bytes).unwrap();
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
    fn stroke_path(&mut self, graphic_state: &GraphicState) {
        let rgb = graphic_state
            .color_state
            .stroke_color_space
            .rgb(&graphic_state.color_state.stroke_color_value)
            .unwrap();
        self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
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
    }
    fn fill_path(&mut self, graphic_state: &GraphicState) {
        let rgb = graphic_state
            .color_state
            .fill_color_space
            .rgb(&graphic_state.color_state.fill_color_value)
            .unwrap();
        self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
        self.paint.set_style(PaintStyle::Fill);
        self.surface
            .canvas()
            .draw_path(&self.pathbuilder.snapshot(), &self.paint);
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

    fn draw_path(&mut self, pathobject: PathObject, paintstyle: PaintStyle) {
        self.surface.canvas().save();
        self.pathbuilder.reset();
        self.paint.reset();
        let matrix = pathobject.matrix();
        let ctm = matrix.mul(&self.matrix);
        self.set_clip(&pathobject);

        let path_m44 = self.pdf_to_skia_m44(ctm.a(), ctm.b(), ctm.c(), ctm.d(), ctm.e(), ctm.f());
        self.surface.canvas().set_matrix(&path_m44);
        let graphic_state = pathobject.graphic_state();
        self.set_path_paint_state(graphic_state);
        self.build_path(&pathobject);
        match paintstyle {
            PaintStyle::Fill => {
                self.fill_path(graphic_state);
            }
            PaintStyle::Stroke => {
                self.stroke_path(graphic_state);
            }
            PaintStyle::StrokeAndFill => {
                self.stroke_path(graphic_state);
                self.fill_path(graphic_state);
            }
        }
        self.surface.canvas().restore();
    }
}

impl Device for SkiaRender {
    fn start_page(&mut self, page_width: f32, page_height: f32) {
        let width = (self.matrix.a() * page_width).round() as i32;
        let height = (self.matrix.d() * page_height).round() as i32;
        self.page_width = width;
        self.page_height = height;
        self.surface = surfaces::raster_n32_premul((width, height)).expect("create surface failed");
        self.surface.canvas().clear(Color::WHITE);
    }

    fn show_text(&mut self, textobject: TextObject) {
        match textobject.state.text_state.text_rendering_mode {
            TextRenderingMode::Fill => {
                let colorspace = textobject.state.color_state.fill_color_space;
                let colorvalue = textobject.state.color_state.fill_color_value;
                let rgb = colorspace.rgb(&colorvalue).unwrap();
                self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
                self.paint.set_style(PaintStyle::Fill);
            }
            TextRenderingMode::Stroke => {
                let colorspace = textobject.state.color_state.stroke_color_space;
                let colorvalue = textobject.state.color_state.stroke_color_value;
                let rgb = colorspace.rgb(&colorvalue).unwrap();
                self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
                self.paint.set_style(PaintStyle::Stroke);
            }
            TextRenderingMode::FillStroke => {
                // TODO stroke different color
                let colorspace = textobject.state.color_state.fill_color_space;
                let colorvalue = textobject.state.color_state.fill_color_value;
                let rgb = colorspace.rgb(&colorvalue).unwrap();
                self.paint.set_color(RGB::from((rgb.r(), rgb.g(), rgb.b())));
                self.paint.set_style(PaintStyle::Fill);
                self.paint.set_style(PaintStyle::StrokeAndFill);
            }
            _ => {
                // TODO other rendering mode
                self.paint.set_style(PaintStyle::Fill);
            }
        }
        if let Some(font) = &textobject.state.text_state.font {
            let objmatrix = textobject.matrix.mul(&self.matrix);

            let fontmgr = FontMgr::new();
            let data = font.fontfile().unwrap();
            let typeface = fontmgr.new_from_data(data, 0).unwrap();
            let mut skia_font = skia_safe::Font::new(
                typeface,
                textobject.state.text_state.font_size * objmatrix.a(),
            );
            skia_font.set_edging(skia_safe::font::Edging::SubpixelAntiAlias);
            skia_font.set_subpixel(true);
            let mut glyphs = Vec::new();
            let mut positions = Vec::new();

            for char_item in textobject.items.iter() {
                let item_pos = self.matrix.transform(char_item.pos());
                let x = item_pos.x();
                let y = item_pos.y();
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
                    self.surface.canvas().draw_glyphs_at(
                        &glyphs,
                        positions.as_slice(),
                        (0, 0),
                        &skia_font,
                        &self.paint,
                    );
                }
            }
        }
    }

    fn do_image(&mut self, imageobject: ImageObject) {
        self.surface.canvas().save();
        let matrix = &imageobject.matrix;
        let ctm = matrix.mul(&self.matrix);
        let m44 = self.pdf_to_skia_m44(ctm.a(), ctm.b(), ctm.c(), ctm.d(), ctm.e(), ctm.f());
        self.surface.canvas().set_matrix(&m44);
        let rgba = imageobject.image.rgba_buffer();
        let info = skia_safe::ImageInfo::new(
            (
                imageobject.image.width() as i32,
                imageobject.image.height() as i32,
            ),
            skia_safe::ColorType::RGBA8888,
            skia_safe::AlphaType::Premul,
            None,
        );
        let row_bytes = info.min_row_bytes();
        let mut bitmap = skia_safe::Bitmap::new();
        unsafe {
            bitmap.install_pixels(&info, rgba.as_ptr() as *mut _, row_bytes);
        }
        let image = bitmap.as_image();
        let dest_rect = skia_safe::Rect::from_wh(1.0, 1.0);
        self.surface.canvas().draw_image_rect(
            &image,
            None,
            dest_rect,
            &skia_safe::paint::Paint::default(),
        );
        self.surface.canvas().restore();
    }

    fn stroke_path(&mut self, pathobject: PathObject) {
        self.draw_path(pathobject, PaintStyle::Stroke);
    }

    fn fill_and_stroke_path(&mut self, pathobject: PathObject) {
        self.draw_path(pathobject, PaintStyle::StrokeAndFill);
    }

    fn fill_path(&mut self, pathobject: PathObject) {
        self.draw_path(pathobject, PaintStyle::Fill);
    }

    fn end_page(&mut self) {}

    fn clip_path(&mut self, pathobject: PathObject) {}
}
