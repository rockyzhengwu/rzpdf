use super::resource_manager::{ResourceKind, ResourceManager};
use crate::{
    error::{PdfError, PdfResult},
    font::{cid_encoding::UnicodeCidEncoding, truetype::true_type_program::TrueTypeProgram},
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream,
        pdf_string::PdfString,
    },
};
use std::io::{Read, Seek};

/// Caller-supplied metrics are in PDF's 1000-unit space, not font design units.
pub struct TrueTypeEmbeddingOptions {
    pub postscript_name: String,
    pub cap_height: f64,
    pub stem_v: f64,
    pub serif: bool,
}

pub struct RegisteredCidFont {
    /// Name in the page scope used for the initial registration.
    pub resource_name: String,
    encoding: UnicodeCidEncoding,
    handle: super::identity::ObjectHandle,
}

/// A direct-CID subset, not a font for subsequent shaping or arbitrary text.
pub struct RegisteredSubsetCidFont {
    pub font: RegisteredCidFont,
    omitted_tables: Vec<[u8; 4]>,
    _report_lease: crate::budget::MemoryLease,
}

impl RegisteredSubsetCidFont {
    pub fn omitted_tables(&self) -> &[[u8; 4]] {
        &self.omitted_tables
    }
}

pub(super) struct EmbeddedCidFontObjects {
    resource_name: Option<String>,
    encoding: UnicodeCidEncoding,
    handle: super::identity::ObjectHandle,
    // Retained by prepared edits while font objects and new content coexist.
    _metric_lease: crate::budget::MemoryLease,
}
impl EmbeddedCidFontObjects {
    #[cfg(test)]
    pub(super) fn metric_bytes(&self) -> usize { self._metric_lease.bytes() }
    pub(super) fn encoding(&self) -> &UnicodeCidEncoding {
        &self.encoding
    }
    pub(super) fn object_handle(&self) -> &super::identity::ObjectHandle {
        &self.handle
    }
    pub(super) fn resource_name(&self) -> Option<&str> {
        self.resource_name.as_deref()
    }
    fn into_registered(self) -> PdfResult<RegisteredCidFont> {
        Ok(RegisteredCidFont {
            resource_name: self.resource_name.ok_or_else(|| {
                PdfError::PageResourceError("Detached font has no page binding".into())
            })?,
            encoding: self.encoding,
            handle: self.handle,
        })
    }
}

pub(super) struct EmbeddedSubsetFontObjects {
    pub(super) font: EmbeddedCidFontObjects,
    omitted_tables: Vec<[u8; 4]>,
    _report_lease: crate::budget::MemoryLease,
}

impl RegisteredCidFont {
    pub fn encoding(&self) -> &UnicodeCidEncoding {
        &self.encoding
    }

    pub fn object_handle(&self) -> &super::identity::ObjectHandle {
        &self.handle
    }

    /// Reuse this font in another page of its owning document. The returned
    /// name is page-local; it may differ from the initial resource_name.
    /// Handles created inside an outer transaction must not escape a rollback.
    pub fn register_on_page<R: Read + Seek>(
        &self,
        document: &super::PdfDocument<R>,
        page_index: u32,
    ) -> PdfResult<String> {
        document
            .resources()
            .register_handle(page_index, ResourceKind::Font, &self.handle)
    }
}

fn name(value: &str) -> PdfObject {
    PdfObject::PdfName(PdfName::new(value.into()))
}
fn number(value: f64) -> PdfResult<PdfObject> {
    Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
        value.to_string().as_bytes(),
    )?))
}
fn reference(id: crate::objects::object_id::ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}

/// Fixed-size metric storage. The caller retains the returned lease through
/// publication; document-resident allocations are not an operation pool.
struct MetricArrayBuilder<'a> {
    items: Vec<PdfObject>,
    count: usize,
    limits: crate::parser::options::Limits,
    budget: &'a crate::budget::OperationBudget,
    lease: crate::budget::MemoryLease,
}

impl<'a> MetricArrayBuilder<'a> {
    fn new(count: usize, limits: crate::parser::options::Limits,
        budget: &'a crate::budget::OperationBudget) -> PdfResult<Self> {
        let bytes = count.checked_mul(std::mem::size_of::<PdfObject>())
            .filter(|_| count <= limits.max_objects)
            .filter(|&bytes| bytes <= limits.max_operation_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("font metric array size".into()))?;
        let mut lease = budget.reserve(bytes)?;
        let mut items = Vec::new();
        items.try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("font metric array allocation".into()))?;
        let actual = items.capacity().checked_mul(std::mem::size_of::<PdfObject>())
            .filter(|&bytes| bytes <= limits.max_operation_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("font metric array capacity".into()))?;
        lease.resize(actual)?;
        Ok(Self { items, count, limits, budget, lease })
    }

    fn push_number(&mut self, value: f64) -> PdfResult<()> {
        use std::io::Write;
        if !value.is_finite() {
            return Err(PdfError::ObjectError("non-finite font metric".into()));
        }
        if self.items.len() >= self.count {
            return Err(PdfError::ObjectError("font metric array overflow".into()));
        }
        // Display for extreme finite f64 values can exceed 300 decimal digits.
        let mut buffer = std::io::Cursor::new([0u8; 512]);
        write!(&mut buffer, "{value}")
            .map_err(|_| PdfError::ResourceLimit("font metric serialization".into()))?;
        let length = buffer.position() as usize;
        if length > self.limits.max_token_bytes {
            return Err(PdfError::ResourceLimit("font metric token bytes".into()));
        }
        let token = std::str::from_utf8(&buffer.get_ref()[..length])
            .map_err(|_| PdfError::ObjectError("font metric token encoding".into()))?;
        let (lexeme, mut lease) = crate::budget::copy_string(token, self.budget)?;
        let number = PdfNumber::from_owned_lexeme(lexeme)?;
        self.lease.absorb(&mut lease)?;
        self.items.push(PdfObject::PdfNumber(number));
        Ok(())
    }

    fn finish(self) -> PdfResult<(PdfArray, crate::budget::MemoryLease)> {
        if self.items.len() != self.count {
            return Err(PdfError::ObjectError("incomplete font metric array".into()));
        }
        Ok((PdfArray::from_items(self.items), self.lease))
    }

    fn finish_widths(self) -> PdfResult<(PdfArray, crate::budget::MemoryLease)> {
        let limits = self.limits;
        let budget = self.budget;
        let count = self.count;
        let (inner, mut inner_lease) = self.finish()?;
        if count == 0 { return Ok((inner, inner_lease)); }
        let mut outer = Self::new(2, limits, budget)?;
        outer.push_number(1.0)?;
        outer.lease.absorb(&mut inner_lease)?;
        outer.items.push(PdfObject::PdfArray(inner));
        outer.finish()
    }
}

#[cfg(test)]
mod metric_array_tests {
    use super::*;
    use crate::{budget::OperationBudget, parser::options::Limits};

    #[test]
    fn registered_metric_owner_survives_until_outer_content_failure() {
        let source = crate::font::truetype::subset::tests::fixture();
        let doc = crate::document::PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let before = doc.save_to_bytes().unwrap();
        let pool = OperationBudget::new(16 * 1024 * 1024);
        let neighbor = pool.reserve(7).unwrap();
        let register = |doc: &crate::document::PdfDocument<std::io::Cursor<Vec<u8>>>| {
            doc.resources().embed_truetype_subset_font_objects(
                Some(0), &source, "A", TrueTypeEmbeddingOptions {
                    postscript_name: "Demo".into(), cap_height: 60.0,
                    stem_v: 80.0, serif: false,
                }, *b"OWNERX", false, &pool,
            )
        };
        let result: PdfResult<()> = doc.edit_transaction(|doc| {
            let registered = register(doc)?;
            let metric_bytes = registered.font._metric_lease.bytes();
            assert!(metric_bytes >= 3 * std::mem::size_of::<PdfObject>());
            assert!(pool.live_bytes() >= 7 + metric_bytes);
            let remaining = pool.maximum_bytes() - pool.live_bytes();
            assert!(pool.reserve(remaining + 1).is_err());
            // Font publication succeeded; later page-content publication fails.
            let result = doc.replace_page_contents(1, b"BT ET".to_vec());
            assert!(result.is_err());
            assert!(pool.live_bytes() >= 7 + metric_bytes);
            result
        });
        assert!(result.is_err());
        assert_eq!(pool.live_bytes(), 7);
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: PdfResult<()> = doc.edit_transaction(|doc| {
                let _registered = register(doc)?;
                doc.replace_page_contents(0, b"BT ET".to_vec())?;
                panic!("registered metric content rollback");
            });
        }));
        assert_eq!(panic.unwrap_err().downcast_ref::<&str>(), Some(&"registered metric content rollback"));
        assert_eq!(pool.live_bytes(), 7);
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        doc.edit_transaction(|doc| {
            let _registered = register(doc)?;
            doc.replace_page_contents(0, b"BT ET".to_vec())
        }).unwrap();
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
        crate::document::PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(), crate::parser::syntax::ParseMode::Strict,
        ).unwrap();
    }

    #[test]
    fn metric_token_and_outer_failure_release_and_unwind_keeps_neighbor() {
        let limits = Limits::default();
        let pool = OperationBudget::new(4096);
        let neighbor = pool.reserve(7).unwrap();
        let mut builder = MetricArrayBuilder::new(1, Limits { max_token_bytes: 1, ..limits }, &pool).unwrap();
        let slots = pool.live_bytes();
        assert!(matches!(builder.push_number(12.0), Err(PdfError::ResourceLimit(_))));
        assert_eq!(pool.live_bytes(), slots);
        builder.push_number(1.0).unwrap();
        drop(builder.finish_widths().unwrap());
        assert_eq!(pool.live_bytes(), 7);
        let mut builder = MetricArrayBuilder::new(1, limits, &pool).unwrap();
        builder.push_number(1.0).unwrap();
        let inner_bytes = pool.live_bytes() - 7;
        drop(builder);
        let tight = OperationBudget::new(inner_bytes);
        let mut builder = MetricArrayBuilder::new(1, limits, &tight).unwrap();
        builder.push_number(1.0).unwrap();
        assert!(builder.finish_widths().is_err());
        assert_eq!(tight.live_bytes(), 0);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut builder = MetricArrayBuilder::new(3, limits, &pool).unwrap();
            for value in [-1000.0, 500.0, 880.0] { builder.push_number(value).unwrap(); }
            let _retained = builder.finish_widths().unwrap();
            panic!("metric owner unwind");
        }));
        assert_eq!(panic.unwrap_err().downcast_ref::<&str>(), Some(&"metric owner unwind"));
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn width_container_counts_outer_slots_and_neighbor_overlap() {
        let limits = Limits::default();
        let build = |pool: &OperationBudget| -> PdfResult<_> {
            let mut builder = MetricArrayBuilder::new(3, limits, pool)?;
            for value in [-1000.0, 500.0, 880.0] { builder.push_number(value)?; }
            builder.finish_widths()
        };
        let pool = OperationBudget::new(4096);
        let (array, lease) = build(&pool).unwrap();
        assert_eq!(array.len(), 2);
        let exact = lease.bytes();
        assert!(exact >= 5 * std::mem::size_of::<PdfObject>() + 12);
        drop((array, lease));
        assert_eq!(pool.live_bytes(), 0);
        let tight = OperationBudget::new(exact);
        let neighbor = tight.reserve(1).unwrap();
        assert!(build(&tight).is_err());
        assert_eq!(tight.live_bytes(), 1);
        drop(neighbor);
        let success = build(&tight).unwrap();
        assert_eq!(tight.live_bytes(), exact);
        drop(success);
        assert_eq!(tight.live_bytes(), 0);
        let empty = MetricArrayBuilder::new(0, limits, &tight).unwrap().finish_widths().unwrap();
        assert_eq!(empty.0.len(), 0);
        assert_eq!(tight.live_bytes(), 0);
    }

    #[test]
    fn metric_array_charges_slots_and_tokens_and_retries_after_release() {
        let limits = Limits::default();
        let pool = OperationBudget::new(4096);
        let mut builder = MetricArrayBuilder::new(3, limits, &pool).unwrap();
        for value in [-1000.0, 250.5, 0.0] {
            builder.push_number(value).unwrap();
        }
        assert!(builder.push_number(1.0).is_err());
        let (array, lease) = builder.finish().unwrap();
        let exact = lease.bytes();
        assert!(exact >= 3 * std::mem::size_of::<PdfObject>() + 11);
        assert_eq!(pool.live_bytes(), exact);
        drop((array, lease));
        assert_eq!(pool.live_bytes(), 0);
        for maximum in [exact - 1, exact] {
            let tight = OperationBudget::new(maximum);
            let result = (|| {
                let mut builder = MetricArrayBuilder::new(3, limits, &tight)?;
                for value in [-1000.0, 250.5, 0.0] { builder.push_number(value)?; }
                builder.finish()
            })();
            assert_eq!(result.is_ok(), maximum == exact);
            drop(result);
            assert_eq!(tight.live_bytes(), 0);
        }
        let neighbor = pool.reserve(7).unwrap();
        let mut builder = MetricArrayBuilder::new(1, limits, &pool).unwrap();
        assert!(builder.push_number(f64::NAN).is_err());
        assert!(builder.finish().is_err());
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
        assert!(MetricArrayBuilder::new(usize::MAX, limits, &pool).is_err());
        assert!(MetricArrayBuilder::new(1, Limits { max_objects: 0, ..limits }, &pool).is_err());
    }
}

impl<R: Read + Seek> ResourceManager<'_, R> {
    /// Embed a fixed-repertoire static TrueType subset. The caller supplies a
    /// document-unique six-uppercase-letter subset tag and reviews the license.
    /// The budget tracks subset buffers and W/W2 array/token capacity through
    /// publication, not legacy parser or document-resident allocations.
    /// Known layout and optional device/printer-table
    /// omission requires explicit opt-in; all omissions are reported.
    pub fn embed_truetype_subset_font(
        &self,
        page_index: u32,
        source: &[u8],
        repertoire: &str,
        options: TrueTypeEmbeddingOptions,
        subset_tag: [u8; 6],
        omit_layout_and_auxiliary: bool,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<RegisteredSubsetCidFont> {
        let objects = self.embed_truetype_subset_font_objects(
            Some(page_index),
            source,
            repertoire,
            options,
            subset_tag,
            omit_layout_and_auxiliary,
            budget,
        )?;
        Ok(RegisteredSubsetCidFont {
            font: objects.font.into_registered()?,
            omitted_tables: objects.omitted_tables,
            _report_lease: objects._report_lease,
        })
    }

    /// Embed a static original-GID subset with explicit Identity-V/W2 metrics.
    /// Requires source vhea/vmtx; does not apply vertical substitutions.
    pub fn embed_truetype_vertical_subset_font(
        &self, page_index: u32, source: &[u8], repertoire: &str,
        options: TrueTypeEmbeddingOptions, subset_tag: [u8; 6],
        omit_layout_and_auxiliary: bool, budget: &crate::budget::OperationBudget,
    ) -> PdfResult<RegisteredSubsetCidFont> {
        let objects = self.embed_truetype_subset_font_objects_with_mode(
            Some(page_index), source, repertoire, options, subset_tag,
            omit_layout_and_auxiliary, budget, true,
        )?;
        Ok(RegisteredSubsetCidFont {
            font: objects.font.into_registered()?, omitted_tables: objects.omitted_tables,
            _report_lease: objects._report_lease,
        })
    }

    /// Create validated font objects in the current transaction. None means
    /// the caller binds the handle into a copied local scope before publication.
    pub(super) fn embed_truetype_subset_font_objects(
        &self,
        page_index: Option<u32>,
        source: &[u8],
        repertoire: &str,
        options: TrueTypeEmbeddingOptions,
        subset_tag: [u8; 6],
        omit_layout_and_auxiliary: bool,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<EmbeddedSubsetFontObjects> {
        self.embed_truetype_subset_font_objects_with_mode(
            page_index, source, repertoire, options, subset_tag,
            omit_layout_and_auxiliary, budget, false,
        )
    }

    pub(super) fn embed_truetype_subset_font_objects_with_mode(
        &self, page_index: Option<u32>, source: &[u8], repertoire: &str,
        mut options: TrueTypeEmbeddingOptions, subset_tag: [u8; 6],
        omit_layout_and_auxiliary: bool, budget: &crate::budget::OperationBudget,
        vertical: bool,
    ) -> PdfResult<EmbeddedSubsetFontObjects> {
        if let Some(page) = page_index {
            self.document.page_location(page)?;
        }
        if !subset_tag.iter().all(u8::is_ascii_uppercase) {
            return Err(PdfError::FontError("Invalid TrueType subset tag".into()));
        }
        // Validate the unprefixed identity first: adding a valid subset tag
        // must not turn an empty or malformed caller name into a valid one.
        if options.postscript_name.is_empty()
            || options.postscript_name.len() > 56
            || !options
                .postscript_name
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || b"-_.".contains(&byte))
            || !options.cap_height.is_finite()
            || options.cap_height < 0.0
            || !options.stem_v.is_finite()
            || options.stem_v <= 0.0
        {
            return Err(PdfError::FontError(
                "Invalid supplied subset FontDescriptor metadata".into(),
            ));
        }
        if self.document.effective_version()? < super::save_options::PdfVersion::V1_3 {
            return Err(PdfError::UnsupportedFeature(
                "CIDFontType2 embedding requires PDF 1.3".into(),
            ));
        }
        let limits = self.document.ctx.limits();
        let program = TrueTypeProgram::try_new_with_limits(source, limits)?;
        program.validate_outline_embedding(true, true)?;
        let encoding = UnicodeCidEncoding::from_text(&program, repertoire, limits)?;
        let prefix = std::str::from_utf8(&subset_tag)
            .map_err(|_| PdfError::FontError("Invalid TrueType subset tag".into()))?;
        options.postscript_name = format!("{prefix}+{}", options.postscript_name);
        let subset = crate::font::truetype::subset::build_subset(
            source,
            &encoding,
            &options.postscript_name,
            omit_layout_and_auxiliary,
            &limits,
            budget,
        )?;
        let (data, lease) = subset.bytes.into_reserved_parts();
        let font = self.embed_truetype_font_impl(
            page_index,
            data,
            repertoire,
            options,
            true,
            Some(lease),
            vertical,
            Some(budget),
        )?;
        Ok(EmbeddedSubsetFontObjects {
            font,
            omitted_tables: subset.omitted_tables,
            _report_lease: subset._report_lease,
        })
    }

    /// Embed a whole static TrueType font with an immutable Identity-H repertoire.
    /// Technical fsType checks do not replace the caller's font license review.
    /// No subsetting, shaping, variable instances, or CFF embedding is performed.
    pub fn embed_truetype_font(
        &self,
        page_index: u32,
        data: Vec<u8>,
        repertoire: &str,
        options: TrueTypeEmbeddingOptions,
    ) -> PdfResult<RegisteredCidFont> {
        self.embed_truetype_font_impl(Some(page_index), data, repertoire, options, false, None, false, None)?
            .into_registered()
    }

    /// Embed static TrueType for Identity-V using explicit source vhea/vmtx
    /// metrics. No vertical glyph substitutions, shaping or synthetic metrics.
    /// The caller remains responsible for font licensing.
    pub fn embed_truetype_vertical_font(
        &self,
        page_index: u32,
        data: Vec<u8>,
        repertoire: &str,
        options: TrueTypeEmbeddingOptions,
    ) -> PdfResult<RegisteredCidFont> {
        self.embed_truetype_font_impl(Some(page_index), data, repertoire, options, false, None, true, None)?
            .into_registered()
    }

    fn embed_truetype_font_impl(
        &self,
        page_index: Option<u32>,
        data: Vec<u8>,
        repertoire: &str,
        options: TrueTypeEmbeddingOptions,
        subset: bool,
        _font_lease: Option<crate::budget::MemoryLease>,
        vertical: bool,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<EmbeddedCidFontObjects> {
        if let Some(page) = page_index {
            self.document.page_location(page)?;
        }
        if self.document.effective_version()? < super::save_options::PdfVersion::V1_3 {
            return Err(PdfError::UnsupportedFeature(
                "CIDFontType2 embedding requires PDF 1.3".into(),
            ));
        }
        let limits = self.document.ctx.limits();
        if data.len() > limits.max_raw_stream_bytes {
            return Err(PdfError::ResourceLimit("FontFile2 bytes".into()));
        }
        if options.postscript_name.is_empty()
            || options.postscript_name.len() > 127
            || !options.postscript_name.bytes().all(|byte| {
                byte.is_ascii_alphanumeric() || b"-_.".contains(&byte) || (subset && byte == b'+')
            })
            || !options.cap_height.is_finite()
            || options.cap_height < 0.0
            || !options.stem_v.is_finite()
            || options.stem_v <= 0.0
        {
            return Err(PdfError::FontError(
                "Invalid supplied FontDescriptor metadata".into(),
            ));
        }
        let program = TrueTypeProgram::try_new_with_limits(&data, limits)?;
        program.validate_outline_embedding(subset, true)?;
        TrueTypeProgram::checked_glyph_locations(&data, limits)?;
        // Default-instance metadata must not masquerade as variable-font embedding.
        let sfnt = crate::font::truetype::sfnt::Sfnt::try_new_with_limits(
            &mut crate::font::truetype::reader::TrueTypeReader::new(&data),
            limits,
        )?;
        if sfnt.get_table("fvar").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "Variable font embedding".into(),
            ));
        }
        let metrics = program.horizontal_metrics().ok_or_else(|| {
            PdfError::FontError("FontDescriptor requires horizontal metrics".into())
        })?;
        let post = program
            .post_metrics()
            .ok_or_else(|| PdfError::FontError("FontDescriptor requires post metrics".into()))?;
        let encoding = UnicodeCidEncoding::from_text(&program, repertoire, limits)?;
        // Legacy whole-font APIs use a local limit, not the caller's shared pool.
        let local_budget = limits.operation_budget();
        let metric_budget = budget.unwrap_or(&local_budget);
        let count = encoding.widths().count();
        let numeric_count = count.checked_mul(if vertical { 4 } else { 1 })
            .and_then(|n| n.checked_add(if count == 0 { 0 } else if vertical { 4 } else { 2 }))
            .filter(|&n| n <= limits.max_objects)
            .ok_or_else(|| PdfError::ResourceLimit("font W/W2 object count".into()))?;
        numeric_count.checked_mul(std::mem::size_of::<PdfObject>())
            .filter(|&bytes| bytes <= limits.max_operation_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("font W/W2 storage bytes".into()))?;
        let mut metric_lease = metric_budget.reserve(0)?;
        let vertical_widths = if vertical {
            let vertical_metrics = TrueTypeProgram::checked_vertical_metrics(&data, limits)?
                .ok_or_else(|| PdfError::FontError("Vertical embedding requires vhea/vmtx".into()))?;
            let locations = TrueTypeProgram::checked_glyph_locations(&data, limits)?;
            let triple_count = count.checked_mul(3)
                .ok_or_else(|| PdfError::ResourceLimit("font W2 object count".into()))?;
            let mut triples = MetricArrayBuilder::new(triple_count, limits, metric_budget)?;
            for (_, _, gid, _) in encoding.glyph_mappings() {
                let y_max = locations.glyph_header(gid)?.map_or(0, |header| header.bounds[3]);
                for value in vertical_metrics.pdf_metric(
                    gid, metrics.units_per_em(), metrics.advance_width(gid)?, y_max,
                )? {
                    triples.push_number(value)?;
                }
            }
            let (array, mut lease) = triples.finish_widths()?;
            metric_lease.absorb(&mut lease)?;
            Some(array)
        } else { None };
        let to_unicode = encoding.to_unicode_cmap()?;
        let gid_map = encoding.cid_to_gid_map()?;
        let scale = 1000.0 / f64::from(metrics.units_per_em());
        let mut descriptor = PdfDict::default();
        descriptor.try_insert("Type".into(), name("FontDescriptor"))?;
        descriptor.try_insert("FontName".into(), name(&options.postscript_name))?;
        let flags = 4
            | u32::from(post.is_fixed_pitch())
            | if options.serif { 2 } else { 0 }
            | if post.italic_angle() != 0.0 { 64 } else { 0 };
        for (key, value) in [
            ("Flags", f64::from(flags)),
            ("ItalicAngle", post.italic_angle()),
            ("Ascent", f64::from(metrics.ascent()) * scale),
            ("Descent", f64::from(metrics.descent()) * scale),
            ("CapHeight", options.cap_height),
            ("StemV", options.stem_v),
        ] {
            descriptor.try_insert(key.into(), number(value)?)?;
        }
        let mut bounds = PdfArray::default();
        for value in metrics.bounds() {
            bounds.try_add_obj(number(f64::from(value) * scale)?)?;
        }
        descriptor.try_insert("FontBBox".into(), PdfObject::PdfArray(bounds))?;
        let mut widths = MetricArrayBuilder::new(count, limits, metric_budget)?;
        for (_, width) in encoding.widths() {
            widths.push_number(width)?;
        }
        let (w, mut width_lease) = widths.finish_widths()?;
        metric_lease.absorb(&mut width_lease)?;
        let mut system = PdfDict::default();
        system.try_insert(
            "Registry".into(),
            PdfObject::PdfString(PdfString::new(b"Adobe".to_vec(), false)),
        )?;
        system.try_insert(
            "Ordering".into(),
            PdfObject::PdfString(PdfString::new(b"Identity".to_vec(), false)),
        )?;
        system.try_insert("Supplement".into(), number(0.0)?)?;
        let (resource_name, handle) = self.document.edit_transaction(|doc| {
            let mut font_file = PdfDict::default();
            font_file.try_insert("Length1".into(), number(data.len() as f64)?)?;
            let file_id = doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(font_file, data)?,
            ))?;
            descriptor.try_insert("FontFile2".into(), reference(file_id))?;
            let descriptor_id = doc.try_insert_object(PdfObject::PdfDict(descriptor))?;
            let gid_id = doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(PdfDict::default(), gid_map)?,
            ))?;
            let unicode_id = doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(PdfDict::default(), to_unicode)?,
            ))?;
            let mut descendant = PdfDict::default();
            for (key, value) in [
                ("Type", "Font"),
                ("Subtype", "CIDFontType2"),
                ("BaseFont", &options.postscript_name),
            ] {
                descendant.try_insert(key.into(), name(value))?;
            }
            descendant.try_insert("CIDSystemInfo".into(), PdfObject::PdfDict(system))?;
            descendant.try_insert("FontDescriptor".into(), reference(descriptor_id))?;
            descendant.try_insert("CIDToGIDMap".into(), reference(gid_id))?;
            descendant.try_insert("DW".into(), number(0.0)?)?;
            descendant.try_insert("W".into(), PdfObject::PdfArray(w))?;
            if let Some(w2) = vertical_widths {
                descendant.try_insert("W2".into(), PdfObject::PdfArray(w2))?;
            }
            let descendant_id = doc.try_insert_object(PdfObject::PdfDict(descendant))?;
            let mut descendants = PdfArray::default();
            descendants.try_add_obj(reference(descendant_id))?;
            let mut font = PdfDict::default();
            for (key, value) in [
                ("Type", "Font"),
                ("Subtype", "Type0"),
                ("BaseFont", &options.postscript_name),
                ("Encoding", if vertical { "Identity-V" } else { "Identity-H" }),
            ] {
                font.try_insert(key.into(), name(value))?;
            }
            font.try_insert("DescendantFonts".into(), PdfObject::PdfArray(descendants))?;
            font.try_insert("ToUnicode".into(), reference(unicode_id))?;
            let id = doc.try_insert_object(PdfObject::PdfDict(font))?;
            let resource_name = page_index
                .map(|page| {
                    doc.resources()
                        .register_existing(page, ResourceKind::Font, id)
                })
                .transpose()?;
            Ok((resource_name, doc.object_handle(id)?))
        })?;
        Ok(EmbeddedCidFontObjects {
            resource_name,
            encoding,
            handle,
            _metric_lease: metric_lease,
        })
    }
}
