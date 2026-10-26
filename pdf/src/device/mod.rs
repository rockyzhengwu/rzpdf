pub(crate) mod gray_group;
pub(crate) mod painting;
pub mod trace;
pub(crate) mod visibility;
use crate::error::PdfResult;
use crate::page::{image_object::ImageObject, path_object::PathObject, text_object::TextObject};

pub trait Device {
    fn begin_group(&mut self, _group: crate::page::transparency::GroupPaint) -> PdfResult<()> {
        Err(crate::error::PdfError::UnsupportedFeature(
            "device transparency group".into(),
        ))
    }
    /// `commit=false` discards this group's paints after a child failure.
    fn end_group(&mut self, _commit: bool) -> PdfResult<()> {
        Err(crate::error::PdfError::UnsupportedFeature(
            "device transparency group".into(),
        ))
    }
    /// Backends must opt in; silently dropping a shading would corrupt display.
    fn do_shading(
        &mut self,
        _shading: crate::page::shading_object::ShadingObject,
    ) -> PdfResult<()> {
        Err(crate::error::PdfError::UnsupportedFeature(
            "device shading rendering".into(),
        ))
    }
    /// Optional read-only check before cloning a draw's path/graphics state.
    /// Payload-specific limits still require validation when the event arrives.
    fn preflight_draw(
        &self,
        _state: &crate::page::graphic_state::GraphicState,
        _path: Option<&crate::path::pdf_path::PdfPath>,
    ) -> PdfResult<()> {
        Ok(())
    }
    /// Borrow immutable scene data during replay. Existing devices retain the
    /// owned-event fallback; rendering backends can avoid copying edit metadata.
    fn do_image_ref(&mut self, object: &ImageObject) -> PdfResult<()> {
        self.do_image(object.clone())
    }
    fn show_text_ref(&mut self, object: &TextObject) -> PdfResult<()> {
        self.show_text(object.clone())
    }
    fn stroke_path_ref(&mut self, object: &PathObject) -> PdfResult<()> {
        self.stroke_path(object.clone())
    }
    fn fill_path_ref(&mut self, object: &PathObject) -> PdfResult<()> {
        self.fill_path(object.clone())
    }
    fn fill_and_stroke_path_ref(&mut self, object: &PathObject) -> PdfResult<()> {
        self.fill_and_stroke_path(object.clone())
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
