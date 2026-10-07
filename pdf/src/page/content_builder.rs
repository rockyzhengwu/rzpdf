#[derive(Debug, Clone, Default)]
pub struct ContentBuilder {
    bytes: Vec<u8>,
}

impl ContentBuilder {
    /// Build one hexadecimal Tj operation with its capacity held in the pool.
    /// Borrowed input codes are accounted by their existing owner.
    pub fn encoded_text_with_budget(
        codes: &[u8],
        limits: crate::parser::options::Limits,
        budget: &crate::budget::OperationBudget,
    ) -> crate::error::PdfResult<crate::budget::BudgetedBytes> {
        let length = encoded_text_length(codes, limits)?;
        crate::filter::check_output_size(
            0,
            length,
            limits.max_raw_stream_bytes,
            "content builder bytes",
        )?;
        let mut lease = budget.reserve(length)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(length)
            .map_err(|_| crate::error::PdfError::ResourceLimit("encoded text allocation".into()))?;
        lease.resize(bytes.capacity())?;
        write_encoded_text(&mut bytes, codes);
        crate::budget::BudgetedBytes::from_reserved_vec(bytes, lease)
    }

    /// Draw an image in an isolated graphics state. Matrix maps its unit square
    /// into page user space; the caller must register the named image resource.
    pub fn draw_image(
        &mut self,
        name: &str,
        matrix: [f32; 6],
    ) -> crate::error::PdfResult<&mut Self> {
        self.draw_image_with_limits(name, matrix, crate::parser::options::Limits::default())
    }

    pub fn draw_image_with_limits(
        &mut self,
        name: &str,
        matrix: [f32; 6],
        limits: crate::parser::options::Limits,
    ) -> crate::error::PdfResult<&mut Self> {
        use crate::{error::PdfError, objects::pdf_name::PdfName};
        if !matrix.into_iter().all(f32::is_finite) {
            return Err(PdfError::ObjectError(
                "image transform must be finite".into(),
            ));
        }
        if name.is_empty() || name.as_bytes().contains(&0) {
            return Err(PdfError::ObjectError(
                "image resource name must be nonempty and contain no zero bytes".into(),
            ));
        }
        if name.len() > limits.max_token_bytes {
            return Err(PdfError::ResourceLimit("image resource name bytes".into()));
        }
        let mut block = Vec::new();
        let transform = format!(
            "q\n{} {} {} {} {} {} cm\n",
            matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]
        );
        block
            .try_reserve_exact(transform.len())
            .map_err(|_| PdfError::ResourceLimit("image content allocation".into()))?;
        block.extend_from_slice(transform.as_bytes());
        crate::writer::write_name(&mut block, &PdfName::new(name.to_owned()))?;
        if block.len() - transform.len() > limits.max_token_bytes {
            return Err(PdfError::ResourceLimit(
                "escaped image resource name bytes".into(),
            ));
        }
        block
            .try_reserve_exact(6)
            .map_err(|_| PdfError::ResourceLimit("image content allocation".into()))?;
        block.extend_from_slice(b" Do\nQ\n");
        crate::filter::check_output_size(
            self.bytes.len(),
            block.len(),
            limits.max_raw_stream_bytes,
            "content builder bytes",
        )?;
        self.bytes
            .try_reserve_exact(block.len())
            .map_err(|_| PdfError::ResourceLimit("content builder allocation".into()))?;
        self.bytes.extend_from_slice(&block);
        Ok(self)
    }

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

    /// Select a PDF text rendering mode (Tr). Backend clipping support may vary.
    pub fn set_text_rendering_mode(
        &mut self,
        mode: crate::page::text_state::TextRenderingMode,
    ) -> &mut Self {
        use crate::page::text_state::TextRenderingMode;
        let operand = match mode {
            TextRenderingMode::Fill => 0,
            TextRenderingMode::Stroke => 1,
            TextRenderingMode::FillStroke => 2,
            TextRenderingMode::INVisible => 3,
            TextRenderingMode::FillClip => 4,
            TextRenderingMode::StrokeClip => 5,
            TextRenderingMode::FillStrokeClip => 6,
            TextRenderingMode::Clip => 7,
        };
        self.push_line(&format!("{operand} Tr"));
        self
    }

    pub fn set_font(&mut self, font_name: &str, font_size: f32) -> &mut Self {
        self.push_line(&format!("/{font_name} {} Tf", format_number(font_size)));
        self
    }

    /// Select a registered font using an escaped resource name.
    pub fn try_set_font(&mut self, name: &str, size: f32) -> crate::error::PdfResult<&mut Self> {
        self.try_set_font_with_limits(name, size, crate::parser::options::Limits::default())
    }

    pub fn try_set_font_with_limits(
        &mut self,
        name: &str,
        size: f32,
        limits: crate::parser::options::Limits,
    ) -> crate::error::PdfResult<&mut Self> {
        use crate::{error::PdfError, objects::pdf_name::PdfName};
        if !size.is_finite() || name.is_empty() || name.as_bytes().contains(&0) {
            return Err(PdfError::ObjectError(
                "font selection requires a finite size and nonempty name without zero bytes".into(),
            ));
        }
        let token_length = name
            .as_bytes()
            .iter()
            .try_fold(1usize, |length, byte| {
                let escaped = *byte <= 32 || *byte >= 127 || b"()<>[]{}/%#".contains(byte);
                length.checked_add(if escaped { 3 } else { 1 })
            })
            .ok_or_else(|| PdfError::ResourceLimit("font resource name size overflow".into()))?;
        if token_length > limits.max_token_bytes {
            return Err(PdfError::ResourceLimit("font resource name bytes".into()));
        }
        let suffix = format!(" {} Tf\n", size);
        let length = token_length
            .checked_add(suffix.len())
            .ok_or_else(|| PdfError::ResourceLimit("font selection size overflow".into()))?;
        crate::filter::check_output_size(
            self.bytes.len(),
            length,
            limits.max_raw_stream_bytes,
            "content builder bytes",
        )?;
        let mut block = Vec::new();
        block
            .try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("font selection allocation".into()))?;
        crate::writer::write_name(&mut block, &PdfName::new(name.into()))?;
        block.extend_from_slice(suffix.as_bytes());
        self.bytes
            .try_reserve_exact(block.len())
            .map_err(|_| PdfError::ResourceLimit("content builder allocation".into()))?;
        self.bytes.extend_from_slice(&block);
        Ok(self)
    }

    pub fn move_text(&mut self, tx: f32, ty: f32) -> &mut Self {
        self.push_line(&format!("{} {} Td", format_number(tx), format_number(ty)));
        self
    }

    pub fn set_text_matrix(&mut self, a: f32, b: f32, c: f32, d: f32, e: f32, f: f32) -> &mut Self {
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

    /// Legacy raw UTF-8 byte output; this does not encode Unicode for a PDF font.
    /// Use `show_encoded_text` with font-specific character codes instead.
    pub fn show_text(&mut self, text: &str) -> &mut Self {
        self.push_line(&format!("({}) Tj", escape_pdf_literal_string(text)));
        self
    }

    /// Write already encoded font character codes, not Unicode or UTF-8 text.
    /// The caller must encode using the selected font; Type0 codes may be multibyte.
    pub fn show_encoded_text(&mut self, codes: &[u8]) -> crate::error::PdfResult<&mut Self> {
        self.show_encoded_text_with_limits(codes, crate::parser::options::Limits::default())
    }

    pub fn show_encoded_text_with_limits(
        &mut self,
        codes: &[u8],
        limits: crate::parser::options::Limits,
    ) -> crate::error::PdfResult<&mut Self> {
        use crate::error::PdfError;
        let length = encoded_text_length(codes, limits)?;
        crate::filter::check_output_size(
            self.bytes.len(),
            length,
            limits.max_raw_stream_bytes,
            "content builder bytes",
        )?;
        self.bytes
            .try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("encoded text allocation".into()))?;
        write_encoded_text(&mut self.bytes, codes);
        Ok(self)
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

    pub fn move_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.push_line(&format!("{} {} m", format_number(x), format_number(y)));
        self
    }

    pub fn line_to(&mut self, x: f32, y: f32) -> &mut Self {
        self.push_line(&format!("{} {} l", format_number(x), format_number(y)));
        self
    }

    pub fn curve_to(&mut self, control1: [f32; 2], control2: [f32; 2], end: [f32; 2]) -> &mut Self {
        self.push_line(&format!(
            "{} {} {} {} {} {} c",
            control1[0], control1[1], control2[0], control2[1], end[0], end[1]
        ));
        self
    }

    pub fn close_path(&mut self) -> &mut Self {
        self.push_operator_line("h");
        self
    }

    pub fn transform(&mut self, matrix: [f32; 6]) -> &mut Self {
        self.push_line(&format!(
            "{} {} {} {} {} {} cm",
            matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5]
        ));
        self
    }

    pub fn set_line_width(&mut self, width: f32) -> &mut Self {
        self.push_line(&format!("{} w", format_number(width)));
        self
    }

    pub fn fill_even_odd(&mut self) -> &mut Self {
        self.push_operator_line("f*");
        self
    }

    /// Mark the current path for nonzero-winding clipping; finish it with
    /// `end_path` or a painting operation before emitting other content.
    pub fn clip(&mut self) -> &mut Self {
        self.push_operator_line("W");
        self
    }

    pub fn clip_even_odd(&mut self) -> &mut Self {
        self.push_operator_line("W*");
        self
    }

    pub fn end_path(&mut self) -> &mut Self {
        self.push_operator_line("n");
        self
    }

    pub fn fill_and_stroke(&mut self) -> &mut Self {
        self.push_operator_line("B");
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

    /// Borrow generated bytes without validation or an additional allocation.
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes
    }

    pub fn build(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    /// Validate generated syntax and state pairing before returning content bytes.
    /// Resource existence and font encoding require document-level validation.
    pub fn try_build(&self) -> crate::error::PdfResult<Vec<u8>> {
        self.try_build_with_limits(crate::parser::options::Limits::default())
    }

    pub fn try_build_with_limits(
        &self,
        limits: crate::parser::options::Limits,
    ) -> crate::error::PdfResult<Vec<u8>> {
        use crate::{
            error::PdfError,
            io::stream_reader::StreamReader,
            page::{content_parser::ContentParser, opcode},
            parser::{
                options::ParseOptions,
                syntax::{ParseMode, SyntaxParser},
            },
        };
        crate::filter::check_output_size(
            0,
            self.bytes.len(),
            limits.max_raw_stream_bytes,
            "content builder bytes",
        )?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(self.bytes.len())
            .map_err(|_| PdfError::ResourceLimit("content validation allocation".into()))?;
        bytes.extend_from_slice(&self.bytes);
        let reader = StreamReader::try_new(std::io::Cursor::new(bytes))?;
        let mut parser = ContentParser::new(SyntaxParser::with_options(
            reader,
            ParseOptions {
                mode: ParseMode::Strict,
                limits,
            },
        ));
        let mut depth = 0usize;
        let mut text = false;
        let mut path_current_point = false;
        let mut pending_clip = false;
        let mut operations = 0usize;
        while let Some(operator) = parser.read_operator()? {
            if operations >= limits.max_content_operations {
                return Err(PdfError::ResourceLimit(
                    "content builder operation count".into(),
                ));
            }
            operations += 1;
            let name = operator.name();
            if pending_clip
                && !matches!(
                    name,
                    "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n"
                )
            {
                return Err(PdfError::ContentParseError(
                    "builder clipping must be followed by a path-ending operation".into(),
                ));
            }
            if !opcode::is_op(name.as_bytes()) {
                return Err(PdfError::UnsupportedFeature(format!(
                    "unknown builder operator at {:?}",
                    operator.byte_offset()
                )));
            }
            if opcode::operand_count(name).is_some_and(|count| count != operator.num_operands()) {
                return Err(PdfError::ContentParseError(
                    "builder operator operand count mismatch".into(),
                ));
            }
            validate_operand_types(&operator)?;
            validate_operand_ranges(&operator)?;
            match name {
                "W" | "W*" => pending_clip = true,
                "m" | "re" => path_current_point = true,
                "l" | "c" | "v" | "y" | "h" if !path_current_point => {
                    return Err(PdfError::ContentParseError(
                        "builder path operation requires a current point".into(),
                    ));
                }
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*" | "n" => {
                    path_current_point = false;
                    pending_clip = false;
                }
                "q" => {
                    if depth >= limits.max_depth {
                        return Err(PdfError::ResourceLimit(
                            "content builder state depth".into(),
                        ));
                    }
                    depth += 1;
                }
                "Q" => {
                    depth = depth
                        .checked_sub(1)
                        .ok_or_else(|| PdfError::ContentParseError("builder Q without q".into()))?;
                }
                "BT" if text => {
                    return Err(PdfError::ContentParseError("nested builder BT".into()));
                }
                "BT" => text = true,
                "ET" if !text => {
                    return Err(PdfError::ContentParseError("builder ET without BT".into()));
                }
                "ET" => text = false,
                "Tj" | "TJ" | "Td" | "TD" | "Tm" | "T*" | "'" | "\"" if !text => {
                    return Err(PdfError::ContentParseError(
                        "builder text operation outside BT/ET".into(),
                    ));
                }
                _ => {}
            }
            for index in 0..operator.num_operands() {
                if operator
                    .operand(index)?
                    .as_number()
                    .is_some_and(|number| !number.value().is_finite())
                {
                    return Err(PdfError::ContentParseError(
                        "builder number must be finite".into(),
                    ));
                }
            }
        }
        if depth != 0 || text || pending_clip {
            return Err(PdfError::ContentParseError(
                "unclosed builder graphics or text state".into(),
            ));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(self.bytes.len())
            .map_err(|_| PdfError::ResourceLimit("content builder output allocation".into()))?;
        output.extend_from_slice(&self.bytes);
        Ok(output)
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
    value.to_string()
}

fn validate_operand_types(
    operator: &crate::page::operator::Operator,
) -> crate::error::PdfResult<()> {
    use crate::{error::PdfError, objects::PdfObject};
    let numeric = |index| operator.get_as_f32(index).map(|_| ());
    match operator.name() {
        "w" | "J" | "j" | "M" | "i" | "cm" | "m" | "l" | "c" | "v" | "y" | "re" | "Tc" | "Tw"
        | "Tz" | "TL" | "Tr" | "Ts" | "Td" | "TD" | "Tm" | "d0" | "d1" | "g" | "G" | "rg"
        | "RG" | "k" | "K" | "sc" | "SC" => {
            for index in 0..operator.num_operands() {
                numeric(index)?;
            }
        }
        "ri" | "gs" | "cs" | "CS" | "sh" | "Do" | "MP" | "BMC" => {
            operator.get_as_name(0)?;
        }
        "Tf" => {
            operator.get_as_name(0)?;
            numeric(1)?;
        }
        "Tj" | "'" => {
            operator.get_as_string(0)?;
        }
        "\"" => {
            numeric(0)?;
            numeric(1)?;
            operator.get_as_string(2)?;
        }
        "TJ" => {
            for value in operator.get_as_array(0)? {
                if !matches!(value, PdfObject::PdfString(_))
                    && !value
                        .as_number()
                        .is_some_and(|number| number.value().is_finite())
                {
                    return Err(PdfError::ContentParseError(
                        "TJ entries must be strings or finite numbers".into(),
                    ));
                }
            }
        }
        "d" => {
            for value in operator.get_as_array(0)? {
                if !value
                    .as_number()
                    .is_some_and(|number| number.value().is_finite())
                {
                    return Err(PdfError::ContentParseError(
                        "dash entries must be finite numbers".into(),
                    ));
                }
            }
            numeric(1)?;
        }
        "DP" | "BDC" => {
            operator.get_as_name(0)?;
            if !matches!(
                operator.operand(1)?,
                PdfObject::PdfName(_) | PdfObject::PdfDict(_)
            ) {
                return Err(PdfError::ContentParseError(
                    "marked content properties must be a name or dictionary".into(),
                ));
            }
        }
        _ => {}
    }
    Ok(())
}

fn validate_operand_ranges(
    operator: &crate::page::operator::Operator,
) -> crate::error::PdfResult<()> {
    use crate::error::PdfError;
    let invalid = || PdfError::ContentParseError("builder operand outside permitted range".into());
    match operator.name() {
        "w" => {
            if operator.get_as_f32(0)? < 0.0 {
                return Err(invalid());
            }
        }
        "M" => {
            if operator.get_as_f32(0)? < 1.0 {
                return Err(invalid());
            }
        }
        "i" => {
            if !(0.0..=100.0).contains(&operator.get_as_f32(0)?) {
                return Err(invalid());
            }
        }
        "J" | "j" | "Tr" => {
            let value = operator
                .operand(0)?
                .as_number()
                .ok_or_else(invalid)?
                .as_u32_checked()?;
            let maximum = if operator.name() == "Tr" { 7 } else { 2 };
            if value > maximum {
                return Err(invalid());
            }
        }
        "d" => {
            let values = operator.get_as_array(0)?;
            let mut sum = 0.0f32;
            for value in values {
                let number = value.as_f32().ok_or_else(invalid)?;
                if number < 0.0 {
                    return Err(invalid());
                }
                sum += number;
                if !sum.is_finite() {
                    return Err(invalid());
                }
            }
            if (values.len() > 0 && sum == 0.0) || operator.get_as_f32(1)? < 0.0 {
                return Err(invalid());
            }
            if values.len() % 2 != 0 && !(sum * 2.0).is_finite() {
                return Err(invalid());
            }
        }
        _ => {}
    }
    Ok(())
}

fn encoded_text_length(
    codes: &[u8],
    limits: crate::parser::options::Limits,
) -> crate::error::PdfResult<usize> {
    use crate::error::PdfError;
    let token = codes
        .len()
        .checked_mul(2)
        .and_then(|size| size.checked_add(2))
        .ok_or_else(|| PdfError::ResourceLimit("encoded text token size overflow".into()))?;
    if token > limits.max_token_bytes {
        return Err(PdfError::ResourceLimit("encoded text token bytes".into()));
    }
    token
        .checked_add(4)
        .ok_or_else(|| PdfError::ResourceLimit("encoded text size overflow".into()))
}

fn write_encoded_text(bytes: &mut Vec<u8>, codes: &[u8]) {
    const HEX: &[u8; 16] = b"0123456789ABCDEF";
    bytes.push(b'<');
    for byte in codes {
        bytes.push(HEX[(byte >> 4) as usize]);
        bytes.push(HEX[(byte & 15) as usize]);
    }
    bytes.extend_from_slice(b"> Tj\n");
}

#[cfg(test)]
mod image_tests {
    use super::*;
    use crate::parser::options::Limits;

    #[test]
    fn encoded_text_budget_holds_capacity_and_preserves_legacy_bytes() {
        for codes in [b"".as_slice(), b"ABC".as_slice(), &[0, 255, 128]] {
            let length = codes.len() * 2 + 6;
            let budget = crate::budget::OperationBudget::new(length);
            let neighbor = budget.reserve(1).unwrap();
            assert!(
                ContentBuilder::encoded_text_with_budget(codes, Limits::default(), &budget)
                    .is_err()
            );
            assert_eq!(budget.live_bytes(), 1);
            drop(neighbor);
            let bytes = ContentBuilder::encoded_text_with_budget(codes, Limits::default(), &budget)
                .unwrap();
            let mut legacy = ContentBuilder::new();
            legacy.show_encoded_text(codes).unwrap();
            assert_eq!(bytes.as_slice(), legacy.as_bytes());
            assert_eq!(budget.live_bytes(), bytes.capacity());
            assert_eq!(bytes.capacity(), length);
            drop(bytes);
            assert_eq!(budget.live_bytes(), 0);
            for limits in [
                Limits {
                    max_token_bytes: codes.len() * 2 + 1,
                    ..Default::default()
                },
                Limits {
                    max_raw_stream_bytes: length - 1,
                    ..Default::default()
                },
            ] {
                assert!(ContentBuilder::encoded_text_with_budget(codes, limits, &budget).is_err());
                assert_eq!(budget.live_bytes(), 0);
            }
        }
    }

    #[test]
    fn typed_text_rendering_modes_emit_all_pdf_operands() {
        use crate::page::text_state::TextRenderingMode;
        let modes = [
            TextRenderingMode::Fill,
            TextRenderingMode::Stroke,
            TextRenderingMode::FillStroke,
            TextRenderingMode::INVisible,
            TextRenderingMode::FillClip,
            TextRenderingMode::StrokeClip,
            TextRenderingMode::FillStrokeClip,
            TextRenderingMode::Clip,
        ];
        for (operand, mode) in modes.into_iter().enumerate() {
            let mut builder = ContentBuilder::new();
            builder
                .begin_text()
                .set_text_rendering_mode(mode)
                .end_text();
            assert_eq!(
                builder.try_build().unwrap(),
                format!("BT\n{operand} Tr\nET\n").as_bytes()
            );
        }
    }

    #[test]
    fn font_selection_escapes_names_and_is_atomic_at_limits() {
        let mut builder = ContentBuilder::new();
        builder.begin_text();
        builder.try_set_font("F /#", 12.0).unwrap();
        builder.show_encoded_text(b"Hi").unwrap();
        builder.end_text();
        assert_eq!(
            String::from_utf8(builder.try_build().unwrap()).unwrap(),
            "BT\n/F#20#2F#23 12 Tf\n<4869> Tj\nET\n"
        );
        let before = builder.build();
        for (name, size) in [("", 12.0), ("zero\0", 12.0), ("F1", f32::INFINITY)] {
            assert!(builder.try_set_font(name, size).is_err());
            assert_eq!(builder.build(), before);
        }
        let limits = Limits {
            max_token_bytes: 4,
            max_raw_stream_bytes: 10,
            ..Default::default()
        };
        let mut exact = ContentBuilder::new();
        exact.try_set_font_with_limits("F1", 12.0, limits).unwrap();
        assert_eq!(exact.build(), b"/F1 12 Tf\n");
        let mut rejected = ContentBuilder::new();
        assert!(rejected
            .try_set_font_with_limits(
                "F1",
                12.0,
                Limits {
                    max_raw_stream_bytes: 9,
                    ..limits
                }
            )
            .is_err());
        assert!(rejected
            .try_set_font_with_limits("F ", 12.0, limits)
            .is_err());
        assert!(rejected.build().is_empty());
    }

    #[test]
    fn encoded_text_preserves_all_bytes_and_multibyte_codes() {
        use crate::{
            io::stream_reader::StreamReader, page::content_parser::ContentParser,
            parser::syntax::SyntaxParser,
        };
        let codes: Vec<u8> = (0..=255).collect();
        let mut builder = ContentBuilder::new();
        builder.begin_text();
        builder.show_encoded_text(&codes).unwrap();
        builder.end_text();
        let bytes = builder.try_build().unwrap();
        let reader = StreamReader::try_new(std::io::Cursor::new(bytes)).unwrap();
        let mut parser = ContentParser::new(SyntaxParser::new(reader));
        assert_eq!(parser.read_operator().unwrap().unwrap().name(), "BT");
        let text = parser.read_operator().unwrap().unwrap();
        assert_eq!(text.name(), "Tj");
        assert_eq!(text.get_as_string(0).unwrap().bytes(), codes);
    }

    #[test]
    fn encoded_text_limits_are_exact_and_failure_keeps_content() {
        let mut builder = ContentBuilder::new();
        let limits = Limits {
            max_token_bytes: 6,
            max_raw_stream_bytes: 10,
            ..Default::default()
        };
        builder
            .show_encoded_text_with_limits(&[0, 255], limits)
            .unwrap();
        assert_eq!(builder.build(), b"<00FF> Tj\n");
        let original = builder.build();
        assert!(builder.show_encoded_text_with_limits(&[], limits).is_err());
        assert_eq!(builder.build(), original);
        let mut other = ContentBuilder::new();
        assert!(other
            .show_encoded_text_with_limits(
                &[0, 255],
                Limits {
                    max_token_bytes: 5,
                    ..limits
                }
            )
            .is_err());
        assert!(other.build().is_empty());
    }

    #[test]
    fn clipping_is_committed_by_path_ending_and_rejects_pending_content() {
        let mut builder = ContentBuilder::new();
        builder
            .save_state()
            .rectangle(0.0, 0.0, 10.0, 20.0)
            .clip_even_odd()
            .end_path()
            .rectangle(1.0, 2.0, 3.0, 4.0)
            .fill()
            .restore_state();
        assert!(builder.try_build().is_ok());
        for input in ["0 0 m W", "0 0 m W q Q", "0 0 m W 1 1 l S", "0 0 m W W* n"] {
            let mut builder = ContentBuilder::new();
            builder.raw_operator(input);
            assert!(builder.try_build().is_err(), "accepted {input}");
        }
        for input in ["W n", "0 0 10 20 re W S", "0 0 10 20 re W* f*"] {
            let mut builder = ContentBuilder::new();
            builder.raw_operator(input);
            assert!(builder.try_build().is_ok(), "rejected {input}");
        }
    }

    #[test]
    fn path_construction_and_transform_have_valid_content_and_current_point() {
        let mut builder = ContentBuilder::new();
        builder
            .save_state()
            .transform([2.0, 0.0, 0.0, 2.0, 10.0, 20.0])
            .set_line_width(2.0)
            .move_to(0.0, 0.0)
            .line_to(10.0, 0.0)
            .curve_to([10.0, 5.0], [5.0, 10.0], [0.0, 10.0])
            .close_path()
            .fill_and_stroke()
            .restore_state();
        assert_eq!(
            String::from_utf8(builder.try_build().unwrap()).unwrap(),
            "q\n2 0 0 2 10 20 cm\n2 w\n0 0 m\n10 0 l\n10 5 5 10 0 10 c\nh\nB\nQ\n"
        );
        for input in ["1 2 l", "0 0 1 1 2 2 c", "h", "0 0 m S 1 1 l", "0 0 m n h"] {
            let mut builder = ContentBuilder::new();
            builder.raw_operator(input);
            assert!(builder.try_build().is_err(), "accepted {input}");
        }
    }

    #[test]
    fn finalization_checks_line_and_text_enums_and_dash_ranges() {
        for line in [
            "-1 w",
            "0 M",
            "-1 i",
            "101 i",
            "3 J",
            "1.5 j",
            "-1 Tr",
            "8 Tr",
            "[-1 2] 0 d",
            "[0 0] 0 d",
            "[] -1 d",
            "[300000000000000000000000000000000000000] 0 d",
        ] {
            let mut builder = ContentBuilder::new();
            builder.raw_operator(line);
            assert!(builder.try_build().is_err(), "accepted {line}");
        }
        for line in [
            "0 w 1 M 0 i 100 i",
            "0 J 2 J 1 j 7 Tr",
            "[] 0 d",
            "[0 2] 0 d",
            "[1] 3 d",
        ] {
            let mut builder = ContentBuilder::new();
            builder.raw_operator(line);
            assert!(builder.try_build().is_ok(), "rejected {line}");
        }
    }

    #[test]
    fn finalization_rejects_wrong_operand_types_without_changing_bytes() {
        for line in [
            "/bad 0 m",
            "(bad) Do",
            "BT 12 12 Tf ET",
            "BT /bad Tj ET",
            "BT [true] TJ ET",
            "[(bad)] 0 d",
            "[1] /bad d",
            "1 2 BDC",
            "true false 0 rg",
        ] {
            let mut builder = ContentBuilder::new();
            builder.raw_operator(line);
            let before = builder.build();
            assert!(builder.try_build().is_err(), "accepted {line}");
            assert_eq!(builder.build(), before);
        }
        for line in [
            "0 0 m 1 1 l S",
            "/Im1 Do",
            "BT /F1 12 Tf [(hi) -10 (there)] TJ ET",
            "[1 2] 0 d",
            "/Tag << /MCID 0 >> BDC EMC",
        ] {
            let mut builder = ContentBuilder::new();
            builder.raw_operator(line);
            assert!(builder.try_build().is_ok(), "rejected {line}");
        }
    }

    #[test]
    fn finalization_checks_pairing_operands_and_operation_limits() {
        for input in [
            "q",
            "Q",
            "BT",
            "ET",
            "BT BT ET ET",
            "(text) Tj",
            "1 2 m 3 l",
            "NaN 0 m",
            "1e20 0 m",
        ] {
            let mut builder = ContentBuilder::new();
            builder.raw_operator(input);
            assert!(builder.try_build().is_err(), "accepted {input}");
        }
        let mut builder = ContentBuilder::new();
        builder
            .save_state()
            .begin_text()
            .set_font("F1", 12.0)
            .move_text(10.0, 20.0)
            .show_text("hello")
            .end_text()
            .restore_state();
        assert_eq!(builder.try_build().unwrap(), builder.build());
        assert!(builder
            .try_build_with_limits(Limits {
                max_content_operations: 6,
                ..Default::default()
            })
            .is_err());
        assert!(builder
            .try_build_with_limits(Limits {
                max_content_operations: 7,
                max_depth: 1,
                ..Default::default()
            })
            .is_ok());
        let mut huge = ContentBuilder::new();
        huge.rectangle(1e20, 0.0, 1.0, 1.0).fill();
        assert!(huge.try_build().is_ok());
        assert!(String::from_utf8(huge.build())
            .unwrap()
            .contains("100000000000000000000"));
    }

    #[test]
    fn image_content_escapes_names_and_preserves_large_finite_values() {
        let mut builder = ContentBuilder::new();
        builder
            .draw_image("A /#", [1e20, 0.0, 0.0, 1.0, -2.0, 3.0])
            .unwrap();
        let output = String::from_utf8(builder.build()).unwrap();
        assert_eq!(
            output,
            "q\n100000000000000000000 0 0 1 -2 3 cm\n/A#20#2F#23 Do\nQ\n"
        );
    }

    #[test]
    fn image_content_failure_does_not_change_existing_content() {
        let matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];
        let mut builder = ContentBuilder::new();
        builder.raw_operator("q Q");
        let original = builder.build();
        for name in ["", "bad\0name"] {
            assert!(builder.draw_image(name, matrix).is_err());
            assert_eq!(builder.build(), original);
        }
        assert!(builder
            .draw_image("Im1", [f32::NAN, 0.0, 0.0, 1.0, 0.0, 0.0])
            .is_err());
        assert!(builder
            .draw_image_with_limits(
                "Im1",
                matrix,
                Limits {
                    max_raw_stream_bytes: original.len(),
                    ..Default::default()
                }
            )
            .is_err());
        assert_eq!(builder.build(), original);
        let mut exact = ContentBuilder::new();
        exact.draw_image("Im1", matrix).unwrap();
        let length = exact.build().len();
        let mut other = ContentBuilder::new();
        other
            .draw_image_with_limits(
                "Im1",
                matrix,
                Limits {
                    max_raw_stream_bytes: length,
                    ..Default::default()
                },
            )
            .unwrap();
        assert_eq!(other.build(), exact.build());
    }
}
