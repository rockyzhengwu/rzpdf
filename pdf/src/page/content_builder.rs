#[derive(Debug, Clone, Default)]
pub struct ContentBuilder {
    bytes: Vec<u8>,
}

impl ContentBuilder {
    pub fn new() -> Self {
        Self { bytes: Vec::new() }
    }

    pub fn save_state(&mut self) -> &mut Self {
        self.push_operator_line("q");
        self
    }

    pub fn restore_state(&mut self) -> &mut Self {
        self.push_operator_line("Q");
        self
    }

    pub fn begin_text(&mut self) -> &mut Self {
        self.push_operator_line("BT");
        self
    }

    pub fn end_text(&mut self) -> &mut Self {
        self.push_operator_line("ET");
        self
    }

    pub fn set_font(&mut self, font_name: &str, font_size: f32) -> &mut Self {
        self.push_line(&format!("/{font_name} {} Tf", format_number(font_size)));
        self
    }

    pub fn move_text(&mut self, tx: f32, ty: f32) -> &mut Self {
        self.push_line(&format!(
            "{} {} Td",
            format_number(tx),
            format_number(ty)
        ));
        self
    }

    pub fn set_text_matrix(
        &mut self,
        a: f32,
        b: f32,
        c: f32,
        d: f32,
        e: f32,
        f: f32,
    ) -> &mut Self {
        self.push_line(&format!(
            "{} {} {} {} {} {} Tm",
            format_number(a),
            format_number(b),
            format_number(c),
            format_number(d),
            format_number(e),
            format_number(f)
        ));
        self
    }

    pub fn show_text(&mut self, text: &str) -> &mut Self {
        self.push_line(&format!("({}) Tj", escape_pdf_literal_string(text)));
        self
    }

    pub fn set_fill_rgb(&mut self, r: f32, g: f32, b: f32) -> &mut Self {
        self.push_line(&format!(
            "{} {} {} rg",
            format_number(r),
            format_number(g),
            format_number(b)
        ));
        self
    }

    pub fn set_stroke_rgb(&mut self, r: f32, g: f32, b: f32) -> &mut Self {
        self.push_line(&format!(
            "{} {} {} RG",
            format_number(r),
            format_number(g),
            format_number(b)
        ));
        self
    }

    pub fn rectangle(&mut self, x: f32, y: f32, width: f32, height: f32) -> &mut Self {
        self.push_line(&format!(
            "{} {} {} {} re",
            format_number(x),
            format_number(y),
            format_number(width),
            format_number(height)
        ));
        self
    }

    pub fn fill(&mut self) -> &mut Self {
        self.push_operator_line("f");
        self
    }

    pub fn stroke(&mut self) -> &mut Self {
        self.push_operator_line("S");
        self
    }

    pub fn raw_operator(&mut self, line: &str) -> &mut Self {
        self.push_line(line);
        self
    }

    pub fn build(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    fn push_operator_line(&mut self, operator: &str) {
        self.push_line(operator);
    }

    fn push_line(&mut self, line: &str) {
        self.bytes.extend_from_slice(line.as_bytes());
        self.bytes.push(b'\n');
    }
}

fn escape_pdf_literal_string(text: &str) -> String {
    let mut escaped = String::new();
    for ch in text.chars() {
        match ch {
            '(' | ')' | '\\' => {
                escaped.push('\\');
                escaped.push(ch);
            }
            '\n' => escaped.push_str("\\n"),
            '\r' => escaped.push_str("\\r"),
            '\t' => escaped.push_str("\\t"),
            '\u{0008}' => escaped.push_str("\\b"),
            '\u{000C}' => escaped.push_str("\\f"),
            _ => escaped.push(ch),
        }
    }
    escaped
}

fn format_number(value: f32) -> String {
    if value.fract() == 0.0 {
        return format!("{}", value as i64);
    }
    format!("{value}")
}
