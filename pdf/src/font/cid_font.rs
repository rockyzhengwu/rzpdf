#[cfg(test)]
use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::font::CharCode;
use crate::font::cmap::MappingTable;
use crate::font::descriptor::Descriptor;
use crate::objects::{PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict};
use crate::pdf_context::PDFContext;

#[derive(Debug, Default, Clone)]
pub struct CidFont {
    sub_type: String,
    base_font: String,
    w2: Option<MappingTable<(f32, f32, f32)>>,
    w: Option<MappingTable<f32>>,
    dw: Option<f32>,
    dw2: Option<(f32, f32)>,
    descriptor: Option<Descriptor>,
    cid_to_gid: Option<Vec<u8>>,
    cff_cid_to_gid: std::collections::HashMap<u32, u32>,
}

impl CidFont {
    pub(crate) fn cache_bytes(&self) -> usize {
        self.sub_type
            .capacity()
            .saturating_add(self.base_font.capacity())
            .saturating_add(self.w.as_ref().map_or(0, MappingTable::storage_bytes))
            .saturating_add(self.w2.as_ref().map_or(0, MappingTable::storage_bytes))
            .saturating_add(self.descriptor.as_ref().map_or(0, Descriptor::cache_bytes))
            .saturating_add(self.cid_to_gid.as_ref().map_or(0, Vec::capacity))
            .saturating_add(
                self.cff_cid_to_gid
                    .capacity()
                    .saturating_mul(24)
                    .saturating_mul(2),
            )
    }

    pub(crate) fn descriptor_flags(&self) -> Option<u32> {
        self.descriptor.as_ref().and_then(|d| d.flags().copied())
    }

    /// TrueType glyph selection; CID-keyed CFF requires a separate charset lookup.
    pub fn glyph_id(&self, cid: u32) -> Option<u32> {
        if self.sub_type == "CIDFontType0" {
            return self.cff_cid_to_gid.get(&cid).copied();
        }
        if self.sub_type != "CIDFontType2" {
            return None;
        }
        match &self.cid_to_gid {
            None => Some(cid),
            Some(bytes) => {
                let entry = usize::try_from(cid).ok()?.checked_mul(2)?;
                Some(
                    bytes
                        .get(entry..entry.checked_add(2)?)
                        .map_or(0, |pair| u16::from_be_bytes([pair[0], pair[1]]) as u32),
                )
            }
        }
    }

    pub(crate) fn unicode_fallback(
        &self,
        limits: crate::parser::options::Limits,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(Vec<Option<String>>, Option<crate::budget::MemoryLease>)> {
        if self.sub_type != "CIDFontType2" {
            return Ok((Vec::new(), None));
        }
        let Some(data) = self.fontfile() else {
            return Ok((Vec::new(), None));
        };
        let face = match super::font_program::load_face_for_operation(data, budget) {
            Ok(face) => face,
            Err(error @ PdfError::ResourceLimit(_)) => return Err(error),
            Err(_) => return Ok((Vec::new(), None)),
        };
        super::font_program::glyph_unicode_map(&face, limits, budget)
    }

    pub fn base_font(&self) -> &str {
        self.base_font.as_str()
    }
    pub fn try_new<R: Seek + Read>(dict: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        Self::try_new_inner(dict, ctx, None).map(|(font, _)| font)
    }

    pub(crate) fn try_new_with_budget<R: Seek + Read>(
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let (font, lease) = Self::try_new_inner(dict, ctx, Some(budget))?;
        Ok((font, lease.unwrap()))
    }

    fn try_new_inner<R: Seek + Read>(
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<(Self, Option<crate::budget::MemoryLease>)> {
        let mut lease = budget.map(|pool| pool.reserve(0)).transpose()?;
        let resolve = |value: &PdfObject| {
            if let Some(pool) = budget {
                ctx.resolve_owned_with_budget(value, pool)
                    .map(|(value, lease)| (value, Some(lease)))
            } else {
                ctx.resolve(value).map(|value| (value, None))
            }
        };
        let mut font = CidFont::default();
        let sub_type = dict
            .get("Subtype")
            .ok_or(PdfError::FontError("CidFont Subtype is need".to_string()))?;
        let (sub_type_object, _subtype_temporary) = resolve(sub_type)?;
        let sub_type = sub_type_object
            .as_name()
            .ok_or_else(|| PdfError::FontError("CIDFont Subtype must be a name".to_string()))?
            .name();
        if !matches!(sub_type, "CIDFontType0" | "CIDFontType2") {
            return Err(PdfError::UnsupportedFeature(format!(
                "CIDFont subtype {sub_type}"
            )));
        }
        let base_font = dict
            .get("BaseFont")
            .ok_or(PdfError::FontError("CidFont Basefont is need".to_string()))?;
        let (base_font_object, _base_temporary) = resolve(base_font)?;
        let base_font = base_font_object
            .as_name()
            .ok_or_else(|| PdfError::FontError("CIDFont BaseFont must be a name".to_string()))?
            .name();
        if let Some(pool) = budget {
            let (name, mut name_lease) = crate::budget::copy_string(sub_type, pool)?;
            lease.as_mut().unwrap().absorb(&mut name_lease)?;
            font.sub_type = name;
            let (name, mut name_lease) = crate::budget::copy_string(base_font, pool)?;
            lease.as_mut().unwrap().absorb(&mut name_lease)?;
            font.base_font = name;
        } else {
            font.sub_type = sub_type.to_string();
            font.base_font = base_font.to_string();
        }
        // CIDToGIDMap defines TrueType glyph indices only. Some producers also
        // emit /Identity on CIDFontType0 OpenType/CFF fonts. This redundant entry
        // must not reject the font or replace its embedded CFF charset mapping.
        if sub_type == "CIDFontType2"
            && let Some(mapping) = dict.get("CIDToGIDMap")
        {
            let (mapping, _temporary) = resolve(mapping)?;
            match mapping {
                PdfObject::PdfName(name) if name.name() == "Identity" => {}
                PdfObject::PdfStream(stream) => {
                    let (bytes, mut decoded_lease) = if let Some(pool) = budget {
                        let (bytes, lease) = stream
                            .decode_data_with_budget(ctx, pool)?
                            .into_reserved_parts();
                        (bytes, Some(lease))
                    } else {
                        (stream.decode_data(ctx)?, None)
                    };
                    if bytes.len() % 2 != 0 {
                        return Err(PdfError::FontError(
                            "CIDToGIDMap must contain two-byte entries".into(),
                        ));
                    }
                    if bytes.len() / 2 > ctx.limits().max_objects {
                        return Err(PdfError::ResourceLimit("CIDToGIDMap entries".into()));
                    }
                    if let Some(decoded) = &mut decoded_lease {
                        lease.as_mut().unwrap().absorb(decoded)?;
                    }
                    font.cid_to_gid = Some(bytes);
                }
                _ => {
                    return Err(PdfError::FontError(
                        "CIDToGIDMap must be Identity or a stream".into(),
                    ));
                }
            }
        }
        if let Some(dw) = dict.get("DW") {
            let (dw, _temporary) = resolve(dw)?;
            let d = dw.as_f32().filter(|n| n.is_finite()).ok_or_else(|| {
                PdfError::FontError("CIDFont DW must be finite numeric".to_string())
            })?;
            font.dw = Some(d);
        }
        if let Some(dw2) = dict.get("DW2") {
            let (resolved, _temporary) = resolve(dw2)?;
            let dw2 = resolved.as_array().ok_or(PdfError::FontError(
                "Dw2 for CidFont is not an array".to_string(),
            ))?;
            let first = dw2
                .get(0)
                .ok_or(PdfError::FontError("Dw2 elemnt error".to_string()))?;
            let (first, _first_temporary) = resolve(first)?;
            let v = first.as_f32().filter(|n| n.is_finite()).ok_or_else(|| {
                PdfError::FontError("CIDFont DW2 must contain finite numbers".to_string())
            })?;
            let second = dw2
                .get(1)
                .ok_or(PdfError::FontError("Dw2 element error".to_string()))?;
            let (second, _second_temporary) = resolve(second)?;
            let w1 = second.as_f32().filter(|n| n.is_finite()).ok_or_else(|| {
                PdfError::FontError("CIDFont DW2 must contain finite numbers".to_string())
            })?;
            font.dw2 = Some((v, w1));
        }
        if let Some(w) = dict.get("W") {
            let (resolved, _temporary) = resolve(w)?;
            let wa = resolved
                .as_array()
                .ok_or_else(|| PdfError::FontError("CIDFont W must be an array".to_string()))?;
            let (widths, mut table) = load_metrics_inner(wa, ctx, |v: [f32; 1]| v[0], budget)?;
            if let Some(table) = &mut table {
                lease.as_mut().unwrap().absorb(table)?;
            }
            font.w = Some(widths);
        }

        if let Some(w2) = dict.get("W2") {
            let (resolved, _temporary) = resolve(w2)?;
            let wa = resolved
                .as_array()
                .ok_or_else(|| PdfError::FontError("CIDFont W2 must be an array".to_string()))?;
            let (w2v, mut table) =
                load_metrics_inner(wa, ctx, |v: [f32; 3]| (v[0], v[1], v[2]), budget)?;
            if let Some(table) = &mut table {
                lease.as_mut().unwrap().absorb(table)?;
            }
            font.w2 = Some(w2v);
        }
        if let Some(descriptor) = dict.get("FontDescriptor") {
            let (resolved, _temporary) = resolve(descriptor)?;
            let descriptor_dict = resolved.as_dict().ok_or_else(|| {
                PdfError::FontError("FontDescriptor must be a dictionary".to_string())
            })?;
            let descriptor = if let Some(pool) = budget {
                let (descriptor, mut descriptor_lease) =
                    Descriptor::try_new_with_budget(descriptor_dict, ctx, pool)?;
                lease.as_mut().unwrap().absorb(&mut descriptor_lease)?;
                descriptor
            } else {
                Descriptor::try_new(descriptor_dict, ctx)?
            };
            font.descriptor = Some(descriptor);
        }
        if sub_type == "CIDFontType0"
            && let Some(bytes) = font.fontfile()
        {
            let face = super::font_program::load_face_for_operation(bytes, budget)?;
            let count = usize::try_from(face.num_glyphs())
                .map_err(|_| PdfError::FontError("invalid CFF glyph count".into()))?;
            if count > ctx.limits().max_objects {
                return Err(PdfError::ResourceLimit("CFF charset entries".into()));
            }
            // FreeType exposes the CFF charset, which maps glyph indices to
            // CIDs (not Unicode). Invert it once for PDF glyph selection.
            let mut retained = budget
                .map(|pool| {
                    pool.reserve(
                        count
                            .checked_mul(32)
                            .ok_or_else(|| PdfError::ResourceLimit("CFF charset bytes".into()))?,
                    )
                })
                .transpose()?;
            let mut mappings = std::collections::HashMap::new();
            mappings
                .try_reserve(count)
                .map_err(|_| PdfError::ResourceLimit("CFF charset allocation".into()))?;
            if let Some(retained) = &mut retained {
                retained.resize(mappings.capacity().saturating_mul(16))?;
                lease.as_mut().unwrap().absorb(retained)?;
            }
            for gid in 0..count as u32 {
                if let Some(cid) = super::font_program::cff_cid(&face, gid) {
                    mappings.entry(cid).or_insert(gid);
                }
            }
            font.cff_cid_to_gid = mappings;
        }

        Ok((font, lease))
    }

    pub fn vertical_metrics(&self, code: &u32) -> Option<(f32, f32, f32)> {
        if let Some(metrics) = self.w2.as_ref().and_then(|w| w.get(code)) {
            return Some(*metrics);
        }
        // ISO 32000-1, Table 117: absent W2 entries use DW2, default [880 -1000].
        let (vy, h) = self.dw2.unwrap_or((880.0, -1000.0));
        Some((h, self.char_width(code).ok()? / 2.0, vy))
    }

    pub fn char_width(&self, code: &u32) -> PdfResult<f32> {
        if let Some(wd) = &self.w {
            if let Some(w) = wd.get(code) {
                return Ok(w.to_owned());
            }
            if let Some(w) = self.dw {
                return Ok(w);
            }
            Ok(1000.0)
        } else {
            // TODO
            if let Some(w) = self.dw {
                Ok(w)
            } else {
                Ok(1000.0)
            }
        }
    }

    pub fn text_widths(&self, chars: &[CharCode]) -> PdfResult<f32> {
        let mut total_widths = 0.0;
        for c in chars {
            total_widths += self.char_width(&c.code)?;
            if !total_widths.is_finite() {
                return Err(PdfError::FontError("Text width overflows".to_string()));
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
fn width_cid(obj: &PdfObject) -> PdfResult<u32> {
    obj.as_number()
        .ok_or_else(|| PdfError::FontError("Width CID must be an integer".to_string()))?
        .as_u32_checked()
        .map_err(|_| PdfError::FontError("Width CID must be a nonnegative u32 integer".to_string()))
}

#[cfg(test)]
fn load_metrics<R: Seek + Read, T, const N: usize>(
    w: &PdfArray,
    ctx: &PDFContext<R>,
    convert: impl Fn([f32; N]) -> T,
) -> PdfResult<MappingTable<T>> {
    load_metrics_inner(w, ctx, convert, None).map(|(table, _)| table)
}

fn load_metrics_inner<R: Seek + Read, T, const N: usize>(
    w: &PdfArray,
    ctx: &PDFContext<R>,
    convert: impl Fn([f32; N]) -> T,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<(MappingTable<T>, Option<crate::budget::MemoryLease>)> {
    let resolve = |value: &PdfObject| {
        if let Some(pool) = budget {
            ctx.resolve_owned_with_budget(value, pool)
                .map(|(value, lease)| (value, Some(lease)))
        } else {
            ctx.resolve(value).map(|value| (value, None))
        }
    };
    let mut table_lease = budget.map(|pool| pool.reserve(0)).transpose()?;
    let mut result = if budget.is_some() {
        MappingTable::new_budgeted()
    } else {
        MappingTable::default()
    };
    let mut remaining = ctx.limits().max_objects;
    let mut i = 0;
    while i < w.len() {
        let (start_object, _start_temporary) = resolve(
            w.get(i)
                .ok_or_else(|| PdfError::FontError("Missing width CID".to_string()))?,
        )?;
        let start = width_cid(&start_object)?;
        let (next, _next_temporary) = resolve(
            w.get(i + 1)
                .ok_or_else(|| PdfError::FontError("Truncated width entry".to_string()))?,
        )?;
        let (count, array, stride) = match &next {
            PdfObject::PdfArray(array) => {
                if array.len() % N != 0 {
                    return Err(PdfError::FontError(
                        "Incomplete vertical width triple".to_string(),
                    ));
                }
                (array.len() as u64 / N as u64, Some(array), 2)
            }
            PdfObject::PdfNumber(_) => {
                let end = width_cid(&next)?;
                if end < start {
                    return Err(PdfError::FontError("Reversed width range".to_string()));
                }
                (u64::from(end) - u64::from(start) + 1, None, N + 2)
            }
            _ => {
                return Err(PdfError::FontError(
                    "Width entry requires an array or range endpoint".to_string(),
                ));
            }
        };
        if count > remaining as u64 {
            return Err(PdfError::ResourceLimit("CIDFont width entries".to_string()));
        }
        if count > 0 && u64::from(start) + count - 1 > u64::from(u32::MAX) {
            return Err(PdfError::FontError("Width CID array overflows".to_string()));
        }
        remaining -= count as usize;
        let read_values = |offset: usize, source: &PdfArray| -> PdfResult<[f32; N]> {
            let mut values = [0.0; N];
            for (k, value) in values.iter_mut().enumerate() {
                let (object, _temporary) =
                    resolve(source.get(offset + k).ok_or_else(|| {
                        PdfError::FontError("Truncated width values".to_string())
                    })?)?;
                *value = object.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                    PdfError::FontError("Width values must be finite numbers".to_string())
                })?;
            }
            Ok(values)
        };
        let constant = if array.is_none() {
            Some(read_values(i + 2, w)?)
        } else {
            None
        };
        for offset in 0..count as usize {
            let values = if let Some(array) = array {
                read_values(offset * N, array)?
            } else {
                constant.ok_or_else(|| PdfError::FontError("Missing width values".to_string()))?
            };
            let code = start + offset as u32;
            if let Some(pool) = budget
                && result.get(&code).is_none()
            {
                result.reserve_with_budget(1, pool, &mut table_lease)?;
            }
            result.insert(code, convert(values));
        }
        i += stride;
    }
    Ok((result, table_lease))
}

#[cfg(test)]
fn load_widths_vertical<R: Seek + Read>(
    w: &PdfArray,
    ctx: &PDFContext<R>,
) -> PdfResult<MappingTable<(f32, f32, f32)>> {
    load_metrics(w, ctx, |v: [f32; 3]| (v[0], v[1], v[2]))
}

#[cfg(test)]
fn load_widths<R: Seek + Read>(w: &PdfArray, ctx: &PDFContext<R>) -> PdfResult<MappingTable<f32>> {
    load_metrics(w, ctx, |v: [f32; 1]| v[0])
}

#[cfg(test)]
mod tests {
    #[test]
    fn width_tables_are_retained_by_cid_font_and_match_ordinary_metrics() {
        use crate::{budget::OperationBudget, document::owned::Document};
        let doc = Document::new().unwrap();
        let source = b"<< /Subtype /CIDFontType2 /BaseFont /Widths /W [1 [500 600] 2 3 700] /W2 [1 3 -1000 250 880] >>";
        let object =
            SyntaxParser::new(StreamReader::try_new(Cursor::new(source.to_vec())).unwrap())
                .get_object()
                .unwrap();
        let dict = object.as_dict().unwrap();
        let ordinary = CidFont::try_new(dict, doc.ctx()).unwrap();
        let probe = OperationBudget::new(65536);
        let (font, lease) = CidFont::try_new_with_budget(dict, doc.ctx(), &probe).unwrap();
        assert_eq!(font.w, ordinary.w);
        assert_eq!(font.w2, ordinary.w2);
        assert_eq!(font.char_width(&1).unwrap(), 500.0);
        assert_eq!(font.char_width(&2).unwrap(), 700.0);
        assert_eq!(font.vertical_metrics(&3), Some((-1000.0, 250.0, 880.0)));
        let expected = font.sub_type.capacity()
            + font.base_font.capacity()
            + font.w.as_ref().unwrap().capacity() * std::mem::size_of::<(u32, f32)>()
            + font.w2.as_ref().unwrap().capacity() * std::mem::size_of::<(u32, (f32, f32, f32))>();
        assert_eq!(lease.bytes(), expected);
        assert_eq!(probe.live_bytes(), expected);
        let peak = probe.peak_bytes();
        drop((font, lease));
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(CidFont::try_new_with_budget(dict, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        drop(CidFont::try_new_with_budget(dict, doc.ctx(), &pool).unwrap());
        assert_eq!(pool.live_bytes(), 0);
        let mut invalid = dict.clone();
        invalid.insert("W2".into(), PdfObject::PdfNull);
        assert!(CidFont::try_new_with_budget(&invalid, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn width_item_budget_resolves_arrays_numbers_and_preserves_overrides() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            objects::{pdf_number::PdfNumber, pdf_reference::PdfReference},
        };
        let doc = Document::new().unwrap();
        let height = doc
            .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(-1000.0)))
            .unwrap();
        let values = doc
            .try_insert_object(PdfObject::PdfArray(PdfArray::from_items(vec![
                PdfObject::PdfReference(PdfReference::from_id(height)),
                PdfObject::PdfNumber(PdfNumber::new(250.0)),
                PdfObject::PdfNumber(PdfNumber::new(880.0)),
                PdfObject::PdfNumber(PdfNumber::new(-1100.0)),
                PdfObject::PdfNumber(PdfNumber::new(260.0)),
                PdfObject::PdfNumber(PdfNumber::new(900.0)),
            ])))
            .unwrap();
        let mut source = array("[1 null 2 3 -1200 270 900]");
        source.remove(1);
        source
            .insert(1, PdfObject::PdfReference(PdfReference::from_id(values)))
            .unwrap();
        let probe = OperationBudget::new(65536);
        let ordinary = load_widths_vertical(&source, doc.ctx()).unwrap();
        let (metrics, lease) = load_metrics_inner(
            &source,
            doc.ctx(),
            |v: [f32; 3]| (v[0], v[1], v[2]),
            Some(&probe),
        )
        .unwrap();
        assert_eq!(metrics, ordinary);
        assert_eq!(metrics.get(&1), Some(&(-1000.0, 250.0, 880.0)));
        assert_eq!(metrics.get(&2), Some(&(-1200.0, 270.0, 900.0)));
        assert_eq!(metrics.get(&3), Some(&(-1200.0, 270.0, 900.0)));
        let lease = lease.unwrap();
        assert_eq!(
            lease.bytes(),
            metrics.capacity() * std::mem::size_of::<(u32, (f32, f32, f32))>()
        );
        assert_eq!(probe.live_bytes(), lease.bytes());
        let peak = probe.peak_bytes();
        assert!(peak > 0);
        drop((metrics, lease));
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(load_metrics_inner(&source, doc.ctx(), |v: [f32; 3]| v, Some(&pool)).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        assert!(load_metrics_inner(&source, doc.ctx(), |v: [f32; 3]| v, Some(&pool)).is_ok());
        assert_eq!(pool.live_bytes(), 0);
        doc.update_object(height, PdfObject::PdfNull).unwrap();
        assert!(load_metrics_inner(&source, doc.ctx(), |v: [f32; 3]| v, Some(&pool)).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn default_width_references_are_resolved_and_budgeted_temporarily() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            objects::{pdf_name::PdfName, pdf_number::PdfNumber, pdf_reference::PdfReference},
        };
        let doc = Document::new().unwrap();
        let width = doc
            .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(600.0)))
            .unwrap();
        let height = doc
            .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(-1100.0)))
            .unwrap();
        let vertical = doc
            .try_insert_object(PdfObject::PdfArray(PdfArray::from_items(vec![
                PdfObject::PdfNumber(PdfNumber::new(900.0)),
                PdfObject::PdfReference(PdfReference::from_id(height)),
            ])))
            .unwrap();
        let mut dict = PdfDict::default();
        dict.insert(
            "Subtype".into(),
            PdfObject::PdfName(PdfName::new("CIDFontType2".into())),
        );
        dict.insert(
            "BaseFont".into(),
            PdfObject::PdfName(PdfName::new("Defaults".into())),
        );
        dict.insert(
            "DW".into(),
            PdfObject::PdfReference(PdfReference::from_id(width)),
        );
        dict.insert(
            "DW2".into(),
            PdfObject::PdfReference(PdfReference::from_id(vertical)),
        );
        let ordinary = CidFont::try_new(&dict, doc.ctx()).unwrap();
        assert_eq!(ordinary.char_width(&42).unwrap(), 600.0);
        assert_eq!(
            ordinary.vertical_metrics(&42),
            Some((-1100.0, 300.0, 900.0))
        );
        let probe = OperationBudget::new(65536);
        let (font, lease) = CidFont::try_new_with_budget(&dict, doc.ctx(), &probe).unwrap();
        assert_eq!(font.vertical_metrics(&42), ordinary.vertical_metrics(&42));
        assert_eq!(
            lease.bytes(),
            font.sub_type.capacity() + font.base_font.capacity()
        );
        assert_eq!(probe.live_bytes(), lease.bytes());
        let peak = probe.peak_bytes();
        assert!(peak > lease.bytes());
        drop((font, lease));
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(CidFont::try_new_with_budget(&dict, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        drop(CidFont::try_new_with_budget(&dict, doc.ctx(), &pool).unwrap());
        assert_eq!(pool.live_bytes(), 0);
        doc.update_object(height, PdfObject::PdfNull).unwrap();
        assert!(CidFont::try_new(&dict, doc.ctx()).is_err());
        assert!(CidFont::try_new_with_budget(&dict, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn budget_cid_names_mapping_and_descriptor_own_quota_until_release() {
        use crate::{
            budget::OperationBudget,
            document::owned::Document,
            objects::{pdf_name::PdfName, pdf_reference::PdfReference, pdf_stream::PdfStream},
        };
        let doc = Document::new().unwrap();
        let subtype = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("CIDFontType2".into())))
            .unwrap();
        let base = doc
            .try_insert_object(PdfObject::PdfName(PdfName::new("BudgetCID".into())))
            .unwrap();
        let mut dict = PdfDict::default();
        dict.insert(
            "Subtype".into(),
            PdfObject::PdfReference(PdfReference::from_id(subtype)),
        );
        dict.insert(
            "BaseFont".into(),
            PdfObject::PdfReference(PdfReference::from_id(base)),
        );
        dict.insert(
            "CIDToGIDMap".into(),
            PdfObject::PdfStream(PdfStream::new(PdfDict::default(), vec![0, 0, 0, 42])),
        );
        let mut descriptor = PdfDict::default();
        descriptor.insert(
            "FontFile2".into(),
            PdfObject::PdfStream(PdfStream::new(PdfDict::default(), b"ABC".to_vec())),
        );
        let descriptor = doc
            .try_insert_object(PdfObject::PdfDict(descriptor))
            .unwrap();
        dict.insert(
            "FontDescriptor".into(),
            PdfObject::PdfReference(PdfReference::from_id(descriptor)),
        );
        let probe = OperationBudget::new(65536);
        let (font, lease) = CidFont::try_new_with_budget(&dict, doc.ctx(), &probe).unwrap();
        assert_eq!(font.base_font(), "BudgetCID");
        assert_eq!(font.glyph_id(1), Some(42));
        assert_eq!(
            font.descriptor.as_ref().unwrap().fontfile(),
            Some(&b"ABC"[..])
        );
        assert_eq!(
            lease.bytes(),
            font.sub_type.capacity()
                + font.base_font.capacity()
                + font.cid_to_gid.as_ref().unwrap().capacity()
                + 3
        );
        assert_eq!(probe.live_bytes(), lease.bytes());
        let peak = probe.peak_bytes();
        drop((font, lease));
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(matches!(
            CidFont::try_new_with_budget(&dict, doc.ctx(), &pool),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        drop(CidFont::try_new_with_budget(&dict, doc.ctx(), &pool).unwrap());
        assert_eq!(pool.live_bytes(), 0);
        dict.insert(
            "CIDToGIDMap".into(),
            PdfObject::PdfStream(PdfStream::new(PdfDict::default(), vec![0, 0, 0])),
        );
        assert!(CidFont::try_new_with_budget(&dict, doc.ctx(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    use super::*;
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::io::Cursor;

    fn array(input: &str) -> PdfArray {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(input.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
            .into_array()
            .unwrap()
    }

    #[test]
    fn cid_to_gid_stream_and_identity_are_loaded() {
        let ctx = PDFContext::try_new(
            PdfParser::new(SyntaxParser::new(
                StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
            )),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let obj = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Subtype /CIDFontType2 /BaseFont /Test >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap();
        let mut dict = obj.as_dict().unwrap().clone();
        let identity = CidFont::try_new(&dict, &ctx).unwrap();
        assert_eq!(identity.glyph_id(7), Some(7));
        dict.try_insert(
            "CIDToGIDMap".into(),
            PdfObject::PdfStream(crate::objects::pdf_stream::PdfStream::new(
                PdfDict::default(),
                vec![0, 0, 1, 44],
            )),
        )
        .unwrap();
        let mapped = CidFont::try_new(&dict, &ctx).unwrap();
        assert_eq!(mapped.glyph_id(1), Some(300));
        assert_eq!(mapped.glyph_id(2), Some(0));
        assert_eq!(mapped.glyph_id(u32::MAX), Some(0));
        dict.try_insert(
            "CIDToGIDMap".into(),
            PdfObject::PdfStream(crate::objects::pdf_stream::PdfStream::new(
                PdfDict::default(),
                vec![0],
            )),
        )
        .unwrap();
        assert!(CidFont::try_new(&dict, &ctx).is_err());
    }

    #[test]
    fn width_arrays_validate_entries_and_preserve_endpoints() {
        let parser = PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let ctx = PDFContext::try_new(
            parser,
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let widths = load_widths(&array("[1 2 500 4 [600 700]]"), &ctx).unwrap();
        assert_eq!(widths.get(&2), Some(&500.0));
        assert_eq!(widths.get(&5), Some(&700.0));
        let vertical =
            load_widths_vertical(&array("[1 2 -1000 250 880 4 [-900 200 800]]"), &ctx).unwrap();
        assert_eq!(vertical.get(&2), Some(&(-1000.0, 250.0, 880.0)));
        assert_eq!(vertical.get(&4), Some(&(-900.0, 200.0, 800.0)));
        for input in [
            "[1]",
            "[1 2]",
            "[2 1 500]",
            "[1 [/bad]]",
            "[-1 [500]]",
            "[1.5 [500]]",
            "[4294967295 [500 600]]",
            "[0 4294967295 500]",
        ] {
            assert!(load_widths(&array(input), &ctx).is_err(), "{input}");
        }
        for input in ["[1 [500 600]]", "[1 2 500 600]", "[1 [500 /bad 700]]"] {
            assert!(
                load_widths_vertical(&array(input), &ctx).is_err(),
                "{input}"
            );
        }
    }

    #[test]
    fn vertical_metrics_fall_back_for_missing_w2_entries() {
        let mut font = CidFont::default();
        assert_eq!(font.vertical_metrics(&2), Some((-1000.0, 500.0, 880.0)));
        font.w = Some(HashMap::from([(2, 600.0)]).into());
        font.w2 = Some(HashMap::from([(1, (-1200.0, 200.0, 900.0))]).into());
        assert_eq!(font.vertical_metrics(&1), Some((-1200.0, 200.0, 900.0)));
        assert_eq!(font.vertical_metrics(&2), Some((-1000.0, 300.0, 880.0)));
        font.dw2 = Some((850.0, -950.0));
        assert_eq!(font.vertical_metrics(&2), Some((-950.0, 300.0, 850.0)));
    }

    #[test]
    fn text_widths_use_pdf_default_and_explicit_fallback() {
        let mut font = CidFont {
            w: Some(HashMap::from([(1, 500.0)]).into()),
            ..Default::default()
        };
        let chars = [CharCode::new(1, 1, 0.0), CharCode::new(2, 1, 0.0)];
        assert_eq!(font.text_widths(&chars).unwrap(), 1500.0);
        font.dw = Some(600.0);
        assert_eq!(font.text_widths(&chars).unwrap(), 1100.0);
    }
}

#[cfg(test)]
mod cff_charset_tests {
    use super::*;
    use crate::{
        document::Document,
        objects::{pdf_name::PdfName, pdf_stream::PdfStream},
    };
    #[test]
    fn cid_keyed_charset_is_not_an_identity_gid_mapping() {
        let doc = Document::new().unwrap();
        let mut font = PdfDict::default();
        font.insert(
            "Subtype".into(),
            PdfObject::PdfName(PdfName::new("CIDFontType0".into())),
        );
        font.insert(
            "BaseFont".into(),
            PdfObject::PdfName(PdfName::new("RzpdfSyntheticCID".into())),
        );
        let mut descriptor = PdfDict::default();
        descriptor.insert(
            "FontFile3".into(),
            PdfObject::PdfStream(PdfStream::new(
                Default::default(),
                include_bytes!("../../tests/fixtures/synthetic-cid.cff").to_vec(),
            )),
        );
        font.insert("FontDescriptor".into(), PdfObject::PdfDict(descriptor));
        let parsed = CidFont::try_new(&font, doc.ctx()).unwrap();
        assert_eq!(parsed.glyph_id(7), Some(1));
        assert_eq!(parsed.glyph_id(42), Some(2));
        assert_eq!(parsed.glyph_id(2), None);
        // WeasyPrint emits this redundant TrueType-only entry on CFF fonts.
        // CIDs 7 and 42 are deliberately different from glyph indices 1 and 2.
        font.insert(
            "CIDToGIDMap".into(),
            PdfObject::PdfName(PdfName::new("Identity".into())),
        );
        let identity = CidFont::try_new(&font, doc.ctx()).unwrap();
        assert_eq!(identity.glyph_id(7), Some(1));
        assert_eq!(identity.glyph_id(42), Some(2));
        assert_eq!(identity.glyph_id(2), None);
        let pool = crate::budget::OperationBudget::new(65536);
        let (parsed, lease) = CidFont::try_new_with_budget(&font, doc.ctx(), &pool).unwrap();
        assert_eq!(parsed.glyph_id(42), Some(2));
        assert_eq!(pool.live_bytes(), lease.bytes());
        drop(parsed);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        // An irrelevant stream mapping is not parsed as a TrueType map either.
        // A real CIDFontType2 still rejects odd-length mappings in its own tests.
        font.insert(
            "CIDToGIDMap".into(),
            PdfObject::PdfStream(PdfStream::new(Default::default(), vec![0])),
        );
        assert_eq!(
            CidFont::try_new(&font, doc.ctx()).unwrap().glyph_id(42),
            Some(2)
        );
        let (parsed, lease) = CidFont::try_new_with_budget(&font, doc.ctx(), &pool).unwrap();
        assert_eq!(parsed.glyph_id(42), Some(2));
        drop((parsed, lease));
        assert_eq!(pool.live_bytes(), 0);
    }
}
