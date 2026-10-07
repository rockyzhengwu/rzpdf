pub mod trace;
use crate::error::PdfResult;
use crate::page::{image_object::ImageObject, path_object::PathObject, text_object::TextObject};

pub trait Device {
    /// Optional read-only check before cloning a draw's path/graphics state.
    /// Payload-specific limits still require validation when the event arrives.
    fn preflight_draw(
        &self,
        _state: &crate::page::graphic_state::GraphicState,
        _path: Option<&crate::path::pdf_path::PdfPath>,
    ) -> PdfResult<()> {
        Ok(())
    }
    fn start_page(&mut self, page_width: f32, page_height: f32) -> PdfResult<()>;
    fn do_image(&mut self, imageobject: ImageObject) -> PdfResult<()>;
    fn stroke_path(&mut self, pathobject: PathObject) -> PdfResult<()>;
    fn fill_path(&mut self, pathobject: PathObject) -> PdfResult<()>;
    fn fill_and_stroke_path(&mut self, pathobject: PathObject) -> PdfResult<()>;
    fn end_page(&mut self) -> PdfResult<()>;
    fn clip_path(&mut self, patobject: PathObject) -> PdfResult<()>;
    fn show_text(&mut self, textobject: TextObject) -> PdfResult<()>;
}
