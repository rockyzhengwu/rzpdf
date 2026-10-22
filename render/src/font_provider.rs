//! Explicit memory font registry. No system-font or network lookup is performed.
use crate::render_device::SubstituteFont;
use pdf::{
    error::{PdfError, PdfResult},
    font::resolver::{FontIdentity, FontMetadata, FontProvider, MAX_REGISTERED_FONTS},
};

#[derive(Clone, Default)]
pub struct MemoryFontProvider {
    entries: Vec<Entry>,
    bytes: usize,
}
#[derive(Clone)]
struct Entry {
    metadata: FontMetadata,
    identity: FontIdentity,
    program: SubstituteFont,
}
impl MemoryFontProvider {
    pub fn byte_len(&self) -> usize {
        self.bytes
    }
    /// Same bytes/index are idempotent only with identical selection metadata.
    /// Failed validation never replaces any previously registered face.
    pub fn register(
        &mut self,
        data: &[u8],
        index: u32,
        metadata: FontMetadata,
    ) -> PdfResult<FontIdentity> {
        metadata.validate()?;
        if data.is_empty() || data.len() > 32 * 1024 * 1024 {
            return Err(PdfError::ResourceLimit("font program bytes".into()));
        }
        let identity = FontIdentity::from_bytes(data, index);
        if let Some(old) = self.entries.iter().find(|e| e.identity == identity) {
            return if old.metadata == metadata {
                Ok(identity)
            } else {
                Err(PdfError::FontError(
                    "conflicting metadata for registered font identity".into(),
                ))
            };
        }
        if self.entries.len() >= MAX_REGISTERED_FONTS
            || self
                .bytes
                .checked_add(data.len())
                .is_none_or(|n| n > 64 * 1024 * 1024)
        {
            return Err(PdfError::ResourceLimit("registered fonts".into()));
        }
        let program = SubstituteFont::from_bytes(data, index as usize)?;
        self.entries
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("font registry allocation".into()))?;
        self.entries.push(Entry {
            metadata,
            identity,
            program,
        });
        self.bytes += data.len();
        Ok(identity)
    }
    pub(crate) fn face(&self, index: usize) -> skia_safe::Typeface {
        self.entries[index].program.face.clone()
    }
}
impl FontProvider for MemoryFontProvider {
    fn len(&self) -> usize {
        self.entries.len()
    }
    fn metadata(&self, index: usize) -> &FontMetadata {
        &self.entries[index].metadata
    }
    fn identity(&self, index: usize) -> FontIdentity {
        self.entries[index].identity
    }
    fn glyph(&self, index: usize, ch: char) -> Option<u16> {
        let gid = skia_safe::Font::new(self.face(index), 12.).unichar_to_glyph(ch as i32);
        (gid != 0).then_some(gid)
    }
}

/// One shared mapping/selection decision for actual painting and preflight.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphChoice {
    Registered(pdf::font::resolver::ResolvedGlyph),
    Legacy(u16),
    MissingUnicode,
    NeedsShaping,
    MissingGlyph(char),
}
pub fn choose_glyph(
    provider: Option<&MemoryFontProvider>,
    legacy: Option<&skia_safe::Typeface>,
    font: &pdf::font::pdf_font::Font,
    code: &pdf::font::CharCode,
) -> PdfResult<GlyphChoice> {
    choose_glyph_with_language(provider, legacy, font, code, None)
}

pub struct InspectedGlyph {
    pub choice: GlyphChoice,
    pub character: Option<char>,
}
pub fn choose_glyph_with_language(
    provider: Option<&MemoryFontProvider>,
    legacy: Option<&skia_safe::Typeface>,
    font: &pdf::font::pdf_font::Font,
    code: &pdf::font::CharCode,
    language: Option<&str>,
) -> PdfResult<GlyphChoice> {
    Ok(inspect_glyph(provider, legacy, font, code, language)?.choice)
}

pub fn inspect_glyph(
    provider: Option<&MemoryFontProvider>,
    legacy: Option<&skia_safe::Typeface>,
    font: &pdf::font::pdf_font::Font,
    code: &pdf::font::CharCode,
    language: Option<&str>,
) -> PdfResult<InspectedGlyph> {
    let Some(mapping) = font.unicode_mapping(code)? else {
        return Ok(InspectedGlyph {
            choice: GlyphChoice::MissingUnicode,
            character: None,
        });
    };
    let mut chars = mapping.chars();
    let Some(character) = chars.next() else {
        return Ok(InspectedGlyph {
            choice: GlyphChoice::MissingUnicode,
            character: None,
        });
    };
    if chars.next().is_some() {
        return Ok(InspectedGlyph {
            choice: GlyphChoice::NeedsShaping,
            character: None,
        });
    }
    if let Some(provider) = provider {
        let mut request = font.substitution_request(character);
        request.language = language;
        if let Some(glyph) = pdf::font::resolver::resolve(provider, request)? {
            return Ok(InspectedGlyph {
                choice: GlyphChoice::Registered(glyph),
                character: Some(character),
            });
        }
    }
    if let Some(face) = legacy {
        let gid = skia_safe::Font::new(face.clone(), 12.).unichar_to_glyph(character as i32);
        if gid != 0 {
            return Ok(InspectedGlyph {
                choice: GlyphChoice::Legacy(gid),
                character: Some(character),
            });
        }
    }
    Ok(InspectedGlyph {
        choice: GlyphChoice::MissingGlyph(character),
        character: Some(character),
    })
}
