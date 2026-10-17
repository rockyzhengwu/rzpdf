//! Paint suppression only. Interpretation, state changes, clipping and budgets
//! remain active; invisible groups do not enter the destination's group stack.
use super::Device;
use crate::{
    error::PdfResult,
    page::{
        graphic_state::GraphicState, image_object::ImageObject, path_object::PathObject,
        shading_object::ShadingObject, text_object::TextObject, transparency::GroupPaint,
    },
    path::pdf_path::PdfPath,
};
pub(crate) struct VisibilityDevice<'a> {
    pub device: &'a mut dyn Device,
    pub visible: bool,
}
impl Device for VisibilityDevice<'_> {
    fn start_page(&mut self, w: f32, h: f32) -> PdfResult<()> {
        self.device.start_page(w, h)
    }
    fn end_page(&mut self) -> PdfResult<()> {
        self.device.end_page()
    }
    fn preflight_draw(&self, s: &GraphicState, p: Option<&PdfPath>) -> PdfResult<()> {
        self.device.preflight_draw(s, p)
    }
    fn clip_path(&mut self, p: PathObject) -> PdfResult<()> {
        self.device.clip_path(p)
    }
    fn begin_group(&mut self, g: GroupPaint) -> PdfResult<()> {
        if self.visible {
            self.device.begin_group(g)
        } else {
            Ok(())
        }
    }
    fn end_group(&mut self, c: bool) -> PdfResult<()> {
        if self.visible {
            self.device.end_group(c)
        } else {
            Ok(())
        }
    }
    fn do_shading(&mut self, o: ShadingObject) -> PdfResult<()> {
        if self.visible {
            self.device.do_shading(o)
        } else {
            Ok(())
        }
    }
    fn do_image(&mut self, o: ImageObject) -> PdfResult<()> {
        if self.visible {
            self.device.do_image(o)
        } else {
            Ok(())
        }
    }
    fn show_text(&mut self, o: TextObject) -> PdfResult<()> {
        if self.visible {
            self.device.show_text(o)
        } else {
            Ok(())
        }
    }
    fn stroke_path(&mut self, o: PathObject) -> PdfResult<()> {
        if self.visible {
            self.device.stroke_path(o)
        } else {
            Ok(())
        }
    }
    fn fill_path(&mut self, o: PathObject) -> PdfResult<()> {
        if self.visible {
            self.device.fill_path(o)
        } else {
            Ok(())
        }
    }
    fn fill_and_stroke_path(&mut self, o: PathObject) -> PdfResult<()> {
        if self.visible {
            self.device.fill_and_stroke_path(o)
        } else {
            Ok(())
        }
    }
}
