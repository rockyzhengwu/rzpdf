//! Gray-only transparency groups can use the RGB compositor without changing
//! blending arithmetic: every source and intermediate has equal RGB channels.
//! Colored paints require a real Gray working-space conversion and stay errors.
use super::Device;
use crate::{
    color::colorspace::ColorSpace,
    error::{PdfError, PdfResult},
    page::{
        graphic_state::GraphicState, image_object::ImageObject, path_object::PathObject,
        shading_object::ShadingObject, text_object::TextObject, transparency::GroupPaint,
    },
    path::pdf_path::PdfPath,
};
pub(crate) struct GrayGroupDevice<'a>(pub &'a mut dyn Device);
fn gray(space: &ColorSpace) -> PdfResult<()> {
    if matches!(space, ColorSpace::DeviceGray(_)) {
        Ok(())
    } else {
        Err(PdfError::UnsupportedFeature(
            "colored paint in DeviceGray transparency group".into(),
        ))
    }
}
impl Device for GrayGroupDevice<'_> {
    fn start_page(&mut self, w: f32, h: f32) -> PdfResult<()> {
        self.0.start_page(w, h)
    }
    fn end_page(&mut self) -> PdfResult<()> {
        self.0.end_page()
    }
    fn preflight_draw(&self, s: &GraphicState, p: Option<&PdfPath>) -> PdfResult<()> {
        self.0.preflight_draw(s, p)
    }
    fn begin_group(&mut self, g: GroupPaint) -> PdfResult<()> {
        self.0.begin_group(g)
    }
    fn end_group(&mut self, c: bool) -> PdfResult<()> {
        self.0.end_group(c)
    }
    fn clip_path(&mut self, p: PathObject) -> PdfResult<()> {
        self.0.clip_path(p)
    }
    fn fill_path(&mut self, p: PathObject) -> PdfResult<()> {
        gray(&p.graphic_state().color_state.fill_color_space)?;
        self.0.fill_path(p)
    }
    fn stroke_path(&mut self, p: PathObject) -> PdfResult<()> {
        gray(&p.graphic_state().color_state.stroke_color_space)?;
        self.0.stroke_path(p)
    }
    fn fill_and_stroke_path(&mut self, p: PathObject) -> PdfResult<()> {
        gray(&p.graphic_state().color_state.fill_color_space)?;
        gray(&p.graphic_state().color_state.stroke_color_space)?;
        self.0.fill_and_stroke_path(p)
    }
    fn do_image(&mut self, i: ImageObject) -> PdfResult<()> {
        if i.image.is_mask() {
            gray(&i.graphic_state.color_state.fill_color_space)?;
        } else {
            gray(i.image.colorspace())?;
        }
        self.0.do_image(i)
    }
    fn do_shading(&mut self, s: ShadingObject) -> PdfResult<()> {
        gray(s.shading.colorspace())?;
        self.0.do_shading(s)
    }
    fn show_text(&mut self, t: TextObject) -> PdfResult<()> {
        use crate::page::text_state::TextRenderingMode::*;
        if !t.is_type3_metadata() {
            let mode = &t.state.text_state.text_rendering_mode;
            if matches!(mode, Fill | FillClip | FillStroke | FillStrokeClip) {
                gray(&t.state.color_state.fill_color_space)?;
            }
            if matches!(mode, Stroke | StrokeClip | FillStroke | FillStrokeClip) {
                gray(&t.state.color_state.stroke_color_space)?;
            }
        }
        self.0.show_text(t)
    }
}
