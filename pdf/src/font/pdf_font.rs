use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::font::CharCode;
use crate::font::simple_font::SimpleFont;
use crate::font::type0::Type0;
use crate::objects::pdf_dict::PdfDict;
use crate::pdf_context::PDFContext;

use super::{GlyphDesc, WritingMode};

#[derive(Debug, Clone)]
pub enum Font {
    Simple(SimpleFont),
    Type0(Type0),
}

impl Font {
    pub fn try_new<R: Seek + Read>(dict: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let subtype = dict
            .get("Subtype")
            .ok_or(PdfError::FontError("Subtye is Null".to_string()))?
            .as_name()
            .unwrap();
        match subtype.name() {
            "Type0" => {
                let type0 = Type0::try_new(dict, ctx)?;
                return Ok(Font::Type0(type0));
            }
            "Type1" => {
                let typ1 = SimpleFont::try_new(dict, ctx)?;
                return Ok(Font::Simple(typ1));
            }
            "TrueType" => {
                let truetype = SimpleFont::try_new(dict, ctx)?;
                return Ok(Font::Simple(truetype));
            }
            "Type3" => {
                unimplemented!("Type3 is not unimplemented");
            }
            _ => {
                panic!("invalid font name");
            }
        }
    }

    pub fn text_widths(&self, chars: &[CharCode]) -> PdfResult<f32> {
        match self {
            Font::Simple(t) => t.text_widths(chars),
            Font::Type0(t) => t.text_widths(chars),
        }
    }

    pub fn unicode(&self, char: &CharCode) -> PdfResult<String> {
        match self {
            Font::Simple(t) => t.unicode(char),
            Font::Type0(t) => t.unicode(char),
        }
    }

    pub fn writting_mode(&self) -> WritingMode {
        match self {
            Font::Simple(_) => WritingMode::Horizontal,
            Font::Type0(tf) => tf.writting_mode(),
        }
    }

    pub fn chars(&self, codes: &[u8]) -> PdfResult<Vec<CharCode>> {
        match self {
            Font::Simple(s) => s.chars(codes),
            Font::Type0(ft) => ft.chars(codes),
        }
    }
    pub fn name(&self) -> &str {
        match self {
            Font::Simple(f) => f.base_font(),
            Font::Type0(t) => t.base_font(),
        }
    }

    pub fn get_glyph(&self, char: &CharCode) -> Option<GlyphDesc> {
        match self {
            Font::Simple(s) => s.get_glyph(char),
            Font::Type0(t0) => t0.get_glyph(char),
        }
    }

    pub fn fontfile(&self) -> Option<&[u8]> {
        match self {
            Font::Simple(s) => s.fontfile(),
            Font::Type0(t) => t.fontfile(),
        }
    }
}
