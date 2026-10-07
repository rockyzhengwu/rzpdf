use super::Device;
use crate::{
    error::PdfResult,
    page::{
        graphic_state::GraphicState, image_object::ImageObject, path_object::PathObject,
        text_object::TextObject, text_state::TextRenderingMode,
    },
    path::pdf_path::PdfPath,
};

/// Filters nonpainting colorants at device dispatch, not object collection.
pub(crate) struct PaintingDevice<'a>(pub &'a mut dyn Device);

impl Device for PaintingDevice<'_> {
    fn preflight_draw(&self, state: &GraphicState, path: Option<&PdfPath>) -> PdfResult<()> {
        self.0.preflight_draw(state, path)
    }
    fn start_page(&mut self, width: f32, height: f32) -> PdfResult<()> {
        self.0.start_page(width, height)
    }
    fn end_page(&mut self) -> PdfResult<()> {
        self.0.end_page()
    }
    fn clip_path(&mut self, path: PathObject) -> PdfResult<()> {
        self.0.clip_path(path)
    }
    fn do_image(&mut self, image: ImageObject) -> PdfResult<()> {
        let space = if image.image.is_mask() {
            &image.graphic_state.color_state.fill_color_space
        } else {
            image.image.colorspace()
        };
        if space.is_nonpainting() {
            Ok(())
        } else {
            self.0.do_image(image)
        }
    }
    fn fill_path(&mut self, path: PathObject) -> PdfResult<()> {
        if path
            .graphic_state()
            .color_state
            .fill_color_space
            .is_nonpainting()
        {
            Ok(())
        } else {
            self.0.fill_path(path)
        }
    }
    fn stroke_path(&mut self, path: PathObject) -> PdfResult<()> {
        if path
            .graphic_state()
            .color_state
            .stroke_color_space
            .is_nonpainting()
        {
            Ok(())
        } else {
            self.0.stroke_path(path)
        }
    }
    fn fill_and_stroke_path(&mut self, path: PathObject) -> PdfResult<()> {
        let state = &path.graphic_state().color_state;
        match (
            !state.fill_color_space.is_nonpainting(),
            !state.stroke_color_space.is_nonpainting(),
        ) {
            (true, true) => self.0.fill_and_stroke_path(path),
            (true, false) => self.0.fill_path(path),
            (false, true) => self.0.stroke_path(path),
            (false, false) => Ok(()),
        }
    }
    fn show_text(&mut self, mut text: TextObject) -> PdfResult<()> {
        use TextRenderingMode::*;
        let mode = &text.state.text_state.text_rendering_mode;
        let fill = matches!(mode, Fill | FillStroke | FillClip | FillStrokeClip)
            && !text.state.color_state.fill_color_space.is_nonpainting();
        let stroke = matches!(mode, Stroke | FillStroke | StrokeClip | FillStrokeClip)
            && !text.state.color_state.stroke_color_space.is_nonpainting();
        let clip = matches!(mode, FillClip | StrokeClip | FillStrokeClip | Clip);
        text.state.text_state.text_rendering_mode = match (fill, stroke, clip) {
            (true, false, false) => Fill,
            (false, true, false) => Stroke,
            (true, true, false) => FillStroke,
            (false, false, false) => INVisible,
            (true, false, true) => FillClip,
            (false, true, true) => StrokeClip,
            (true, true, true) => FillStrokeClip,
            (false, false, true) => Clip,
        };
        self.0.show_text(text)
    }
}
