use std::char;
use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::font::cid_font::CidFont;
use crate::font::cmap::Cmap;
use crate::objects::{PdfObject, pdf_dict::PdfDict};
use crate::pdf_context::PDFContext;

use super::{CharCode, GlyphDesc, WritingMode};

#[derive(Debug, Default, Clone)]
pub struct Type0 {
    base_font: Option<String>,
    encoding: Option<Cmap>,
    to_unicode: Option<Cmap>,
    descent_font: CidFont,
}

impl Type0 {
    pub fn try_new<R: Seek + Read>(dict: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let mut font = Type0::default();
        if let Some(base_font) = dict.get("BaseFont") {
            let base = base_font.as_name().unwrap();
            font.base_font = Some(base.name().to_string());
        }
        if let Some(encoding) = dict.get("Encoding") {
            let encoding_obj = ctx.resolve(encoding)?;
            match encoding_obj {
                PdfObject::PdfName(enc_name) => {
                    font.encoding = Some(Cmap::new_from_predefined(enc_name.name())?);
                }
                PdfObject::PdfStream(stream) => {
                    let cmap = Cmap::try_new(stream.decode_data(ctx)?)?;
                    font.encoding = Some(cmap);
                }
                _ => {
                    return Err(PdfError::FontError(format!(
                        "Type0 Encoidng need a PdfName or PdfStream got{:?}",
                        encoding_obj
                    )));
                }
            }
        } else {
            return Err(PdfError::FontError(
                "Type0 Encoding is required".to_string(),
            ));
        }

        let descendant = match dict.get("DescendantFonts") {
            Some(desobj) => {
                let des = ctx.resolve(desobj)?;
                let array = des.as_array().unwrap();
                array
            }

            None => {
                return Err(PdfError::FontError(
                    "type0 Descdantfonts is need".to_string(),
                ));
            }
        };

        let descendant_font = ctx.resolve(descendant.get(0).unwrap())?;
        let cid_font = CidFont::try_new(descendant_font.as_dict().unwrap(), ctx)?;
        font.descent_font = cid_font;
        if let Some(tu) = dict.get("ToUnicode") {
            let tu_obj = ctx.resolve(tu)?;
            let tu_cmap = Cmap::try_new(tu_obj.as_stream().unwrap().decode_data(ctx)?)?;
            font.to_unicode = Some(tu_cmap);
        }

        Ok(font)
    }

    fn char_width(&self, char: &CharCode) -> PdfResult<f32> {
        self.descent_font.char_width(&char.code)
    }

    pub fn unicode(&self, ch: &CharCode) -> PdfResult<String> {
        match &self.to_unicode {
            Some(cmap) => {
                if let Some(u) = cmap.unicode(ch) {
                    return Ok(u.to_string());
                } else {
                    let mut s = String::new();
                    s.push(char::REPLACEMENT_CHARACTER);
                    return Ok(s);
                }
            }
            None => {
                println!("Type0 font to_unicode is None");
                let mut s = String::new();
                s.push(char::REPLACEMENT_CHARACTER);
                return Ok(s);
                //return Err(PdfError::Font("Type0 Font to_unicode is None".to_string()));
            }
        }
    }

    pub fn text_widths(&self, chars: &[CharCode]) -> PdfResult<f32> {
        self.descent_font.text_widths(chars)
    }

    pub fn writting_mode(&self) -> WritingMode {
        if let Some(cmap) = &self.encoding {
            let wmode = cmap.wmode();
            match wmode {
                Some(1) => WritingMode::Vertical,
                Some(0) => WritingMode::Horizontal,
                _ => WritingMode::Horizontal,
            }
        } else {
            WritingMode::Horizontal
        }
    }

    pub fn chars(&self, codes: &[u8]) -> PdfResult<Vec<CharCode>> {
        match &self.encoding {
            Some(cmap) => {
                let mut chars = cmap.chars(codes);
                match self.writting_mode() {
                    WritingMode::Vertical => {
                        for char in chars.iter_mut() {
                            if let Some(w) = self.descent_font.vertical_metrics(&char.code()) {
                                char.set_with(w.0);
                                char.set_origin_x(w.1);
                                char.set_origin_y(w.2);
                            } else {
                                println!("width is None {:?},{:?}", self.writting_mode(), char);
                            }
                        }
                    }
                    WritingMode::Horizontal => {
                        for char in chars.iter_mut() {
                            char.width = self.char_width(char)?;
                        }
                    }
                }
                Ok(chars)
            }
            None => {
                return Err(PdfError::FontError("ENcoding is None in type0".to_string()));
            }
        }
    }
    pub fn base_font(&self) -> &str {
        match &self.base_font {
            Some(s) => s.as_str(),
            None => self.descent_font.base_font(),
        }
    }

    pub fn get_glyph(&self, code: &CharCode) -> Option<GlyphDesc> {
        Some(GlyphDesc::Gid(code.code()))
    }
    pub fn fontfile(&self) -> Option<&[u8]> {
        self.descent_font.fontfile()
    }
}
