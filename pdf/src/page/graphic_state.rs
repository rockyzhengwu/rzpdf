use crate::page::{
    clip_path::ClipPath, color_state::ColorState, general_state::GeneralState,
    graph_state::GraphState, text_state::TextState,
};

#[derive(Debug, Clone, Default, PartialEq, PartialOrd)]
pub enum FillType {
    #[default]
    NoFill,
    EvenOdd,
    Winding,
}

#[derive(Debug, Clone, Default)]
pub struct GraphicState {
    pub clip_path: ClipPath,
    pub color_state: ColorState,
    pub graph_state: GraphState,
    pub text_state: TextState,
    pub general_state: GeneralState,
}

impl GraphicState {
    pub fn mut_clippath(&mut self) -> &mut ClipPath {
        &mut self.clip_path
    }
}
