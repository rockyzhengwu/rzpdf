use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::font::CharCode;
use crate::font::simple_font::SimpleFont;
use crate::font::type0::Type0;
use crate::objects::pdf_dict::PdfDict;
use crate::pdf_context::PDFContext;

use super::{GlyphDesc, WritingMode};

#[derive(Debug, Clone)]
#[expect(
    clippy::large_enum_variant,
    reason = "preserve public font variants and inline budget accounting"
)]
pub enum Font {
    Simple(SimpleFont),
    Type0(Type0),
    Type3(std::rc::Rc<super::type3::Type3Font>),
}

/// How a font program was selected for drawing; extraction may still be possible
/// when the result is `Missing`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FontProgramSource {
    Embedded,
    BundledBase14,
    DeclaredSubstitute { base14_name: String },
    Missing,
}

/// Bound input preflight work before constructing encoding tables.
pub(crate) fn encoding_character_count(
    text: &str,
    limits: crate::parser::options::Limits,
) -> PdfResult<usize> {
    if text.len() > limits.max_decoded_bytes {
        return Err(PdfError::ResourceLimit("font encoding input bytes".into()));
    }
    let count = text
        .chars()
        .take(limits.max_text_chars.saturating_add(1))
        .count();
    if count > limits.max_text_chars {
        return Err(PdfError::ResourceLimit(
            "font encoding input characters".into(),
        ));
    }
    Ok(count)
}

impl Font {
    pub(crate) fn text_advance(
        &self,
        ch: &CharCode,
        size: f32,
        char_space: f32,
        word_space: f32,
        horizontal: f32,
    ) -> PdfResult<[f32; 2]> {
        let is_space = ch.code() == 32 && ch.length() == 1;
        let mut ordinary = (ch.width() / 1000.0) * size + char_space;
        if is_space {
            ordinary += word_space;
        }
        let value = match self {
            Self::Type3(font) => {
                let advance = font.text_space_advance(type3_code(ch)?)?;
                let mut x = advance[0] * size + char_space;
                if is_space {
                    x += word_space;
                }
                [x * horizontal, advance[1] * size]
            }
            _ if self.writting_mode() == WritingMode::Vertical => [0., ordinary],
            _ => [ordinary * horizontal, 0.],
        };
        if value.iter().any(|v| !v.is_finite()) {
            return Err(PdfError::FontError("non-finite text advance".into()));
        }
        Ok(value)
    }
    /// Shared ownership path: output capacity is reserved before encoding.
    pub(crate) fn encode_text_with_budget(
        &self,
        text: &str,
        limits: crate::parser::options::Limits,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<crate::budget::BudgetedBytes> {
        match self {
            Self::Simple(font) => font.encode_text_with_budget(text, limits, budget),
            Self::Type0(font) => font.encode_text_with_budget(text, limits, budget),
            Self::Type3(_) => Err(PdfError::UnsupportedFeature(
                "Type3 Unicode encoding/editing".into(),
            )),
        }
    }

    /// Resolve a PDF character through this font's code/CID-to-GID mapping and
    /// return its unhinted simple TrueType outline in em coordinates.
    /// Does not substitute Unicode cmap lookup for PDF encoding. Composite/CFF/
    /// Type 1 outlines and unavailable program/GID mappings reject explicitly.
    pub fn simple_glyph_outline(
        &self,
        character: &CharCode,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<Vec<crate::font::truetype::tables::glyf::GlyphOutlineCommand>> {
        self.mapped_glyph_outline(character, limits, false)
    }

    /// PDF code/CID-to-GID outline, including unhinted default composites.
    /// No Unicode remapping, phantom alignment, hinting or variable deltas.
    pub fn glyph_outline(
        &self,
        character: &CharCode,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<Vec<crate::font::truetype::tables::glyf::GlyphOutlineCommand>> {
        self.mapped_glyph_outline(character, limits, true)
    }

    fn mapped_glyph_outline(
        &self,
        character: &CharCode,
        limits: crate::parser::options::Limits,
        composites: bool,
    ) -> PdfResult<Vec<crate::font::truetype::tables::glyf::GlyphOutlineCommand>> {
        let gid = match self.get_glyph(character) {
            Some(GlyphDesc::Gid(gid)) => u16::try_from(gid)
                .map_err(|_| PdfError::FontError("Glyph ID exceeds TrueType range".into()))?,
            _ => {
                return Err(PdfError::UnsupportedFeature(
                    "Font has no numeric glyph mapping".into(),
                ));
            }
        };
        let data = self.fontfile().ok_or_else(|| {
            PdfError::UnsupportedFeature("Font program unavailable for outlines".into())
        })?;
        use crate::font::truetype::true_type_program::TrueTypeProgram;
        if composites {
            TrueTypeProgram::checked_glyph_outline(data, gid, limits)
        } else {
            TrueTypeProgram::checked_simple_glyph_outline(data, gid, limits)
        }
    }

    pub fn try_new<R: Seek + Read>(dict: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let subtype = dict
            .get("Subtype")
            .ok_or(PdfError::FontError("Subtye is Null".to_string()))?;
        let subtype = ctx.resolve(subtype)?;
        let subtype = subtype
            .as_name()
            .ok_or_else(|| PdfError::FontError("font Subtype must be a name".into()))?;
        match subtype.name() {
            "Type0" => {
                let type0 = Type0::try_new(dict, ctx)?;
                Ok(Font::Type0(type0))
            }
            "Type1" => {
                let typ1 = SimpleFont::try_new(dict, ctx)?;
                Ok(Font::Simple(typ1))
            }
            "TrueType" => {
                let truetype = SimpleFont::try_new(dict, ctx)?;
                Ok(Font::Simple(truetype))
            }
            "Type3" => Ok(Self::Type3(std::rc::Rc::new(
                super::type3::Type3Font::try_new(dict, ctx)?,
            ))),
            _ => Err(PdfError::FontError(format!(
                "unsupported font subtype: {}",
                subtype.name()
            )))?,
        }
    }

    /// Partial construction budget: SimpleFont BaseFont, Encoding, AFM and decoded
    /// Descriptor font-program and ToUnicode string storage, plus field/width
    /// resolution, CMap tables, sources and inheritance chains. Shared control blocks,
    /// FreeType input copies are tracked through face lifetime; native allocations
    /// remain excluded. Type0 tracks
    /// BaseFont/CMaps, descendant resolution, CID names/maps and Descriptor storage;
    /// CID width-table capacity and per-item resolution are also tracked.
    pub(crate) fn try_new_with_budget<R: Seek + Read>(
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let subtype = dict
            .get("Subtype")
            .ok_or_else(|| PdfError::FontError("Subtye is Null".into()))?;
        let (subtype, _temporary) = ctx.resolve_owned_with_budget(subtype, budget)?;
        let subtype = subtype
            .as_name()
            .ok_or_else(|| PdfError::FontError("font Subtype must be a name".into()))?;
        match subtype.name() {
            "Type1" | "TrueType" => {
                let (font, lease) = SimpleFont::try_new_with_budget(dict, ctx, budget)?;
                Ok((Self::Simple(font), lease))
            }
            "Type0" => {
                let (font, lease) = Type0::try_new_with_budget(dict, ctx, budget)?;
                Ok((Self::Type0(font), lease))
            }
            "Type3" => Ok((
                Self::Type3(std::rc::Rc::new(
                    super::type3::Type3Font::try_new_with_budget(dict, ctx, budget)?,
                )),
                budget.reserve(0)?,
            )),
            name => Err(PdfError::FontError(format!(
                "unsupported font subtype: {name}"
            ))),
        }
    }

    pub fn text_widths(&self, chars: &[CharCode]) -> PdfResult<f32> {
        match self {
            Font::Simple(t) => t.text_widths(chars),
            Font::Type0(t) => t.text_widths(chars),
            Font::Type3(_) => Err(PdfError::UnsupportedFeature(
                "Type3 editing width aggregation".into(),
            )),
        }
    }

    pub fn unicode(&self, char: &CharCode) -> PdfResult<String> {
        match self {
            Font::Simple(t) => t.unicode(char),
            Font::Type0(t) => t.unicode(char),
            Font::Type3(t) => Ok(t
                .unicode_mapping(type3_code(char)?)
                .unwrap_or_else(|| "\u{fffd}".into())),
        }
    }

    pub fn writting_mode(&self) -> WritingMode {
        match self {
            Font::Simple(_) => WritingMode::Horizontal,
            Font::Type0(tf) => tf.writting_mode(),
            Font::Type3(_) => WritingMode::Horizontal,
        }
    }

    /// Unlike unicode(), absence is not replaced by U+FFFD.
    pub fn unicode_mapping(&self, ch: &CharCode) -> PdfResult<Option<String>> {
        match self {
            Font::Simple(font) => font.unicode_mapping(ch),
            Font::Type0(font) => Ok(font.unicode_mapping(ch).map(str::to_owned)),
            Font::Type3(font) => Ok(font.unicode_mapping(type3_code(ch)?)),
        }
    }

    pub(crate) fn matches_unicode(&self, ch: &CharCode, expected: &str) -> bool {
        match self {
            Self::Simple(font) => font.matches_unicode(ch, expected),
            Self::Type0(font) => font.unicode_mapping(ch) == Some(expected),
            Self::Type3(_) => false,
        }
    }

    pub fn chars(&self, codes: &[u8]) -> PdfResult<Vec<CharCode>> {
        match self {
            Font::Simple(s) => s.chars(codes),
            Font::Type0(ft) => ft.chars(codes),
            Font::Type3(ft) => ft.chars_limited(codes, usize::MAX),
        }
    }
    pub fn chars_limited(&self, codes: &[u8], limit: usize) -> PdfResult<Vec<CharCode>> {
        match self {
            Font::Simple(font) => font.chars_limited(codes, limit),
            Font::Type0(font) => font.chars_limited(codes, limit),
            Font::Type3(font) => font.chars_limited(codes, limit),
        }
    }
    /// Validate and visit codes without allocating a retained character vector.
    pub(crate) fn visit_chars_limited(
        &self,
        codes: &[u8],
        limit: usize,
        visitor: impl FnMut(CharCode) -> PdfResult<()>,
    ) -> PdfResult<usize> {
        match self {
            Font::Simple(font) => font.visit_chars_limited(codes, limit, visitor),
            Font::Type0(font) => font.visit_chars_limited(codes, limit, visitor),
            Font::Type3(font) => font.visit_chars_limited(codes, limit, visitor),
        }
    }

    pub fn name(&self) -> &str {
        match self {
            Font::Simple(f) => f.base_font(),
            Font::Type0(t) => t.base_font(),
            Font::Type3(_) => "Type3",
        }
    }

    pub fn get_glyph(&self, char: &CharCode) -> Option<GlyphDesc> {
        match self {
            Font::Simple(s) => s.get_glyph(char),
            Font::Type0(t0) => t0.get_glyph(char),
            Font::Type3(font) => type3_code(char)
                .ok()
                .and_then(|code| font.glyph_name(code))
                .map(|name| GlyphDesc::Name(name.into())),
        }
    }

    pub fn fontfile(&self) -> Option<&[u8]> {
        match self {
            Font::Simple(s) => s.fontfile(),
            Font::Type0(t) => t.fontfile(),
            Font::Type3(_) => None,
        }
    }

    pub fn font_program_source(&self) -> FontProgramSource {
        match self {
            Font::Simple(font) if font.has_embedded_program() => FontProgramSource::Embedded,
            Font::Simple(font) => match super::builtin_font::find_builtin_font(font.base_font()) {
                Some(name) if name == font.base_font() => FontProgramSource::BundledBase14,
                Some(name) => FontProgramSource::DeclaredSubstitute {
                    base14_name: name.to_owned(),
                },
                None => FontProgramSource::Missing,
            },
            Font::Type0(font) if font.fontfile().is_some() => FontProgramSource::Embedded,
            Font::Type0(_) => FontProgramSource::Missing,
            Font::Type3(_) => FontProgramSource::Embedded,
        }
    }
}

fn type3_code(ch: &CharCode) -> PdfResult<u8> {
    if ch.length() != 1 {
        return Err(PdfError::FontError("Type3 code must be one byte".into()));
    }
    u8::try_from(ch.code()).map_err(|_| PdfError::FontError("Type3 code outside 0..255".into()))
}

#[cfg(test)]
mod encoding_input_tests {
    use super::*;

    #[test]
    fn font_program_source_distinguishes_builtin_alias_and_missing() {
        use crate::{
            document::owned::Document,
            objects::{PdfObject, pdf_name::PdfName},
        };
        let doc = Document::new().unwrap();
        for (name, expected) in [
            ("Courier", FontProgramSource::BundledBase14),
            (
                "Arial",
                FontProgramSource::DeclaredSubstitute {
                    base14_name: "Helvetica".into(),
                },
            ),
            ("Unlisted-Font", FontProgramSource::Missing),
        ] {
            let mut dict = PdfDict::default();
            dict.insert(
                "Subtype".into(),
                PdfObject::PdfName(PdfName::new("Type1".into())),
            );
            dict.insert(
                "BaseFont".into(),
                PdfObject::PdfName(PdfName::new(name.into())),
            );
            let font = Font::try_new(&dict, doc.ctx()).unwrap();
            assert_eq!(font.font_program_source(), expected);
        }
    }

    #[test]
    fn two_font_results_share_one_pool_and_recover_after_coexistence_rejection() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName},
        };
        let doc = Document::new().unwrap();
        let dictionary = |name: &str| {
            let mut dict = PdfDict::default();
            dict.insert(
                "Subtype".into(),
                PdfObject::PdfName(PdfName::new("Type1".into())),
            );
            dict.insert(
                "BaseFont".into(),
                PdfObject::PdfName(PdfName::new(name.into())),
            );
            dict
        };
        let first = dictionary("Courier");
        let second = dictionary("Helvetica");
        let mut individual_peak = 0;
        for dict in [&first, &second] {
            let pool = OperationBudget::new(1024 * 1024);
            drop(Font::try_new_with_budget(dict, doc.ctx(), &pool).unwrap());
            individual_peak = individual_peak.max(pool.peak_bytes());
            assert_eq!(pool.live_bytes(), 0);
        }
        let probe = OperationBudget::new(1024 * 1024);
        let first_result = Font::try_new_with_budget(&first, doc.ctx(), &probe).unwrap();
        let first_retained = first_result.1.bytes();
        let second_result = Font::try_new_with_budget(&second, doc.ctx(), &probe).unwrap();
        assert_eq!(probe.live_bytes(), first_retained + second_result.1.bytes());
        let coexistence_peak = probe.peak_bytes();
        assert!(coexistence_peak > individual_peak);
        drop((first_result, second_result));
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(coexistence_peak - 1);
        assert!(pool.maximum_bytes() >= individual_peak);
        for dict in [&first, &second] {
            drop(Font::try_new_with_budget(dict, doc.ctx(), &pool).unwrap());
            assert_eq!(pool.live_bytes(), 0);
        }
        let first_result = Font::try_new_with_budget(&first, doc.ctx(), &pool).unwrap();
        assert!(matches!(
            Font::try_new_with_budget(&second, doc.ctx(), &pool),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), first_retained);
        assert_eq!(first_result.0.name(), "Courier");
        drop(first_result);
        assert_eq!(pool.live_bytes(), 0);
        drop(Font::try_new_with_budget(&second, doc.ctx(), &pool).unwrap());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn indirect_subtype_uses_budget_constructor_and_ordinary_resolution() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            objects::{
                PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_reference::PdfReference,
            },
        };
        let doc = Document::new().unwrap();
        let subtype = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("Type1".into())))
            .unwrap();
        let mut dict = PdfDict::default();
        dict.insert(
            "Subtype".into(),
            PdfObject::PdfReference(PdfReference::from_id(subtype)),
        );
        dict.insert(
            "BaseFont".into(),
            PdfObject::PdfName(PdfName::new("Helvetica".into())),
        );
        assert_eq!(Font::try_new(&dict, doc.ctx()).unwrap().name(), "Helvetica");
        let probe = OperationBudget::new(
            65536
                + crate::font::builtin_font::load_builtin_font_data("Helvetica")
                    .unwrap()
                    .len(),
        );
        let (font, lease) = Font::try_new_with_budget(&dict, doc.ctx(), &probe).unwrap();
        assert_eq!(font.name(), "Helvetica");
        assert!(lease.bytes() >= "Helvetica".len());
        assert_eq!(probe.live_bytes(), lease.bytes());
        let peak = probe.peak_bytes();
        drop((font, lease));
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(Font::try_new_with_budget(&dict, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        drop(Font::try_new_with_budget(&dict, doc.ctx(), &pool).unwrap());
        assert_eq!(pool.live_bytes(), 0);
        doc.update_object(subtype, PdfObject::PdfNull).unwrap();
        assert!(Font::try_new(&dict, doc.ctx()).is_err());
        assert!(Font::try_new_with_budget(&dict, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn simple_basefont_budget_is_retained_and_rejects_before_construction() {
        use crate::{
            budget::OperationBudget,
            document::{owned::Document, resource_manager::StandardFont},
        };
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let object = doc
            .get_page(0)
            .unwrap()
            .get_resource_font(&name)
            .unwrap()
            .unwrap();
        let dict = object.as_dict().unwrap();
        let old_pool = OperationBudget::new(65536);
        assert!(matches!(
            Font::try_new_with_budget(dict, doc.ctx(), &old_pool),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(old_pool.live_bytes(), 0);
        let probe = OperationBudget::new(
            65536
                + crate::font::builtin_font::load_builtin_font_data("Helvetica")
                    .unwrap()
                    .len(),
        );
        let (font, lease) = Font::try_new_with_budget(dict, doc.ctx(), &probe).unwrap();
        let retained_bytes = lease.bytes();
        assert!(retained_bytes >= "Helvetica".len());
        assert_eq!(probe.live_bytes(), retained_bytes);
        let peak = probe.peak_bytes();
        assert!(peak >= probe.live_bytes());
        drop(font);
        drop(lease);
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(Font::try_new_with_budget(dict, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let (font, lease) = Font::try_new_with_budget(dict, doc.ctx(), &pool).unwrap();
        assert_eq!(font.name(), "Helvetica");
        assert_eq!(pool.live_bytes(), retained_bytes);
        drop(font);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        let id = doc
            .try_insert_object(crate::objects::PdfObject::PdfName(
                crate::objects::pdf_name::PdfName::new("Helvetica".into()),
            ))
            .unwrap();
        let mut indirect = dict.clone();
        indirect.insert(
            "BaseFont".into(),
            crate::objects::PdfObject::PdfReference(
                crate::objects::pdf_reference::PdfReference::from_id(id),
            ),
        );
        let (font, lease) = Font::try_new_with_budget(&indirect, doc.ctx(), &pool).unwrap();
        assert_eq!(font.name(), "Helvetica");
        drop(font);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        indirect.insert(
            "FirstChar".into(),
            crate::objects::PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new(256.0)),
        );
        assert!(Font::try_new_with_budget(&indirect, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn input_limits_count_scalars_and_handle_zero_and_unbounded_limits() {
        let limits = crate::parser::options::Limits {
            max_text_chars: 2,
            max_decoded_bytes: 7,
            ..Default::default()
        };
        assert_eq!(encoding_character_count("中😀", limits).unwrap(), 2);
        assert!(encoding_character_count("中😀a", limits).is_err());
        assert!(encoding_character_count("abc", limits).is_err());
        let zero = crate::parser::options::Limits {
            max_text_chars: 0,
            max_decoded_bytes: 0,
            ..limits
        };
        assert_eq!(encoding_character_count("", zero).unwrap(), 0);
        assert!(encoding_character_count("a", zero).is_err());
        let unbounded = crate::parser::options::Limits {
            max_text_chars: usize::MAX,
            max_decoded_bytes: usize::MAX,
            ..limits
        };
        assert_eq!(encoding_character_count("中😀a", unbounded).unwrap(), 3);
    }
}
