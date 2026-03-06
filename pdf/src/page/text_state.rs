use crate::font::pdf_font::Font;

#[derive(Debug, Clone, Default, PartialEq, PartialOrd)]
pub enum TextRenderingMode {
    #[default]
    Fill,
    Stroke,
    FillStroke,
    INVisible,
    FillClip,
    StrokeClip,
    FillStrokeClip,
    Clip,
}

#[derive(Debug, Clone)]
pub struct TextState {
    pub text_rendering_mode: TextRenderingMode,
    pub font_size: f32,
    pub word_space: f32,
    pub char_space: f32,
    pub font: Option<Font>,
}

impl Default for TextState {
    fn default() -> Self {
        TextState {
            text_rendering_mode: TextRenderingMode::default(),
            font_size: 1.0,
            word_space: 0.0,
            char_space: 0.0,
            font: None,
        }
    }
}
impl TextState {
    pub fn set_text_rendering_mode(&mut self, mode: u8) {
        match mode {
            0 => self.text_rendering_mode = TextRenderingMode::Fill,
            1 => self.text_rendering_mode = TextRenderingMode::Stroke,
            2 => self.text_rendering_mode = TextRenderingMode::FillStroke,
            3 => self.text_rendering_mode = TextRenderingMode::INVisible,
            4 => self.text_rendering_mode = TextRenderingMode::FillClip,
            5 => self.text_rendering_mode = TextRenderingMode::StrokeClip,
            6 => self.text_rendering_mode = TextRenderingMode::FillStrokeClip,
            7 => self.text_rendering_mode = TextRenderingMode::Clip,
            _ => {}
        }
    }
}
