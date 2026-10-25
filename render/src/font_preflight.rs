//! Read-only font discovery over the same interpreted drawing events as rendering.
use crate::{
    font_provider::{GlyphChoice, MemoryFontProvider, inspect_glyph},
    render_device::SubstituteFont,
};
use pdf::{
    device::Device,
    error::{PdfError, PdfResult},
    font::{pdf_font::Font, resolver::FontIdentity},
    page::{
        display_list::DisplayList, graphic_state::GraphicState, image_object::ImageObject,
        path_object::PathObject, text_object::TextObject, text_state::TextRenderingMode,
    },
    parser::options::Limits,
};
use std::{
    collections::{BTreeSet, HashSet},
    rc::Rc,
};

pub struct FontRequirement {
    pub name: String,
    pub language: Option<String>,
    pub weight: u16,
    pub italic: bool,
    pub serif: bool,
    pub missing: BTreeSet<u32>,
    pub codepoints: BTreeSet<u32>,
    pub missing_unicode: usize,
    pub needs_shaping: usize,
    /// None identifies the explicit legacy single face.
    pub selected: Vec<(Option<FontIdentity>, usize)>,
}
pub struct FontPreflightReport {
    fonts: Vec<FontRequirement>,
    _lease: pdf::budget::MemoryLease,
}
impl std::ops::Deref for FontPreflightReport {
    type Target = [FontRequirement];
    fn deref(&self) -> &Self::Target {
        &self.fonts
    }
}
pub struct FontPreflight<'a> {
    provider: &'a MemoryFontProvider,
    legacy: Option<&'a SubstituteFont>,
    limits: Limits,
    faces: Vec<Rc<Font>>,
    requirements: Vec<FontRequirement>,
    visited: HashSet<usize>,
    depth: usize,
    operations: usize,
    characters: usize,
    _lease: pdf::budget::MemoryLease,
}
impl<'a> FontPreflight<'a> {
    pub fn new(
        provider: &'a MemoryFontProvider,
        legacy: Option<&'a SubstituteFont>,
        limits: Limits,
    ) -> PdfResult<Self> {
        // Conservative charge for bounded sets, vectors and names. Native face
        // allocation and interpreted lists retain their own existing limits.
        let bytes = limits
            .max_text_chars
            .checked_mul(64)
            .and_then(|n| {
                limits
                    .max_content_operations
                    .checked_mul(32)
                    .and_then(|o| n.checked_add(o))
            })
            .and_then(|n| n.checked_add(128 * 2048))
            .ok_or_else(|| PdfError::ResourceLimit("font preflight accounting".into()))?;
        let budget = pdf::budget::OperationBudget::new(limits.max_operation_bytes);
        Ok(Self {
            provider,
            legacy,
            limits,
            faces: vec![],
            requirements: vec![],
            visited: HashSet::new(),
            depth: 0,
            operations: 0,
            characters: 0,
            _lease: budget.reserve(bytes)?,
        })
    }
    pub fn finish(self) -> FontPreflightReport {
        FontPreflightReport {
            fonts: self.requirements,
            _lease: self._lease,
        }
    }
    fn tick(&mut self) -> PdfResult<()> {
        self.operations += 1;
        if self.operations > self.limits.max_content_operations {
            return Err(PdfError::ResourceLimit("font preflight operations".into()));
        }
        Ok(())
    }
    fn walk(&mut self, list: &DisplayList) -> PdfResult<()> {
        if self.depth >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit("font preflight depth".into()));
        }
        if !self.visited.insert(list as *const DisplayList as usize) {
            return Ok(());
        }
        if self.visited.len() > self.limits.max_content_operations {
            return Err(PdfError::ResourceLimit("font preflight lists".into()));
        }
        self.depth += 1;
        let result = list.replay(self);
        self.depth -= 1;
        result
    }
    fn state(&mut self, state: &GraphicState, fill: bool, stroke: bool) -> PdfResult<()> {
        if let Some(mask) = &state.general_state.soft_mask {
            self.walk(&mask.list)?;
        }
        for (used, space) in [
            (fill, &state.color_state.fill_color_space),
            (stroke, &state.color_state.stroke_color_space),
        ] {
            if used
                && let pdf::color::colorspace::ColorSpace::Pattern(pattern) = space
                && let Some(paint) = pattern.paint()
            {
                self.walk(&paint.list)?;
            }
        }
        Ok(())
    }
}
impl Device for FontPreflight<'_> {
    fn start_page(&mut self, _: f32, _: f32) -> PdfResult<()> {
        Ok(())
    }
    fn end_page(&mut self) -> PdfResult<()> {
        Ok(())
    }
    fn begin_group(&mut self, paint: pdf::page::transparency::GroupPaint) -> PdfResult<()> {
        self.tick()?;
        if let Some(mask) = paint.soft_mask {
            self.walk(&mask.list)?;
        }
        Ok(())
    }
    fn end_group(&mut self, _: bool) -> PdfResult<()> {
        self.tick()
    }
    fn do_shading(&mut self, object: pdf::page::shading_object::ShadingObject) -> PdfResult<()> {
        self.tick()?;
        self.state(&object.graphic_state, false, false)
    }
    fn do_image(&mut self, image: ImageObject) -> PdfResult<()> {
        self.tick()?;
        self.state(&image.graphic_state, image.image.is_mask(), false)
    }
    fn clip_path(&mut self, _: PathObject) -> PdfResult<()> {
        self.tick()
    }
    fn fill_path(&mut self, path: PathObject) -> PdfResult<()> {
        self.tick()?;
        if path.pdf_path().is_empty() {
            return Ok(());
        }
        self.state(path.graphic_state(), true, false)
    }
    fn stroke_path(&mut self, path: PathObject) -> PdfResult<()> {
        self.tick()?;
        if path.pdf_path().is_empty() {
            return Ok(());
        }
        self.state(path.graphic_state(), false, true)
    }
    fn fill_and_stroke_path(&mut self, path: PathObject) -> PdfResult<()> {
        self.tick()?;
        if path.pdf_path().is_empty() {
            return Ok(());
        }
        self.state(path.graphic_state(), true, true)
    }
    fn show_text(&mut self, text: TextObject) -> PdfResult<()> {
        self.tick()?;
        use TextRenderingMode::*;
        let mode = &text.state.text_state.text_rendering_mode;
        if text.is_type3_metadata() || matches!(mode, INVisible | Clip) {
            return Ok(());
        }
        self.state(
            &text.state,
            matches!(mode, Fill | FillClip | FillStroke | FillStrokeClip),
            matches!(mode, Stroke | StrokeClip | FillStroke | FillStrokeClip),
        )?;
        let Some(font) = &text.state.text_state.font else {
            return Ok(());
        };
        if font.fontfile().is_some() {
            return Ok(());
        }
        if matches!(font.as_ref(), Font::Type3(_)) {
            return Err(PdfError::UnsupportedFeature(
                "Type3 raw text requires interpreted glyph graphics".into(),
            ));
        }
        let index = if let Some(i) = self.faces.iter().enumerate().position(|(i, old)| {
            Rc::ptr_eq(old, font) && self.requirements[i].language.as_deref() == text.language()
        }) {
            i
        } else {
            if self.faces.len() >= 128 || font.name().len() > 1024 {
                return Err(PdfError::ResourceLimit("font preflight faces/names".into()));
            }
            let style = font.substitution_request(' ');
            self.faces.push(font.clone());
            self.requirements.push(FontRequirement {
                name: font.name().into(),
                language: text.language().map(str::to_owned),
                weight: style.weight,
                italic: style.italic,
                serif: style.serif,
                missing: BTreeSet::new(),
                codepoints: BTreeSet::new(),
                missing_unicode: 0,
                needs_shaping: 0,
                selected: vec![],
            });
            self.faces.len() - 1
        };
        for item in &text.items {
            self.characters += 1;
            if self.characters > self.limits.max_text_chars {
                return Err(PdfError::ResourceLimit("font preflight characters".into()));
            }
            let inspected = inspect_glyph(
                Some(self.provider),
                self.legacy.map(|p| &p.face),
                font,
                item.charcode(),
                text.language(),
            )?;
            let requirement = &mut self.requirements[index];
            if let Some(ch) = inspected.character {
                requirement.codepoints.insert(ch as u32);
            }
            let identity = match inspected.choice {
                GlyphChoice::MissingUnicode => {
                    requirement.missing_unicode += 1;
                    continue;
                }
                GlyphChoice::NeedsShaping => {
                    requirement.needs_shaping += 1;
                    continue;
                }
                GlyphChoice::MissingGlyph(ch) => {
                    requirement.missing.insert(ch as u32);
                    continue;
                }
                GlyphChoice::Registered(value) => Some(value.identity),
                GlyphChoice::Legacy(_) => None,
            };
            if let Some((_, count)) = requirement
                .selected
                .iter_mut()
                .find(|(old, _)| *old == identity)
            {
                *count += 1
            } else {
                requirement.selected.push((identity, 1))
            }
        }
        Ok(())
    }
}
