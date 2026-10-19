use crate::{
    error::{PdfError, PdfResult},
    geom::rectangle::Rectangle,
    objects::pdf_dict::PdfDict,
};

/// Configuration for a newly inserted page. Coordinates are PDF user-space units.
pub struct PageBuilder {
    pub(crate) media_box: Rectangle,
    pub(crate) crop_box: Option<Rectangle>,
    pub(crate) rotation: i32,
    pub(crate) user_unit: Option<f32>,
    pub(crate) resources: PdfDict,
    pub(crate) content: Vec<u8>,
}

impl Default for PageBuilder {
    fn default() -> Self {
        Self {
            media_box: Rectangle::new_a4(),
            crop_box: None,
            rotation: 0,
            user_unit: None,
            resources: PdfDict::default(),
            content: Vec::new(),
        }
    }
}

impl PageBuilder {
    pub fn new() -> Self {
        Self::default()
    }
    pub fn size(mut self, width: f32, height: f32) -> Self {
        self.media_box = Rectangle::new(0.0, 0.0, width, height);
        self
    }
    pub fn media_box(mut self, rect: Rectangle) -> Self {
        self.media_box = rect;
        self
    }
    pub fn crop_box(mut self, rect: Rectangle) -> Self {
        self.crop_box = Some(rect);
        self
    }
    pub fn rotation(mut self, degrees: i32) -> Self {
        self.rotation = degrees;
        self
    }
    /// Declare the size of a user-space unit in multiples of 1/72 inch (PDF 1.6).
    /// Validated on insertion; does not rescale content or box coordinates.
    pub fn user_unit(mut self, unit: f32) -> Self {
        self.user_unit = Some(unit);
        self
    }
    pub fn resources(mut self, resources: PdfDict) -> Self {
        self.resources = resources;
        self
    }
    pub fn content(mut self, bytes: Vec<u8>) -> Self {
        self.content = bytes;
        self
    }

    pub(crate) fn validate(&self) -> PdfResult<()> {
        if self
            .user_unit
            .is_some_and(|unit| !unit.is_finite() || unit <= 0.)
        {
            return Err(PdfError::DocumentError(
                "UserUnit must be a finite positive number".into(),
            ));
        }
        fn valid(rect: &Rectangle) -> bool {
            [rect.llx(), rect.lly(), rect.urx(), rect.ury()]
                .into_iter()
                .all(f32::is_finite)
                && (rect.urx() - rect.llx()).is_finite()
                && rect.urx() > rect.llx()
                && (rect.ury() - rect.lly()).is_finite()
                && rect.ury() > rect.lly()
        }
        if !valid(&self.media_box) || self.crop_box.as_ref().is_some_and(|rect| !valid(rect)) {
            return Err(PdfError::DocumentError(
                "page boxes must have finite positive dimensions".into(),
            ));
        }
        if self.rotation % 90 != 0 {
            return Err(PdfError::DocumentError(
                "page rotation must be a multiple of 90".into(),
            ));
        }
        if let Some(crop) = &self.crop_box
            && (crop.llx().max(self.media_box.llx()) >= crop.urx().min(self.media_box.urx())
                || crop.lly().max(self.media_box.lly()) >= crop.ury().min(self.media_box.ury()))
        {
            return Err(PdfError::DocumentError(
                "CropBox must overlap MediaBox with positive area".into(),
            ));
        }
        Ok(())
    }
}
