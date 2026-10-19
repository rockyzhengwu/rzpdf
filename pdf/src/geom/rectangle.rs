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
    /// Resolve both the array and its four coordinates using document limits.
    pub(crate) fn try_from_with_context<R: std::io::Read + std::io::Seek>(
        object: &PdfObject,
        ctx: &crate::pdf_context::PDFContext<R>,
    ) -> PdfResult<Self> {
        let resolved = ctx.resolve_owned(object)?;
        let array = resolved.as_array().ok_or_else(|| {
            PdfError::RectangleFromPdfObjectError("PdfObject is not PdfArray".into())
        })?;
        if array.len() != 4 {
            return Err(PdfError::RectangleFromPdfObjectError(
                "Pdfarray length is not 4".into(),
            ));
        }
        let mut direct = crate::objects::pdf_array::PdfArray::default();
        for coordinate in array {
            direct.try_add_obj(ctx.resolve_owned(coordinate)?)?;
        }
        Self::try_from(&PdfObject::PdfArray(direct))
    }

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
        let urx = self.urx.min(other.urx);
        let ury = self.ury.min(other.ury);
        *self = Rectangle { llx, lly, urx, ury };
    }
}

impl TryFrom<&PdfObject> for Rectangle {
    type Error = PdfError;

    fn try_from(value: &PdfObject) -> PdfResult<Self> {
        match value {
            PdfObject::PdfArray(array) => {
                if array.len() != 4 {
                    Err(PdfError::RectangleFromPdfObjectError(
                        "Pdfarray length is not 4".to_string(),
                    ))
                } else {
                    let fvalues: PdfResult<Vec<f32>> = array
                        .into_iter()
                        .map(|v| {
                            v.as_f32().ok_or(PdfError::RectangleFromPdfObjectError(
                                "PdfArray element is not number".to_string(),
                            ))
                        })
                        .collect();
                    let fvalues = fvalues?;
                    let llx = fvalues[0];
                    let lly = fvalues[1];
                    let urx = fvalues[2];
                    let ury = fvalues[3];
                    if fvalues.iter().any(|v| !v.is_finite()) {
                        return Err(PdfError::RectangleFromPdfObjectError(
                            "Rectangle requires finite coordinates".into(),
                        ));
                    }
                    Ok(Rectangle::new(
                        llx.min(urx),
                        lly.min(ury),
                        llx.max(urx),
                        lly.max(ury),
                    ))
                }
            }
            _ => Err(PdfError::RectangleFromPdfObjectError(
                "PdfObject is not PdfArray".to_string(),
            )),
        }
    }
}
