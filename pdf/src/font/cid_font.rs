use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::font::CharCode;
use crate::font::descriptor::Descriptor;
use crate::font::font_program::FontProgram;
use crate::objects::{PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict};
use crate::pdf_context::PDFContext;

#[derive(Debug, Default, Clone)]
pub struct CidFont {
    sub_type: String,
    base_font: String,
    w2: Option<HashMap<u32, (f32, f32, f32)>>,
    w: Option<HashMap<u32, f32>>,
    dw: Option<f32>,
    dw2: Option<(f32, f32)>,
    descriptor: Option<Descriptor>,
    program: Option<FontProgram>,
}

impl CidFont {
    pub fn base_font(&self) -> &str {
        self.base_font.as_str()
    }
    pub fn try_new<R: Seek + Read>(dict: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let mut font = CidFont::default();
        let sub_type = dict
            .get("Subtype")
            .ok_or(PdfError::FontError("CidFont Subtype is need".to_string()))?;
        let sub_type = sub_type.as_name().unwrap().name();
        let base_font = dict
            .get("BaseFont")
            .ok_or(PdfError::FontError("CidFont Basefont is need".to_string()))?
            .as_name()
            .unwrap()
            .name();
        font.sub_type = sub_type.to_string();
        font.base_font = base_font.to_string();
        if let Some(dw) = dict.get("DW") {
            let d = dw.as_f32().unwrap();
            font.dw = Some(d);
        }
        if let Some(dw2) = dict.get("DW2") {
            let dw2 = dw2.as_array().ok_or(PdfError::FontError(
                "Dw2 for CidFont is not an array".to_string(),
            ))?;
            let v = dw2
                .get(0)
                .ok_or(PdfError::FontError("Dw2 elemnt error".to_string()))?
                .as_f32()
                .unwrap();
            let w1 = dw2
                .get(1)
                .ok_or(PdfError::FontError("Dw2 element error".to_string()))?
                .as_f32()
                .unwrap();
            font.dw2 = Some((v, w1));
        }
        if let Some(w) = dict.get("W") {
            let wa = ctx.resolve(w)?.as_array().unwrap();
            let widths = load_widths(wa, ctx)?;
            font.w = Some(widths);
        }

        if let Some(w2) = dict.get("W2") {
            let wa = ctx.resolve(w2)?.as_array().unwrap();
            let w2v = load_widths_vertical(wa, ctx)?;
            font.w2 = Some(w2v);
        }
        if let Some(descriptor) = dict.get("FontDescriptor") {
            let descriptor_dict = ctx.resolve(descriptor)?.as_dict().unwrap();
            let descriptor = Descriptor::try_new(descriptor_dict, ctx)?;
            font.descriptor = Some(descriptor);
        }
        // TODO load font program

        Ok(font)
    }

    pub fn vertical_metrics(&self, code: &u32) -> Option<(f32, f32, f32)> {
        match &self.w2 {
            Some(w) => w.get(code).map(|x| x.to_owned()),
            None => match self.dw2 {
                Some((vy, h)) => {
                    let w0 = self.char_width(code).unwrap_or(0.0);
                    Some((h, w0 / 2.0, vy))
                }
                None => None,
            },
        }
    }

    pub fn char_width(&self, code: &u32) -> PdfResult<f32> {
        if let Some(wd) = &self.w {
            if let Some(w) = wd.get(code) {
                return Ok(w.to_owned());
            }
            if let Some(w) = self.dw {
                return Ok(w as f32);
            }
            return Err(PdfError::FontError(format!(
                "Type0 font char width is None: {:?}",
                code
            )));
        } else {
            // TODO
            if let Some(w) = self.dw {
                return Ok(w as f32);
            } else {
                return Err(PdfError::FontError(format!(
                    "Type0 font char width is None: {:?}",
                    code
                )));
            }
        }
    }

    pub fn text_widths(&self, chars: &[CharCode]) -> PdfResult<f32> {
        let mut total_widths = 0.0;
        if let Some(wd) = &self.w {
            for c in chars {
                total_widths += wd.get(&c.code).unwrap()
            }
        }
        Ok(total_widths)
    }
    pub fn fontfile(&self) -> Option<&[u8]> {
        match &self.descriptor {
            Some(desc) => desc.fontfile(),
            None => None,
        }
    }
}
fn load_widths_vertical<R: Seek + Read>(
    w: &PdfArray,
    ctx: &PDFContext<R>,
) -> PdfResult<HashMap<u32, (f32, f32, f32)>> {
    let mut res = HashMap::new();
    let n = w.len();
    let mut i = 0;
    while i < n {
        let obj1 = w.get(i).unwrap().as_u32().unwrap();
        let obj2 = w.get(i + 1).unwrap();
        match obj2 {
            PdfObject::PdfArray(arr) => {
                let wn = arr.len();
                let mut k = 0;
                let mut k = 0;
                let mut key = obj1;
                while k < wn {
                    let w1 = arr.get(k).unwrap().as_f32().unwrap();
                    let vx = arr.get(k + 1).unwrap().as_f32().unwrap();
                    let vy = arr.get(k + 2).unwrap().as_f32().unwrap();
                    k += 3;
                    res.insert(key, (w1, vx, vy));
                    key += 1;
                }
                i += 2;
            }
            PdfObject::PdfNumber(end) => {
                let start = obj1;
                let end = end.get_u32();
                let w1 = w.get(i + 2).unwrap().as_f32().unwrap();
                let vx = w.get(i + 3).unwrap().as_f32().unwrap();
                let vy = w.get(i + 4).unwrap().as_f32().unwrap();
                for key in start..=end {
                    res.insert(key, (w1, vx, vy));
                }
                i += 5;
            }
            _ => return Err(PdfError::FontError("Dw2 format error".to_string())),
        }
    }

    Ok(res)
}

fn load_widths<R: Seek + Read>(w: &PdfArray, ctx: &PDFContext<R>) -> PdfResult<HashMap<u32, f32>> {
    let mut widths = HashMap::new();
    let n = w.len();
    let mut i = 0;
    while i < n {
        let obj1 = w.get(i).unwrap().as_u32().unwrap();
        let obj2 = w.get(i + 1).unwrap();
        match obj2 {
            PdfObject::PdfArray(arr) => {
                let mut start = obj1;
                for a in arr.into_iter() {
                    let aw = a.as_f32().unwrap();
                    widths.insert(start, aw);
                    start += 1;
                }
                i += 2;
            }
            PdfObject::PdfNumber(vn) => {
                let aw = w.get(i + 2).unwrap().as_f32().unwrap();
                let end = vn.get_u32();
                for k in obj1..end {
                    widths.insert(k, aw);
                }
                i += 3;
            }
            _ => {
                return Err(PdfError::FontError(format!(
                    "CidFont w need PdfNumber or PdfArray got:{:?}",
                    obj2
                )));
            }
        }
    }
    Ok(widths)
}
