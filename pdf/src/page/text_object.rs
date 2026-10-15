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
    array_item_index: Option<usize>,
    decoded_code_range: Option<std::ops::Range<usize>>,
}

impl CharItem {
    pub fn new(pos: Point, charcode: CharCode, unicode: Option<String>) -> Self {
        Self {
            pos,
            charcode,
            unicode,
            array_item_index: None,
            decoded_code_range: None,
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
    /// TJ array entry (numbers count too); absent for single-string operators.
    pub fn array_item_index(&self) -> Option<usize> {
        self.array_item_index
    }
    pub(crate) fn with_array_item_index(mut self, index: usize) -> Self {
        self.array_item_index = Some(index);
        self
    }
    /// Byte range within the decoded PDF string, not its escaped source token.
    pub fn decoded_code_range(&self) -> Option<&std::ops::Range<usize>> {
        self.decoded_code_range.as_ref()
    }
    pub(crate) fn with_decoded_code_range(mut self, range: std::ops::Range<usize>) -> Self {
        self.decoded_code_range = Some(range);
        self
    }
}

/// Text-show operator location in one revision's decoded page Contents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextDrawSource {
    pub operation_index: usize,
    pub operator_byte_offset: Option<u64>,
}

#[derive(Debug, Clone)]
pub struct TextObject {
    type3_metadata: bool,
    source: Option<TextDrawSource>,
    nested_source: Option<super::source_provenance::NestedDrawSource>,
    pub state: GraphicState,
    pub items: Vec<CharItem>,
    pub origin_x: f32,
    pub origin_y: f32,
    pub matrix: Matrix,
    /// Tz as a unit factor (50 percent = 0.5), independent of the text matrix.
    pub horizontal_scale: f32,
}

impl TextObject {
    /// Type3 glyph graphics were emitted separately. Keep this event for
    /// extraction/provenance only; painting it again would duplicate the glyph.
    pub fn is_type3_metadata(&self) -> bool {
        self.type3_metadata
    }

    pub(crate) fn with_type3_metadata(mut self) -> Self {
        self.type3_metadata = true;
        self
    }
    /// Nonzero-winding location in an unhinted simple character outline.
    /// Geometry only: text rendering mode, clipping and visibility are ignored.
    pub fn character_outline_location_display(
        &self,
        index: usize,
        point: [f64; 2],
        limits: crate::parser::options::Limits,
    ) -> crate::error::PdfResult<crate::font::truetype::tables::outline_hit::OutlineLocation> {
        if !point.iter().all(|v| v.is_finite()) {
            return Err(crate::error::PdfError::ObjectError(
                "Non-finite character outline query".into(),
            ));
        }
        let commands = self.character_outline_display(index, limits)?;
        crate::font::truetype::tables::outline_hit::outline_location(&commands, point, &limits)
    }

    /// Tight unhinted simple-outline display bounds, not painted/visible bounds.
    /// Computes extrema after transforming curves; does not transform an AABB.
    pub fn character_outline_display_bounds(
        &self,
        index: usize,
        limits: crate::parser::options::Limits,
    ) -> crate::error::PdfResult<Option<[f64; 4]>> {
        let commands = self.character_outline_display(index, limits)?;
        crate::font::truetype::tables::glyf::outline_bounds(&commands, &limits)
    }

    /// Unhinted simple/composite TrueType outline for one painted character, in display
    /// coordinates. Uses the character's advancing baseline, not the run origin.
    /// No clipping, opacity, stroking, occlusion, phantom alignment or variable deltas.
    pub fn character_outline_display(
        &self,
        index: usize,
        limits: crate::parser::options::Limits,
    ) -> crate::error::PdfResult<Vec<crate::font::truetype::tables::glyf::GlyphOutlineCommand>>
    {
        use crate::{error::PdfError, font::truetype::tables::glyf::GlyphOutlineCommand};
        let item = self
            .items
            .get(index)
            .ok_or_else(|| PdfError::ObjectError("Character outline index out of range".into()))?;
        let font = self
            .state
            .text_state
            .font
            .as_ref()
            .ok_or_else(|| PdfError::FontError("Character outline requires a font".into()))?;
        let size = self.state.text_state.font_size;
        if !size.is_finite() || size <= 0. {
            return Err(PdfError::ObjectError(
                "Character outline font size must be positive".into(),
            ));
        }
        let (ox, oy) = if font.writting_mode() == crate::font::WritingMode::Vertical {
            (-item.charcode().origin_x() / 1000., -item.charcode().origin_y() / 1000.)
        } else { (0., 0.) };
        let matrix = Matrix::new(1., 0., 0., 1., ox, oy)
            .checked_mul(&Matrix::new(size * self.horizontal_scale, 0., 0., size, 0., 0.))?.checked_mul(
            &Matrix::new(
                self.matrix.a(),
                self.matrix.b(),
                self.matrix.c(),
                self.matrix.d(),
                item.pos.x(),
                item.pos.y(),
            ),
        )?;
        let mut commands = font.glyph_outline(item.charcode(), limits)?;
        let transform = |point: &mut [f64; 2]| -> crate::error::PdfResult<()> {
            let x = point[0] * f64::from(matrix.a())
                + point[1] * f64::from(matrix.c())
                + f64::from(matrix.e());
            let y = point[0] * f64::from(matrix.b())
                + point[1] * f64::from(matrix.d())
                + f64::from(matrix.f());
            if !x.is_finite()
                || !y.is_finite()
                || x.abs() > f64::from(f32::MAX)
                || y.abs() > f64::from(f32::MAX)
            {
                return Err(PdfError::ObjectError(
                    "Character outline display coordinates overflow".into(),
                ));
            }
            *point = [x, y];
            Ok(())
        };
        for command in &mut commands {
            match command {
                GlyphOutlineCommand::Move(point) | GlyphOutlineCommand::Line(point) => {
                    transform(point)?
                }
                GlyphOutlineCommand::Quadratic { control, end } => {
                    transform(control)?;
                    transform(end)?;
                }
                GlyphOutlineCommand::Close => {}
            }
        }
        Ok(commands)
    }

    pub fn new(state: GraphicState) -> Self {
        TextObject {
            type3_metadata: false,
            source: None,
            nested_source: None,
            state,
            items: Vec::new(),
            origin_x: 0.0,
            origin_y: 0.0,
            matrix: Matrix::identity(),
            horizontal_scale: 1.0,
        }
    }
    pub fn add_item(&mut self, item: CharItem) {
        self.items.push(item)
    }
    pub fn source(&self) -> Option<TextDrawSource> {
        self.source
    }
    pub fn nested_source(&self) -> Option<&super::source_provenance::NestedDrawSource> {
        self.nested_source.as_ref()
    }
    pub(crate) fn with_nested_source(
        mut self,
        source: Option<super::source_provenance::NestedDrawSource>,
    ) -> Self {
        self.nested_source = source;
        self
    }
    pub(crate) fn with_source(mut self, source: TextDrawSource) -> Self {
        self.source = Some(source);
        self
    }
}
