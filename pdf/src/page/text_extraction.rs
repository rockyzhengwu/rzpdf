use super::{PdfPage, page_object::PageObject};
use crate::{
    error::{PdfError, PdfResult},
    geom::{matrix::Matrix, point::Point},
};
use std::io::{Read, Seek};

/// An interpreted character in paint order, not inferred reading order.
/// Position is the advancing text origin in rotated/cropped page display
/// coordinates (top-left origin, y downward), not unrotated PDF user space.
/// Matrix is text-rise translation * initial text-matrix * CTM * page transform;
/// its translation is the run origin, not each advancing character origin.
#[derive(Debug, Clone)]
pub struct ExtractedCharacter {
    pub source: Option<super::text_object::TextDrawSource>,
    /// Form stream location, separate from the page's top-level source.
    pub nested_source: Option<super::source_provenance::NestedDrawSource>,
    pub array_item_index: Option<usize>,
    /// Within the decoded string operand (or TJ string item), not source bytes.
    pub decoded_code_range: Option<std::ops::Range<usize>>,
    pub object_index: usize,
    pub charcode: u32,
    pub code_length: u8,
    /// Declared Tf size, before text/page transforms; not a visual height.
    pub font_size: f32,
    /// Tz as a factor, not a percentage. Kept separate from `matrix`.
    pub horizontal_scale: f32,
    /// BaseFont name, not a resource key or a stable document font identity.
    pub font_name: Option<String>,
    /// Font writing mode, independent of page rotation; absent if no font state.
    pub writing_mode: Option<crate::font::WritingMode>,
    pub unicode: Option<String>,
    pub position: Point,
    /// Display-space advance including character/word spacing, excluding TJ adjustments.
    /// A layout vector, not a glyph ink bounding box.
    pub advance: [f32; 2],
    /// W2 vector from horizontal glyph origin to vertical text origin, in
    /// thousandths of text space; [0, 0] for horizontal fonts. Not yet applied
    /// to `position`. Use `glyph_origin_display()` for the horizontal outline.
    pub vertical_origin: [f32; 2],
    pub matrix: Matrix,
}

impl ExtractedCharacter {
    /// Horizontal outline origin, including the vertical font's W2 offset.
    /// The text origin remains available independently in `position`.
    pub fn glyph_origin_display(&self) -> PdfResult<Point> {
        let x = -f64::from(self.vertical_origin[0])
            * f64::from(self.font_size)
            * f64::from(self.horizontal_scale)
            / 1000.;
        let y = -f64::from(self.vertical_origin[1]) * f64::from(self.font_size) / 1000.;
        let x_display = f64::from(self.position.x())
            + x * f64::from(self.matrix.a())
            + y * f64::from(self.matrix.c());
        let y_display = f64::from(self.position.y())
            + x * f64::from(self.matrix.b())
            + y * f64::from(self.matrix.d());
        let point = Point::new(x_display as f32, y_display as f32);
        if !point.x().is_finite() || !point.y().is_finite() {
            return Err(PdfError::ObjectError(
                "character glyph origin overflows".into(),
            ));
        }
        Ok(point)
    }

    /// Map local em coordinates anchored at this character's baseline origin
    /// into display space, including Tf, Tz, text/page matrices and UserUnit.
    /// Not a glyph outline/bounding box or visibility test. Vertical origins are
    /// applied by `glyph_origin_display()`, separately from the text origin.
    pub fn em_to_display_matrix(&self) -> PdfResult<Matrix> {
        if !self.font_size.is_finite() || self.font_size <= 0. {
            return Err(PdfError::ObjectError(
                "character font size must be finite and positive".into(),
            ));
        }
        let origin = self.glyph_origin_display()?;
        let linear = Matrix::new(
            self.matrix.a(),
            self.matrix.b(),
            self.matrix.c(),
            self.matrix.d(),
            origin.x(),
            origin.y(),
        );
        Matrix::new(
            self.font_size * self.horizontal_scale,
            0.,
            0.,
            self.font_size,
            0.,
            0.,
        )
        .checked_mul(&linear)
    }

    /// Convert a display point to this character's local em coordinates.
    /// Singular/zero horizontal scaling rejects; does not perform glyph hit testing.
    pub fn display_to_em_point(&self, point: &Point) -> PdfResult<Point> {
        self.em_to_display_matrix()?
            .checked_inverse()?
            .checked_transform(point)
    }

    /// Display-space vector for the positive font writing axis. Not normalized,
    /// and not the actual advance: negative widths/spacing can reverse motion.
    /// Horizontal uses text x scaled by Tz; vertical uses text y. Zero stays zero.
    /// Returns None if no font is selected or the resulting vector overflows.
    pub fn writing_axis(&self) -> Option<[f32; 2]> {
        match self.writing_mode? {
            crate::font::WritingMode::Horizontal => {
                let axis = [
                    self.matrix.a() * self.horizontal_scale,
                    self.matrix.b() * self.horizontal_scale,
                ];
                axis.iter().all(|value| value.is_finite()).then_some(axis)
            }
            crate::font::WritingMode::Vertical => Some([self.matrix.c(), self.matrix.d()]),
        }
    }
}

/// One interpreted text-show object, not a line or paragraph. The optional
/// source identifies its operator; TJ characters also retain array entry indices.
#[derive(Debug, Clone)]
pub struct ExtractedTextRun {
    pub source: Option<super::text_object::TextDrawSource>,
    pub object_index: usize,
    pub characters: Vec<ExtractedCharacter>,
}

impl ExtractedTextRun {
    /// Borrow the show object's Form location without duplicating its call chain.
    pub fn nested_source(&self) -> Option<&super::source_provenance::NestedDrawSource> {
        self.characters
            .first()
            .and_then(|character| character.nested_source.as_ref())
    }
}

/// Paint-order character events and warnings from the same interpretation.
#[derive(Debug, Clone)]
pub struct TextExtractionReport {
    provenance: super::source_provenance::PageSourceProvenance,
    pub characters: Vec<ExtractedCharacter>,
    pub diagnostics: Vec<super::diagnostic::ContentDiagnostic>,
}

/// Text-show runs and warnings from the same interpretation.
#[derive(Debug, Clone)]
pub struct TextRunExtractionReport {
    provenance: super::source_provenance::PageSourceProvenance,
    pub runs: Vec<ExtractedTextRun>,
    pub diagnostics: Vec<super::diagnostic::ContentDiagnostic>,
}

impl TextExtractionReport {
    pub fn source_stamp(&self) -> &crate::document::identity::DocumentStamp {
        &self.provenance.stamp
    }
    /// Snapshot identity/revision/page check, not nested leaf validation or a
    /// guarantee that the paired historical snapshots are current for editing.
    pub fn matches_program(&self, program: &super::content_program::ContentProgram) -> bool {
        self.provenance.matches_program(program)
    }
    /// Explicit single-column, display-horizontal, left-to-right spatial policy.
    /// Returns indices into the unchanged paint-order characters. Lines are
    /// grouped against their topmost baseline using a caller-supplied tolerance.
    /// Not tagged-PDF order, bidi, column detection, or paragraph reconstruction.
    /// Unsupported writing directions are rejected rather than guessed.
    pub fn single_column_reading_order_indices(
        &self,
        line_tolerance: f32,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<Vec<usize>> {
        if !line_tolerance.is_finite() || line_tolerance < 0.0 {
            return Err(PdfError::ObjectError(
                "reading-order tolerance must be finite and nonnegative".into(),
            ));
        }
        let count = self.characters.len();
        if count > limits.max_text_chars
            || count
                .checked_mul(std::mem::size_of::<usize>())
                .is_none_or(|bytes| bytes > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit("reading-order indices".into()));
        }
        for character in &self.characters {
            let axis = character
                .writing_axis()
                .ok_or_else(|| PdfError::UnsupportedFeature("reading-order writing axis".into()))?;
            if character.writing_mode != Some(crate::font::WritingMode::Horizontal)
                || !character.font_size.is_finite()
                || character.font_size <= 0.0
                || axis[0] <= 0.0
                || axis[1].abs() > axis[0] * 0.00001
            {
                return Err(PdfError::UnsupportedFeature(
                    "single-column order requires display-horizontal left-to-right text".into(),
                ));
            }
            if !character.position.x().is_finite() || !character.position.y().is_finite() {
                return Err(PdfError::ObjectError(
                    "non-finite reading-order position".into(),
                ));
            }
        }
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("reading-order allocation".into()))?;
        indices.extend(0..count);
        indices.sort_unstable_by(|left, right| {
            self.characters[*left]
                .position
                .y()
                .total_cmp(&self.characters[*right].position.y())
                .then(left.cmp(right))
        });
        let mut start = 0;
        while start < count {
            let baseline = self.characters[indices[start]].position.y() as f64;
            let mut end = start + 1;
            while end < count
                && self.characters[indices[end]].position.y() as f64 - baseline
                    <= line_tolerance as f64
            {
                end += 1;
            }
            indices[start..end].sort_unstable_by(|left, right| {
                self.characters[*left]
                    .position
                    .x()
                    .total_cmp(&self.characters[*right].position.x())
                    .then(left.cmp(right))
            });
            start = end;
        }
        Ok(indices)
    }
}

impl<R: Read + Seek> PdfPage<'_, R> {
    /// Group interpreted characters by text-show object in paint order.
    /// Empty text objects do not generate runs. Character events are moved,
    /// not cloned; this is not reading-order layout.
    pub fn extract_text_runs_paint_order(&mut self) -> PdfResult<Vec<ExtractedTextRun>> {
        Ok(self.extract_text_runs_paint_order_report()?.runs)
    }

    /// Group characters into interpreted runs while retaining content warnings.
    /// Payloads move into runs; no second interpretation or Unicode copies.
    pub fn extract_text_runs_paint_order_report(&mut self) -> PdfResult<TextRunExtractionReport> {
        let TextExtractionReport {
            provenance,
            characters,
            diagnostics,
        } = self.extract_characters_paint_order_report()?;
        let mut runs: Vec<ExtractedTextRun> = Vec::new();
        for character in characters {
            if runs
                .last()
                .is_none_or(|run| run.object_index != character.object_index)
            {
                runs.try_reserve(1).map_err(|_| {
                    PdfError::ResourceLimit("Extracted text runs allocation".into())
                })?;
                runs.push(ExtractedTextRun {
                    source: character.source,
                    object_index: character.object_index,
                    characters: Vec::new(),
                });
            }
            let run = runs
                .last_mut()
                .ok_or_else(|| PdfError::DocumentError("Missing extracted run".into()))?;
            run.characters.try_reserve(1).map_err(|_| {
                PdfError::ResourceLimit("Extracted run characters allocation".into())
            })?;
            run.characters.push(character);
        }
        Ok(TextRunExtractionReport {
            runs,
            diagnostics,
            provenance,
        })
    }

    /// Interpret current page content and return character events in paint order.
    /// Missing Unicode mappings stay None; no guessed text or reading-order sort.
    /// Does not include annotation appearances or implement a full ToUnicode parser.
    /// Discards compatibility diagnostics; use the report API to retain warnings.
    pub fn extract_characters_paint_order(&mut self) -> PdfResult<Vec<ExtractedCharacter>> {
        Ok(self.extract_characters_paint_order_report()?.characters)
    }

    /// Interpret once, preserving warnings alongside paint-order character events.
    /// Missing Unicode stays None. Group blending spaces and soft-mask graphics
    /// are not evaluated; fonts, content syntax and invocation geometry are.
    pub fn extract_characters_paint_order_report(&mut self) -> PdfResult<TextExtractionReport> {
        let ctx = self.ctx;
        let list = ctx.with_text_geometry(|| self.display_list())?;
        self.extract_characters_from_list(list)
    }

    /// Paint-order characters under an explicit layer selection. The same
    /// visibility model is used for drawing; hidden content is still validated.
    pub fn extract_characters_with_optional_content_report(
        &mut self,
        visibility: &crate::document::optional_content::OptionalContentVisibility,
    ) -> PdfResult<TextExtractionReport> {
        let ctx = self.ctx;
        let list =
            ctx.with_text_geometry(|| self.display_list_with_optional_content(visibility))?;
        self.extract_characters_from_list(list)
    }

    fn extract_characters_from_list(
        &self,
        list: super::display_list::DisplayList,
    ) -> PdfResult<TextExtractionReport> {
        let provenance = self.source_provenance();
        let limits = self.ctx.limits();
        let (objects, diagnostics) = list.into_objects_with_diagnostics();
        let mut output = Vec::new();
        let mut retained_bytes = 0usize;
        for (object_index, object) in objects.into_iter().enumerate() {
            let PageObject::Text(text) = object else {
                continue;
            };
            for item in &text.items {
                if output.len() >= limits.max_text_chars {
                    return Err(PdfError::ResourceLimit("Extracted character count".into()));
                }
                let unicode = if let Some(value) = item.unicode() {
                    retained_bytes = retained_bytes.checked_add(value.len()).ok_or_else(|| {
                        PdfError::ResourceLimit("Extracted Unicode bytes overflow".into())
                    })?;
                    if retained_bytes > limits.max_decoded_bytes {
                        return Err(PdfError::ResourceLimit("Extracted Unicode bytes".into()));
                    }
                    let mut copy = String::new();
                    copy.try_reserve_exact(value.len()).map_err(|_| {
                        PdfError::ResourceLimit("Extracted Unicode allocation".into())
                    })?;
                    copy.push_str(value);
                    Some(copy)
                } else {
                    None
                };
                let font_name = if let Some(font) = text.state.text_state.font.as_ref() {
                    let name = font.name();
                    retained_bytes = retained_bytes.checked_add(name.len()).ok_or_else(|| {
                        PdfError::ResourceLimit("Extracted metadata bytes overflow".into())
                    })?;
                    if name.len() > limits.max_token_bytes
                        || retained_bytes > limits.max_decoded_bytes
                    {
                        return Err(PdfError::ResourceLimit("Extracted metadata bytes".into()));
                    }
                    let mut copy = String::new();
                    copy.try_reserve_exact(name.len()).map_err(|_| {
                        PdfError::ResourceLimit("Extracted font name allocation".into())
                    })?;
                    copy.push_str(name);
                    Some(copy)
                } else {
                    None
                };
                output.try_reserve(1).map_err(|_| {
                    PdfError::ResourceLimit("Extracted character allocation".into())
                })?;
                let nested_source = if let Some(source) = text.nested_source() {
                    retained_bytes = retained_bytes
                        .checked_add(source.retained_bytes()?)
                        .filter(|n| *n <= limits.max_decoded_bytes)
                        .ok_or_else(|| {
                            PdfError::ResourceLimit("Extracted nested source bytes".into())
                        })?;
                    Some(source.try_copy()?)
                } else {
                    None
                };
                let local_advance = text
                    .state
                    .text_state
                    .font
                    .as_ref()
                    .map(|font| {
                        font.text_advance(
                            item.charcode(),
                            text.state.text_state.font_size,
                            text.state.text_state.char_space,
                            text.state.text_state.word_space,
                            text.horizontal_scale,
                        )
                    })
                    .transpose()?
                    .unwrap_or([0., 0.]);
                let advance = [
                    f64::from(local_advance[0]) * f64::from(text.matrix.a())
                        + f64::from(local_advance[1]) * f64::from(text.matrix.c()),
                    f64::from(local_advance[0]) * f64::from(text.matrix.b())
                        + f64::from(local_advance[1]) * f64::from(text.matrix.d()),
                ]
                .map(|v| v as f32);
                if advance.iter().any(|v| !v.is_finite()) {
                    return Err(PdfError::ResourceLimit(
                        "extracted character advance overflow".into(),
                    ));
                }
                output.push(ExtractedCharacter {
                    source: text.source(),
                    nested_source,
                    array_item_index: item.array_item_index(),
                    decoded_code_range: item.decoded_code_range().cloned(),
                    object_index,
                    charcode: item.charcode().code(),
                    code_length: item.charcode().length(),
                    font_size: text.state.text_state.font_size,
                    horizontal_scale: text.horizontal_scale,
                    font_name,
                    writing_mode: text
                        .state
                        .text_state
                        .font
                        .as_ref()
                        .map(|font| font.writting_mode()),
                    unicode,
                    position: item.pos().clone(),
                    advance,
                    vertical_origin: if text.state.text_state.font.as_ref().is_some_and(|font| {
                        font.writting_mode() == crate::font::WritingMode::Vertical
                    }) {
                        [item.charcode().origin_x(), item.charcode().origin_y()]
                    } else {
                        [0., 0.]
                    },
                    matrix: text.matrix.clone(),
                });
            }
        }
        Ok(TextExtractionReport {
            provenance,
            characters: output,
            diagnostics,
        })
    }
}

impl TextRunExtractionReport {
    pub fn source_stamp(&self) -> &crate::document::identity::DocumentStamp {
        &self.provenance.stamp
    }
    pub fn matches_program(&self, program: &super::content_program::ContentProgram) -> bool {
        self.provenance.matches_program(program)
    }
}
