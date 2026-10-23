use std::cell::{Cell, OnceCell, RefCell};
use std::collections::HashMap;
use std::io::{Read, Seek};

use crate::document::cross_ref_table::ObjectState;
use crate::document::store::StoreEntry;
use crate::objects::object_streams::ObjectStreams;
use crate::{
    document::cross_ref_table::CrossRefTable,
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference},
    parser::parser::PdfParser,
};

#[derive(Debug)]
pub struct PDFContext<R: Seek + Read> {
    derived_fonts: RefCell<
        crate::derived_cache::DerivedCache<ObjectId, std::rc::Rc<crate::font::pdf_font::Font>>,
    >,
    structure_diagnostics: crate::parser::diagnostic::DiagnosticLog,
    parser: Option<RefCell<PdfParser<R>>>,
    mode: crate::parser::syntax::ParseMode,
    cross_ref_table: CrossRefTable,
    indirect_objects: HashMap<ObjectId, OnceCell<PdfObject>>,
    limits: crate::parser::options::Limits,
    loading_objects: RefCell<Vec<ObjectId>>,
    decoding_depth: Cell<usize>,
    color_depth: Cell<usize>,
    cached_stream_bytes: Cell<usize>,
    source_resolution_depth: Cell<usize>,
    security: Option<crate::security::SecurityHandler>,
    encryption_dictionary_id: Option<ObjectId>,
    form_edit_depth: Cell<u32>,
    source_has_signatures: OnceCell<bool>,
    state: crate::document::state::DocumentState,
}

struct DecodeGuard<'a>(&'a Cell<usize>);
impl Drop for DecodeGuard<'_> {
    fn drop(&mut self) {
        self.0.set(self.0.get() - 1);
    }
}

struct LoadingGuard<'a>(&'a RefCell<Vec<ObjectId>>);
impl Drop for LoadingGuard<'_> {
    fn drop(&mut self) {
        self.0.borrow_mut().pop();
    }
}

struct BudgetedLoadingGuard<'a> {
    previous: Option<Vec<ObjectId>>,
    stack: &'a RefCell<Vec<ObjectId>>,
    _lease: crate::budget::MemoryLease,
}

impl Drop for BudgetedLoadingGuard<'_> {
    fn drop(&mut self) {
        let current = self.stack.replace(self.previous.take().unwrap());
        // Free this frame's replacement before releasing its reservation.
        drop(current);
    }
}

impl<R: Seek + Read> PDFContext<R> {
    pub(crate) fn cached_font(
        &self,
        id: ObjectId,
    ) -> Option<std::rc::Rc<crate::font::pdf_font::Font>> {
        self.derived_fonts.borrow_mut().get(self.revision(), &id)
    }
    pub(crate) fn retain_font(&self, id: ObjectId, font: std::rc::Rc<crate::font::pdf_font::Font>) {
        if let Some(bytes) = font.cache_bytes() {
            self.derived_fonts
                .borrow_mut()
                .insert(self.revision(), id, font, bytes);
        }
    }
    pub fn font_cache_statistics(&self) -> (u64, u64, usize, usize) {
        let c = self.derived_fonts.borrow();
        (c.hits, c.misses, c.len(), c.bytes())
    }
    pub(crate) fn clear_font_cache(&self) {
        self.derived_fonts.borrow_mut().clear();
    }

    fn begin_loading_with_budget(
        &self,
        id: ObjectId,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedLoadingGuard<'_>> {
        let mut loading = self.loading_objects.borrow_mut();
        if loading.contains(&id) {
            return Err(PdfError::InvalidReference(format!(
                "cycle loading object {id:?}"
            )));
        }
        if loading.len() >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit(format!(
                "object loading depth at {id:?}"
            )));
        }
        let count = loading.len() + 1;
        let mut lease = budget.reserve(
            count
                .checked_mul(std::mem::size_of::<ObjectId>())
                .ok_or_else(|| PdfError::ResourceLimit("object loading stack size".into()))?,
        )?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("object loading stack allocation".into()))?;
        lease.resize(
            replacement
                .capacity()
                .checked_mul(std::mem::size_of::<ObjectId>())
                .ok_or_else(|| PdfError::ResourceLimit("object loading stack capacity".into()))?,
        )?;
        replacement.extend_from_slice(&loading);
        replacement.push(id);
        let previous = std::mem::replace(&mut *loading, replacement);
        drop(loading);
        Ok(BudgetedLoadingGuard {
            previous: Some(previous),
            stack: &self.loading_objects,
            _lease: lease,
        })
    }
    /// Initialize owned document state without constructing or parsing an input source.
    pub(crate) fn new_without_source(
        trailer: PdfDict,
        options: crate::parser::options::ParseOptions,
    ) -> Self {
        Self::new_without_source_with_version(
            trailer,
            options,
            crate::document::save_options::PdfVersion::V1_7,
        )
    }

    pub(crate) fn new_without_source_with_version(
        trailer: PdfDict,
        options: crate::parser::options::ParseOptions,
        version: crate::document::save_options::PdfVersion,
    ) -> Self {
        Self {
            parser: None,
            structure_diagnostics: crate::parser::diagnostic::DiagnosticLog::new(
                options.limits.max_operation_bytes,
            ),
            mode: options.mode,
            cross_ref_table: CrossRefTable::new_empty(),
            indirect_objects: HashMap::new(),
            state: crate::document::state::DocumentState::new(trailer, 1, version.as_str().into()),
            limits: options.limits,
            loading_objects: RefCell::new(Vec::new()),
            decoding_depth: Cell::new(0),
            color_depth: Cell::new(0),
            cached_stream_bytes: Cell::new(0),
            source_resolution_depth: Cell::new(0),
            security: None,
            encryption_dictionary_id: None,
            form_edit_depth: Cell::new(0),
            source_has_signatures: OnceCell::new(),
            derived_fonts: RefCell::new(crate::derived_cache::DerivedCache::new(
                options.limits.max_operation_bytes.min(16 * 1024 * 1024),
                32,
            )),
        }
    }

    /// Commit an edit group on success; restore edits and allocation state on
    /// error or unwinding. Nested groups act as savepoints.
    pub(crate) fn transaction<T>(&self, edit: impl FnOnce() -> PdfResult<T>) -> PdfResult<T> {
        self.state.transaction(edit)
    }

    pub(crate) fn transaction_with_budget<T>(
        &self,
        budget: &crate::budget::OperationBudget,
        edit: impl FnOnce() -> PdfResult<T>,
    ) -> PdfResult<T> {
        self.state
            .transaction_with_budget(budget, self.limits(), edit)
    }

    pub fn try_new(parser: PdfParser<R>, cross_ref_table: CrossRefTable) -> PdfResult<Self> {
        Self::try_new_with_version(parser, cross_ref_table, "1.7".into())
    }

    pub(crate) fn try_new_with_version(
        parser: PdfParser<R>,
        cross_ref_table: CrossRefTable,
        source_version: String,
    ) -> PdfResult<Self> {
        Self::try_new_with_version_and_password(parser, cross_ref_table, source_version, None)
    }

    pub(crate) fn try_new_with_version_and_password(
        mut parser: PdfParser<R>,
        mut cross_ref_table: CrossRefTable,
        source_version: String,
        password: Option<&[u8]>,
    ) -> PdfResult<Self> {
        let limits = parser.limits();
        let mode = parser.mode();
        if cross_ref_table.objects().len() > limits.max_objects {
            return Err(PdfError::ResourceLimit("document object count".into()));
        }
        let budget = limits.operation_budget();
        let mut encryption_dictionary_id = None;
        let security = match cross_ref_table.trailer().get("Encrypt") {
            None | Some(PdfObject::PdfNull) => None,
            Some(encrypt) => {
                let (_lease, encrypt) = if let Some(id) = encrypt.as_object_id() {
                    encryption_dictionary_id = Some(id);
                    let info = cross_ref_table
                        .lookup_id(id)
                        .filter(|info| matches!(info.state(), ObjectState::Normal))
                        .ok_or_else(|| {
                            PdfError::MalformedSecurity(
                                "Encrypt must be an uncompressed dictionary".into(),
                            )
                        })?;
                    let (value, lease) = parser.parse_indirect_obj_checked_with_budget(
                        info.offset(),
                        id,
                        &budget,
                    )?;
                    (lease, value)
                } else {
                    let (value, lease) =
                        crate::objects::clone_object_with_budget(encrypt, &budget, limits, 0)?;
                    (lease, value)
                };
                let dict = encrypt.as_dict().ok_or_else(|| {
                    PdfError::MalformedSecurity("Encrypt must be a dictionary".into())
                })?;
                // Authenticate before Catalog/source objects are published.
                let handler = crate::security::SecurityHandler::authenticate_with_file_id(
                    dict,
                    password,
                    cross_ref_table
                        .trailer()
                        .get("ID")
                        .and_then(PdfObject::as_array)
                        .and_then(|a| a.get(0))
                        .and_then(PdfObject::as_string)
                        .map(|s| s.bytes()),
                    limits,
                    &budget,
                )?;
                let ids = cross_ref_table
                    .trailer()
                    .get("ID")
                    .and_then(PdfObject::as_array)
                    .filter(|ids| ids.len() == 2)
                    .ok_or_else(|| {
                        PdfError::MalformedSecurity(
                            "encrypted document requires two file IDs".into(),
                        )
                    })?;
                if ids
                    .into_iter()
                    .any(|id| id.as_string().is_none_or(|value| value.bytes().is_empty()))
                {
                    return Err(PdfError::MalformedSecurity(
                        "file IDs must be nonempty strings".into(),
                    ));
                }
                Some(handler)
            }
        };
        let mut indirect_objects = HashMap::new();
        for (objnum, objinfo) in cross_ref_table.objects() {
            if matches!(objinfo.state(), ObjectState::Free) {
                continue;
            }
            let generation = if matches!(objinfo.state(), ObjectState::Compressed) {
                0
            } else {
                objinfo.gennum()
            };
            indirect_objects.insert(ObjectId::new(*objnum, generation), OnceCell::new());
        }
        let next_obj_num = cross_ref_table
            .objects()
            .keys()
            .max()
            .copied()
            .map(|v| {
                v.checked_add(1)
                    .ok_or_else(|| PdfError::ResourceLimit("object number exhausted".into()))
            })
            .transpose()?
            .unwrap_or(1);
        // /Size is also an allocation high-water mark. Do not reuse holes or
        // numbers reserved by an earlier revision with a larger Size.
        let next_obj_num = next_obj_num.max(
            cross_ref_table
                .trailer()
                .get("Size")
                .and_then(PdfObject::as_u32)
                .unwrap_or(0),
        );
        let trailer = cross_ref_table.take_trailer();
        Ok(PDFContext {
            structure_diagnostics: parser.diagnostic_log(),
            parser: Some(RefCell::new(parser)),
            mode,
            cross_ref_table,
            indirect_objects,
            state: crate::document::state::DocumentState::new(
                trailer,
                next_obj_num,
                source_version,
            ),
            limits,
            loading_objects: RefCell::new(Vec::new()),
            decoding_depth: Cell::new(0),
            color_depth: Cell::new(0),
            cached_stream_bytes: Cell::new(0),
            source_resolution_depth: Cell::new(0),
            security,
            encryption_dictionary_id,
            form_edit_depth: Cell::new(0),
            source_has_signatures: OnceCell::new(),
            derived_fonts: RefCell::new(crate::derived_cache::DerivedCache::new(
                limits.max_operation_bytes.min(16 * 1024 * 1024),
                32,
            )),
        })
    }

    pub fn parse_indirect_objects(&mut self) -> PdfResult<()> {
        for object_id in self.indirect_objects.keys() {
            let object_ref = PdfObject::PdfReference(PdfReference::from_id(*object_id));
            let _ = self.resolve(&object_ref)?;
        }
        Ok(())
    }

    pub fn get_root(&self) -> PdfResult<PdfDict> {
        let trailer = self.state.trailer.borrow();
        let root = trailer
            .get("Root")
            .ok_or(PdfError::DocumentError("Root not found".to_string()))?;

        let root = self.resolve(root)?;
        let root = root
            .into_dict()
            .ok_or(PdfError::DocumentError("Root is not a Dict".to_string()))?;
        Ok(root)
    }

    pub fn get_root_owned(&self) -> PdfResult<PdfDict> {
        let trailer = self.state.trailer.borrow();
        let root = trailer
            .get("Root")
            .ok_or(PdfError::DocumentError("Root not found".to_string()))?;

        let root = self.resolve_owned(root)?;
        let root = root
            .into_dict()
            .ok_or(PdfError::DocumentError("Root is not a Dict".to_string()))?;
        Ok(root)
    }

    pub fn trailer(&self) -> std::cell::Ref<'_, PdfDict> {
        self.state.trailer.borrow()
    }

    pub(crate) fn source_xref(&self) -> &CrossRefTable {
        &self.cross_ref_table
    }

    pub(crate) fn save_changes_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(
        crate::document::store::StoreSnapshot,
        crate::budget::MemoryLease,
    )> {
        self.state
            .store
            .borrow()
            .snapshot_with_budget(budget, self.limits)
    }

    pub(crate) fn replace_trailer(&self, trailer: PdfDict) -> PdfResult<()> {
        let revision = self.next_revision()?;
        *self
            .state
            .trailer
            .try_borrow_mut()
            .map_err(|_| PdfError::DocumentError("Trailer is borrowed".into()))? = trailer;
        self.state.did_mutate(revision);
        Ok(())
    }

    pub(crate) fn record_structure_recovery(
        &self,
        reason: crate::parser::diagnostic::StructureReason,
    ) -> PdfResult<()> {
        self.structure_diagnostics
            .record(crate::parser::diagnostic::StructureDiagnostic {
                offset: 0,
                object: None,
                reason,
            })
    }

    pub(crate) fn is_unmodified_source_object(&self, id: ObjectId) -> bool {
        self.parser.is_some() && self.state.store.borrow().entry(id).is_none()
    }

    pub fn revision(&self) -> u64 {
        self.state.revision.get()
    }

    pub fn commit_revision(&self) -> u64 {
        self.state.commit_revision.get()
    }

    pub fn identity(&self) -> crate::document::identity::DocumentIdentity {
        self.state.identity.clone()
    }

    pub fn source_version(&self) -> &str {
        &self.state.source_version
    }

    pub(crate) fn source_security(&self) -> Option<&crate::security::SecurityHandler> {
        self.security.as_ref()
    }

    pub(crate) fn encryption_dictionary_id(&self) -> Option<ObjectId> {
        self.encryption_dictionary_id
    }

    pub(crate) fn require_annotation_permission(&self) -> PdfResult<()> {
        self.require_permission("annotations", |p| p.annotate)
    }

    pub(crate) fn require_assembly_permission(&self) -> PdfResult<()> {
        self.require_permission("document assembly", |p| p.assemble)
    }

    fn require_permission(
        &self,
        operation: &str,
        allowed: impl FnOnce(crate::security::Permissions) -> bool,
    ) -> PdfResult<()> {
        if let Some(handler) = &self.security {
            let report = handler.report();
            if report.authentication == crate::security::Authentication::User
                && !allowed(report.permissions)
            {
                return Err(PdfError::PermissionDenied(operation.into()));
            }
        }
        Ok(())
    }

    pub(crate) fn with_form_edit<T>(&self, edit: impl FnOnce() -> PdfResult<T>) -> PdfResult<T> {
        if self.security.as_ref().is_some_and(|handler| {
            handler.report().authentication != crate::security::Authentication::Owner
                && !handler.report().permissions.fill_forms
        }) {
            return Err(PdfError::PermissionDenied("form filling".into()));
        }
        struct Grant<'a>(&'a Cell<u32>, u32);
        impl Drop for Grant<'_> {
            fn drop(&mut self) {
                self.0.set(self.1);
            }
        }
        let previous = self.form_edit_depth.get();
        self.form_edit_depth.set(
            previous
                .checked_add(1)
                .ok_or_else(|| PdfError::ResourceLimit("form edit depth".into()))?,
        );
        let _grant = Grant(&self.form_edit_depth, previous);
        edit()
    }

    /// Authenticate before edit preparation; denial must not invalidate sources.
    pub(crate) fn require_content_edit_permission(&self) -> PdfResult<()> {
        self.require_unsigned_source()?;
        if self.form_edit_depth.get() == 0
            && self.security.as_ref().is_some_and(|handler| {
                handler.report().authentication != crate::security::Authentication::Owner
                    && !handler.report().permissions.modify
            })
        {
            return Err(PdfError::PermissionDenied("document modification".into()));
        }
        Ok(())
    }

    fn next_revision(&self) -> PdfResult<u64> {
        self.require_content_edit_permission()?;
        self.state.next_revision()
    }

    /// Legacy convenience editors join an enclosing transaction's pool. Explicit
    /// caller-budget APIs still reject a genuinely different pool.
    pub(crate) fn operation_budget(&self) -> crate::budget::OperationBudget {
        self.state
            .active_operation_budget()
            .unwrap_or_else(|| self.limits.operation_budget())
    }

    pub(crate) fn require_unsigned_source(&self) -> PdfResult<()> {
        let budget = self
            .state
            .active_operation_budget()
            .unwrap_or_else(|| self.limits.operation_budget());
        self.require_unsigned_source_with_budget(&budget)
    }

    pub(crate) fn require_unsigned_source_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        let signed = if let Some(value) = self.source_has_signatures.get() {
            *value
        } else {
            let mut signed = false;
            let mut inspected_members = 0usize;
            // A later xref may replace or free a signature object. Inspect its
            // original source version without publishing it to the current
            // object cache or resolving it through the mutable overlay.
            for info in self.source_xref().historical_objects() {
                let (object, _lease) = self
                    .parser
                    .as_ref()
                    .ok_or_else(|| PdfError::InvalidReference("historical source parser".into()))?
                    .borrow_mut()
                    .parse_indirect_obj_checked_with_budget(
                        info.offset(),
                        info.object_id(),
                        budget,
                    )?;
                if crate::signatures::has_signature(&object, self.limits, 0)? {
                    signed = true;
                    break;
                }
                if object.as_stream().is_some_and(|stream| {
                    stream
                        .dict()
                        .get("Type")
                        .and_then(PdfObject::as_name)
                        .is_some_and(|n| n.name() == "ObjStm")
                }) {
                    let decrypted = self
                        .security
                        .as_ref()
                        .map(|handler| {
                            self.with_source_resolution(|| {
                                handler.transform_source(self, &object, info.object_id(), budget)
                            })
                        })
                        .transpose()?;
                    let inspected = decrypted.as_ref().map_or(&object, |(value, _)| value);
                    if self.signature_policy_stream(inspected, budget, &mut inspected_members)? {
                        signed = true;
                        break;
                    }
                }
            }
            for info in self.source_xref().objects().values() {
                if signed {
                    break;
                }
                if matches!(info.state(), ObjectState::Free)
                    || Some(info.object_id()) == self.encryption_dictionary_id
                {
                    continue;
                }
                let (object, _lease) =
                    self.source_object_for_policy_with_budget(info.object_id(), budget)?;
                if crate::signatures::has_signature(&object, self.limits, 0)?
                    || self.signature_policy_stream(&object, budget, &mut inspected_members)?
                {
                    signed = true;
                    break;
                }
            }
            let _ = self.source_has_signatures.set(signed);
            signed
        };
        if signed {
            return Err(PdfError::UnsupportedFeature(
                "editing or rewriting signature-bearing source is unsupported".into(),
            ));
        }
        Ok(())
    }

    // Inspect all members, including ones hidden by later free/replacement
    // entries. Do not populate the current object cache from historical data.
    fn signature_policy_stream(
        &self,
        object: &PdfObject,
        budget: &crate::budget::OperationBudget,
        inspected: &mut usize,
    ) -> PdfResult<bool> {
        let Some(stream) = object.as_stream().filter(|s| {
            s.dict()
                .get("Type")
                .and_then(PdfObject::as_name)
                .is_some_and(|n| n.name() == "ObjStm")
        }) else {
            return Ok(false);
        };
        let data = self.with_source_resolution(|| stream.decode_data_with_budget(self, budget))?;
        let mut parser =
            ObjectStreams::try_new_with_budget(stream.dict(), data, self.options(), budget)?;
        let objects = parser.parse_objects()?;
        *inspected = inspected
            .checked_add(objects.objects.len())
            .filter(|n| *n <= self.limits.max_objects)
            .ok_or_else(|| {
                PdfError::ResourceLimit("signature policy object-stream members".into())
            })?;
        for (_, object) in &objects.objects {
            if crate::signatures::has_signature(object, self.limits, 0)? {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub(crate) fn source_object_end_with_budget(
        &self,
        offset: u64,
        id: ObjectId,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<u64> {
        self.parser
            .as_ref()
            .ok_or_else(|| PdfError::InvalidReference("signature source parser".into()))?
            .borrow_mut()
            .indirect_object_end_with_budget(offset, id, budget)
    }

    // Policy validation is not materialization. In particular, replacing a
    // normal source object must not publish its old body into the read cache.
    // Compressed members reuse the bounded object-stream loader, which owns
    // their decode-once semantics; already published source cells are reused.
    fn source_object_for_policy_with_budget(
        &self,
        id: ObjectId,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
        let cell = self
            .indirect_objects
            .get(&id)
            .ok_or_else(|| PdfError::InvalidReference("source policy object".into()))?;
        if let Some(object) = cell.get() {
            return crate::objects::clone_object_with_budget(object, budget, self.limits, 0);
        }
        let info = self
            .source_xref()
            .lookup_id(id)
            .ok_or_else(|| PdfError::InvalidReference("source policy xref".into()))?;
        if matches!(info.state(), ObjectState::Compressed) {
            return self.resolve_source_owned_with_budget(id, budget);
        }
        if !matches!(info.state(), ObjectState::Normal) {
            return Err(PdfError::InvalidReference(
                "source policy free object".into(),
            ));
        }
        let _guard = self.begin_loading_with_budget(id, budget)?;
        let (object, lease) = self
            .parser
            .as_ref()
            .ok_or_else(|| PdfError::InvalidReference("missing source parser".into()))?
            .borrow_mut()
            .parse_indirect_obj_checked_with_budget(info.offset(), id, budget)?;
        if Some(id) != self.encryption_dictionary_id
            && let Some(handler) = &self.security
        {
            return self
                .with_source_resolution(|| handler.transform_source(self, &object, id, budget));
        }
        Ok((object, lease))
    }

    pub fn limits(&self) -> crate::parser::options::Limits {
        self.limits
    }

    pub fn options(&self) -> crate::parser::options::ParseOptions {
        crate::parser::options::ParseOptions {
            mode: self.mode,
            limits: self.limits,
        }
    }

    pub fn structure_diagnostics(
        &self,
    ) -> PdfResult<Vec<crate::parser::diagnostic::StructureDiagnostic>> {
        self.structure_diagnostics.snapshot()
    }
    pub fn read_provenance(&self) -> crate::parser::diagnostic::ReadProvenance {
        if self.parser.is_some() {
            crate::parser::diagnostic::ReadProvenance::Parsed(self.mode)
        } else {
            crate::parser::diagnostic::ReadProvenance::Created
        }
    }

    pub(crate) fn with_decode_depth<T>(
        &self,
        decode: impl FnOnce() -> PdfResult<T>,
    ) -> PdfResult<T> {
        if self.decoding_depth.get() >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit(
                "nested stream decoding depth".into(),
            ));
        }
        self.decoding_depth.set(self.decoding_depth.get() + 1);
        let _guard = DecodeGuard(&self.decoding_depth);
        decode()
    }

    pub(crate) fn with_color_depth<T>(
        &self,
        construct: impl FnOnce() -> PdfResult<T>,
    ) -> PdfResult<T> {
        if self.color_depth.get() >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit("color space nesting".into()));
        }
        self.color_depth.set(self.color_depth.get() + 1);
        let _guard = DecodeGuard(&self.color_depth);
        construct()
    }

    pub fn object_ids(&self) -> Vec<ObjectId> {
        let mut ids: Vec<_> = self.indirect_objects.keys().copied().collect();
        ids.sort_by_key(|id| (id.obj_num(), id.gen_num()));
        ids
    }

    pub fn serializable_object_ids(&self) -> Vec<ObjectId> {
        let mut ids = self.object_ids();
        for object_id in self.state.store.borrow().entries().keys() {
            if !ids.contains(object_id) {
                ids.push(*object_id);
            }
        }
        ids.sort_by_key(|id| (id.obj_num(), id.gen_num()));
        ids
    }

    pub fn materialize_all_objects(&self) -> PdfResult<()> {
        for object_id in self.object_ids() {
            self.load_indirect_object(object_id)?;
        }
        Ok(())
    }

    pub(crate) fn object_ids_with_budget(
        &self,
        include_edits: bool,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Vec<ObjectId>, crate::budget::MemoryLease)> {
        let store = self.state.store.borrow();
        let count = self
            .indirect_objects
            .len()
            .checked_add(if include_edits {
                store.entries().len()
            } else {
                0
            })
            .ok_or_else(|| PdfError::ResourceLimit("save object ID count".into()))?;
        let mut lease = budget.reserve(
            count
                .checked_mul(std::mem::size_of::<ObjectId>())
                .ok_or_else(|| PdfError::ResourceLimit("save object ID table size".into()))?,
        )?;
        let mut ids = Vec::new();
        ids.try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("save object ID allocation".into()))?;
        lease.resize(
            ids.capacity()
                .checked_mul(std::mem::size_of::<ObjectId>())
                .ok_or_else(|| PdfError::ResourceLimit("save object ID capacity".into()))?,
        )?;
        ids.extend(self.indirect_objects.keys().copied());
        if include_edits {
            ids.extend(store.entries().keys().copied());
        }
        ids.sort_unstable_by_key(|id| (id.obj_num(), id.gen_num()));
        ids.dedup();
        if ids.len() > self.limits.max_objects {
            return Err(PdfError::ResourceLimit("save object count".into()));
        }
        Ok((ids, lease))
    }

    pub(crate) fn materialize_all_objects_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        let (_lease, ids) = {
            let (ids, lease) = self.object_ids_with_budget(false, budget)?;
            (lease, ids)
        };
        for id in ids {
            let owned = self.resolve_source_owned_with_budget(id, budget)?;
            drop(owned);
        }
        Ok(())
    }

    pub fn resolve(&self, obj: &PdfObject) -> PdfResult<PdfObject> {
        match obj {
            PdfObject::PdfReference(reference) => self.resolve_object_id(reference.id()),
            _ => Ok(obj.clone()),
        }
    }

    pub fn resolve_owned(&self, obj: &PdfObject) -> PdfResult<PdfObject> {
        match obj {
            PdfObject::PdfReference(reference) => self.resolve_object_owned(reference.id()),
            _ => Ok(obj.clone()),
        }
    }

    #[cfg(test)]
    pub(crate) fn source_object_is_cached(&self, id: ObjectId) -> bool {
        self.indirect_objects
            .get(&id)
            .is_some_and(|cell| cell.get().is_some())
    }

    /// Budget the owned copy without first making an untracked Clone.
    /// Normal source objects reserve while parsing and while the source and
    /// returned copy coexist. Object-stream decoding, tables and payloads use
    /// the same pool, including scoped replacement loading stacks.
    pub(crate) fn resolve_owned_with_budget(
        &self,
        obj: &PdfObject,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
        let Some(reference) = obj.as_reference() else {
            return crate::objects::clone_object_with_budget(obj, budget, self.limits, 0);
        };
        let id = reference.id();
        if self.source_resolution_depth.get() == 0 {
            let store = self.state.store.borrow();
            match store.entry(id) {
                Some(StoreEntry::Upsert(object)) => {
                    return crate::objects::clone_object_with_budget(
                        object,
                        budget,
                        self.limits,
                        0,
                    );
                }
                Some(StoreEntry::Deleted) => {
                    return Err(PdfError::ObjectError(format!(
                        "object has been deleted: {:?}",
                        id
                    )));
                }
                None => {}
            }
        }
        self.resolve_source_owned_with_budget(id, budget)
    }

    pub(crate) fn resolve_source_owned_with_budget(
        &self,
        id: ObjectId,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
        let cell = self.indirect_objects.get(&id).ok_or_else(|| {
            PdfError::ObjectError(format!("indirect object does not exist: {id:?}"))
        })?;
        if let Some(object) = cell.get() {
            return crate::objects::clone_object_with_budget(object, budget, self.limits, 0);
        }
        let info = self
            .cross_ref_table
            .lookup_id(id)
            .ok_or_else(|| PdfError::ObjectError(format!("xref entry does not exist: {id:?}")))?;
        if matches!(info.state(), ObjectState::Compressed) {
            let _guard = self.begin_loading_with_budget(id, budget)?;
            let stream_number = u32::try_from(info.offset()).map_err(|_| {
                PdfError::InvalidReference("object stream number exceeds u32".into())
            })?;
            let stream_id = ObjectId::new(stream_number, 0);
            let (_stream_lease, stream) = {
                let (stream, lease) = self.resolve_source_owned_with_budget(stream_id, budget)?;
                (lease, stream)
            };
            if cell.get().is_none() {
                self.populate_object_stream_entries_with_budget(&stream, stream_id, budget)?;
            }
            let object = cell.get().ok_or_else(|| {
                PdfError::ObjectError(format!(
                    "compressed object was not populated from object stream: {id:?}"
                ))
            })?;
            return crate::objects::clone_object_with_budget(object, budget, self.limits, 0);
        }
        if !matches!(info.state(), ObjectState::Normal) {
            return Err(PdfError::ObjectError(format!(
                "attempted to resolve free object: {id:?}"
            )));
        }
        let _guard = self.begin_loading_with_budget(id, budget)?;
        let (_source_lease, source) = {
            let (source, lease) = self
                .parser
                .as_ref()
                .ok_or_else(|| PdfError::InvalidReference("document has no source objects".into()))?
                .borrow_mut()
                .parse_indirect_obj_checked_with_budget(info.offset(), id, budget)?;
            (lease, source)
        };
        let mut _decrypted_lease = None;
        let source = if Some(id) != self.encryption_dictionary_id {
            if let Some(handler) = &self.security {
                let (object, lease) = self.with_source_resolution(|| {
                    handler.transform_source(self, &source, id, budget)
                })?;
                _decrypted_lease = Some(lease);
                object
            } else {
                source
            }
        } else {
            source
        };
        let bytes = source
            .as_stream()
            .map(|stream| stream.raw_data().len())
            .unwrap_or(0);
        crate::filter::check_output_size(
            self.cached_stream_bytes.get(),
            bytes,
            self.limits.max_cached_stream_bytes,
            "cached raw streams",
        )?;
        let (copy_lease, copy) = {
            let (copy, lease) =
                crate::objects::clone_object_with_budget(&source, budget, self.limits, 0)?;
            (lease, copy)
        };
        self.populate_object_stream_entries_with_budget(&source, id, budget)?;
        // Publish only after both parse and owned-copy reservations succeed.
        // Persistent cache residency is not charged to the operation pool.
        let _ = cell.set(source);
        self.cached_stream_bytes
            .set(self.cached_stream_bytes.get() + bytes);
        Ok((copy, copy_lease))
    }

    pub fn resolve_object_id(&self, object_id: ObjectId) -> PdfResult<PdfObject> {
        self.resolve_object_owned(object_id)
    }

    /// Read the unmodified source object, ignoring edits in the document store.
    pub fn resolve_original_object(&self, object_id: ObjectId) -> PdfResult<&PdfObject> {
        self.load_indirect_object(object_id)
    }

    pub fn resolve_object_owned(&self, object_id: ObjectId) -> PdfResult<PdfObject> {
        if self.source_resolution_depth.get() != 0 {
            return Ok(self.load_indirect_object(object_id)?.clone());
        }
        match self.state.store.borrow().entry(object_id).cloned() {
            Some(StoreEntry::Upsert(object)) => Ok(object),
            Some(StoreEntry::Deleted) => Err(PdfError::ObjectError(format!(
                "object has been deleted: {:?}",
                object_id
            ))),
            None => Ok(self.load_indirect_object(object_id)?.clone()),
        }
    }

    pub fn object_for_write(&self, object_id: ObjectId) -> PdfResult<Option<PdfObject>> {
        if let Some(entry) = self.state.store.borrow().entry(object_id).cloned() {
            return match entry {
                StoreEntry::Upsert(object) => Ok(Some(object)),
                StoreEntry::Deleted => Ok(None),
            };
        }

        if !self.indirect_objects.contains_key(&object_id) {
            return Err(PdfError::ObjectError(format!(
                "object does not exist in document store: {:?}",
                object_id
            )));
        }
        Ok(Some(self.resolve_object_id(object_id)?.clone()))
    }

    pub(crate) fn object_for_write_with_budget(
        &self,
        id: ObjectId,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<Option<(PdfObject, crate::budget::MemoryLease)>> {
        {
            let store = self.state.store.borrow();
            match store.entry(id) {
                Some(StoreEntry::Upsert(object)) => {
                    return crate::objects::clone_object_with_budget(
                        object,
                        budget,
                        self.limits,
                        0,
                    )
                    .map(Some);
                }
                Some(StoreEntry::Deleted) => return Ok(None),
                None => {}
            }
        }
        if !self.indirect_objects.contains_key(&id) {
            return Err(PdfError::ObjectError(format!(
                "object does not exist in document store: {id:?}"
            )));
        }
        self.resolve_source_owned_with_budget(id, budget).map(Some)
    }

    pub fn insert_object(&self, object: PdfObject) -> ObjectId {
        self.try_insert_object(object)
            .expect("object allocation failed; use try_insert_object")
    }

    pub fn try_insert_object(&self, object: PdfObject) -> PdfResult<ObjectId> {
        let revision = self.next_revision()?;
        let mut store = self.state.store.borrow_mut();
        // Deleted entries still retain state and are counted until explicit cleanup.
        let added = store
            .entries()
            .keys()
            .filter(|id| !self.indirect_objects.contains_key(id))
            .count();
        let count = self
            .indirect_objects
            .len()
            .checked_add(added)
            .and_then(|count| count.checked_add(1))
            .ok_or_else(|| PdfError::ResourceLimit("document object count overflow".into()))?;
        if count > self.limits.max_objects {
            return Err(PdfError::ResourceLimit("document object count".into()));
        }
        let id = store.try_insert_object(object)?;
        self.state.did_mutate(revision);
        Ok(id)
    }

    pub fn update_object(&self, object_id: ObjectId, object: PdfObject) -> PdfResult<()> {
        let revision = self.next_revision()?;
        if !self.indirect_objects.contains_key(&object_id)
            && self.state.store.borrow().entry(object_id).is_none()
        {
            return Err(PdfError::ObjectError(format!(
                "cannot update missing object: {:?}",
                object_id
            )));
        }
        self.state
            .store
            .borrow_mut()
            .update_object(object_id, object)?;
        self.state.did_mutate(revision);
        Ok(())
    }

    pub fn delete_object(&self, object_id: ObjectId) -> PdfResult<()> {
        let revision = self.next_revision()?;
        if !self.indirect_objects.contains_key(&object_id)
            && self.state.store.borrow().entry(object_id).is_none()
        {
            return Err(PdfError::ObjectError(format!(
                "cannot delete missing object: {:?}",
                object_id
            )));
        }
        self.state.store.borrow_mut().delete_object(object_id)?;
        self.state.did_mutate(revision);
        Ok(())
    }

    fn load_indirect_object(&self, object_id: ObjectId) -> PdfResult<&PdfObject> {
        let Some(cell) = self.indirect_objects.get(&object_id) else {
            return Err(PdfError::ObjectError(format!(
                "indirect object does not exist: {:?}",
                object_id
            )));
        };

        if let Some(obj) = cell.get() {
            return Ok(obj);
        }

        {
            let mut loading = self.loading_objects.borrow_mut();
            if loading.contains(&object_id) {
                return Err(PdfError::InvalidReference(format!(
                    "cycle loading object {object_id:?}"
                )));
            }
            if loading.len() >= self.limits.max_depth {
                return Err(PdfError::ResourceLimit(format!(
                    "object loading depth at {object_id:?}"
                )));
            }
            loading.push(object_id);
        }
        let _guard = LoadingGuard(&self.loading_objects);

        let objinfo = self
            .cross_ref_table
            .lookup_id(object_id)
            .ok_or(PdfError::ObjectError(format!(
                "xref entry does not exist: {:?}",
                object_id
            )))?;

        let object = match objinfo.state() {
            ObjectState::Normal => self.parse_indirect_object(objinfo.offset(), object_id),
            ObjectState::Compressed => {
                let stream_number = u32::try_from(objinfo.offset()).map_err(|_| {
                    PdfError::InvalidReference("object stream number exceeds u32".into())
                })?;
                let stream_ref = ObjectId::new(stream_number, 0);
                self.load_object_stream(stream_ref)?;
                self.indirect_objects
                    .get(&object_id)
                    .and_then(OnceCell::get)
                    .cloned()
                    .ok_or(PdfError::ObjectError(format!(
                        "compressed object was not populated from object stream: {:?}",
                        object_id
                    )))
            }
            ObjectState::Free => Err(PdfError::ObjectError(format!(
                "attempted to resolve free object: {:?}",
                object_id
            ))),
        }?;

        let _ = cell.set(object);
        cell.get().ok_or(PdfError::ObjectError(format!(
            "failed to cache resolved object: {:?}",
            object_id
        )))
    }

    fn parse_indirect_object(&self, offset: u64, expected: ObjectId) -> PdfResult<PdfObject> {
        let obj = self
            .parser
            .as_ref()
            .ok_or_else(|| PdfError::InvalidReference("document has no source objects".into()))?
            .borrow_mut()
            .parse_indirect_obj_checked(offset, expected)?;
        let mut _decrypted_lease = None;
        let obj = if Some(expected) != self.encryption_dictionary_id {
            if let Some(handler) = &self.security {
                let (object, lease) = self.with_source_resolution(|| {
                    handler.transform_source(self, &obj, expected, &self.limits.operation_budget())
                })?;
                _decrypted_lease = Some(lease);
                object
            } else {
                obj
            }
        } else {
            obj
        };
        let bytes = obj
            .as_stream()
            .map(|stream| stream.raw_data().len())
            .unwrap_or(0);
        crate::filter::check_output_size(
            self.cached_stream_bytes.get(),
            bytes,
            self.limits.max_cached_stream_bytes,
            "cached raw streams",
        )?;
        self.populate_object_stream_entries(&obj, expected)?;
        self.cached_stream_bytes
            .set(self.cached_stream_bytes.get() + bytes);
        Ok(obj)
    }

    fn populate_object_stream_entries(&self, obj: &PdfObject, source: ObjectId) -> PdfResult<()> {
        let Some(stream) = obj.as_stream() else {
            return Ok(());
        };
        let Some(st) = stream.dict().get("Type") else {
            return Ok(());
        };
        let Some(name) = st.as_name() else {
            return Ok(());
        };
        if name.name() != "ObjStm" {
            return Ok(());
        }

        // Expanded source objects must never depend on tentative or committed
        // edits to decoding dependencies, even on the first lazy load.
        let data = self.with_source_resolution(|| stream.decode_data(self))?;
        let options = crate::parser::options::ParseOptions {
            mode: self.mode,
            limits: self.limits,
        };
        let mut object_streams = ObjectStreams::try_new_with_options(stream.dict(), data, options)?;
        let objects = object_streams.parse_objects()?;
        let mut pending = Vec::new();
        for (index, (objnum, obj)) in objects.into_iter().enumerate() {
            let object_id = ObjectId::new(objnum, 0);
            let Some(info) = self.cross_ref_table.lookup_id(object_id) else {
                continue;
            };
            // Historical entries must not populate current normal/free objects or
            // compressed objects belonging to a different object stream.
            if !matches!(info.state(), ObjectState::Compressed)
                || info.offset() != u64::from(source.obj_num())
                || source.gen_num() != 0
            {
                continue;
            }
            if info.gennum() as usize != index {
                return Err(PdfError::InvalidReference(format!(
                    "compressed object {object_id:?} index {} does not match {index}",
                    info.gennum()
                )));
            }
            pending.push((object_id, obj));
        }
        // Validate all selected entries before committing any cache cells.
        for (object_id, obj) in pending {
            if let Some(cell) = self.indirect_objects.get(&object_id) {
                let _ = cell.set(obj);
            }
        }
        Ok(())
    }

    fn populate_object_stream_entries_with_budget(
        &self,
        object: &PdfObject,
        source: ObjectId,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        let Some(stream) = object.as_stream() else {
            return Ok(());
        };
        if stream
            .dict()
            .get("Type")
            .and_then(PdfObject::as_name)
            .map(|name| name.name())
            != Some("ObjStm")
        {
            return Ok(());
        }
        let data = self.with_source_resolution(|| stream.decode_data_with_budget(self, budget))?;
        let mut parser = ObjectStreams::try_new_with_budget(
            stream.dict(),
            data,
            crate::parser::options::ParseOptions {
                mode: self.mode,
                limits: self.limits,
            },
            budget,
        )?;
        let objects = parser.parse_objects()?;
        let count = objects.objects.len();
        let mut pending_lease = budget.reserve(
            count
                .checked_mul(std::mem::size_of::<(ObjectId, PdfObject)>())
                .ok_or_else(|| {
                    PdfError::ResourceLimit("object stream pending table size".into())
                })?,
        )?;
        let mut pending = Vec::new();
        pending
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("object stream pending allocation".into()))?;
        pending_lease.resize(
            pending
                .capacity()
                .checked_mul(std::mem::size_of::<(ObjectId, PdfObject)>())
                .ok_or_else(|| {
                    PdfError::ResourceLimit("object stream pending table capacity".into())
                })?,
        )?;
        for (index, (number, object)) in objects.objects.into_iter().enumerate() {
            let id = ObjectId::new(number, 0);
            let Some(info) = self.cross_ref_table.lookup_id(id) else {
                continue;
            };
            if !matches!(info.state(), ObjectState::Compressed)
                || info.offset() != u64::from(source.obj_num())
                || source.gen_num() != 0
            {
                continue;
            }
            if info.gennum() as usize != index {
                return Err(PdfError::InvalidReference(format!(
                    "compressed object {id:?} index {} does not match {index}",
                    info.gennum()
                )));
            }
            pending.push((id, object));
        }
        // The parsed payload lease remains held while pending takes ownership.
        // Publish only after all selected entries have been validated.
        for (id, object) in pending {
            if let Some(cell) = self.indirect_objects.get(&id) {
                let _ = cell.set(object);
            }
        }
        drop(objects.lease);
        Ok(())
    }

    fn load_object_stream(&self, stream_ref: ObjectId) -> PdfResult<()> {
        let object_stream = self.load_indirect_object(stream_ref)?;
        let stream = object_stream
            .as_stream()
            .ok_or(PdfError::ObjectError(format!(
                "object stream reference does not point to a stream: {:?}",
                stream_ref
            )))?;
        self.populate_object_stream_entries(&PdfObject::PdfStream(stream.clone()), stream_ref)
    }

    fn with_source_resolution<T>(&self, decode: impl FnOnce() -> PdfResult<T>) -> PdfResult<T> {
        if self.source_resolution_depth.get() >= self.limits.max_depth {
            return Err(PdfError::ResourceLimit(
                "source decoding dependency depth".into(),
            ));
        }
        self.source_resolution_depth
            .set(self.source_resolution_depth.get() + 1);
        let _guard = DecodeGuard(&self.source_resolution_depth);
        decode()
    }
}

#[cfg(test)]
mod loading_tests {
    #[test]
    fn save_budget_ids_preserve_edits_tombstones_and_original_materialization() {
        use crate::{budget::OperationBudget, objects::pdf_string::PdfString};
        let xref = CrossRefTable::new(
            HashMap::from([(1, ObjectInfo::new(1, 0, 0, ObjectState::Normal))]),
            PdfDict::default(),
        );
        let syntax = SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(b"1 0 obj (source) endobj".to_vec()))
                .unwrap(),
        )
        .with_xref_table(xref.clone());
        let ctx = PDFContext::try_new(PdfParser::new(syntax), xref).unwrap();
        let original = ObjectId::new(1, 0);
        ctx.update_object(
            original,
            PdfObject::PdfString(PdfString::new(b"edited".to_vec(), true)),
        )
        .unwrap();
        let added = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        ctx.delete_object(added).unwrap();
        let revision = ctx.revision();
        let pool = OperationBudget::new(65536);
        let (ids, lease) = ctx.object_ids_with_budget(true, &pool).unwrap();
        assert_eq!(ids, ctx.serializable_object_ids());
        assert_eq!(ids, vec![original, added]);
        assert_eq!(
            pool.live_bytes(),
            ids.capacity() * std::mem::size_of::<ObjectId>()
        );
        drop((ids, lease));
        assert_eq!(pool.live_bytes(), 0);
        assert!(
            ctx.object_for_write_with_budget(added, &pool)
                .unwrap()
                .is_none()
        );
        let (object, lease) = ctx
            .object_for_write_with_budget(original, &pool)
            .unwrap()
            .unwrap();
        assert_eq!(object.as_string().unwrap().bytes(), b"edited");
        drop((object, lease));
        assert_eq!(pool.live_bytes(), 0);
        let blocked = OperationBudget::new(0);
        assert!(ctx.materialize_all_objects_with_budget(&blocked).is_err());
        assert!(ctx.indirect_objects[&original].get().is_none());
        ctx.materialize_all_objects_with_budget(&pool).unwrap();
        assert_eq!(
            ctx.indirect_objects[&original]
                .get()
                .unwrap()
                .as_string()
                .unwrap()
                .bytes(),
            b"source"
        );
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(ctx.revision(), revision);
    }
    #[test]
    fn budgeted_loading_stack_restores_prior_storage_and_quotas() {
        use crate::budget::OperationBudget;
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        let original = ObjectId::new(9, 0);
        ctx.loading_objects.borrow_mut().push(original);
        let capacity = ctx.loading_objects.borrow().capacity();
        let first = ObjectId::new(1, 0);
        let second = ObjectId::new(2, 0);
        let pool = OperationBudget::new(65536);
        let root = ctx.begin_loading_with_budget(first, &pool).unwrap();
        let root_bytes = root._lease.bytes();
        assert_eq!(pool.live_bytes(), root_bytes);
        assert_eq!(&*ctx.loading_objects.borrow(), &[original, first]);
        let nested = ctx.begin_loading_with_budget(second, &pool).unwrap();
        let peak = pool.live_bytes();
        assert_eq!(peak, root_bytes + nested._lease.bytes());
        assert_eq!(&*ctx.loading_objects.borrow(), &[original, first, second]);
        assert!(matches!(
            ctx.begin_loading_with_budget(first, &pool),
            Err(PdfError::InvalidReference(_))
        ));
        assert_eq!(pool.live_bytes(), peak);
        drop(nested);
        assert_eq!(pool.live_bytes(), root_bytes);
        assert_eq!(&*ctx.loading_objects.borrow(), &[original, first]);
        drop(root);
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(&*ctx.loading_objects.borrow(), &[original]);
        assert_eq!(ctx.loading_objects.borrow().capacity(), capacity);
        let small = OperationBudget::new(peak - 1);
        let root = ctx.begin_loading_with_budget(first, &small).unwrap();
        assert!(matches!(
            ctx.begin_loading_with_budget(second, &small),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(small.live_bytes(), root._lease.bytes());
        assert_eq!(&*ctx.loading_objects.borrow(), &[original, first]);
        drop(root);
        assert_eq!(small.live_bytes(), 0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _root = ctx.begin_loading_with_budget(first, &pool).unwrap();
            let _nested = ctx.begin_loading_with_budget(second, &pool).unwrap();
            panic!("budgeted loading stack unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(&*ctx.loading_objects.borrow(), &[original]);
        assert_eq!(ctx.loading_objects.borrow().capacity(), capacity);
        let shallow: PDFContext<std::io::Cursor<Vec<u8>>> = PDFContext::new_without_source(
            PdfDict::default(),
            crate::parser::options::ParseOptions {
                limits: crate::parser::options::Limits {
                    max_depth: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let root = shallow.begin_loading_with_budget(first, &pool).unwrap();
        assert!(matches!(
            shallow.begin_loading_with_budget(second, &pool),
            Err(PdfError::ResourceLimit(_))
        ));
        drop(root);
        assert_eq!(pool.live_bytes(), 0);
        assert!(shallow.loading_objects.borrow().is_empty());
    }
    #[test]
    fn budgeted_compressed_source_validates_before_cache_publication() {
        use crate::budget::OperationBudget;
        let fixture = |bad_index| {
            let source = b"10 0 obj << /Type /ObjStm /N 2 /First 8 /Length 18 >> stream\n1 0 2 5 true false\nendstream endobj".to_vec();
            let xref = CrossRefTable::new(
                HashMap::from([
                    (10, ObjectInfo::new(10, 0, 0, ObjectState::Normal)),
                    (1, ObjectInfo::new(1, 10, 0, ObjectState::Compressed)),
                    (
                        2,
                        ObjectInfo::new(
                            2,
                            10,
                            if bad_index { 9 } else { 1 },
                            ObjectState::Compressed,
                        ),
                    ),
                ]),
                PdfDict::default(),
            );
            let syntax =
                SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(source)).unwrap())
                    .with_xref_table(xref.clone());
            PDFContext::try_new(PdfParser::new(syntax), xref).unwrap()
        };
        let reference = PdfObject::PdfReference(PdfReference::from_id(ObjectId::new(1, 0)));
        let pool = OperationBudget::new(1024 * 1024);
        let expected = fixture(false).resolve_owned(&reference).unwrap();
        let ctx = fixture(false);
        let (copy, lease) = ctx.resolve_owned_with_budget(&reference, &pool).unwrap();
        assert_eq!(copy, expected);
        assert_eq!(pool.live_bytes(), lease.bytes());
        let peak = pool.peak_bytes();
        drop((copy, lease));
        assert_eq!(pool.live_bytes(), 0);
        assert!(ctx.loading_objects.borrow().is_empty());
        let small = OperationBudget::new(peak - 1);
        let denied = fixture(false);
        assert!(matches!(
            denied.resolve_owned_with_budget(&reference, &small),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(small.live_bytes(), 0);
        assert!(denied.loading_objects.borrow().is_empty());
        let (copy, lease) = denied.resolve_owned_with_budget(&reference, &pool).unwrap();
        assert_eq!(copy, expected);
        drop((copy, lease));
        assert_eq!(pool.live_bytes(), 0);
        for _ in 0..2 {
            let invalid = fixture(true);
            assert!(matches!(
                invalid.resolve_owned_with_budget(&reference, &pool),
                Err(PdfError::InvalidReference(_))
            ));
            assert_eq!(pool.live_bytes(), 0);
            assert!(invalid.loading_objects.borrow().is_empty());
            assert_eq!(invalid.cached_stream_bytes.get(), 0);
            for number in [1, 2, 10] {
                assert!(
                    invalid.indirect_objects[&ObjectId::new(number, 0)]
                        .get()
                        .is_none()
                );
            }
        }
    }
    #[test]
    fn cold_normal_source_budget_covers_parse_copy_and_cache_publication() {
        use crate::budget::OperationBudget;
        let fixture = |bytes: Vec<u8>| {
            let xref = CrossRefTable::new(
                HashMap::from([(1, ObjectInfo::new(1, 0, 0, ObjectState::Normal))]),
                PdfDict::default(),
            );
            let syntax =
                SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(bytes)).unwrap())
                    .with_xref_table(xref.clone());
            PDFContext::try_new(PdfParser::new(syntax), xref).unwrap()
        };
        let id = ObjectId::new(1, 0);
        let reference = PdfObject::PdfReference(PdfReference::from_id(id));
        let stream_source = format!(
            "1 0 obj << /Length 16384 /Private (kept) >> stream\n{}\nendstream endobj",
            "x".repeat(16384)
        )
        .into_bytes();
        for source in [
            b"1 0 obj (hello) endobj".to_vec(),
            b"1 0 obj << /A [(one) /Two 3] /B true >> endobj".to_vec(),
            stream_source,
        ] {
            let ordinary = fixture(source.clone()).resolve_owned(&reference).unwrap();
            let ctx = fixture(source.clone());
            let pool = OperationBudget::new(1024 * 1024);
            let (copy, lease) = ctx.resolve_owned_with_budget(&reference, &pool).unwrap();
            assert_eq!(copy, ordinary);
            assert_eq!(pool.live_bytes(), lease.bytes());
            assert!(ctx.indirect_objects.get(&id).unwrap().get().is_some());
            assert!(ctx.loading_objects.borrow().is_empty());
            let peak = pool.peak_bytes();
            let retained = lease.bytes();
            if copy.as_stream().is_some() {
                assert!(peak > retained * 3 / 2);
            }
            drop((copy, lease));
            assert_eq!(pool.live_bytes(), 0);
            let denied = fixture(source.clone());
            let small = OperationBudget::new(peak - 1);
            assert!(matches!(
                denied.resolve_owned_with_budget(&reference, &small),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(small.live_bytes(), 0);
            assert!(denied.indirect_objects.get(&id).unwrap().get().is_none());
            assert_eq!(denied.cached_stream_bytes.get(), 0);
            assert!(denied.loading_objects.borrow().is_empty());
            let (copy, lease) = denied.resolve_owned_with_budget(&reference, &pool).unwrap();
            assert_eq!(copy, ordinary);
            drop((copy, lease));
            assert_eq!(pool.live_bytes(), 0);
            // Cache residency is persistent; only the next owned copy is paid.
            let warm = OperationBudget::new(peak);
            let (copy, lease) = denied.resolve_owned_with_budget(&reference, &warm).unwrap();
            assert_eq!(copy, ordinary);
            assert_eq!(warm.live_bytes(), lease.bytes());
            if copy.as_stream().is_some() {
                assert!(warm.peak_bytes() < peak);
            }
            drop((copy, lease));
            assert_eq!(warm.live_bytes(), 0);
            let ctx = fixture(source);
            let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _owned = ctx.resolve_owned_with_budget(&reference, &pool).unwrap();
                panic!("cold source copy unwind");
            }));
            assert!(caught.is_err());
            assert_eq!(pool.live_bytes(), 0);
            assert!(ctx.loading_objects.borrow().is_empty());
        }
        for source in [
            b"2 0 obj (bad id) endobj".as_slice(),
            b"1 0 obj (bad end) nope".as_slice(),
        ] {
            let ctx = fixture(source.to_vec());
            let pool = OperationBudget::new(65536);
            assert!(ctx.resolve_owned_with_budget(&reference, &pool).is_err());
            assert_eq!(pool.live_bytes(), 0);
            assert!(ctx.indirect_objects.get(&id).unwrap().get().is_none());
            assert!(ctx.loading_objects.borrow().is_empty());
        }
    }

    #[test]
    fn budgeted_owned_resolution_preserves_store_deletion_and_source_scope() {
        use crate::{
            budget::OperationBudget,
            objects::{pdf_reference::PdfReference, pdf_string::PdfString},
        };
        let ctx: super::PDFContext<std::io::Cursor<Vec<u8>>> =
            super::PDFContext::new_without_source(PdfDict::default(), Default::default());
        let object = PdfObject::PdfString(PdfString::new(b"abc".to_vec(), true));
        let id = ctx.try_insert_object(object.clone()).unwrap();
        let reference = PdfObject::PdfReference(PdfReference::from_id(id));
        let budget = OperationBudget::new(3);
        let neighbor = budget.reserve(1).unwrap();
        for input in [&object, &reference] {
            assert!(ctx.resolve_owned_with_budget(input, &budget).is_err());
            assert_eq!(budget.live_bytes(), 1);
        }
        drop(neighbor);
        for input in [&object, &reference] {
            let (copy, lease) = ctx.resolve_owned_with_budget(input, &budget).unwrap();
            assert_eq!(copy, object);
            assert_eq!(budget.live_bytes(), 3);
            drop(copy);
            drop(lease);
            assert_eq!(budget.live_bytes(), 0);
        }
        assert!(
            ctx.with_source_resolution(|| ctx.resolve_owned_with_budget(&reference, &budget))
                .is_err()
        );
        assert_eq!(ctx.source_resolution_depth.get(), 0);
        assert_eq!(budget.live_bytes(), 0);
        ctx.delete_object(id).unwrap();
        assert!(ctx.resolve_owned_with_budget(&reference, &budget).is_err());
        assert_eq!(budget.live_bytes(), 0);
        let (copy, lease) = ctx.resolve_owned_with_budget(&object, &budget).unwrap();
        assert_eq!(copy, object);
        drop(copy);
        drop(lease);
        assert_eq!(budget.live_bytes(), 0);
    }

    use super::*;
    use crate::{
        document::cross_ref_table::ObjectInfo, io::stream_reader::StreamReader,
        parser::syntax::SyntaxParser,
    };

    #[test]
    fn revision_exhaustion_reserves_nested_rollback_capacity() {
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        let id = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        ctx.state.revision.set(u64::MAX - 2);
        let result: PdfResult<()> = ctx.transaction(|| {
            ctx.update_object(id, PdfObject::PdfDict(PdfDict::default()))?;
            assert_eq!(ctx.revision(), u64::MAX - 1);
            assert!(matches!(
                ctx.update_object(id, PdfObject::PdfNull),
                Err(PdfError::ResourceLimit(_))
            ));
            assert!(matches!(
                ctx.transaction(|| Ok(())),
                Err(PdfError::ResourceLimit(_))
            ));
            Err(PdfError::DocumentError("rollback".into()))
        });
        assert!(result.is_err());
        assert_eq!(ctx.revision(), u64::MAX);
        assert_eq!(ctx.state.transaction_depth.get(), 0);
        assert_eq!(ctx.resolve_object_owned(id).unwrap(), PdfObject::PdfNull);
        assert!(matches!(
            ctx.try_insert_object(PdfObject::PdfNull),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn parsed_trailer_is_owned_by_document_state_not_source_xref() {
        use crate::objects::pdf_name::PdfName;
        let reader = StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        let parser = PdfParser::new(SyntaxParser::new(reader));
        let mut trailer = PdfDict::default();
        trailer
            .try_insert(
                "Custom".into(),
                PdfObject::PdfName(PdfName::new("preserved".into())),
            )
            .unwrap();
        let ctx = PDFContext::try_new(parser, CrossRefTable::new(HashMap::new(), trailer)).unwrap();
        assert!(ctx.cross_ref_table.trailer().get("Custom").is_none());
        assert_eq!(
            ctx.trailer()
                .get("Custom")
                .unwrap()
                .as_name()
                .unwrap()
                .bytes(),
            b"preserved"
        );
    }

    #[test]
    fn editing_object_limit_counts_deleted_entries_and_rolls_back() {
        use crate::parser::options::{Limits, ParseOptions};
        let reader = StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        let syntax = SyntaxParser::with_options(
            reader,
            ParseOptions {
                limits: Limits {
                    max_objects: 2,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let ctx = PDFContext::try_new(
            PdfParser::new(syntax),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let first = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        let rolled_back: PdfResult<()> = ctx.transaction(|| {
            ctx.try_insert_object(PdfObject::PdfNull)?;
            ctx.try_insert_object(PdfObject::PdfNull)?;
            Ok(())
        });
        assert!(matches!(rolled_back, Err(PdfError::ResourceLimit(_))));
        let second = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        assert_eq!(second, ObjectId::new(2, 0));
        ctx.delete_object(first).unwrap();
        for _ in 0..2 {
            assert!(matches!(
                ctx.try_insert_object(PdfObject::PdfNull),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert_eq!(ctx.state.store.borrow().entries().len(), 2);
    }

    #[test]
    fn context_rejects_exhausted_source_object_number() {
        let reader = StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        let parser = PdfParser::new(SyntaxParser::new(reader));
        let entries = HashMap::from([(
            u32::MAX,
            ObjectInfo::new(u32::MAX, 0, 0, ObjectState::Normal),
        )]);
        assert!(matches!(
            PDFContext::try_new(parser, CrossRefTable::new(entries, PdfDict::default())),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn editing_count_does_not_double_charge_source_updates() {
        use crate::parser::options::{Limits, ParseOptions};
        let reader =
            StreamReader::try_new(std::io::Cursor::new(b"1 0 obj null endobj".to_vec())).unwrap();
        let syntax = SyntaxParser::with_options(
            reader,
            ParseOptions {
                limits: Limits {
                    max_objects: 2,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        let id = ObjectId::new(1, 0);
        let entries = HashMap::from([(1, ObjectInfo::new(1, 0, 0, ObjectState::Normal))]);
        let ctx = PDFContext::try_new(
            PdfParser::new(syntax),
            CrossRefTable::new(entries, PdfDict::default()),
        )
        .unwrap();
        ctx.update_object(id, PdfObject::PdfNull).unwrap();
        assert_eq!(
            ctx.try_insert_object(PdfObject::PdfNull).unwrap(),
            ObjectId::new(2, 0)
        );
        assert!(matches!(
            ctx.try_insert_object(PdfObject::PdfNull),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn compressed_object_cycle_returns_error_and_clears_loading_state() {
        let reader = StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap();
        let parser = PdfParser::new(SyntaxParser::new(reader));
        let mut entries = HashMap::new();
        entries.insert(1, ObjectInfo::new(1, 1, 0, ObjectState::Compressed));
        let ctx =
            PDFContext::try_new(parser, CrossRefTable::new(entries, PdfDict::default())).unwrap();
        for _ in 0..2 {
            assert!(matches!(
                ctx.resolve_object_owned(ObjectId::new(1, 0)),
                Err(PdfError::InvalidReference(_))
            ));
            assert!(ctx.loading_objects.borrow().is_empty());
        }
    }

    #[test]
    fn incorrect_xref_header_is_rejected_without_caching() {
        for header in ["2 0 obj 42 endobj", "1 1 obj 42 endobj"] {
            let reader = StreamReader::try_new(std::io::Cursor::new(header.as_bytes())).unwrap();
            let parser = PdfParser::new(SyntaxParser::new(reader));
            let ctx = PDFContext::try_new(
                parser,
                CrossRefTable::new(
                    HashMap::from([(1, ObjectInfo::new(1, 0, 0, ObjectState::Normal))]),
                    PdfDict::default(),
                ),
            )
            .unwrap();
            for _ in 0..2 {
                assert!(matches!(
                    ctx.resolve_object_owned(ObjectId::new(1, 0)),
                    Err(PdfError::InvalidReference(_))
                ));
                assert!(ctx.indirect_objects[&ObjectId::new(1, 0)].get().is_none());
                assert!(ctx.loading_objects.borrow().is_empty());
            }
        }
    }

    #[test]
    fn object_stream_cache_respects_current_xref_and_validates_before_commit() {
        let source = "10 0 obj << /Type /ObjStm /N 2 /First 8 /Length 13 >> stream\n1 0 2 3 11 22\nendstream\nendobj\n2 0 obj 99 endobj";
        for bad_index in [false, true] {
            let reader = StreamReader::try_new(std::io::Cursor::new(source.as_bytes())).unwrap();
            let entry2 = if bad_index {
                ObjectInfo::new(2, 10, 0, ObjectState::Compressed)
            } else {
                ObjectInfo::new(
                    2,
                    source.find("2 0 obj").unwrap() as u64,
                    0,
                    ObjectState::Normal,
                )
            };
            let ctx = PDFContext::try_new(
                PdfParser::new(SyntaxParser::new(reader)),
                CrossRefTable::new(
                    HashMap::from([
                        (10, ObjectInfo::new(10, 0, 0, ObjectState::Normal)),
                        (1, ObjectInfo::new(1, 10, 0, ObjectState::Compressed)),
                        (2, entry2),
                    ]),
                    PdfDict::default(),
                ),
            )
            .unwrap();
            if bad_index {
                for _ in 0..2 {
                    assert!(matches!(
                        ctx.resolve_object_owned(ObjectId::new(1, 0)),
                        Err(PdfError::InvalidReference(_))
                    ));
                    for id in [1, 2, 10] {
                        assert!(ctx.indirect_objects[&ObjectId::new(id, 0)].get().is_none());
                    }
                    assert_eq!(ctx.cached_stream_bytes.get(), 0);
                    assert!(ctx.loading_objects.borrow().is_empty());
                }
            } else {
                assert_eq!(
                    ctx.resolve_object_owned(ObjectId::new(1, 0))
                        .unwrap()
                        .as_number()
                        .unwrap()
                        .lexeme(),
                    "11"
                );
                assert_eq!(
                    ctx.resolve_object_owned(ObjectId::new(2, 0))
                        .unwrap()
                        .as_number()
                        .unwrap()
                        .lexeme(),
                    "99"
                );
            }
        }
    }

    #[test]
    fn object_stream_source_decode_ignores_edits_to_indirect_parameters() {
        use crate::objects::pdf_number::PdfNumber;
        use std::io::Write;
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(b"1 0 11").unwrap();
        let encoded = encoder.finish().unwrap();
        for (delete, array, use_budget) in
            [(false, false), (true, false), (false, true), (true, true)]
                .into_iter()
                .flat_map(|(delete, array)| {
                    [false, true].map(move |use_budget| (delete, array, use_budget))
                })
        {
            let (filter, params) = if array {
                ("[7 0 R]", "[6 0 R]")
            } else {
                ("7 0 R", "6 0 R")
            };
            let mut source = format!("10 0 obj << /Type /ObjStm /N 1 /First 4 /Filter {filter} /DecodeParms {params} /Length {} >> stream\n", encoded.len()).into_bytes();
            source.extend_from_slice(&encoded);
            source.extend_from_slice(b"\nendstream\nendobj\n");
            let params_offset = source.len() as u64;
            source.extend_from_slice(b"9 0 obj << /Predictor 8 0 R >> endobj\n");
            let scalar_offset = source.len() as u64;
            source.extend_from_slice(b"8 0 obj 1 endobj\n");
            let filter_offset = source.len() as u64;
            source.extend_from_slice(b"7 0 obj /FlateDecode endobj\n");
            let chain_offset = source.len() as u64;
            source.extend_from_slice(b"6 0 obj 9 0 R endobj\n");
            let reader = StreamReader::try_new(std::io::Cursor::new(source)).unwrap();
            let ctx = PDFContext::try_new(
                PdfParser::new(SyntaxParser::new(reader)),
                CrossRefTable::new(
                    HashMap::from([
                        (10, ObjectInfo::new(10, 0, 0, ObjectState::Normal)),
                        (1, ObjectInfo::new(1, 10, 0, ObjectState::Compressed)),
                        (9, ObjectInfo::new(9, params_offset, 0, ObjectState::Normal)),
                        (8, ObjectInfo::new(8, scalar_offset, 0, ObjectState::Normal)),
                        (7, ObjectInfo::new(7, filter_offset, 0, ObjectState::Normal)),
                        (6, ObjectInfo::new(6, chain_offset, 0, ObjectState::Normal)),
                    ]),
                    PdfDict::default(),
                ),
            )
            .unwrap();
            let params_id = ObjectId::new(9, 0);
            if delete {
                ctx.delete_object(params_id).unwrap();
            } else {
                let mut params = PdfDict::default();
                params
                    .try_insert(
                        "Predictor".into(),
                        PdfObject::PdfNumber(PdfNumber::new(99.)),
                    )
                    .unwrap();
                ctx.update_object(params_id, PdfObject::PdfDict(params))
                    .unwrap();
            }
            let pool = crate::budget::OperationBudget::new(1024 * 1024);
            let (lease, object) = if use_budget {
                let (object, lease) = ctx
                    .resolve_owned_with_budget(
                        &PdfObject::PdfReference(PdfReference::from_id(ObjectId::new(1, 0))),
                        &pool,
                    )
                    .unwrap();
                (Some(lease), object)
            } else {
                (None, ctx.resolve_object_owned(ObjectId::new(1, 0)).unwrap())
            };
            assert_eq!(object.as_number().unwrap().lexeme(), "11");
            drop(object);
            drop(lease);
            assert_eq!(pool.live_bytes(), 0);
            // Decoding the same stream normally still uses the latest view.
            let stream = ctx.resolve_object_owned(ObjectId::new(10, 0)).unwrap();
            assert!(stream.as_stream().unwrap().decode_data(&ctx).is_err());
            if delete {
                assert!(ctx.resolve_object_owned(params_id).is_err());
            } else {
                assert_eq!(
                    ctx.resolve_object_owned(params_id)
                        .unwrap()
                        .as_dict()
                        .unwrap()
                        .get("Predictor")
                        .unwrap()
                        .as_number()
                        .unwrap()
                        .lexeme(),
                    "99"
                );
            }
        }
    }

    #[test]
    fn source_resolution_scope_is_restored_after_error_and_unwind() {
        use std::panic::{AssertUnwindSafe, catch_unwind};
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        let id = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        let result: PdfResult<()> = ctx.with_source_resolution(|| {
            assert!(ctx.resolve_object_owned(id).is_err());
            Err(PdfError::DocumentError("scope error".into()))
        });
        assert!(result.is_err());
        assert_eq!(ctx.source_resolution_depth.get(), 0);
        assert_eq!(ctx.resolve_object_owned(id).unwrap(), PdfObject::PdfNull);
        assert!(
            catch_unwind(AssertUnwindSafe(|| {
                let _: PdfResult<()> = ctx.with_source_resolution(|| {
                    panic!("scope unwind");
                });
            }))
            .is_err()
        );
        assert_eq!(ctx.source_resolution_depth.get(), 0);
        assert_eq!(ctx.resolve_object_owned(id).unwrap(), PdfObject::PdfNull);
    }
}
