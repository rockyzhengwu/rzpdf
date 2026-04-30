use crate::{
    device::Device,
    error::PdfResult,
    page::{graphic_state::FillType, page_object::PageObject},
};

#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    page_width: f32,
    page_height: f32,
    objects: Vec<PageObject>,
}

impl DisplayList {
    pub fn new(page_width: f32, page_height: f32, objects: Vec<PageObject>) -> Self {
        Self {
            page_width,
            page_height,
            objects,
        }
    }

    pub fn page_width(&self) -> f32 {
        self.page_width
    }

    pub fn page_height(&self) -> f32 {
        self.page_height
    }

    pub fn objects(&self) -> &[PageObject] {
        &self.objects
    }

    pub fn replay(&self, device: &mut dyn Device) -> PdfResult<()> {
        device.start_page(self.page_width, self.page_height);
        for object in &self.objects {
            match object {
                PageObject::Image(image) => device.do_image(image.clone()),
                PageObject::Text(text) => device.show_text(text.clone()),
                PageObject::Path(path) => match (path.stroke(), path.fill_type()) {
                    (true, FillType::NoFill) => device.stroke_path(path.clone()),
                    (false, FillType::NoFill) => device.fill_path(path.clone()),
                    _ => device.fill_and_stroke_path(path.clone()),
                },
                PageObject::Form | PageObject::Shading => {}
            }
        }
        device.end_page();
        Ok(())
    }
}
