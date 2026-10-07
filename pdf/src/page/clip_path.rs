use crate::{geom::matrix::Matrix, page::graphic_state::FillType, path::pdf_path::PdfPath};

#[derive(Debug, Clone)]
pub struct ClipElement {
    pub path: PdfPath,
    pub fill_type: FillType,
    pub matrix: Matrix,
}

impl ClipElement {
    pub fn new(path: PdfPath, fill_type: FillType, matrix: Matrix) -> Self {
        Self {
            path,
            fill_type,
            matrix,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClipPath {
    pub elements: Vec<ClipElement>,
}

impl ClipPath {
    /// Append one intersection operand after validating the aggregate logical
    /// size. Failure leaves the existing clipping operands unchanged.
    pub fn try_add_element(
        &mut self,
        element: ClipElement,
        limits: &crate::parser::options::Limits,
    ) -> crate::error::PdfResult<()> {
        use crate::error::PdfError;
        let count = self
            .elements
            .len()
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("clip count overflow".into()))?;
        let mut segments = 0usize;
        let mut subpaths = 0usize;
        for entry in self.elements.iter().chain(std::iter::once(&element)) {
            for path in entry.path.subpaths() {
                subpaths = subpaths
                    .checked_add(1)
                    .ok_or_else(|| PdfError::ResourceLimit("clip subpaths overflow".into()))?;
                segments = segments
                    .checked_add(path.segments().len())
                    .ok_or_else(|| PdfError::ResourceLimit("clip segments overflow".into()))?;
            }
        }
        let bytes = count
            .checked_mul(std::mem::size_of::<ClipElement>())
            .and_then(|b| {
                segments
                    .checked_mul(std::mem::size_of::<crate::path::pdf_path::Segment>())
                    .and_then(|s| b.checked_add(s))
            })
            .and_then(|b| {
                subpaths
                    .checked_mul(std::mem::size_of::<crate::path::pdf_path::SubPath>())
                    .and_then(|s| b.checked_add(s))
            })
            .ok_or_else(|| PdfError::ResourceLimit("clip storage overflow".into()))?;
        if count > limits.max_objects
            || subpaths > limits.max_objects
            || segments > limits.max_path_segments
            || bytes > limits.max_decoded_bytes
        {
            return Err(PdfError::ResourceLimit(
                "clip operands/segments/storage".into(),
            ));
        }
        self.elements
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("clip operand allocation".into()))?;
        self.elements.push(element);
        Ok(())
    }
    pub fn add_path(&mut self, path: PdfPath, fill_type: FillType, matrix: Matrix) {
        let element = ClipElement {
            path,
            fill_type,
            matrix,
        };
        self.elements.push(element);
    }
    pub fn is_empty(&self) -> bool {
        self.elements.is_empty()
    }

    pub fn elements(&self) -> &[ClipElement] {
        self.elements.as_slice()
    }
}
