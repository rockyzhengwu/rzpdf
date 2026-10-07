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
        *self = self.mul(right);
    }

    pub fn is_finite(&self) -> bool {
        [self.a, self.b, self.c, self.d, self.e, self.f]
            .iter()
            .all(|v| v.is_finite())
    }

    /// Invert a finite, nonsingular affine matrix with f64 intermediates.
    /// Rejects results that cannot be represented by this f32 matrix type.
    pub fn checked_inverse(&self) -> PdfResult<Self> {
        if !self.is_finite() {
            return Err(PdfError::MatrixFromPdfObjectError(
                "non-finite inverse matrix input".into(),
            ));
        }
        let [a, b, c, d, e, f] = [self.a, self.b, self.c, self.d, self.e, self.f].map(f64::from);
        let determinant = a * d - b * c;
        if determinant == 0. {
            return Err(PdfError::MatrixFromPdfObjectError(
                "singular inverse matrix input".into(),
            ));
        }
        let result = Self::new(
            (d / determinant) as f32,
            (-b / determinant) as f32,
            (-c / determinant) as f32,
            (a / determinant) as f32,
            ((c * f - d * e) / determinant) as f32,
            ((b * e - a * f) / determinant) as f32,
        );
        if !result.is_finite() {
            return Err(PdfError::MatrixFromPdfObjectError(
                "inverse matrix exceeds finite f32 range".into(),
            ));
        }
        Ok(result)
    }

    pub fn checked_mul(&self, right: &Matrix) -> PdfResult<Self> {
        let result = self.mul(right);
        if !self.is_finite() || !right.is_finite() || !result.is_finite() {
            return Err(PdfError::MatrixFromPdfObjectError(
                "non-finite matrix composition".into(),
            ));
        }
        Ok(result)
    }

    pub fn transform(&self, point: &Point) -> Point {
        let x = self.a * point.x() + self.c * point.y() + self.e;
        let y = self.b * point.x() + self.d * point.y() + self.f;
        Point::new(x, y)
    }

    pub fn checked_transform(&self, point: &Point) -> PdfResult<Point> {
        let result = self.transform(point);
        if !self.is_finite()
            || !point.x().is_finite()
            || !point.y().is_finite()
            || !result.x().is_finite()
            || !result.y().is_finite()
        {
            return Err(PdfError::MatrixFromPdfObjectError(
                "non-finite transformed point".into(),
            ));
        }
        Ok(result)
    }

    /// Axis-aligned bounds of all four transformed corners, not an oriented box.
    pub fn transform_rect(&self, rect: &Rectangle) -> Rectangle {
        let points = self
            .rectangle_corners(rect)
            .map(|point| self.transform(&point));
        Self::bounds_of_points(points)
    }

    /// Reject non-finite input or transformed corners before returning bounds.
    pub fn checked_transform_rect(&self, rect: &Rectangle) -> PdfResult<Rectangle> {
        let corners = self.rectangle_corners(rect);
        let points = [
            self.checked_transform(&corners[0])?,
            self.checked_transform(&corners[1])?,
            self.checked_transform(&corners[2])?,
            self.checked_transform(&corners[3])?,
        ];
        Ok(Self::bounds_of_points(points))
    }

    fn rectangle_corners(&self, rect: &Rectangle) -> [Point; 4] {
        [
            Point::new(rect.llx(), rect.lly()),
            Point::new(rect.urx(), rect.lly()),
            Point::new(rect.llx(), rect.ury()),
            Point::new(rect.urx(), rect.ury()),
        ]
    }

    fn bounds_of_points(points: [Point; 4]) -> Rectangle {
        let mut bounds = [points[0].x(), points[0].y(), points[0].x(), points[0].y()];
        for point in &points[1..] {
            bounds[0] = bounds[0].min(point.x());
            bounds[1] = bounds[1].min(point.y());
            bounds[2] = bounds[2].max(point.x());
            bounds[3] = bounds[3].max(point.y());
        }
        Rectangle::new(bounds[0], bounds[1], bounds[2], bounds[3])
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
                            v.as_f32().filter(|v| v.is_finite()).ok_or(
                                PdfError::MatrixFromPdfObjectError(format!(
                                    "PdfArray element is not number"
                                )),
                            )
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

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rectangle_bounds_include_all_rotated_sheared_and_reflected_corners() {
        let rect = Rectangle::new(0., 0., 2., 1.);
        for (matrix, expected) in [
            (Matrix::identity(), [0., 0., 2., 1.]),
            (Matrix::new(0., 1., -1., 0., 0., 0.), [-1., 0., 0., 2.]),
            (Matrix::new(1., 1., -1., 1., 0., 0.), [-1., 0., 2., 3.]),
            (Matrix::new(1., 0., -2., 1., 3., -4.), [1., -4., 5., -3.]),
            (Matrix::new(-1., 0., 0., -1., 0., 0.), [-2., -1., 0., 0.]),
        ] {
            for actual in [
                matrix.transform_rect(&rect),
                matrix.checked_transform_rect(&rect).unwrap(),
            ] {
                assert_eq!(
                    [actual.llx(), actual.lly(), actual.urx(), actual.ury()],
                    expected
                );
            }
        }
        assert!(
            Matrix::identity()
                .checked_transform_rect(&Rectangle::new(f32::NAN, 0., 1., 1.))
                .is_err()
        );
        assert!(
            Matrix::new(f32::MAX, 0., 0., 1., 0., 0.)
                .checked_transform_rect(&rect)
                .is_err()
        );
        assert!(
            Matrix::new(1., 0., 0., 1., 0., f32::INFINITY)
                .checked_transform_rect(&rect)
                .is_err()
        );
        let reversed = Rectangle::new(2., 1., 0., 0.);
        let actual = Matrix::identity()
            .checked_transform_rect(&reversed)
            .unwrap();
        assert_eq!(
            [actual.llx(), actual.lly(), actual.urx(), actual.ury()],
            [0., 0., 2., 1.]
        );
    }

    #[test]
    fn affine_inverse_round_trips_and_rejects_invalid_results() {
        for matrix in [
            Matrix::identity(),
            Matrix::new(2., 1., 0.5, 3., 40., -20.),
            Matrix::new(0., 1., -1., 0., 80., 50.),
            Matrix::new(-2., 0., 0., 4., 10., 20.),
        ] {
            let inverse = matrix.checked_inverse().unwrap();
            for point in [
                Point::new(0., 0.),
                Point::new(2., -3.),
                Point::new(100., 50.),
            ] {
                let restored = inverse
                    .checked_transform(&matrix.checked_transform(&point).unwrap())
                    .unwrap();
                assert!((restored.x() - point.x()).abs() < 0.0001);
                assert!((restored.y() - point.y()).abs() < 0.0001);
            }
        }
        for scale in [1e30f32, 1e-30] {
            let inverse = Matrix::new(scale, 0., 0., scale, 0., 0.)
                .checked_inverse()
                .unwrap();
            assert!((inverse.a() * scale - 1.).abs() < 0.00001);
            assert!((inverse.d() * scale - 1.).abs() < 0.00001);
        }
        for matrix in [
            Matrix::new(1., 2., 2., 4., 0., 0.),
            Matrix::new(f32::NAN, 0., 0., 1., 0., 0.),
            Matrix::new(1., 0., 0., 1., f32::INFINITY, 0.),
            Matrix::new(f32::from_bits(1), 0., 0., 1., 0., 0.),
        ] {
            assert!(matrix.checked_inverse().is_err());
        }
    }

    #[test]
    fn concatenation_uses_original_components_and_checks_overflow() {
        let mut left = Matrix::new(1.0, 2.0, 3.0, 4.0, 5.0, 6.0);
        let right = Matrix::new(7.0, 8.0, 9.0, 10.0, 11.0, 12.0);
        left.concat(&right);
        assert_eq!(
            [left.a(), left.b(), left.c(), left.d(), left.e(), left.f()],
            [25.0, 28.0, 57.0, 64.0, 100.0, 112.0]
        );
        assert!(left.checked_mul(&Matrix::identity()).is_ok());
        let huge = Matrix::new(f32::MAX, 0.0, 0.0, 1.0, 0.0, 0.0);
        assert!(huge.checked_mul(&huge).is_err());
        assert!(
            Matrix::new(f32::NAN, 0.0, 0.0, 1.0, 0.0, 0.0)
                .checked_mul(&right)
                .is_err()
        );
    }
}
