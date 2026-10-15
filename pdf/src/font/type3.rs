//! Bounded Type 3 fonts and glyph programs, shared by inspection and execution.
use std::io::{Read, Seek};

use crate::{
    budget::{MemoryLease, OperationBudget},
    document::identity::DocumentIdentity,
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict},
    page::content_program::{BudgetedContentProgram, ContentProgram},
    pdf_context::PDFContext,
};

use super::encoding::Encoding;

fn invalid(message: &str) -> PdfError {
    PdfError::FontError(format!("Type3: {message}"))
}

/// An owned, non-Clone font snapshot. Its allocations remain charged until drop.
/// Indirect CharProcs/resources belong to the original document and revision.
#[derive(Debug)]
pub struct Type3Font {
    matrix: [f32; 6],
    bbox: [f32; 4],
    first: u8,
    last: u8,
    widths: [f32; 256],
    encoding: Encoding,
    to_unicode: Option<super::cmap::Cmap>,
    char_procs: PdfObject,
    resources: Option<PdfObject>,
    document: DocumentIdentity,
    revision: u64,
    budget: OperationBudget,
    _lease: MemoryLease,
}

/// d0 has no declared bounding box (a coloured glyph); d1 declares one.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Type3GlyphMetrics {
    pub width: [f32; 2],
    pub bbox: Option<[f32; 4]>,
}

/// Parsed bytes/operations retain their reservations independently of the font.
/// Successful parsing is not successful execution or proof of d1 colour safety.
#[derive(Debug)]
pub struct Type3GlyphProgram {
    metrics: Type3GlyphMetrics,
    program: BudgetedContentProgram,
}

impl Type3GlyphProgram {
    pub fn metrics(&self) -> Type3GlyphMetrics {
        self.metrics
    }
    pub fn program(&self) -> &ContentProgram {
        self.program.program()
    }
}

impl Type3Font {
    pub fn try_new<R: Read + Seek>(dict: &PdfDict, ctx: &PDFContext<R>) -> PdfResult<Self> {
        Self::try_new_with_budget(dict, ctx, &ctx.limits().operation_budget())
    }

    /// Owned field/encoding copies, decoded glyph bytes and parsed operations
    /// share this pool. Source document caches and native allocations are excluded.
    pub fn try_new_with_budget<R: Read + Seek>(
        dict: &PdfDict,
        ctx: &PDFContext<R>,
        budget: &OperationBudget,
    ) -> PdfResult<Self> {
        let mut lease = budget.reserve(std::mem::size_of::<Self>())?;
        let field = |key: &str| -> PdfResult<(PdfObject, MemoryLease)> {
            ctx.resolve_owned_with_budget(
                dict.get(key)
                    .ok_or_else(|| invalid(&format!("missing {key}")))?,
                budget,
            )
        };
        {
            let (kind, _temporary) = field("Type")?;
            if kind.as_name().map(|name| name.name()) != Some("Font") {
                return Err(invalid("Type must be Font"));
            }
        }
        let (subtype, _temporary) = field("Subtype")?;
        if subtype.as_name().map(|name| name.name()) != Some("Type3") {
            return Err(invalid("Subtype must be Type3"));
        }
        drop((subtype, _temporary));
        let numbers = |key: &str, count: usize| -> PdfResult<[f32; 6]> {
            let (value, _temporary) = field(key)?;
            let array = value
                .as_array()
                .filter(|array| array.len() == count)
                .ok_or_else(|| invalid(&format!("invalid {key} array length")))?;
            // At most six entries; no returned dynamic allocation is needed.
            let mut result = [0.0; 6];
            for (index, entry) in array.into_iter().enumerate() {
                let (entry, _temporary) = ctx.resolve_owned_with_budget(entry, budget)?;
                result[index] = entry
                    .as_f32()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| invalid(&format!("{key} requires finite numbers")))?;
            }
            Ok(result)
        };
        let matrix = numbers("FontMatrix", 6)?;
        let bbox_values = numbers("FontBBox", 4)?;
        let bbox = [
            bbox_values[0],
            bbox_values[1],
            bbox_values[2],
            bbox_values[3],
        ];
        if bbox[0] > bbox[2] || bbox[1] > bbox[3] {
            return Err(invalid("reversed FontBBox"));
        }
        let code = |key: &str| -> PdfResult<u8> {
            let (value, _temporary) = field(key)?;
            value
                .as_number()
                .ok_or_else(|| invalid("character bound must be integer"))?
                .as_u32_checked()
                .and_then(|n| {
                    u8::try_from(n).map_err(|_| invalid("character bound outside 0..255"))
                })
        };
        let first = code("FirstChar")?;
        let last = code("LastChar")?;
        if first > last {
            return Err(invalid("reversed character bounds"));
        }
        let mut widths = [0.0; 256];
        {
            let (value, _temporary) = field("Widths")?;
            let array = value
                .as_array()
                .filter(|array| array.len() == usize::from(last - first) + 1)
                .ok_or_else(|| invalid("Widths length does not match character bounds"))?;
            for (index, entry) in array.into_iter().enumerate() {
                let (entry, _temporary) = ctx.resolve_owned_with_budget(entry, budget)?;
                widths[usize::from(first) + index] = entry
                    .as_f32()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| invalid("Widths requires finite numbers"))?;
            }
        }
        let encoding = {
            let (value, _temporary) = field("Encoding")?;
            match &value {
                PdfObject::PdfName(name) => Encoding::new_from_name(name.name())?,
                PdfObject::PdfDict(dict) => {
                    let (encoding, mut retained) =
                        Encoding::try_new_with_context_and_budget(dict, ctx, budget)?;
                    lease.absorb(&mut retained)?;
                    encoding
                }
                _ => return Err(invalid("Encoding must be a name or dictionary")),
            }
        };
        let to_unicode = if let Some(value) = dict.get("ToUnicode") {
            let (value, _temporary) = ctx.resolve_owned_with_budget(value, budget)?;
            let stream = value
                .as_stream()
                .ok_or_else(|| invalid("ToUnicode must be a stream"))?;
            let (cmap, mut retained) =
                super::cmap::Cmap::from_stream_with_budget(stream, ctx, budget)?;
            lease.absorb(&mut retained)?;
            Some(cmap)
        } else {
            None
        };
        let (char_procs, mut retained) = field("CharProcs")?;
        let procs = char_procs
            .as_dict()
            .ok_or_else(|| invalid("CharProcs must be a dictionary"))?;
        if procs.into_iter().count() > ctx.limits().max_objects {
            return Err(PdfError::ResourceLimit("Type3 CharProcs entries".into()));
        }
        lease.absorb(&mut retained)?;
        let resources = if let Some(value) = dict.get("Resources") {
            let (value, mut retained) = ctx.resolve_owned_with_budget(value, budget)?;
            match &value {
                PdfObject::PdfNull => None,
                PdfObject::PdfDict(_) => {
                    lease.absorb(&mut retained)?;
                    Some(value)
                }
                _ => return Err(invalid("Resources must be a dictionary")),
            }
        } else {
            None
        };
        Ok(Self {
            matrix,
            bbox,
            first,
            last,
            widths,
            encoding,
            to_unicode,
            char_procs,
            resources,
            document: ctx.identity(),
            revision: ctx.revision(),
            budget: budget.clone(),
            _lease: lease,
        })
    }

    pub fn font_matrix(&self) -> [f32; 6] {
        self.matrix
    }
    pub fn font_bbox(&self) -> [f32; 4] {
        self.bbox
    }
    pub fn code_range(&self) -> std::ops::RangeInclusive<u8> {
        self.first..=self.last
    }
    pub fn glyph_name(&self, code: u8) -> Option<&str> {
        self.encoding.get_glyph_name(code)
    }
    pub fn unicode_mapping(&self, code: u8) -> Option<String> {
        if let Some(cmap) = &self.to_unicode {
            return cmap
                .unicode(&super::CharCode::new(u32::from(code), 1, 0.))
                .map(str::to_owned);
        }
        self.encoding
            .unicode_mapping(code)
            .and_then(char::from_u32)
            .map(|c| c.to_string())
    }
    pub(crate) fn chars_limited(
        &self,
        bytes: &[u8],
        limit: usize,
    ) -> PdfResult<Vec<super::CharCode>> {
        if bytes.len() > limit {
            return Err(PdfError::ResourceLimit("Type3 character count".into()));
        }
        let mut result = Vec::new();
        result
            .try_reserve_exact(bytes.len())
            .map_err(|_| invalid("character allocation"))?;
        self.visit_chars_limited(bytes, limit, |ch| {
            result.push(ch);
            Ok(())
        })?;
        Ok(result)
    }
    pub(crate) fn visit_chars_limited(
        &self,
        bytes: &[u8],
        limit: usize,
        mut visitor: impl FnMut(super::CharCode) -> PdfResult<()>,
    ) -> PdfResult<usize> {
        if bytes.len() > limit {
            return Err(PdfError::ResourceLimit("Type3 character count".into()));
        }
        for &code in bytes {
            visitor(super::CharCode::new(u32::from(code), 1, self.width(code)?))?;
        }
        Ok(bytes.len())
    }
    pub fn resources(&self) -> Option<&PdfDict> {
        self.resources.as_ref().and_then(PdfObject::as_dict)
    }

    /// Widths is measured in glyph space, not universally in thousandths of em.
    pub fn width(&self, code: u8) -> PdfResult<f32> {
        if !self.code_range().contains(&code) {
            return Err(invalid("code outside Widths range"));
        }
        Ok(self.widths[usize::from(code)])
    }

    /// Horizontal text displacement: use only the transformed width's x
    /// component, even when FontMatrix rotates or shears the glyph shape.
    pub fn text_space_advance(&self, code: u8) -> PdfResult<[f32; 2]> {
        let width = self.width(code)?;
        let advance = [width * self.matrix[0], 0.0];
        if advance.iter().any(|v| !v.is_finite()) {
            return Err(invalid("non-finite transformed width"));
        }
        Ok(advance)
    }

    pub fn glyph_program<R: Read + Seek>(
        &self,
        code: u8,
        ctx: &PDFContext<R>,
    ) -> PdfResult<Type3GlyphProgram> {
        self.glyph_program_with_decode_check(code, ctx, |_| Ok(()))
    }

    pub(crate) fn glyph_program_with_decode_check<R: Read + Seek>(
        &self,
        code: u8,
        ctx: &PDFContext<R>,
        check_decoded: impl FnOnce(usize) -> PdfResult<()>,
    ) -> PdfResult<Type3GlyphProgram> {
        if self.document != ctx.identity() || self.revision != ctx.revision() {
            return Err(invalid(
                "font snapshot belongs to a different document or revision",
            ));
        }
        self.width(code)?;
        let name = self
            .glyph_name(code)
            .ok_or_else(|| invalid("code has no glyph-name mapping"))?;
        let object = self
            .char_procs
            .as_dict()
            .unwrap()
            .get(name)
            .ok_or_else(|| invalid("mapped CharProc is missing"))?;
        let (object, _temporary) = ctx.resolve_owned_with_budget(object, &self.budget)?;
        let stream = object
            .as_stream()
            .ok_or_else(|| invalid("CharProc must be a stream"))?;
        let decoded = stream.decode_data_with_budget(ctx, &self.budget)?;
        check_decoded(decoded.as_slice().len())?;
        let program =
            ContentProgram::from_decoded_bytes_with_budget(decoded, ctx.options(), &self.budget)?;
        let operations = program.program().operations();
        let first = &operations
            .first()
            .ok_or_else(|| invalid("empty CharProc"))?
            .operator;
        let bbox = match (first.name(), first.num_operands()) {
            ("d0", 2) => None,
            ("d1", 6) => {
                let bbox = [
                    first.get_as_f32(2)?,
                    first.get_as_f32(3)?,
                    first.get_as_f32(4)?,
                    first.get_as_f32(5)?,
                ];
                if bbox[0] > bbox[2] || bbox[1] > bbox[3] {
                    return Err(invalid("reversed d1 bounding box"));
                }
                Some(bbox)
            }
            _ => {
                return Err(invalid(
                    "CharProc must begin with d0 or d1 and exact operands",
                ));
            }
        };
        let width = [first.get_as_f32(0)?, first.get_as_f32(1)?];
        if width[1] != 0.0 {
            return Err(invalid("d0/d1 vertical width must be zero"));
        }
        if operations
            .iter()
            .skip(1)
            .any(|op| matches!(op.operator.name(), "d0" | "d1"))
        {
            return Err(invalid("duplicate glyph metrics operator"));
        }
        Ok(Type3GlyphProgram {
            metrics: Type3GlyphMetrics { width, bbox },
            program,
        })
    }
}
