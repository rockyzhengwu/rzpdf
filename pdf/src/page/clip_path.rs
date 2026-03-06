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
