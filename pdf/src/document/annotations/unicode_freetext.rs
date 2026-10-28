//! Explicit static TrueType normal appearances. No fallback, wrapping or shaping.
use super::{Annotation, AnnotationKind, TextAlignment};
use crate::{
    budget::OperationBudget,
    document::{
        PdfDocument, appearance::reference, embedded_font::TrueTypeEmbeddingOptions,
        identity::ObjectHandle,
    },
    error::{PdfError, PdfResult},
    font::{
        GlyphDesc,
        cid_encoding::UnicodeCidEncoding,
        pdf_font::Font,
        truetype::{tables::glyf::outline_bounds, true_type_program::TrueTypeProgram},
    },
    objects::{PdfObject, pdf_dict::PdfDict, pdf_string::PdfString},
};
use std::{
    collections::BTreeMap,
    fmt::Write,
    io::{Read, Seek},
};

/// Font bytes are borrowed for one operation; the resulting subset is owned by
/// the PDF. Display font registrations are not accepted as embedding authority.
pub struct UnicodeFreeTextFont<'a> {
    pub bytes: &'a [u8],
    pub embedding: TrueTypeEmbeddingOptions,
    pub omit_layout_and_auxiliary: bool,
}

/// Persistent, document-local information, not a cross-document write token.
#[derive(Debug, Clone)]
pub struct UnicodeFreeTextBinding {
    pub font: ObjectHandle,
    pub repertoire: String,
}

fn unsupported(message: &str) -> PdfError {
    PdfError::UnsupportedFeature(format!("Unicode FreeText: {message}"))
}
fn invalid(message: &str) -> PdfError {
    PdfError::DocumentError(format!("Unicode FreeText: {message}"))
}

impl<R: Read + Seek> PdfDocument<R> {
    pub fn add_unicode_freetext(
        &self,
        page: u32,
        annotation: &Annotation,
        font: UnicodeFreeTextFont<'_>,
    ) -> PdfResult<ObjectHandle> {
        self.add_unicode_freetext_with_budget(
            page,
            annotation,
            font,
            &self.ctx().operation_budget(),
        )
    }

    pub fn add_unicode_freetext_with_budget(
        &self,
        page: u32,
        annotation: &Annotation,
        font: UnicodeFreeTextFont<'_>,
        budget: &OperationBudget,
    ) -> PdfResult<ObjectHandle> {
        self.ctx().require_annotation_permission()?;
        self.edit_transaction_with_budget(budget, |doc| {
            doc.add_annotation_encoded(page, annotation, |doc, dict| {
                doc.encode_unicode_freetext(annotation, dict, Some(font), budget)
            })
        })
    }

    /// None reuses the current immutable repertoire. New characters require a
    /// fresh asset; no shared font dictionary or other appearance is modified.
    pub fn update_unicode_freetext(
        &self,
        page: u32,
        handle: &ObjectHandle,
        annotation: &Annotation,
        font: Option<UnicodeFreeTextFont<'_>>,
    ) -> PdfResult<()> {
        self.update_unicode_freetext_with_budget(
            page,
            handle,
            annotation,
            font,
            &self.ctx().operation_budget(),
        )
    }

    pub fn update_unicode_freetext_with_budget(
        &self,
        page: u32,
        handle: &ObjectHandle,
        annotation: &Annotation,
        font: Option<UnicodeFreeTextFont<'_>>,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        self.edit_transaction_with_budget(budget, |doc| {
            doc.update_annotation_encoded(page, handle, annotation, |doc, dict| {
                doc.encode_unicode_freetext(annotation, dict, font, budget)
            })
        })
    }

    pub fn unicode_freetext_binding(
        &self,
        handle: &ObjectHandle,
    ) -> PdfResult<Option<UnicodeFreeTextBinding>> {
        let object = self.resolve_handle(handle)?;
        let dict = object
            .as_dict()
            .ok_or_else(|| invalid("annotation dictionary expected"))?;
        self.unicode_binding(dict)
    }

    pub(super) fn unicode_binding(
        &self,
        annotation: &PdfDict,
    ) -> PdfResult<Option<UnicodeFreeTextBinding>> {
        let Some(value) = annotation.get("RzFreeText") else {
            return Ok(None);
        };
        let binding = value
            .as_dict()
            .ok_or_else(|| invalid("binding dictionary expected"))?;
        if binding
            .into_iter()
            .any(|(k, _)| !matches!(k.as_str(), "Version" | "Font" | "Repertoire"))
            || binding.get("Version").and_then(PdfObject::as_u32) != Some(1)
        {
            return Err(unsupported("unknown binding version or fields"));
        }
        let font = binding
            .get("Font")
            .and_then(PdfObject::as_reference)
            .ok_or_else(|| invalid("bound font reference expected"))?
            .id();
        let repertoire = binding
            .get("Repertoire")
            .and_then(PdfObject::as_string)
            .ok_or_else(|| invalid("bound repertoire expected"))?
            .text_content_with_limits(&self.ctx().limits())?;
        if repertoire.is_empty() || repertoire.chars().count() > self.ctx().limits().max_text_chars
        {
            return Err(invalid("invalid bound repertoire"));
        }
        let ap = annotation
            .get("AP")
            .ok_or_else(|| invalid("bound AP missing"))?;
        let ap = self
            .ctx()
            .resolve_owned(ap)?
            .into_dict()
            .ok_or_else(|| invalid("bound AP dictionary expected"))?;
        let normal = self.ctx().resolve_owned(
            ap.get("N")
                .ok_or_else(|| invalid("bound normal appearance missing"))?,
        )?;
        let normal = normal
            .as_stream()
            .ok_or_else(|| invalid("bound normal appearance must be stream"))?;
        let resources = self.ctx().resolve_owned(
            normal
                .dict()
                .get("Resources")
                .ok_or_else(|| invalid("bound appearance resources missing"))?,
        )?;
        let resources = resources
            .as_dict()
            .ok_or_else(|| invalid("bound resources dictionary expected"))?;
        let fonts = self.ctx().resolve_owned(
            resources
                .get("Font")
                .ok_or_else(|| invalid("bound font resources missing"))?,
        )?;
        if fonts
            .as_dict()
            .and_then(|f| f.get("F0"))
            .and_then(PdfObject::as_reference)
            .map(|r| r.id())
            != Some(font)
        {
            return Err(invalid("AP font differs from binding"));
        }
        Ok(Some(UnicodeFreeTextBinding {
            font: self.object_handle(font)?,
            repertoire,
        }))
    }

    fn encode_unicode_freetext(
        &self,
        annotation: &Annotation,
        dict: PdfDict,
        source: Option<UnicodeFreeTextFont<'_>>,
        budget: &OperationBudget,
    ) -> PdfResult<PdfDict> {
        let AnnotationKind::FreeText {
            font_size,
            alignment,
        } = annotation.kind
        else {
            return Err(invalid("FreeText kind required"));
        };
        let limits = self.ctx().limits();
        let count = annotation.contents.chars().count();
        if count > limits.max_text_chars || annotation.contents.len() > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit(
                "Unicode FreeText text bytes/characters".into(),
            ));
        }
        if annotation.contents.chars().any(|c| {
            c != '\n'
                && (matches!(c, '\u{3099}' | '\u{309a}')
                    || !(c == ' '
                        || c.is_ascii_graphic()
                        || ('\u{a0}'..='\u{2ff}').contains(&c)
                        || ('\u{20000}'..='\u{323af}').contains(&c)
                        || ('\u{3000}'..='\u{30ff}').contains(&c)
                        || ('\u{3400}'..='\u{9fff}').contains(&c)
                        || ('\u{ff01}'..='\u{ff60}').contains(&c)))
        }) {
            return Err(unsupported(
                "first profile accepts independent Latin/CJK glyphs; shaping/control sequences require another layout profile",
            ));
        }
        let workspace = count
            .checked_mul(256)
            .and_then(|n| n.checked_add(annotation.contents.len()))
            .ok_or_else(|| PdfError::ResourceLimit("Unicode FreeText workspace".into()))?;
        let _workspace = budget.reserve(workspace)?;
        let mut _embedded = None;
        let binding = if let Some(source) = source {
            let mut characters: Vec<_> =
                annotation.contents.chars().filter(|c| *c != '\n').collect();
            characters.sort_unstable();
            characters.dedup();
            let repertoire: String = characters.into_iter().collect();
            let repertoire = if repertoire.is_empty() {
                " ".to_string()
            } else {
                repertoire
            };
            let tag = self.resources().next_subset_tag_with_budget(budget)?;
            let font = self.resources().embed_truetype_subset_font_objects(
                None,
                source.bytes,
                &repertoire,
                source.embedding,
                tag,
                source.omit_layout_and_auxiliary,
                budget,
            )?;
            let binding = UnicodeFreeTextBinding {
                font: font.font.object_handle().clone(),
                repertoire,
            };
            _embedded = Some(font);
            binding
        } else {
            self.unicode_binding(&dict)?
                .ok_or_else(|| unsupported("font asset required for an unbound annotation"))?
        };
        let _binding_workspace = budget.reserve(
            binding
                .repertoire
                .chars()
                .count()
                .checked_mul(256)
                .ok_or_else(|| {
                    PdfError::ResourceLimit("Unicode FreeText binding workspace".into())
                })?,
        )?;
        let font_id = binding.font.object_id();
        let (object, _object_lease) = self
            .ctx()
            .resolve_owned_with_budget(&reference(font_id), budget)?;
        let font_dict = object
            .as_dict()
            .ok_or_else(|| invalid("bound font dictionary expected"))?;
        if font_dict
            .get("Subtype")
            .and_then(PdfObject::as_name)
            .is_none_or(|n| n.name() != "Type0")
            || font_dict
                .get("Encoding")
                .and_then(PdfObject::as_name)
                .is_none_or(|n| n.name() != "Identity-H")
        {
            return Err(unsupported(
                "bound font must be horizontal Identity-H Type0",
            ));
        }
        let (font, _font_lease) = Font::try_new_with_budget(font_dict, self.ctx(), budget)?;
        let data = font
            .fontfile()
            .ok_or_else(|| invalid("embedded TrueType program missing"))?;
        let program = TrueTypeProgram::try_new_with_limits(data, limits)?;
        // A saved binding is not embedding authority: recheck the actual font
        // after reopen, including when only already-mapped text is requested.
        program.validate_outline_embedding(true, true)?;
        let sfnt = crate::font::truetype::sfnt::Sfnt::try_new_with_limits(
            &mut crate::font::truetype::reader::TrueTypeReader::new(data),
            limits,
        )?;
        if ["fvar", "gvar", "CFF ", "CFF2"]
            .iter()
            .any(|tag| sfnt.get_table(tag).is_some())
        {
            return Err(unsupported(
                "binding requires a static glyf TrueType program",
            ));
        }
        let encoding = UnicodeCidEncoding::from_text(&program, &binding.repertoire, limits)?;
        // Private binding metadata is untrusted. Verify every encoded CID against
        // the real font's Unicode, GID and width tables before producing bytes.
        for (cid, unicode, gid, width) in encoding.glyph_mappings() {
            let code = cid.to_be_bytes();
            let visited = font.visit_chars_limited(&code, 1, |ch| {
                if !matches!(font.get_glyph(&ch), Some(GlyphDesc::Gid(actual)) if actual == u32::from(gid))
                    || font.unicode_mapping(&ch)?.as_deref() != Some(unicode.encode_utf8(&mut [0; 4]))
                    || !ch.width().is_finite() || (f64::from(ch.width()) - width).abs() > 0.01
                { return Err(invalid("bound encoding/Unicode/width does not match embedded program")); }
                Ok(())
            })?;
            if visited != 1 {
                return Err(invalid("bound CID must encode exactly one character"));
            }
        }
        let commands = layout(
            annotation, font_size, alignment, data, &program, &encoding, limits, budget,
        )?;
        let mut fonts = PdfDict::default();
        fonts.try_insert("F0".into(), reference(font_id))?;
        let mut resources = PdfDict::default();
        resources.try_insert("Font".into(), PdfObject::PdfDict(fonts.clone()))?;
        let mut result =
            self.encode_annotation_prepared(annotation, dict, Some((resources, commands)))?;
        result.try_insert(
            "DA".into(),
            PdfObject::PdfString(PdfString::new(
                format!(
                    "/F0 {font_size} Tf {} {} {} rg",
                    annotation.color[0], annotation.color[1], annotation.color[2]
                )
                .into_bytes(),
                false,
            )),
        )?;
        let mut marker = PdfDict::default();
        marker.try_insert(
            "Version".into(),
            PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new(1.)),
        )?;
        marker.try_insert("Font".into(), reference(font_id))?;
        marker.try_insert(
            "Repertoire".into(),
            PdfObject::PdfString(PdfString::from_text_with_limits(
                &binding.repertoire,
                &limits,
            )?),
        )?;
        result.try_insert("RzFreeText".into(), PdfObject::PdfDict(marker))?;
        // An opacity wrapper still exposes DA's F0 at the outer AP scope.
        let ap = result
            .get("AP")
            .and_then(PdfObject::as_dict)
            .and_then(|a| a.get("N"))
            .and_then(PdfObject::as_reference)
            .ok_or_else(|| invalid("normal appearance missing"))?
            .id();
        let appearance = self
            .resolve_object(ap)?
            .into_stream()
            .ok_or_else(|| invalid("normal appearance must be stream"))?;
        let mut resources = appearance
            .dict()
            .get("Resources")
            .and_then(PdfObject::as_dict)
            .cloned()
            .ok_or_else(|| invalid("appearance resources missing"))?;
        resources.try_insert("Font".into(), PdfObject::PdfDict(fonts))?;
        let (mut appearance_dict, data) = appearance.into_raw_parts();
        appearance_dict.try_insert("Resources".into(), PdfObject::PdfDict(resources))?;
        self.update_object(
            ap,
            PdfObject::PdfStream(crate::objects::pdf_stream::PdfStream::from_decoded_data(
                appearance_dict,
                data,
            )?),
        )?;
        Ok(result)
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "keep explicit font, geometry and operation budget together"
)]
fn layout(
    annotation: &Annotation,
    font_size: f32,
    alignment: TextAlignment,
    data: &[u8],
    program: &TrueTypeProgram,
    encoding: &UnicodeCidEncoding,
    limits: crate::parser::options::Limits,
    budget: &OperationBudget,
) -> PdfResult<String> {
    if !font_size.is_finite() || font_size <= 0. {
        return Err(invalid("positive finite font size required"));
    }
    let width = f64::from(annotation.rect[2]) - f64::from(annotation.rect[0]);
    let height = f64::from(annotation.rect[3]) - f64::from(annotation.rect[1]);
    if !width.is_finite() || !height.is_finite() || width <= 4. || height <= 4. {
        return Err(invalid("invalid text box"));
    }
    let metrics = program
        .horizontal_metrics()
        .ok_or_else(|| invalid("horizontal metrics missing"))?;
    let scale = f64::from(font_size) / f64::from(metrics.units_per_em());
    let mut ascent = f64::from(metrics.ascent()).max(0.) * scale;
    let mut descent = f64::from(metrics.descent()).min(0.) * scale;
    let locations = TrueTypeProgram::checked_glyph_locations(data, limits)?;
    let mappings: BTreeMap<_, _> = encoding
        .glyph_mappings()
        .map(|(cid, c, gid, width)| (c, (cid, gid, width)))
        .collect();
    let mut outlines = BTreeMap::new();
    let mut outline_leases = Vec::new();
    let mut segments = 0usize;
    for c in annotation.contents.chars().filter(|c| *c != '\n') {
        if outlines.contains_key(&c) {
            continue;
        }
        let (_, gid, _) = mappings.get(&c).ok_or_else(|| {
            PdfError::FontError(format!(
                "Unicode FreeText: U+{:04X} outside embedded repertoire; provide font asset",
                c as u32
            ))
        })?;
        let glyph = locations.expanded_glyph(*gid, &limits)?;
        let _points = budget.reserve(
            glyph.points.capacity()
                * std::mem::size_of::<crate::font::truetype::tables::expanded_glyf::ExpandedPoint>(
                ),
        )?;
        let commands = glyph.outline_commands(&limits)?;
        let _commands = budget.reserve(
            commands.capacity()
                * std::mem::size_of::<crate::font::truetype::tables::glyf::GlyphOutlineCommand>(),
        )?;
        segments = segments
            .checked_add(commands.len())
            .filter(|n| *n <= limits.max_path_segments)
            .ok_or_else(|| PdfError::ResourceLimit("Unicode FreeText outline segments".into()))?;
        outlines.insert(c, outline_bounds(&commands, &limits)?);
        outline_leases.push(budget.reserve(128)?);
    }
    for bounds in outlines.values().flatten() {
        ascent = ascent.max(bounds[3] * scale);
        descent = descent.min(bounds[1] * scale);
    }
    let leading = ascent - descent + f64::from(font_size) * 0.2;
    if !leading.is_finite() || leading <= 0. {
        return Err(invalid("invalid font line metrics"));
    }
    let lines = annotation.contents.split('\n').count();
    if lines
        .checked_mul(5)
        .and_then(|n| n.checked_add(14))
        .is_none_or(|n| n > limits.max_content_operations)
    {
        return Err(PdfError::ResourceLimit(
            "Unicode FreeText appearance operations".into(),
        ));
    }
    let bytes = annotation
        .contents
        .len()
        .checked_mul(4)
        .and_then(|n| {
            annotation
                .contents
                .split('\n')
                .count()
                .checked_mul(512)
                .and_then(|extra| n.checked_add(extra))
        })
        .filter(|n| *n <= limits.max_raw_stream_bytes && *n <= limits.max_decoded_bytes)
        .ok_or_else(|| PdfError::ResourceLimit("Unicode FreeText appearance bytes".into()))?;
    let mut content_lease = budget.reserve(bytes)?;
    let mut output = String::new();
    output
        .try_reserve_exact(bytes)
        .map_err(|_| PdfError::ResourceLimit("Unicode FreeText appearance allocation".into()))?;
    content_lease.resize(output.capacity())?;
    let mut baseline = height - 2. - ascent;
    for line in annotation.contents.split('\n') {
        let mut pen = 0.;
        let mut left: f64 = 0.;
        let mut right: f64 = 0.;
        for c in line.chars() {
            let (_, _, advance) = mappings
                .get(&c)
                .ok_or_else(|| invalid("character outside binding"))?;
            if let Some(bounds) = outlines[&c] {
                left = left.min(pen + bounds[0] * scale);
                right = right.max(pen + bounds[2] * scale);
                if baseline + bounds[1] * scale < 2. || baseline + bounds[3] * scale > height - 2. {
                    return Err(invalid("glyph outline exceeds text box"));
                }
            }
            pen += advance * f64::from(font_size) / 1000.;
        }
        right = right.max(pen);
        let span = right - left;
        if !span.is_finite()
            || span > width - 4.
            || baseline + descent < 2.
            || baseline + ascent > height - 2.
        {
            return Err(invalid(
                "text exceeds box; no automatic shrinking or wrapping",
            ));
        }
        let x = match alignment {
            TextAlignment::Left => 2.,
            TextAlignment::Center => (width - span) / 2.,
            TextAlignment::Right => width - 2. - span,
        } - left;
        let encoded = encoding.encode_text_with_budget(line, budget)?;
        write!(output, "BT /F0 {font_size} Tf 1 0 0 1 {x} {baseline} Tm <")
            .map_err(|_| invalid("appearance formatting"))?;
        for byte in encoded.as_slice() {
            write!(output, "{byte:02X}").map_err(|_| invalid("appearance hex encoding"))?;
        }
        output.push_str("> Tj ET\n");
        baseline -= leading;
    }
    Ok(output)
}
