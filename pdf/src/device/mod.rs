pub mod trace;
use crate::page::{image_object::ImageObject, path_object::PathObject, text_object::TextObject};

pub trait Device {
    fn start_page(&mut self, page_width: f32, page_height: f32);
    fn do_image(&mut self, imageobject: ImageObject);
    fn stroke_path(&mut self, pathobject: PathObject);
    fn fill_path(&mut self, pathobject: PathObject);
    fn fill_and_stroke_path(&mut self, pathobject: PathObject);
    fn end_page(&mut self);
    fn clip_path(&mut self, patobject: PathObject);
    fn show_text(&mut self, textobject: TextObject);
}
