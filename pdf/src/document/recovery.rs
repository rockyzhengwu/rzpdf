//! Explicit, bounded index reconstruction. Never upgrades ordinary parsing.
use super::{
    PdfDocument,
    catalog::Catalog,
    cross_ref_table::{CrossRefTable, ObjectInfo, ObjectState},
};
use crate::{
    budget::{MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict},
    parser::{
        diagnostic::{ReadProvenance, StructureDiagnostic, StructureReason},
        options::ParseOptions,
        parser::PdfParser,
        syntax::{ParseMode, SyntaxParser},
    },
    pdf_context::PDFContext,
};
use std::{collections::HashMap, io::Cursor};

/// Policy ceilings may be lowered, never raised beyond the frozen profile.
#[derive(Debug, Clone, Copy)]
pub struct RecoveryLimits {
    pub max_input_bytes: usize,
    pub max_candidates: usize,
    pub max_scan_steps: u64,
}
impl Default for RecoveryLimits {
    fn default() -> Self {
        Self {
            max_input_bytes: 64 * 1024 * 1024,
            max_candidates: 100_000,
            max_scan_steps: u64::MAX,
        }
    }
}

/// Separate read-only capability: no Deref, context, page handle, conversion,
/// edit, save, incremental or signature entry point is exposed.
///
/// ```compile_fail
/// # use pdf::document::recovery::RecoveredDocument;
/// fn cannot_save(doc: &RecoveredDocument) { doc.save_to_bytes().unwrap(); }
/// ```
/// ```compile_fail
/// # use pdf::document::recovery::RecoveredDocument;
/// fn cannot_unwrap(doc: &RecoveredDocument) { doc.ctx(); }
/// ```
/// ```compile_fail
/// # use pdf::document::{Document, recovery::RecoveredDocument};
/// fn cannot_relabel(doc: &RecoveredDocument) { let _: &Document = doc; }
/// ```
/// ```compile_fail
/// # use pdf::document::recovery::RecoveredDocument;
/// fn cannot_edit(doc: &RecoveredDocument) { doc.add_page(0).unwrap(); }
/// ```
pub struct RecoveredDocument {
    inner: PdfDocument<Cursor<Vec<u8>>>,
    report: Vec<StructureDiagnostic>,
    _source_lease: MemoryLease,
    _index_lease: MemoryLease,
    scan_steps: u64,
}
impl RecoveredDocument {
    pub fn from_bytes(bytes: Vec<u8>) -> PdfResult<Self> {
        Self::from_bytes_with_options(bytes, ParseOptions::default(), RecoveryLimits::default())
    }
    pub fn from_bytes_with_options(
        bytes: Vec<u8>,
        options: ParseOptions,
        limits: RecoveryLimits,
    ) -> PdfResult<Self> {
        let budget = options.limits.operation_budget();
        Self::from_bytes_with_budget(bytes, options, limits, &budget)
    }
    pub fn from_bytes_with_budget(
        bytes: Vec<u8>,
        mut options: ParseOptions,
        limits: RecoveryLimits,
        budget: &OperationBudget,
    ) -> PdfResult<Self> {
        if bytes.len() > limits.max_input_bytes.min(64 * 1024 * 1024) {
            return Err(PdfError::ResourceLimit("recovery input bytes".into()));
        }
        let source_lease = budget.reserve(bytes.capacity())?;
        let maximum_steps = (bytes.len() as u64)
            .checked_mul(16)
            .and_then(|n| n.checked_add(1024))
            .ok_or_else(|| PdfError::ResourceLimit("recovery scan arithmetic".into()))?
            .min(limits.max_scan_steps);
        let maximum_candidates = limits
            .max_candidates
            .min(100_000)
            .min(options.limits.max_objects);
        options.mode = ParseMode::Strict;
        let mut syntax =
            SyntaxParser::with_options(StreamReader::try_new(Cursor::new(bytes))?, options);
        syntax.limit_work(maximum_steps);
        let version = syntax.read_header()?.version().to_owned();
        let mut entries = HashMap::new();
        let mut index_lease = budget.reserve(0)?;
        let mut seen_xref = false;
        let trailer: PdfDict;
        let trailer_offset;
        loop {
            let offset = syntax.position();
            let word = syntax.get_next_word()?;
            if word.is_equal(b"trailer") {
                trailer_offset = offset;
                let (value, _lease) = syntax.get_content_object_with_budget(budget)?;
                trailer = value
                    .into_dict()
                    .ok_or_else(|| invalid("trailer dictionary required"))?;
                break;
            }
            if word.is_equal(b"xref") {
                if seen_xref {
                    return Err(invalid("multiple xref sections"));
                }
                seen_xref = true;
                // No substring search through arbitrary damaged bytes.
                let mut tokens = 0usize;
                loop {
                    let saved = syntax.position();
                    let token = syntax.get_next_word()?;
                    if token.is_equal(b"trailer") {
                        syntax.set_pos(saved)?;
                        break;
                    }
                    tokens = tokens
                        .checked_add(1)
                        .ok_or_else(|| invalid("xref tokens"))?;
                    if tokens > maximum_candidates.saturating_mul(5).saturating_add(16) {
                        return Err(PdfError::ResourceLimit("recovery xref tokens".into()));
                    }
                    if !(token.raw().iter().all(u8::is_ascii_digit) && !token.is_empty())
                        && !token.is_equal(b"n")
                        && !token.is_equal(b"f")
                    {
                        return Err(invalid("ambiguous damaged xref table"));
                    }
                }
                continue;
            }
            if seen_xref || !word.is_number() {
                return Err(invalid("expected sequential indirect object"));
            }
            syntax.set_pos(offset)?;
            let (object, lease) = syntax.get_indirect_object_with_budget(budget)?;
            let id = object.id();
            if id.obj_num() == 0 || entries.contains_key(&id.obj_num()) {
                return Err(invalid("conflicting object number"));
            }
            if entries.len() >= maximum_candidates {
                return Err(PdfError::ResourceLimit("recovery candidates".into()));
            }
            reject_unsupported(object.obj(), 0, options.limits.max_depth)?;
            if entries.len() == entries.capacity() {
                let expected = entries.capacity().saturating_mul(2).max(4);
                let mut replacement_lease = budget.reserve(
                    expected
                        .checked_mul(128)
                        .ok_or_else(|| invalid("index allocation"))?,
                )?;
                let mut replacement = HashMap::new();
                replacement
                    .try_reserve(expected)
                    .map_err(|_| PdfError::ResourceLimit("recovery index allocation".into()))?;
                replacement_lease.resize(replacement.capacity().saturating_mul(128))?;
                replacement.extend(entries.drain());
                entries = replacement;
                index_lease = replacement_lease;
            }
            entries.insert(
                id.obj_num(),
                ObjectInfo::new(id.obj_num(), offset, id.gen_num(), ObjectState::Normal),
            );
            drop(lease);
        }
        for key in ["Prev", "XRefStm", "Encrypt", "Type", "W", "Index"] {
            if trailer.contains_key(key) {
                return Err(invalid("protected, historical or stream trailer"));
            }
        }
        reject_unsupported(
            &PdfObject::PdfDict(trailer.clone()),
            0,
            options.limits.max_depth,
        )?;
        let root = trailer
            .get("Root")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| invalid("unambiguous indirect Root required"))?;
        let size = trailer
            .get("Size")
            .and_then(PdfObject::as_u32)
            .ok_or_else(|| invalid("trailer Size required"))?;
        if size as usize > options.limits.max_objects || entries.keys().any(|id| *id >= size) {
            return Err(invalid("trailer Size inconsistent"));
        }
        let root_info = entries
            .get(&root.obj_num())
            .filter(|info| info.object_id() == root)
            .ok_or_else(|| invalid("Root not scanned"))?;
        let root_offset = root_info.offset();
        if !syntax.get_next_word()?.is_equal(b"startxref") {
            return Err(invalid("startxref suffix required"));
        }
        syntax.get_next_word()?.as_u64_checked()?;
        // Fixed-size suffix buffer, never allocate from untrusted tail length.
        let mut tail = [0u8; 64];
        let mut count = 0;
        while !syntax.is_eof() {
            if count == tail.len() {
                return Err(invalid("excessive EOF suffix"));
            }
            tail[count] = syntax.get_next_char()?;
            count += 1;
        }
        if trim_pdf_space(&tail[..count]) != b"%%EOF" {
            return Err(invalid("single final EOF required"));
        }
        syntax.set_pos(root_offset)?;
        let (root_value, _root_lease) = syntax.get_indirect_object_with_budget(budget)?;
        if root_value
            .obj()
            .as_dict()
            .and_then(|d| d.get("Type"))
            .and_then(PdfObject::as_name)
            .is_none_or(|n| n.name() != "Catalog")
        {
            return Err(invalid("Root must be Catalog"));
        }
        let scan_steps = maximum_steps - syntax.remaining_work().unwrap_or(0);
        syntax.finish_work();
        syntax.report(
            trailer_offset,
            Some(root),
            StructureReason::RebuiltObjectIndex,
        )?;
        let report = syntax.diagnostics.snapshot()?;
        let xref = CrossRefTable::new(entries, trailer);
        let parser = PdfParser::new(syntax.with_xref_table(xref.clone()));
        let ctx = PDFContext::try_new_with_version(parser, xref, version)?;
        let catalog = Catalog::try_new(&ctx)?;
        let maximum_cache_bytes = ctx.limits().max_operation_bytes.min(32 * 1024 * 1024);
        let inner = PdfDocument {
            ctx,
            catalog,
            source_xref_offset: None,
            prepared_pages: std::cell::RefCell::new(crate::derived_cache::DerivedCache::new(
                maximum_cache_bytes,
                2,
            )),
        };
        inner.try_page_count()?;
        Ok(Self {
            inner,
            report,
            _source_lease: source_lease,
            _index_lease: index_lease,
            scan_steps,
        })
    }
    pub fn provenance(&self) -> ReadProvenance {
        ReadProvenance::RebuiltReadOnly
    }
    pub fn diagnostics(&self) -> &[StructureDiagnostic] {
        &self.report
    }
    pub fn scan_steps(&self) -> u64 {
        self.scan_steps
    }
    pub fn version(&self) -> &str {
        self.inner.version()
    }
    pub fn try_page_count(&self) -> PdfResult<usize> {
        self.inner.try_page_count()
    }
    pub fn display_list(&self, page: u32) -> PdfResult<crate::page::display_list::DisplayList> {
        self.inner.get_page(page)?.display_list()
    }
    pub fn page_geometry(&self, page: u32) -> PdfResult<crate::page::PageGeometry> {
        self.inner.get_page(page)?.geometry()
    }
    pub fn info_dict(&self) -> PdfResult<Option<PdfDict>> {
        self.inner.info_dict()
    }
    pub fn metadata_packet(&self) -> PdfResult<Option<Vec<u8>>> {
        self.inner.metadata_packet()
    }
}
fn invalid(reason: &str) -> PdfError {
    PdfError::UnsupportedFeature(format!("read-only recovery: {reason}"))
}
fn trim_pdf_space(mut bytes: &[u8]) -> &[u8] {
    while bytes
        .first()
        .is_some_and(|b| crate::parser::character::is_white_space(*b))
    {
        bytes = &bytes[1..];
    }
    while bytes
        .last()
        .is_some_and(|b| crate::parser::character::is_white_space(*b))
    {
        bytes = &bytes[..bytes.len() - 1];
    }
    bytes
}
fn reject_unsupported(object: &PdfObject, depth: usize, maximum: usize) -> PdfResult<()> {
    if depth >= maximum {
        return Err(PdfError::ResourceLimit("recovery inspection depth".into()));
    }
    if object.as_stream().is_some_and(|s| {
        s.dict().contains_key("F")
            || s.dict().contains_key("FFilter")
            || s.dict().contains_key("FDecodeParms")
    }) {
        return Err(invalid("external stream source"));
    }
    if let Some(dict) = object
        .as_dict()
        .or_else(|| object.as_stream().map(|s| s.dict()))
    {
        for (key, value) in dict {
            if matches!(
                key.as_str(),
                "ByteRange"
                    | "Encrypt"
                    | "Perms"
                    | "DocMDP"
                    | "FieldMDP"
                    | "Linearized"
                    | "SubFilter"
            ) || (key == "SigFlags" && value.as_u32() != Some(0))
            {
                return Err(invalid("security or linearization marker"));
            }
            if matches!(key.as_str(), "Type" | "FT")
                && (value.as_object_id().is_some()
                    || value.as_name().is_some_and(|n| {
                        matches!(n.name(), "XRef" | "ObjStm" | "Sig" | "DocTimeStamp")
                    }))
            {
                return Err(invalid("unsupported ownership or signature type"));
            }
            reject_unsupported(value, depth + 1, maximum)?;
        }
    } else if let Some(array) = object.as_array() {
        for value in array {
            reject_unsupported(value, depth + 1, maximum)?;
        }
    }
    Ok(())
}
