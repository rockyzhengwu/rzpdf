#[derive(Default, Debug, Clone)]
pub enum BlendMode {
    #[default]
    Normal,
}

#[derive(Debug, Clone, Default)]
pub struct GeneralState {
    pub stroke_alpha: f32,
    pub fill_alpha: f32,
    pub flat_ness: f32,
}
