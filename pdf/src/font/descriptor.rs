use std::io::{Read, Seek};

use crate::error::PdfResult;
use crate::objects::pdf_dict::PdfDict;
use crate::pdf_context::PDFContext;

#[derive(Debug, Default, Clone)]
pub struct Descriptor {
    flags: Option<u8>,
    italic_angle: Option<f32>,
    ascent: f32,
    descent: f32,
    leading: f32,
    cap_height: f32,
    x_height: f32,
    stem_v: f32,
    stem_h: f32,
    avg_width: f32,
    missing_width: f32,
    max_width: f32,
    fontfile: Option<Vec<u8>>,
    is_embed: bool,
}

impl Descriptor {
    pub fn try_new<R: Seek + Read>(dict: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let mut desc = Descriptor {
            flags: None,
            is_embed: false,
            ..Default::default()
        };
        if let Some(f) = dict.get("Flags") {
            let v = f.as_u8().unwrap();
            desc.flags = Some(v);
        }
        if let Some(i) = dict.get("ItalicAngle") {
            let v = i.as_f32().unwrap();
            desc.italic_angle = Some(v);
        }
        if let Some(a) = dict.get("Ascent") {
            let v = a.as_f32().unwrap();
            desc.ascent = v;
        }
        if let Some(d) = dict.get("Descent") {
            let v = d.as_f32().unwrap();
            desc.descent = v;
        }
        if let Some(l) = dict.get("Leading") {
            let l = l.as_f32().unwrap();
            desc.leading = l;
        }
        if let Some(c) = dict.get("CapHeight") {
            let ch = c.as_f32().unwrap();
            desc.cap_height = ch;
        }
        if let Some(x) = dict.get("XHeight") {
            let x = x.as_f32().unwrap();
            desc.x_height = x;
        }
        if let Some(sv) = dict.get("StemV") {
            let s = sv.as_f32().unwrap();
            desc.stem_v = s;
        }
        if let Some(sh) = dict.get("StemH") {
            let s = sh.as_f32().unwrap();
            desc.stem_h = s;
        }
        if let Some(av) = dict.get("AvgWidth") {
            let a = av.as_f32().unwrap();
            desc.avg_width = a;
        }
        if let Some(ms) = dict.get("MissingWidth") {
            let m = ms.as_f32().unwrap();
            desc.missing_width = m;
        }
        if let Some(ms) = dict.get("MaxWidth") {
            let m = ms.as_f32().unwrap();
            desc.max_width = m;
        }
        let f1 = dict.get("FontFile");
        let f2 = dict.get("FontFile2");
        let f3 = dict.get("FontFile3");
        let ff = f1.or(f2).or(f3);
        if let Some(fo) = ff {
            let o = ctx.resolve(fo)?.as_stream().unwrap();
            desc.fontfile = Some(o.decode_data(ctx)?);
            desc.is_embed = true;
        }

        Ok(desc)
    }

    pub fn fontfile(&self) -> Option<&[u8]> {
        if let Some(v) = &self.fontfile {
            Some(v.as_slice())
        } else {
            None
        }
    }

    pub fn flags(&self) -> Option<&u8> {
        self.flags.as_ref()
    }
    pub fn is_symbolic(&self) -> bool {
        if let Some(flag) = self.flags {
            (flag & 4) == 0
        } else {
            false
        }
    }

    pub fn is_embed(&self) -> bool {
        self.is_embed
    }
}
