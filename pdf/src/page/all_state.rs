use crate::{
    error::PdfResult,
    geom::{matrix::Matrix, point::Point},
    objects::pdf_dict::PdfDict,
    page::{clip_path::ClipPath, graphic_state::GraphicState},
};

#[derive(Debug, Clone)]
pub struct AllState {
    pub(crate) graphic_state: GraphicState,
    pub(crate) text_matrix: Matrix,
    pub(crate) text_line_matrix: Matrix,
    pub(crate) ctm: Matrix,
    pub(crate) text_horz_scale: f32,
    pub(crate) text_leading: f32,
    pub(crate) text_rise: f32,
}
impl Default for AllState {
    fn default() -> Self {
        AllState {
            graphic_state: GraphicState::default(),
            text_matrix: Matrix::identity(),
            ctm: Matrix::identity(),
            text_line_matrix: Matrix::identity(),
            text_horz_scale: 1.0,
            text_leading: 0.0,
            text_rise: 0.0,
        }
    }
}

impl AllState {
    pub fn ctm(&self) -> &Matrix {
        &self.ctm
    }

    pub fn mut_graphic_state(&mut self) -> &mut GraphicState {
        &mut self.graphic_state
    }

    pub fn process_ext_gs(&mut self, state: &PdfDict) -> PdfResult<()> {
        // TODO
        //println!("{:?}", state);
        //for (key, obj) in state.into_iter() {
        //    println!("extgs: {:?},{:?}", key, obj);
        //}
        Ok(())
    }
    pub fn set_text_matrix(&mut self, matrix: Matrix) {
        self.text_matrix = matrix;
    }
}
