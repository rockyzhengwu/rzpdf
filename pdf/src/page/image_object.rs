use crate::{
    geom::matrix::Matrix,
    page::{
        graphic_state::{self, GraphicState},
        pdf_image::PdfImage,
    },
};

#[derive(Debug)]
pub struct ImageObject {
    pub matrix: Matrix,
    pub image: PdfImage,
    pub graphic_state: GraphicState,
}

impl ImageObject {
    pub fn new(matrix: Matrix, image: PdfImage, graphic_state: GraphicState) -> Self {
        Self {
            matrix,
            image,
            graphic_state,
        }
    }
}
