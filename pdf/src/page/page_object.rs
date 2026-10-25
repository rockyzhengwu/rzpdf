use crate::page::{image_object::ImageObject, path_object::PathObject, text_object::TextObject};

#[derive(Debug, Clone)]
pub enum PageObject {
    Image(ImageObject),
    Text(TextObject),
    Path(PathObject),
    Form,
    GroupBegin(super::transparency::GroupPaint),
    GroupEnd,
    Shading(super::shading_object::ShadingObject),
}
