#[derive(Debug, Clone, Default)]
pub enum LineCap {
    #[default]
    Butt,
    Round,
    Square,
}

impl LineCap {
    pub fn new_from_value(value: u8) -> Self {
        match value {
            0 => LineCap::Butt,
            1 => LineCap::Round,
            2 => LineCap::Square,
            _ => LineCap::Butt,
        }
    }
}

#[derive(Debug, Clone, Default)]
pub enum LineJoin {
    #[default]
    Miter,
    Round,
    Bevel,
}

impl LineJoin {
    pub fn new_from_value(value: u8) -> Self {
        match value {
            0 => LineJoin::Miter,
            1 => LineJoin::Round,
            2 => LineJoin::Bevel,
            _ => LineJoin::default(),
        }
    }
}

#[derive(Debug, Clone)]
pub struct GraphState {
    pub line_cap: LineCap,
    pub line_join: LineJoin,
    pub miter_limit: f32,
    pub line_width: f32,
    pub dash_phrase: f32,
    pub dash_array: Vec<f32>,
}

impl Default for GraphState {
    fn default() -> Self {
        GraphState {
            line_cap: LineCap::Butt,
            line_join: LineJoin::Miter,
            miter_limit: 10.0,
            line_width: 1.0,
            dash_phrase: 0.0,
            dash_array: Vec::new(),
        }
    }
}
