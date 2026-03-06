use crate::{
    geom::matrix::Matrix,
    page::graphic_state::{FillType, GraphicState},
    path::pdf_path::PdfPath,
};

#[derive(Debug, Clone, Default)]
pub struct PathObject {
    path: PdfPath,
    matrix: Matrix,
    graphic_state: GraphicState,
    fill_type: FillType,
    stroke: bool,
}

impl PathObject {
    pub fn new(
        path: PdfPath,
        matrix: Matrix,
        graphic_state: GraphicState,
        fill_type: FillType,
        stroke: bool,
    ) -> Self {
        PathObject {
            matrix,
            path,
            graphic_state,
            fill_type,
            stroke,
        }
    }
    pub fn matrix(&self) -> &Matrix {
        &self.matrix
    }
    pub fn graphic_state(&self) -> &GraphicState {
        &self.graphic_state
    }
    pub fn fill_type(&self) -> &FillType {
        &self.fill_type
    }

    pub fn stroke(&self) -> bool {
        self.stroke
    }

    pub fn pdf_path(&self) -> &PdfPath {
        &self.path
    }
}
