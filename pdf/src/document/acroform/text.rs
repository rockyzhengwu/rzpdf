use super::{
    graph::{FieldNode, invalid},
    *,
};
use crate::{
    document::annotations::TextAlignment,
    font::{
        WritingMode,
        pdf_font::{Font, FontProgramSource},
    },
    io::stream_reader::StreamReader,
    page::{content_builder::ContentBuilder, content_parser::ContentParser},
    parser::{options::ParseOptions, syntax::SyntaxParser},
};
use std::io::{Cursor, Write};

pub(super) struct TextContext {
    pub font: Font,
    pub resource: String,
    pub font_object: PdfObject,
    pub size: f32,
    pub alignment: u32,
    pub color: String,
    _lease: crate::budget::MemoryLease,
}

#[derive(Default)]
pub(crate) struct AppearanceWork {
    characters: usize,
    operations: usize,
    bytes: usize,
}

fn helvetica() -> PdfResult<PdfObject> {
    let mut font = PdfDict::default();
    font.try_insert("Type".into(), name("Font"))?;
    font.try_insert("Subtype".into(), name("Type1"))?;
    font.try_insert("BaseFont".into(), name("Helvetica"))?;
    font.try_insert("Encoding".into(), name("WinAnsiEncoding"))?;
    Ok(PdfObject::PdfDict(font))
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(super) fn install_form_style(
        &self,
        form: &mut PdfDict,
        field: &mut PdfDict,
        style: &FormTextStyle,
    ) -> PdfResult<()> {
        if !style.font_size.is_finite() || style.font_size <= 0. {
            return Err(invalid("field font size must be finite and positive"));
        }
        let object = match &style.font {
            Some(h) => reference(self.validate_handle(h)?),
            None => helvetica()?,
        };
        let mut resources = match form.get("DR") {
            Some(v) => self
                .ctx()
                .resolve(v)?
                .into_dict()
                .ok_or_else(|| invalid("form DR dictionary"))?,
            None => PdfDict::default(),
        };
        let mut fonts = match resources.get("Font") {
            Some(v) => self
                .ctx()
                .resolve(v)?
                .into_dict()
                .ok_or_else(|| invalid("form DR Font dictionary"))?,
            None => PdfDict::default(),
        };
        let resource = if fonts.get("Helv").is_none() && style.font.is_none() {
            "Helv".to_owned()
        } else if let Some((key, _)) = (&fonts)
            .into_iter()
            .filter(|(_, v)| *v == &object)
            .min_by_key(|(key, _)| key.as_str())
        {
            key.clone()
        } else {
            if (&fonts).into_iter().len() >= self.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("form font resources".into()));
            }
            (0..=(&fonts).into_iter().len())
                .map(|i| format!("RzFieldFont{i}"))
                .find(|key| fonts.get(key).is_none())
                .ok_or_else(|| invalid("form font name allocation"))?
        };
        fonts.try_insert(resource.clone(), object)?;
        resources.try_insert("Font".into(), PdfObject::PdfDict(fonts))?;
        form.try_insert("DR".into(), PdfObject::PdfDict(resources))?;
        let budget = self.ctx().limits().operation_budget();
        let selection = ContentBuilder::font_selection_with_budget(
            &resource,
            style.font_size,
            self.ctx().limits(),
            &budget,
        )?;
        let mut da = selection.as_slice().to_vec();
        da.extend_from_slice(b"0 g");
        field.try_insert("DA".into(), PdfObject::PdfString(PdfString::new(da, false)))?;
        field.try_insert(
            "Q".into(),
            number(match style.alignment {
                TextAlignment::Left => 0.,
                TextAlignment::Center => 1.,
                TextAlignment::Right => 2.,
            }),
        )
    }

    pub(super) fn form_text_context(
        &self,
        form: &PdfDict,
        node: &FieldNode,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<TextContext> {
        let da = node.effective.get("DA").or_else(|| form.get("DA"));
        let bytes = match da {
            Some(v) => self
                .ctx()
                .resolve(v)?
                .as_string()
                .ok_or_else(|| invalid("field DA byte string"))?
                .bytes()
                .to_vec(),
            None => b"/Helv 12 Tf 0 g".to_vec(),
        };
        if bytes.len() > self.ctx().limits().max_token_bytes {
            return Err(PdfError::ResourceLimit("field DA bytes".into()));
        }
        let syntax = SyntaxParser::with_options(
            StreamReader::try_new(Cursor::new(bytes))?,
            ParseOptions {
                limits: self.ctx().limits(),
                ..Default::default()
            },
        );
        let mut parser = ContentParser::new(syntax);
        let mut selection = None;
        let mut color = "0 g".to_owned();
        let mut count = 0;
        while let Some(op) = parser.read_operator()? {
            count += 1;
            if count > self.ctx().limits().max_content_operations {
                return Err(PdfError::ResourceLimit("field DA operators".into()));
            }
            match op.name() {
                "Tf" if op.num_operands() == 2 && selection.is_none() => {
                    let resource = op
                        .operand(0)?
                        .as_name()
                        .ok_or_else(|| invalid("DA font resource name"))?
                        .name()
                        .to_owned();
                    let size = op.get_as_f32(1)?;
                    if size <= 0. {
                        return Err(PdfError::UnsupportedFeature(
                            "zero/negative DA font size (automatic sizing)".into(),
                        ));
                    }
                    selection = Some((resource, size));
                }
                "g" | "rg" | "k" => {
                    let expected = match op.name() {
                        "g" => 1,
                        "rg" => 3,
                        _ => 4,
                    };
                    if op.num_operands() != expected {
                        return Err(invalid("DA color operand count"));
                    }
                    let mut values = Vec::new();
                    for i in 0..expected {
                        let v = op.get_as_f32(i)?;
                        if !(0. ..=1.).contains(&v) {
                            return Err(invalid("DA color component"));
                        }
                        values.push(v.to_string());
                    }
                    color = format!("{} {}", values.join(" "), op.name());
                }
                _ => {
                    return Err(PdfError::UnsupportedFeature(format!(
                        "field DA operator {}",
                        op.name()
                    )));
                }
            }
        }
        let (resource, size) = selection.ok_or_else(|| invalid("DA font selection missing"))?;
        let font_object = match form.get("DR") {
            Some(dr) => {
                let dr = self
                    .ctx()
                    .resolve(dr)?
                    .into_dict()
                    .ok_or_else(|| invalid("form DR dictionary"))?;
                let fonts = dr
                    .get("Font")
                    .map(|v| self.ctx().resolve(v))
                    .transpose()?
                    .and_then(PdfObject::into_dict)
                    .ok_or_else(|| invalid("form DR Font missing"))?;
                fonts
                    .get(&resource)
                    .cloned()
                    .ok_or_else(|| invalid("DA font missing from DR"))?
            }
            None if resource == "Helv" => helvetica()?,
            None => return Err(invalid("DA font resources missing")),
        };
        let dict = self
            .ctx()
            .resolve(&font_object)?
            .into_dict()
            .ok_or_else(|| invalid("field font dictionary"))?;
        let (font, lease) = Font::try_new_with_budget(&dict, self.ctx(), budget)?;
        if font.writting_mode() != WritingMode::Horizontal || matches!(&font, Font::Type3(_)) {
            return Err(PdfError::UnsupportedFeature(
                "field appearance requires a horizontal non-Type3 font".into(),
            ));
        }
        if !matches!(
            font.font_program_source(),
            FontProgramSource::Embedded | FontProgramSource::BundledBase14
        ) {
            return Err(PdfError::UnsupportedFeature(
                "field appearance requires an embedded or bundled font program".into(),
            ));
        }
        let alignment = if node.effective.get("Q").is_some() {
            self.form_flags(&node.effective, "Q")?
        } else {
            self.form_flags(form, "Q")?
        };
        if alignment > 2 {
            return Err(invalid("field Q must be 0, 1 or 2"));
        }
        Ok(TextContext {
            font,
            resource,
            font_object,
            size,
            alignment,
            color,
            _lease: lease,
        })
    }

    pub(super) fn field_text_appearance(
        &self,
        context: &TextContext,
        width: f32,
        height: f32,
        lines: &[(String, bool)],
        multiline: bool,
        budget: &crate::budget::OperationBudget,
        work: &mut AppearanceWork,
    ) -> PdfResult<PdfDict> {
        let (object, _payload_lease) = self.field_text_appearance_object(context, width, height, lines, multiline, budget, work)?;
        let id = self.try_insert_object(PdfObject::PdfStream(object))?;
        let mut ap = PdfDict::default();
        ap.try_insert("N".into(), reference(id))?;
        Ok(ap)
    }

    fn field_text_appearance_object(
        &self, context: &TextContext, width: f32, height: f32,
        lines: &[(String, bool)], multiline: bool,
        budget: &crate::budget::OperationBudget, work: &mut AppearanceWork,
    ) -> PdfResult<(crate::objects::pdf_stream::PdfStream, crate::budget::MemoryLease)> {
        let limits = self.ctx().limits();
        work.operations = work
            .operations
            .checked_add(lines.len().saturating_mul(8).saturating_add(24))
            .filter(|n| *n <= limits.max_content_operations)
            .ok_or_else(|| PdfError::ResourceLimit("field cumulative AP operations".into()))?;
        let size = context.size;
        let leading = size * 1.2;
        if !leading.is_finite() || size > height - 6. {
            return Err(invalid("field font does not fit widget"));
        }
        if lines.len() > self.ctx().limits().max_text_chars
            || lines.len().saturating_mul(8).saturating_add(24)
                > self.ctx().limits().max_content_operations
        {
            return Err(PdfError::ResourceLimit(
                "field appearance lines/operations".into(),
            ));
        }
        if multiline && (lines.len().saturating_sub(1) as f32 * leading + size > height - 6.) {
            return Err(invalid("field text exceeds widget height"));
        }
        let mut content = format!("q 1 1 1 rg 0 0 {width} {height} re f 0 G 1 w .5 .5 {} {} re S 2 2 {} {} re W n /Tx BMC\n", width - 1., height - 1., width - 4., height - 4.).into_bytes();
        let mut content_lease = budget.reserve(content.capacity())?;
        let mut chars_left = self.ctx().limits().max_text_chars;
        for (i, (line, selected)) in lines.iter().enumerate() {
            if line.chars().any(char::is_control) {
                return Err(invalid("unsupported field text control character"));
            }
            let codes = context
                .font
                .encode_text_with_budget(line, self.ctx().limits(), budget)?;
            let mut text_width = 0.;
            let chars = context
                .font
                .visit_chars_limited(codes.as_slice(), chars_left, |ch| {
                    match context.font.get_glyph(&ch) {
                        None | Some(crate::font::GlyphDesc::Gid(0)) => {
                            return Err(PdfError::FontError(
                                "field text maps to a missing glyph".into(),
                            ));
                        }
                        Some(crate::font::GlyphDesc::Name(name)) if name == ".notdef" => {
                            return Err(PdfError::FontError("field text maps to .notdef".into()));
                        }
                        _ => {}
                    }
                    text_width += ch.width() * size / 1000.;
                    Ok(())
                })?;
            chars_left = chars_left
                .checked_sub(chars)
                .ok_or_else(|| PdfError::ResourceLimit("field characters".into()))?;
            work.characters = work
                .characters
                .checked_add(chars)
                .filter(|n| *n <= limits.max_text_chars)
                .ok_or_else(|| PdfError::ResourceLimit("field cumulative AP characters".into()))?;
            if !text_width.is_finite() || text_width > width - 6. {
                return Err(invalid("field text exceeds widget width"));
            }
            let x = match context.alignment {
                0 => 3.,
                1 => (width - text_width) / 2.,
                _ => width - 3. - text_width,
            };
            // Leave a descent allowance; explicit lines, no wrapping/shaping.
            let y = if multiline {
                height - 3. - size - i as f32 * leading + size * 0.2
            } else {
                (height - size) / 2. + size * 0.2
            };
            let selection = ContentBuilder::font_selection_with_budget(
                &context.resource,
                size,
                self.ctx().limits(),
                budget,
            )?;
            let encoded = ContentBuilder::encoded_text_with_budget(
                codes.as_slice(),
                self.ctx().limits(),
                budget,
            )?;
            let row = if *selected {
                format!(
                    ".15 .3 .65 rg 2 {} {} {} re f 1 g",
                    y - size * 0.2,
                    width - 4.,
                    leading
                )
            } else {
                context.color.clone()
            };
            let prefix = format!("{row} BT\n");
            let position = format!("1 0 0 1 {x} {y} Tm\n");
            let extra = prefix.len()
                + selection.as_slice().len()
                + position.len()
                + encoded.as_slice().len()
                + 4;
            let total = content
                .len()
                .checked_add(extra)
                .filter(|n| {
                    *n <= self.ctx().limits().max_raw_stream_bytes
                        && *n <= self.ctx().limits().max_decoded_bytes
                })
                .ok_or_else(|| PdfError::ResourceLimit("field AP bytes".into()))?;
            content_lease.resize(total)?;
            content
                .try_reserve_exact(extra)
                .map_err(|_| PdfError::ResourceLimit("field AP allocation".into()))?;
            content_lease.resize(content.capacity())?;
            content.write_all(prefix.as_bytes())?;
            content.write_all(selection.as_slice())?;
            content.write_all(position.as_bytes())?;
            content.write_all(encoded.as_slice())?;
            content.write_all(b" ET\n")?;
        }
        let total = content
            .len()
            .checked_add(6)
            .filter(|n| {
                *n <= self.ctx().limits().max_raw_stream_bytes
                    && *n <= self.ctx().limits().max_decoded_bytes
            })
            .ok_or_else(|| PdfError::ResourceLimit("field AP final bytes".into()))?;
        content_lease.resize(total)?;
        content
            .try_reserve_exact(6)
            .map_err(|_| PdfError::ResourceLimit("field AP final allocation".into()))?;
        content.extend_from_slice(b"EMC Q\n");
        content_lease.resize(content.capacity())?;
        work.bytes = work
            .bytes
            .checked_add(content.len())
            .filter(|n| *n <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("field cumulative AP bytes".into()))?;
        let mut fonts = PdfDict::default();
        fonts.try_insert(context.resource.clone(), context.font_object.clone())?;
        let mut resources = PdfDict::default();
        resources.try_insert("Font".into(), PdfObject::PdfDict(fonts))?;
        let mut dict = PdfDict::default();
        dict.try_insert("Type".into(), name("XObject"))?;
        dict.try_insert("Subtype".into(), name("Form"))?;
        dict.try_insert("BBox".into(), crate::document::appearance::nums(&[0., 0., width, height])?)?;
        dict.try_insert("Matrix".into(), crate::document::appearance::nums(&[1., 0., 0., 1., 0., 0.])?)?;
        dict.try_insert("Resources".into(), PdfObject::PdfDict(resources))?;
        Ok((crate::objects::pdf_stream::PdfStream::from_decoded_data(dict, content)?, content_lease))
    }

    pub(crate) fn signature_text_appearance(
        &self, text: &str, style: &FormTextStyle, width: f32, height: f32,
        budget: &crate::budget::OperationBudget,
        work: &mut AppearanceWork,
    ) -> PdfResult<(crate::objects::pdf_stream::PdfStream, crate::budget::MemoryLease)> {
        let limits = self.ctx().limits();
        if text.len() > limits.max_decoded_bytes || text.chars().count() > limits.max_text_chars {
            return Err(PdfError::ResourceLimit("signature appearance text".into()));
        }
        let count = text.split('\n').count();
        if count.saturating_mul(8).saturating_add(24) > limits.max_content_operations {
            return Err(PdfError::ResourceLimit("signature appearance lines".into()));
        }
        let _lines_lease = budget.reserve(text.len().checked_add(count.saturating_mul(std::mem::size_of::<(String,bool)>()))
            .ok_or_else(|| PdfError::ResourceLimit("signature line construction".into()))?)?;
        let mut form = PdfDict::default();
        let mut dict = PdfDict::default();
        self.install_form_style(&mut form, &mut dict, style)?;
        let node = FieldNode { id: ObjectId::new(0, 0), effective: dict.clone(), dict,
            parent: None, children: Vec::new(), widgets: Vec::new(), partial: None, name: String::new() };
        let context = self.form_text_context(&form, &node, budget)?;
        let mut lines = Vec::new();
        lines.try_reserve_exact(count).map_err(|_| PdfError::ResourceLimit("signature line allocation".into()))?;
        for line in text.split('\n') { lines.push((line.to_owned(),false)); }
        self.field_text_appearance_object(&context, width, height, &lines, true, budget, work)
    }
}
