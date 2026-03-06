use crate::page::{image_object::ImageObject, path_object::PathObject, text_object::TextObject};

#[derive(Debug)]
pub enum PageObject {
    Image(ImageObject),
    Text(TextObject),
    Path(PathObject),
    Form,
    Shading,
}
