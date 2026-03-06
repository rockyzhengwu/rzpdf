use crate::{
    font::{CharCode, pdf_font::Font},
    geom::{matrix::Matrix, point::Point},
    page::graphic_state::GraphicState,
};

#[derive(Debug, Clone)]
pub struct CharItem {
    pos: Point,
    charcode: CharCode,
    unicode: Option<String>,
}

impl CharItem {
    pub fn new(pos: Point, charcode: CharCode, unicode: Option<String>) -> Self {
        Self {
            pos,
            charcode,
            unicode,
        }
    }
    pub fn pos(&self) -> &Point {
        &self.pos
    }
    pub fn charcode(&self) -> &CharCode {
        &self.charcode
    }

    pub fn unicode(&self) -> Option<&String> {
        self.unicode.as_ref()
    }
}

#[derive(Debug, Clone)]
pub struct TextObject {
    pub state: GraphicState,
    pub items: Vec<CharItem>,
    pub origin_x: f32,
    pub origin_y: f32,
    pub matrix: Matrix,
}

impl TextObject {
    pub fn new(state: GraphicState) -> Self {
        TextObject {
            state,
            items: Vec::new(),
            origin_x: 0.0,
            origin_y: 0.0,
            matrix: Matrix::identity(),
        }
    }
    pub fn add_item(&mut self, item: CharItem) {
        self.items.push(item)
    }
}
