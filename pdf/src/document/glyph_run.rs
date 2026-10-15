//! Explicit PDF-code runs, not shaping, font fallback, or a GID encoder.
use super::PdfDocument;
use crate::{
    budget::{BudgetedBytes, OperationBudget},
    error::{PdfError, PdfResult},
    font::{
        GlyphDesc,
        pdf_font::{Font, FontProgramSource},
    },
    geom::matrix::Matrix,
    page::content_builder::ContentBuilder,
};
use std::io::{Read, Seek, Write};

/// Exactly one PDF character code, not a UTF-8 string or a font-program GID.
#[derive(Debug, Clone, Copy)]
pub struct PositionedGlyph<'a> {
    pub code: &'a [u8],
    /// Optional assertion against the existing code -> CID -> GID mapping.
    /// This never changes the target font's encoding.
    pub expected_gid: Option<u32>,
    /// Optional exact extraction assertion (including multi-scalar sequences).
    /// Does not add or rewrite ToUnicode. Missing mapping is not U+FFFD.
    pub unicode: Option<&'a str>,
    /// Text-origin offset before `GlyphRun::matrix`, in user units, not em.
    /// For vertical fonts this is the text origin; W2 origin offsets still apply.
    pub position: [f32; 2],
}

/// Independently positioned glyphs using an existing page font resource.
/// Each glyph receives its own Tm/Tj; implicit advances never position its neighbour.
/// Tr=0, Tc=Tw=Ts=0 and Tz=100; no shaping, font selection or font mutation.
#[derive(Debug)]
pub struct GlyphRun<'a> {
    pub font_name: &'a str,
    pub font_size: f32,
    /// Maps text-origin offsets into unrotated default page user space.
    pub matrix: [f32; 6],
    pub color: [f32; 3],
    pub glyphs: &'a [PositionedGlyph<'a>],
}

impl<R: Read + Seek> PdfDocument<R> {
    pub fn overlay_page_glyph_run(&self, page: u32, run: &GlyphRun<'_>) -> PdfResult<()> {
        self.overlay_page_glyph_run_with_budget(page, run, &self.ctx.limits().operation_budget())
    }

    /// Preparation uses one shared pool for font resolution and output bytes.
    /// Legacy overlay validation, document storage and native font allocations
    /// retain their existing accounting exclusions; this is not an RSS ceiling.
    /// Permission/signature checks precede preparation. Failure leaves no edit.
    pub fn overlay_page_glyph_run_with_budget(
        &self,
        page: u32,
        run: &GlyphRun<'_>,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        self.ctx.require_content_edit_permission()?;
        let limits = self.ctx.limits();
        if run.glyphs.is_empty()
            || !run.font_size.is_finite()
            || run.font_size <= 0.0
            || !run.matrix.iter().all(|v| v.is_finite())
            || !run
                .color
                .iter()
                .all(|v| v.is_finite() && (0.0..=1.0).contains(v))
        {
            return Err(PdfError::ObjectError(
                "invalid glyph run geometry/color or empty run".into(),
            ));
        }
        if run.glyphs.len() > limits.max_text_chars
            || run
                .glyphs
                .len()
                .checked_mul(2)
                .and_then(|v| v.checked_add(13))
                .is_none_or(|v| v > limits.max_content_operations)
        {
            return Err(PdfError::ResourceLimit(
                "glyph run characters/operations".into(),
            ));
        }
        let selection = ContentBuilder::font_selection_with_budget(
            run.font_name,
            run.font_size,
            limits,
            budget,
        )?;
        let page_ref = self.get_page(page)?;
        let font = page_ref.named_font_with_budget(run.font_name, budget)?;
        if matches!(&*font, Font::Type3(_)) {
            return Err(PdfError::UnsupportedFeature(
                "Type3 glyph-run writing".into(),
            ));
        }
        if matches!(
            font.font_program_source(),
            FontProgramSource::Missing | FontProgramSource::DeclaredSubstitute { .. }
        ) {
            return Err(PdfError::FontError(
                "glyph run requires an embedded or bundled font program".into(),
            ));
        }
        let face = crate::font::font_program::load_face_for_operation(
            font.fontfile()
                .ok_or_else(|| PdfError::FontError("glyph run font program missing".into()))?,
            Some(budget),
        )?;
        let matrix = Matrix::new(
            run.matrix[0],
            run.matrix[1],
            run.matrix[2],
            run.matrix[3],
            run.matrix[4],
            run.matrix[5],
        );
        let mut unicode_bytes = 0usize;
        for glyph in run.glyphs {
            if glyph.code.is_empty()
                || glyph.code.len() > 4
                || !glyph.position.iter().all(|v| v.is_finite())
            {
                return Err(PdfError::ObjectError(
                    "glyph requires one 1..4-byte PDF code and finite position".into(),
                ));
            }
            if let Some(unicode) = glyph.unicode {
                unicode_bytes = unicode_bytes
                    .checked_add(unicode.len())
                    .filter(|v| *v <= limits.max_decoded_bytes)
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("glyph run Unicode assertion bytes".into())
                    })?;
                if unicode.len() > limits.max_token_bytes {
                    return Err(PdfError::ResourceLimit(
                        "glyph Unicode assertion length".into(),
                    ));
                }
            }
            let count = font.visit_chars_limited(glyph.code, 1, |ch| {
                let Some(GlyphDesc::Gid(gid)) = font.get_glyph(&ch) else {
                    return Err(PdfError::FontError(
                        "glyph code has no numeric glyph mapping".into(),
                    ));
                };
                if gid == 0
                    || i64::from(gid) >= face.num_glyphs() as i64
                    || glyph.expected_gid.is_some_and(|expected| expected != gid)
                {
                    return Err(PdfError::FontError(
                        "glyph is .notdef or GID assertion differs".into(),
                    ));
                }
                if glyph
                    .unicode
                    .is_some_and(|expected| !font.matches_unicode(&ch, expected))
                {
                    return Err(PdfError::FontError(
                        "glyph Unicode assertion differs or mapping is missing".into(),
                    ));
                }
                if ![ch.width(), ch.origin_x(), ch.origin_y()]
                    .into_iter()
                    .all(|v| (v * run.font_size / 1000.0).is_finite())
                {
                    return Err(PdfError::ObjectError("glyph metrics overflow".into()));
                }
                Ok(())
            })?;
            if count != 1 {
                return Err(PdfError::FontError(
                    "glyph must encode exactly one character".into(),
                ));
            }
            let positioned = translated(&matrix, glyph.position)?;
            Matrix::new(run.font_size, 0., 0., run.font_size, 0., 0.).checked_mul(&positioned)?;
        }
        // Size pass uses a stack-only writer. Allocate once, with quota held,
        // then emit the exact same bytes (no quadratic buffer growth).
        let mut counter = Counter(0);
        emit(&mut counter, run, &matrix, selection.as_slice())?;
        if counter.0 > limits.max_raw_stream_bytes || counter.0 > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit("glyph run output bytes".into()));
        }
        let mut lease = budget.reserve(counter.0)?;
        let mut bytes = Vec::new();
        bytes
            .try_reserve_exact(counter.0)
            .map_err(|_| PdfError::ResourceLimit("glyph run output allocation".into()))?;
        lease.resize(bytes.capacity())?;
        emit(&mut bytes, run, &matrix, selection.as_slice())?;
        let bytes = BudgetedBytes::from_reserved_vec(bytes, lease)?;
        self.overlay_page_bytes(page, bytes.as_slice())
    }
}

fn translated(matrix: &Matrix, position: [f32; 2]) -> PdfResult<Matrix> {
    Matrix::new(1., 0., 0., 1., position[0], position[1]).checked_mul(matrix)
}

struct Counter(usize);
impl Write for Counter {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0 = self
            .0
            .checked_add(bytes.len())
            .ok_or_else(|| std::io::Error::other("glyph run length overflow"))?;
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

fn emit(
    out: &mut impl Write,
    run: &GlyphRun<'_>,
    matrix: &Matrix,
    selection: &[u8],
) -> PdfResult<()> {
    writeln!(
        out,
        "q\n{} {} {} rg\nBT\n0 Tc 0 Tw 100 Tz 0 Ts 0 Tr 0 TL",
        run.color[0], run.color[1], run.color[2]
    )?;
    out.write_all(selection)?;
    const HEX: &[u8] = b"0123456789ABCDEF";
    for glyph in run.glyphs {
        let m = translated(matrix, glyph.position)?;
        writeln!(
            out,
            "{} {} {} {} {} {} Tm",
            m.a(),
            m.b(),
            m.c(),
            m.d(),
            m.e(),
            m.f()
        )?;
        out.write_all(b"<")?;
        for byte in glyph.code {
            out.write_all(&[HEX[(byte >> 4) as usize], HEX[(byte & 15) as usize]])?;
        }
        out.write_all(b"> Tj\n")?;
    }
    out.write_all(b"ET\nQ\n")?;
    Ok(())
}
