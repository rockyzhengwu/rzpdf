use crate::{
    geom::matrix::Matrix,
    page::{graphic_state::GraphicState, pdf_image::PdfImage},
};

/// Location in the concatenated decoded page Contents, not a persistent edit handle.
/// Valid only for the page revision that produced the drawing.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ImageDrawSource {
    pub operation_index: usize,
    /// Offset of the Do operator token, not its resource-name operand.
    pub operator_byte_offset: u64,
}

#[derive(Debug, Clone)]
pub struct ImageObject {
    pub matrix: Matrix,
    pub image: PdfImage,
    pub graphic_state: GraphicState,
    source: Option<ImageDrawSource>,
    nested_source: Option<super::source_provenance::NestedDrawSource>,
}

impl ImageObject {
    pub fn nested_source(&self) -> Option<&super::source_provenance::NestedDrawSource> {
        self.nested_source.as_ref()
    }
    pub(crate) fn with_nested_source(
        mut self,
        source: Option<super::source_provenance::NestedDrawSource>,
    ) -> Self {
        self.nested_source = source;
        self
    }
    /// Test the transformed unit square, including its edges. This is geometry,
    /// not visibility: clip paths, opacity, masks and later occlusion are ignored.
    /// Singular transforms have no two-dimensional hit area.
    pub fn geometry_contains_display_point(&self, x: f32, y: f32) -> crate::error::PdfResult<bool> {
        if !self.matrix.is_finite() || !x.is_finite() || !y.is_finite() {
            return Err(crate::error::PdfError::MatrixFromPdfObjectError(
                "non-finite image hit geometry".into(),
            ));
        }
        // Widen before multiplying: finite f32 scales can overflow a f32 determinant.
        let [a, b, c, d, e, f] = [
            self.matrix.a(),
            self.matrix.b(),
            self.matrix.c(),
            self.matrix.d(),
            self.matrix.e(),
            self.matrix.f(),
        ]
        .map(f64::from);
        let determinant = a * d - b * c;
        if determinant == 0.0 {
            return Ok(false);
        }
        let dx = f64::from(x) - e;
        let dy = f64::from(y) - f;
        let u = (d * dx - c * dy) / determinant;
        let v = (a * dy - b * dx) / determinant;
        Ok((0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v))
    }

    pub fn new(matrix: Matrix, image: PdfImage, graphic_state: GraphicState) -> Self {
        Self {
            matrix,
            image,
            graphic_state,
            source: None,
            nested_source: None,
        }
    }

    /// Manual construction has no source; interpreted top-level image draws do.
    pub fn source(&self) -> Option<ImageDrawSource> {
        self.source
    }

    pub(crate) fn with_source(mut self, source: Option<ImageDrawSource>) -> Self {
        self.source = source;
        self
    }
}
