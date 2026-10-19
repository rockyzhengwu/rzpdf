use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::objects::pdf_dict::PdfDict;
use crate::pdf_context::PDFContext;

#[derive(Debug, Default, Clone)]
pub struct Descriptor {
    flags: Option<u32>,
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
        Self::try_new_inner(dict, ctx, None).map(|(descriptor, _)| descriptor)
    }

    pub(crate) fn try_new_with_budget<R: Seek + Read>(
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let (descriptor, lease) = Self::try_new_inner(dict, ctx, Some(budget))?;
        Ok((descriptor, lease.unwrap()))
    }

    fn try_new_inner<R: Seek + Read>(
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(Self, Option<crate::budget::MemoryLease>)> {
        let mut lease = budget.map(|pool| pool.reserve(0)).transpose()?;
        let resolve = |value: &crate::objects::PdfObject| {
            if let Some(pool) = budget {
                ctx.resolve_owned_with_budget(value, pool)
                    .map(|(value, lease)| (value, Some(lease)))
            } else {
                ctx.resolve(value).map(|value| (value, None))
            }
        };
        let mut desc = Descriptor {
            flags: None,
            is_embed: false,
            ..Default::default()
        };
        if let Some(f) = dict.get("Flags") {
            let (resolved, _temporary) = resolve(f)?;
            let v = resolved
                .as_number()
                .ok_or_else(|| PdfError::FontError("Font Flags must be an integer".to_string()))?
                .as_u32_checked()
                .map_err(|_| {
                    PdfError::FontError("Font Flags must be a nonnegative u32 integer".to_string())
                })?;
            desc.flags = Some(v);
        }
        if let Some(i) = dict.get("ItalicAngle") {
            let (resolved, _temporary) = resolve(i)?;
            let v = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font ItalicAngle must be finite numeric".to_string())
            })?;
            desc.italic_angle = Some(v);
        }
        if let Some(a) = dict.get("Ascent") {
            let (resolved, _temporary) = resolve(a)?;
            let v = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font Ascent must be finite numeric".to_string())
            })?;
            desc.ascent = v;
        }
        if let Some(d) = dict.get("Descent") {
            let (resolved, _temporary) = resolve(d)?;
            let v = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font Descent must be finite numeric".to_string())
            })?;
            desc.descent = v;
        }
        if let Some(l) = dict.get("Leading") {
            let (resolved, _temporary) = resolve(l)?;
            let l = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font Leading must be finite numeric".to_string())
            })?;
            desc.leading = l;
        }
        if let Some(c) = dict.get("CapHeight") {
            let (resolved, _temporary) = resolve(c)?;
            let ch = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font CapHeight must be finite numeric".to_string())
            })?;
            desc.cap_height = ch;
        }
        if let Some(x) = dict.get("XHeight") {
            let (resolved, _temporary) = resolve(x)?;
            let x = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font XHeight must be finite numeric".to_string())
            })?;
            desc.x_height = x;
        }
        if let Some(sv) = dict.get("StemV") {
            let (resolved, _temporary) = resolve(sv)?;
            let s = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font StemV must be finite numeric".to_string())
            })?;
            desc.stem_v = s;
        }
        if let Some(sh) = dict.get("StemH") {
            let (resolved, _temporary) = resolve(sh)?;
            let s = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font StemH must be finite numeric".to_string())
            })?;
            desc.stem_h = s;
        }
        if let Some(av) = dict.get("AvgWidth") {
            let (resolved, _temporary) = resolve(av)?;
            let a = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font AvgWidth must be finite numeric".to_string())
            })?;
            desc.avg_width = a;
        }
        if let Some(ms) = dict.get("MissingWidth") {
            let (resolved, _temporary) = resolve(ms)?;
            let m = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font MissingWidth must be finite numeric".to_string())
            })?;
            desc.missing_width = m;
        }
        if let Some(ms) = dict.get("MaxWidth") {
            let (resolved, _temporary) = resolve(ms)?;
            let m = resolved.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                PdfError::FontError("Font MaxWidth must be finite numeric".to_string())
            })?;
            desc.max_width = m;
        }
        let f1 = dict.get("FontFile");
        let f2 = dict.get("FontFile2");
        let f3 = dict.get("FontFile3");
        if [f1, f2, f3].iter().filter(|file| file.is_some()).count() > 1 {
            return Err(PdfError::FontError(
                "FontDescriptor has multiple font programs".to_string(),
            ));
        }
        let ff = f1.or(f2).or(f3);
        if let Some(fo) = ff {
            let (resolved, _temporary) = resolve(fo)?;
            let o = resolved
                .as_stream()
                .ok_or_else(|| PdfError::FontError("FontFile must be a stream".to_string()))?;
            desc.fontfile = Some(if let Some(pool) = budget {
                let decoded = o.decode_data_with_budget(ctx, pool)?;
                let (bytes, mut decoded_lease) = decoded.into_reserved_parts();
                lease.as_mut().unwrap().absorb(&mut decoded_lease)?;
                bytes
            } else {
                o.decode_data(ctx)?
            });
            desc.is_embed = true;
        }

        Ok((desc, lease))
    }

    pub fn fontfile(&self) -> Option<&[u8]> {
        if let Some(v) = &self.fontfile {
            Some(v.as_slice())
        } else {
            None
        }
    }

    pub fn flags(&self) -> Option<&u32> {
        self.flags.as_ref()
    }

    pub(crate) fn missing_width(&self) -> f32 {
        self.missing_width
    }
    pub fn is_symbolic(&self) -> bool {
        if let Some(flag) = self.flags {
            (flag & 4) != 0
        } else {
            false
        }
    }

    pub fn is_embed(&self) -> bool {
        self.is_embed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::{collections::HashMap, io::Cursor};

    #[test]
    fn budget_font_program_owns_decoded_capacity_and_recovers_after_failure() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            objects::{
                PdfObject, pdf_name::PdfName, pdf_reference::PdfReference, pdf_stream::PdfStream,
            },
        };
        let doc = Document::new().unwrap();
        let mut stream_dict = PdfDict::default();
        stream_dict.insert(
            "Filter".into(),
            PdfObject::PdfName(PdfName::new("ASCIIHexDecode".into())),
        );
        let stream = PdfStream::new(stream_dict, b"414243>".to_vec());
        let id = doc.try_insert_object(PdfObject::PdfStream(stream)).unwrap();
        let mut dict = PdfDict::default();
        dict.insert(
            "FontFile2".into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        );
        let probe = OperationBudget::new(65536);
        let (descriptor, lease) =
            Descriptor::try_new_with_budget(&dict, doc.ctx(), &probe).unwrap();
        assert_eq!(descriptor.fontfile(), Some(&b"ABC"[..]));
        assert!(descriptor.is_embed());
        assert_eq!(
            lease.bytes(),
            descriptor.fontfile.as_ref().unwrap().capacity()
        );
        assert_eq!(probe.live_bytes(), lease.bytes());
        let peak = probe.peak_bytes();
        drop((descriptor, lease));
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(matches!(
            Descriptor::try_new_with_budget(&dict, doc.ctx(), &pool),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let result = Descriptor::try_new_with_budget(&dict, doc.ctx(), &pool).unwrap();
        drop(result);
        assert_eq!(pool.live_bytes(), 0);
        doc.update_object(id, PdfObject::PdfNull).unwrap();
        assert!(Descriptor::try_new_with_budget(&dict, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn descriptor_rejects_invalid_fields_and_preserves_flag_bits() {
        let parser = PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let ctx = PDFContext::try_new(
            parser,
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let parse = |input: &str| {
            let mut syntax = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(input.as_bytes().to_vec())).unwrap(),
            );
            let obj = syntax.get_object().unwrap();
            Descriptor::try_new(obj.as_dict().unwrap(), &ctx)
        };
        for field in [
            "ItalicAngle",
            "Ascent",
            "Descent",
            "Leading",
            "CapHeight",
            "XHeight",
            "StemV",
            "StemH",
            "AvgWidth",
            "MissingWidth",
            "MaxWidth",
        ] {
            assert!(parse(&format!("<< /{field} /bad >>")).is_err());
        }
        for input in [
            "<< /Flags /bad >>",
            "<< /Flags -1 >>",
            "<< /Flags 1.5 >>",
            "<< /FontFile 1 >>",
            "<< /FontFile2 1 >>",
            "<< /FontFile3 1 >>",
            "<< /FontFile 1 /FontFile2 2 >>",
        ] {
            assert!(parse(input).is_err(), "{input}");
        }
        let symbolic = parse("<< /Flags 262148 /Ascent 900 >>").unwrap();
        assert_eq!(symbolic.flags(), Some(&262148));
        assert!(symbolic.is_symbolic());
        assert_eq!(symbolic.ascent, 900.0);
        assert!(!parse("<< /Flags 32 >>").unwrap().is_symbolic());
    }
}
