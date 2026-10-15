//! Bounded inspection of Type 3 fonts and glyph programs.
//!
//! This model does not execute CharProcs or enable Type 3 page rendering.
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

    /// Transform the horizontal width vector without applying matrix translation.
    pub fn text_space_advance(&self, code: u8) -> PdfResult<[f32; 2]> {
        let width = self.width(code)?;
        let advance = [width * self.matrix[0], width * self.matrix[1]];
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
