use crate::{
    error::{PdfError, PdfResult},
    geom::{point::Point, rectangle::Rectangle},
    objects::PdfObject,
};

#[derive(Debug, Clone)]
pub struct Matrix {
    a: f32,
    b: f32,
    c: f32,
    d: f32,
    e: f32,
    f: f32,
}

impl Matrix {
    pub fn new(a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> Self {
        Matrix { a, b, c, d, e, f }
    }
    pub fn identity() -> Matrix {
        Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    }

    pub fn mul(&self, right: &Matrix) -> Self {
        let a = self.a * right.a + self.b * right.c;
        let b = self.a * right.b + self.b * right.d;
        let c = self.c * right.a + self.d * right.c;
        let d = self.c * right.b + self.d * right.d;
        let e = self.e * right.a + self.f * right.c + right.e;
        let f = self.e * right.b + self.f * right.d + right.f;
        Matrix::new(a, b, c, d, e, f)
    }

    pub fn concat(&mut self, right: &Matrix) {
        self.a = self.a * right.a + self.b * right.c;
        self.b = self.a * right.b + self.b * right.d;
        self.c = self.c * right.a + self.d * right.c;
        self.d = self.c * right.b + self.d * right.d;
        self.e = self.e * right.a + self.f * right.c + right.e;
        self.f = self.e * right.b + self.f * right.d + right.f;
    }

    pub fn transform(&self, point: &Point) -> Point {
        let x = self.a * point.x() + self.c * point.y() + self.e;
        let y = self.b * point.x() + self.d * point.y() + self.f;
        Point::new(x, y)
    }

    pub fn transform_rect(&self, rect: &Rectangle) -> Rectangle {
        let lower_left = rect.lower_left();
        let uper_right = rect.uper_right();
        let new_lower_left = self.transform(&lower_left);
        let new_uper_right = self.transform(&uper_right);
        Rectangle::new(
            new_lower_left.x(),
            new_lower_left.y(),
            new_uper_right.x(),
            new_uper_right.y(),
        )
    }
    pub fn a(&self) -> f32 {
        self.a
    }
    pub fn b(&self) -> f32 {
        self.b
    }
    pub fn c(&self) -> f32 {
        self.c
    }
    pub fn d(&self) -> f32 {
        self.d
    }
    pub fn e(&self) -> f32 {
        self.e
    }
    pub fn f(&self) -> f32 {
        self.f
    }
}

impl Default for Matrix {
    fn default() -> Self {
        Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
    }
}

impl TryFrom<&PdfObject> for Matrix {
    type Error = PdfError;

    fn try_from(value: &PdfObject) -> PdfResult<Self> {
        match value {
            PdfObject::PdfArray(array) => {
                if array.len() != 6 {
                    Err(PdfError::MatrixFromPdfObjectError(format!(
                        "Pdfarray length is not 6"
                    )))
                } else {
                    let fvalues: PdfResult<Vec<f32>> = array
                        .into_iter()
                        .map(|v| {
                            v.as_f32().ok_or(PdfError::MatrixFromPdfObjectError(format!(
                                "PdfArray element is not number"
                            )))
                        })
                        .collect();
                    let fvalues = fvalues?;
                    let a = fvalues[0];
                    let b = fvalues[1];
                    let c = fvalues[2];
                    let d = fvalues[3];
                    let e = fvalues[4];
                    let f = fvalues[5];
                    Ok(Matrix::new(a, b, c, d, e, f))
                }
            }
            _ => Err(PdfError::MatrixFromPdfObjectError(
                "PdfObject is not PdfArray".to_string(),
            )),
        }
    }
}
