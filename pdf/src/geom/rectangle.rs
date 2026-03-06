use crate::{
    error::{PdfError, PdfResult},
    geom::point::Point,
    objects::PdfObject,
};

#[derive(Debug, Clone, Default)]
pub struct Rectangle {
    llx: f32,
    lly: f32,
    urx: f32,
    ury: f32,
}

impl Rectangle {
    pub fn new(llx: f32, lly: f32, urx: f32, ury: f32) -> Self {
        Self { llx, lly, urx, ury }
    }
    pub fn new_a4() -> Self {
        Self::new(0.0, 0.0, 612.0, 792.0)
    }
    pub fn llx(&self) -> f32 {
        self.llx
    }
    pub fn lly(&self) -> f32 {
        self.lly
    }
    pub fn urx(&self) -> f32 {
        self.urx
    }

    pub fn ury(&self) -> f32 {
        self.ury
    }
    pub fn lower_left(&self) -> Point {
        Point::new(self.llx, self.lly)
    }

    pub fn uper_right(&self) -> Point {
        Point::new(self.urx, self.ury)
    }

    pub fn width(&self) -> f32 {
        self.urx - self.llx
    }

    pub fn height(&self) -> f32 {
        self.ury - self.lly
    }

    pub fn intersect(&mut self, other: &Rectangle) {
        let llx = self.llx.max(other.llx);
        let lly = self.lly.max(other.lly);
        let urx = self.urx.min(other.ury);
        let ury = self.ury.min(other.ury);
        Rectangle { llx, lly, urx, ury };
    }
}

impl TryFrom<&PdfObject> for Rectangle {
    type Error = PdfError;

    fn try_from(value: &PdfObject) -> PdfResult<Self> {
        match value {
            PdfObject::PdfArray(array) => {
                if array.len() != 4 {
                    Err(PdfError::RectangleFromPdfObjectError(format!(
                        "Pdfarray length is not 4"
                    )))
                } else {
                    let fvalues: PdfResult<Vec<f32>> = array
                        .into_iter()
                        .map(|v| {
                            v.as_f32()
                                .ok_or(PdfError::RectangleFromPdfObjectError(format!(
                                    "PdfArray element is not number"
                                )))
                        })
                        .collect();
                    let fvalues = fvalues?;
                    let llx = fvalues[0];
                    let lly = fvalues[1];
                    let urx = fvalues[2];
                    let ury = fvalues[3];
                    Ok(Rectangle::new(llx, lly, urx, ury))
                }
            }
            _ => Err(PdfError::RectangleFromPdfObjectError(
                "PdfObject is not PdfArray".to_string(),
            )),
        }
    }
}
