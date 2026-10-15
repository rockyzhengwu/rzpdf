use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    path::Path,
};

use crate::{
    document::catalog::Catalog,
    error::{PdfError, PdfResult},
    geom::rectangle::Rectangle,
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream,
    },
    page::{PdfPage, content_builder::ContentBuilder},
    parser::{
        parser::{PdfParser, parse_xref_with_offset},
        syntax::SyntaxParser,
    },
    pdf_context::PDFContext,
};

pub mod acroform;
pub mod annotations;
pub mod attachments;
pub mod bookmarks;
pub mod catalog;
mod content_edit;
pub mod cross_ref_table;
pub mod destinations;
pub mod embedded_font;
pub mod glyph_run;
pub mod identity;
pub mod links;
pub mod metadata;
mod nested_edit;
pub mod owned;
mod span_tree;
pub mod subset_text_edit;
mod text_event_edit;
pub use owned::Document;
mod page_copy;
mod page_geometry;
mod page_order;
pub mod page_overlay;
pub mod page_tree;
mod resource_import;
pub mod resource_manager;
pub mod save_options;
pub(crate) mod state;
pub mod store;

use page_tree::PageTreeEntry;

pub struct PdfDocument<R: Seek + Read> {
    ctx: PDFContext<R>,
    catalog: Catalog,
    // A merged xref table alone cannot identify the latest revision address.
    source_xref_offset: Option<u64>,
}

impl PdfDocument<BufReader<File>> {
    pub fn open<P: AsRef<Path>>(path: P) -> PdfResult<PdfDocument<BufReader<File>>> {
        Self::open_with_mode(path, crate::parser::syntax::ParseMode::Compatible)
    }

    pub fn open_with_mode<P: AsRef<Path>>(
        path: P,
        mode: crate::parser::syntax::ParseMode,
    ) -> PdfResult<PdfDocument<BufReader<File>>> {
        Self::open_with_options(
            path,
            crate::parser::options::ParseOptions {
                mode,
                ..Default::default()
            },
        )
    }

    pub fn open_with_options<P: AsRef<Path>>(
        path: P,
        options: crate::parser::options::ParseOptions,
    ) -> PdfResult<PdfDocument<BufReader<File>>> {
        let file = File::open(path)
            .map_err(|e| PdfError::DocumentError(format!("load document failed:{:?}", e)))?;
        let reader = BufReader::new(file);
        Self::from_reader_with_options(reader, options)
    }

    pub fn load<P: AsRef<Path>>(path: P) -> PdfResult<PdfDocument<BufReader<File>>> {
        Self::open(path)
    }
}
impl PdfDocument<Cursor<Vec<u8>>> {
    /// Create an empty PDF 1.7 document, without reading a template PDF.
    pub fn new() -> PdfResult<Self> {
        Self::new_with_options(crate::parser::options::ParseOptions::default())
    }

    pub fn new_with_options(options: crate::parser::options::ParseOptions) -> PdfResult<Self> {
        Self::create_with_options(options)
    }

    /// Select the initial header version, not a PDF conformance profile.
    pub fn new_with_version(version: save_options::PdfVersion) -> PdfResult<Self> {
        Self::new_with_version_and_options(version, Default::default())
    }

    pub fn new_with_version_and_options(
        version: save_options::PdfVersion,
        options: crate::parser::options::ParseOptions,
    ) -> PdfResult<Self> {
        Self::create_with_version_and_options(version, options)
    }

    pub fn from_bytes(buffer: Vec<u8>) -> PdfResult<PdfDocument<Cursor<Vec<u8>>>> {
        Self::from_bytes_with_mode(buffer, crate::parser::syntax::ParseMode::Compatible)
    }

    pub fn from_bytes_with_mode(
        buffer: Vec<u8>,
        mode: crate::parser::syntax::ParseMode,
    ) -> PdfResult<PdfDocument<Cursor<Vec<u8>>>> {
        Self::from_reader_with_mode(Cursor::new(buffer), mode)
    }

    pub fn from_bytes_with_options(
        buffer: Vec<u8>,
        options: crate::parser::options::ParseOptions,
    ) -> PdfResult<PdfDocument<Cursor<Vec<u8>>>> {
        Self::from_reader_with_options(Cursor::new(buffer), options)
    }

    pub fn load(buffer: Vec<u8>) -> PdfResult<PdfDocument<Cursor<Vec<u8>>>> {
        Self::from_bytes(buffer)
    }
}

impl<R: Seek + Read> PdfDocument<R> {
    pub(crate) fn create_with_options(
        options: crate::parser::options::ParseOptions,
    ) -> PdfResult<Self> {
        Self::create_with_version_and_options(save_options::PdfVersion::V1_7, options)
    }

    pub(crate) fn create_with_version_and_options(
        version: save_options::PdfVersion,
        options: crate::parser::options::ParseOptions,
    ) -> PdfResult<Self> {
        let mut trailer = PdfDict::default();
        trailer.try_insert(
            "Root".into(),
            PdfObject::PdfReference(PdfReference::from_id(ObjectId::new(2, 0))),
        )?;
        let ctx = PDFContext::new_without_source_with_version(trailer, options, version);
        let mut pages = PdfDict::default();
        pages.try_insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new("Pages".into())),
        )?;
        pages.try_insert("Kids".into(), PdfObject::PdfArray(PdfArray::default()))?;
        pages.try_insert("Count".into(), PdfObject::PdfNumber(PdfNumber::new(0.0)))?;
        let pages_id = ctx.try_insert_object(PdfObject::PdfDict(pages))?;
        let mut root = PdfDict::default();
        root.try_insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new("Catalog".into())),
        )?;
        root.try_insert(
            "Pages".into(),
            PdfObject::PdfReference(PdfReference::from_id(pages_id)),
        )?;
        ctx.try_insert_object(PdfObject::PdfDict(root))?;
        let catalog = Catalog::try_new(&ctx)?;
        Ok(Self {
            ctx,
            catalog,
            source_xref_offset: None,
        })
    }

    pub(crate) fn ctx(&self) -> &PDFContext<R> {
        &self.ctx
    }

    pub fn from_reader(reader: R) -> PdfResult<PdfDocument<R>> {
        Self::from_reader_with_mode(reader, crate::parser::syntax::ParseMode::Compatible)
    }

    pub fn from_reader_with_mode(
        reader: R,
        mode: crate::parser::syntax::ParseMode,
    ) -> PdfResult<PdfDocument<R>> {
        Self::from_reader_with_options(
            reader,
            crate::parser::options::ParseOptions {
                mode,
                ..Default::default()
            },
        )
    }

    pub fn from_reader_with_options(
        reader: R,
        options: crate::parser::options::ParseOptions,
    ) -> PdfResult<PdfDocument<R>> {
        Self::from_reader_with_password_bytes(reader, options, None)
    }

    pub fn from_reader_with_password_and_options(
        reader: R,
        password: &str,
        options: crate::parser::options::ParseOptions,
    ) -> PdfResult<Self> {
        Self::from_reader_with_password_bytes(reader, options, Some(password.as_bytes()))
    }

    pub(crate) fn from_reader_with_password_bytes(
        reader: R,
        options: crate::parser::options::ParseOptions,
        password: Option<&[u8]>,
    ) -> PdfResult<Self> {
        let reader = StreamReader::try_new(reader)?;
        let mut syntax = SyntaxParser::with_options(reader, options);
        let version = syntax.read_header()?.version().to_string();
        let (cross_ref_table, source_xref_offset) = parse_xref_with_offset(&mut syntax)?;
        let parser = PdfParser::new(syntax.with_xref_table(cross_ref_table.clone()));
        let ctx = PDFContext::try_new_with_version_and_password(
            parser,
            cross_ref_table,
            version,
            password,
        )?;
        let catalog = Catalog::try_new(&ctx)?;
        let document = PdfDocument {
            ctx,
            catalog,
            source_xref_offset: Some(source_xref_offset),
        };
        if document.ctx.source_security().is_some() {
            let budget = document.ctx.limits().operation_budget();
            document.validate_security_version(
                document.effective_version_with_budget(&budget)?,
                &budget,
            )?;
        }
        Ok(document)
    }

    pub fn security_report(&self) -> Option<crate::security::SecurityReport> {
        self.ctx.source_security().map(|handler| handler.report())
    }

    pub(crate) fn validate_security_version(
        &self,
        version: save_options::PdfVersion,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        if version >= save_options::PdfVersion::V2_0 {
            return Ok(());
        }
        if version != save_options::PdfVersion::V1_7 {
            return Err(PdfError::UnsupportedSecurity(
                "R6 requires PDF 2.0 or PDF 1.7 extension level 8".into(),
            ));
        }
        let root = self
            .ctx
            .trailer()
            .get("Root")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| PdfError::MalformedSecurity("encrypted Root must be indirect".into()))?;
        let (mut value, mut lease) = self.ctx.resolve_owned_with_budget(
            &PdfObject::PdfReference(PdfReference::from_id(root)),
            budget,
        )?;
        for key in ["Extensions", "ADBE"] {
            let next = value
                .as_dict()
                .and_then(|dict| dict.get(key))
                .ok_or_else(|| {
                    PdfError::UnsupportedSecurity("missing PDF 1.7 R6 extension declaration".into())
                })?;
            let (next, next_lease) = self.ctx.resolve_owned_with_budget(next, budget)?;
            value = next;
            lease = next_lease;
        }
        let dict = value
            .as_dict()
            .ok_or_else(|| PdfError::MalformedSecurity("R6 extension dictionary".into()))?;
        if dict
            .get("BaseVersion")
            .and_then(PdfObject::as_name)
            .is_none_or(|name| name.name() != "1.7")
            || dict
                .get("ExtensionLevel")
                .and_then(PdfObject::as_u32)
                .is_none_or(|level| level < 8)
        {
            return Err(PdfError::UnsupportedSecurity(
                "R6 requires PDF 1.7 extension level 8".into(),
            ));
        }
        drop(value);
        drop(lease);
        Ok(())
    }

    /// Source header version. Catalog /Version, if present, remains in the
    /// object graph and can specify a higher effective document version.
    pub fn version(&self) -> &str {
        self.ctx.source_version()
    }

    /// Latest effective version: the greater of Header and Catalog /Version.
    /// Does not establish conformance to that version.
    pub fn effective_version(&self) -> PdfResult<save_options::PdfVersion> {
        let header = save_options::PdfVersion::parse(self.version())
            .ok_or_else(|| PdfError::UnsupportedFeature("unknown PDF Header version".into()))?;
        let root = self.ctx.get_root_owned()?;
        let Some(value) = root.get("Version") else {
            return Ok(header);
        };
        let value = self.ctx.resolve(value)?;
        Self::merge_catalog_version(header, &value)
    }

    pub(crate) fn effective_version_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<save_options::PdfVersion> {
        let header = save_options::PdfVersion::parse(self.version())
            .ok_or_else(|| PdfError::UnsupportedFeature("unknown PDF Header version".into()))?;
        let (_root_lease, root) = {
            let trailer = self.ctx.trailer();
            let root = trailer
                .get("Root")
                .ok_or_else(|| PdfError::DocumentError("Root not found".into()))?;
            let (root, lease) = self.ctx.resolve_owned_with_budget(root, budget)?;
            (lease, root)
        };
        let root = root
            .as_dict()
            .ok_or_else(|| PdfError::DocumentError("Root is not a Dict".into()))?;
        let Some(value) = root.get("Version") else {
            return Ok(header);
        };
        let (_value_lease, value) = {
            let (value, lease) = self.ctx.resolve_owned_with_budget(value, budget)?;
            (lease, value)
        };
        Self::merge_catalog_version(header, &value)
    }

    fn merge_catalog_version(
        header: save_options::PdfVersion,
        value: &PdfObject,
    ) -> PdfResult<save_options::PdfVersion> {
        if matches!(value, PdfObject::PdfNull) {
            return Ok(header);
        }
        let name = value
            .as_name()
            .ok_or_else(|| PdfError::DocumentError("Catalog Version must be a name".into()))?;
        let catalog = save_options::PdfVersion::parse(name.name())
            .ok_or_else(|| PdfError::UnsupportedFeature("unknown Catalog Version".into()))?;
        Ok(header.max(catalog))
    }

    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn resources(&self) -> resource_manager::ResourceManager<'_, R> {
        resource_manager::ResourceManager { document: self }
    }

    pub fn page_count(&self) -> usize {
        self.try_page_count().unwrap_or(0)
    }

    pub fn try_page_count(&self) -> PdfResult<usize> {
        self.catalog.total_page(&self.ctx)
    }

    pub fn resolve_object(&self, object_id: ObjectId) -> PdfResult<PdfObject> {
        self.ctx.resolve_object_owned(object_id)
    }

    /// Opaque, document-local cache invalidation token. Changes after successful
    /// writes and transaction rollback (including unwind); never rolls back.
    /// It is not a commit count or a cross-document identity.
    pub fn revision(&self) -> u64 {
        self.ctx.revision()
    }

    /// Committed-write counter: one per successful outermost dirty transaction,
    /// or per successful low-level write outside a transaction. Rollbacks and
    /// read-only transactions do not advance it. Not a cache invalidation token.
    pub fn commit_revision(&self) -> u64 {
        self.ctx.commit_revision()
    }

    pub fn identity(&self) -> identity::DocumentIdentity {
        self.ctx.identity()
    }

    /// Capture cache provenance. This does not copy objects or freeze edits.
    pub fn stamp(&self) -> identity::DocumentStamp {
        identity::DocumentStamp {
            document: self.identity(),
            revision: self.revision(),
        }
    }

    pub fn is_stamp_current(&self, stamp: &identity::DocumentStamp) -> bool {
        stamp.document == self.identity() && stamp.revision == self.revision()
    }

    /// Bind an existing object address to this document, verifying it resolves.
    pub fn object_handle(&self, id: ObjectId) -> PdfResult<identity::ObjectHandle> {
        self.resolve_object(id)?;
        Ok(identity::ObjectHandle {
            document: self.identity(),
            id,
        })
    }

    pub(crate) fn validate_handle(&self, handle: &identity::ObjectHandle) -> PdfResult<ObjectId> {
        if handle.document != self.identity() {
            return Err(PdfError::InvalidReference(
                "object handle belongs to another document".into(),
            ));
        }
        Ok(handle.id)
    }

    pub fn resolve_handle(&self, handle: &identity::ObjectHandle) -> PdfResult<PdfObject> {
        self.resolve_object(self.validate_handle(handle)?)
    }

    pub fn update_handle(
        &self,
        handle: &identity::ObjectHandle,
        object: PdfObject,
    ) -> PdfResult<()> {
        self.update_object(self.validate_handle(handle)?, object)
    }

    pub fn delete_handle(&self, handle: &identity::ObjectHandle) -> PdfResult<()> {
        self.delete_object(self.validate_handle(handle)?)
    }

    /// Legacy infallible allocation. Panics on allocation failure or exhausted IDs.
    /// Prefer `try_insert_object` for recoverable editing operations.
    pub fn insert_object(&self, object: PdfObject) -> ObjectId {
        self.ctx.insert_object(object)
    }

    pub fn try_insert_object(&self, object: PdfObject) -> PdfResult<ObjectId> {
        self.ctx.try_insert_object(object)
    }

    pub fn update_object(&self, object_id: ObjectId, object: PdfObject) -> PdfResult<()> {
        self.ctx.update_object(object_id, object)
    }

    /// Replace an indirect stream with decoded bytes, clearing stale encoding
    /// parameters. This updates every reference to the object; page-specific
    /// content edits should use the page content replacement API instead.
    pub fn replace_stream_decoded_data(&self, object_id: ObjectId, data: Vec<u8>) -> PdfResult<()> {
        if data.len() > self.ctx.limits().max_decoded_bytes
            || data.len() > self.ctx.limits().max_raw_stream_bytes
        {
            return Err(PdfError::ResourceLimit("replacement stream bytes".into()));
        }
        let object = self.resolve_object(object_id)?;
        let PdfObject::PdfStream(stream) = object else {
            return Err(PdfError::DocumentError(
                "replacement target is not a stream".into(),
            ));
        };
        let stream = stream.with_decoded_data(data)?;
        self.update_object(object_id, PdfObject::PdfStream(stream))
    }

    pub fn delete_object(&self, object_id: ObjectId) -> PdfResult<()> {
        self.ctx.delete_object(object_id)
    }

    pub fn add_page(&self, pagenum: u32) -> PdfResult<()> {
        self.add_page_with_builder(pagenum, crate::page::page_builder::PageBuilder::default())
    }

    pub fn add_page_with_builder(
        &self,
        pagenum: u32,
        builder: crate::page::page_builder::PageBuilder,
    ) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        builder.validate()?;
        if let Some(unit) = builder.user_unit {
            if self.effective_version()? < save_options::PdfVersion::V1_6 {
                return Err(PdfError::UnsupportedFeature(
                    "UserUnit requires PDF 1.6 or later".into(),
                ));
            }
            // Validate the same display geometry used by readers before any allocation
            // in the object store; no Contents or resource decoding is needed.
            let mut page = PdfDict::default();
            page.try_insert(
                "MediaBox".into(),
                PdfObject::PdfArray(rect_to_array(builder.media_box.clone())?),
            )?;
            if let Some(crop) = &builder.crop_box {
                page.try_insert(
                    "CropBox".into(),
                    PdfObject::PdfArray(rect_to_array(crop.clone())?),
                )?;
            }
            page.try_insert(
                "Rotate".into(),
                PdfObject::PdfNumber(PdfNumber::new(builder.rotation.rem_euclid(360) as f32)),
            )?;
            page.try_insert(
                "UserUnit".into(),
                PdfObject::PdfNumber(PdfNumber::new(unit)),
            )?;
            crate::page::PdfPage::new(PdfObject::PdfDict(page), &self.ctx).geometry()?;
        }
        if builder.content.len() > self.ctx.limits().max_raw_stream_bytes
            || builder.content.len() > self.ctx.limits().max_decoded_bytes
        {
            return Err(PdfError::ResourceLimit("new page content bytes".into()));
        }
        self.edit_transaction(|doc| doc.add_page_inner(pagenum, builder))
    }

    /// Apply a group of edits atomically. An error rolls back the entire group.
    /// Each nested transaction is a savepoint. Original source caches are kept.
    pub fn edit_transaction<T>(&self, edit: impl FnOnce(&Self) -> PdfResult<T>) -> PdfResult<T> {
        self.ctx.transaction(|| edit(self))
    }

    /// Atomically edit with budgeted savepoint table slots, payloads and Trailer.
    /// Rollback reuses existing store buckets. Working-store growth, source
    /// caches and allocations made by `edit` are not automatically tracked;
    /// use the same pool for budget-aware edit APIs.
    pub fn edit_transaction_with_budget<T>(
        &self,
        budget: &crate::budget::OperationBudget,
        edit: impl FnOnce(&Self) -> PdfResult<T>,
    ) -> PdfResult<T> {
        self.ctx.transaction_with_budget(budget, || edit(self))
    }

    fn add_page_inner(
        &self,
        pagenum: u32,
        builder: crate::page::page_builder::PageBuilder,
    ) -> PdfResult<()> {
        let page_count = u32::try_from(self.try_page_count()?)
            .map_err(|_| PdfError::ResourceLimit("page count exceeds u32".into()))?;
        if pagenum > page_count {
            return Err(PdfError::PageNotExist);
        }

        let insertion = self.page_insertion_point(pagenum)?;
        let new_page_id = self.create_page(insertion.parent_id, builder)?;
        self.update_pages_kids(
            insertion.parent_id,
            insertion.kid_index,
            PdfObject::PdfReference(PdfReference::from_id(new_page_id)),
            false,
        )?;
        self.adjust_page_counts(&insertion.ancestor_ids, 1)?;
        Ok(())
    }

    pub fn delete_page(&self, pagenum: u32) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        self.edit_transaction(|doc| doc.delete_page_inner(pagenum))
    }

    fn delete_page_inner(&self, pagenum: u32) -> PdfResult<()> {
        let page_count = self.try_page_count()? as u32;
        if pagenum >= page_count {
            return Err(PdfError::PageNotExist);
        }

        // Removing a referenced page would leave an outline destination dangling.
        // Reject until page-deletion destination remapping is implemented.
        let root = self.ctx.get_root_owned()?;
        let has_named_targets = if let Some(names) = root.get("Names") {
            self.ctx
                .resolve(names)?
                .as_dict()
                .is_some_and(|dict| dict.get("Dests").is_some())
        } else {
            false
        };
        if root.get("Outlines").is_some() || root.get("Dests").is_some() || has_named_targets {
            return Err(PdfError::UnsupportedFeature(
                "page deletion with document navigation requires destination remapping".into(),
            ));
        }
        for entry in self.collect_page_entries()? {
            if let Some(annotations) = entry.page_dict.get("Annots") {
                let annotations = self.ctx.resolve(annotations)?;
                let annotations = annotations
                    .as_array()
                    .ok_or_else(|| PdfError::DocumentError("Annots must be an array".into()))?;
                if annotations.len() > 0 {
                    return Err(PdfError::UnsupportedFeature(
                        "page deletion with annotations requires relationship cleanup".into(),
                    ));
                }
            }
        }

        let location = self.page_location(pagenum)?;
        self.update_pages_kids(
            location.parent_id,
            location.kid_index,
            PdfObject::PdfNull,
            true,
        )?;
        self.adjust_page_counts(&location.ancestor_ids, -1)?;
        self.delete_object(location.page_id)?;
        // Contents and resources can be shared. Unlinking the page does not
        // authorize deleting those objects; explicit graph cleanup handles them.
        Ok(())
    }

    pub fn replace_page_contents(&self, pagenum: u32, content: Vec<u8>) -> PdfResult<()> {
        self.validate_content_bytes(content.len())?;
        self.edit_transaction(|doc| doc.replace_page_contents_inner(pagenum, content))
    }

    fn replace_page_contents_inner(&self, pagenum: u32, content: Vec<u8>) -> PdfResult<()> {
        let location = self.page_location(pagenum)?;
        let mut page_dict = location.page_dict;
        let stream_id = self.try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(PdfDict::default(), content)?,
        ))?;
        page_dict.insert(
            "Contents".to_string(),
            PdfObject::PdfReference(PdfReference::from_id(stream_id)),
        );
        self.update_object(location.page_id, PdfObject::PdfDict(page_dict))?;
        Ok(())
    }

    pub fn replace_page_content_builder(
        &self,
        pagenum: u32,
        builder: &ContentBuilder,
    ) -> PdfResult<()> {
        self.replace_page_contents(pagenum, builder.try_build_with_limits(self.ctx.limits())?)
    }

    pub fn append_page_contents(&self, pagenum: u32, content: Vec<u8>) -> PdfResult<()> {
        self.validate_content_bytes(content.len())?;
        self.edit_transaction(|doc| doc.append_page_contents_inner(pagenum, content))
    }

    fn validate_content_bytes(&self, length: usize) -> PdfResult<()> {
        if length > self.ctx.limits().max_raw_stream_bytes
            || length > self.ctx.limits().max_decoded_bytes
        {
            return Err(PdfError::ResourceLimit("page content bytes".into()));
        }
        Ok(())
    }

    fn append_page_contents_inner(&self, pagenum: u32, content: Vec<u8>) -> PdfResult<()> {
        let location = self.page_location(pagenum)?;
        let mut page_dict = location.page_dict;
        let stream_id = self.try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(PdfDict::default(), content)?,
        ))?;
        let new_ref = PdfObject::PdfReference(PdfReference::from_id(stream_id));
        let contents = match page_dict.get("Contents").cloned() {
            Some(PdfObject::PdfArray(mut items)) => {
                items.try_add_obj(new_ref)?;
                PdfObject::PdfArray(items)
            }
            Some(existing) => {
                let mut items = PdfArray::default();
                items.try_add_obj(existing)?;
                items.try_add_obj(new_ref)?;
                PdfObject::PdfArray(items)
            }
            None => new_ref,
        };
        page_dict.insert("Contents".to_string(), contents);
        self.update_object(location.page_id, PdfObject::PdfDict(page_dict))?;
        Ok(())
    }

    pub fn append_page_content_builder(
        &self,
        pagenum: u32,
        builder: &ContentBuilder,
    ) -> PdfResult<()> {
        self.append_page_contents(pagenum, builder.try_build_with_limits(self.ctx.limits())?)
    }

    pub fn get_page(&self, page: u32) -> PdfResult<PdfPage<'_, R>> {
        if let Some(pagenode) = self.catalog.get_page(page, &self.ctx)? {
            let page = PdfPage::new(pagenode, &self.ctx);
            return Ok(page);
        }
        Err(PdfError::PageNotExist)
    }

    /// Operation-internal page lookup. The returned page owns only an inline
    /// indirect reference; traversal copies are released before returning.
    fn get_page_with_budget(
        &self,
        page: u32,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<PdfPage<'_, R>> {
        let (_lease, location) = {
            let (location, lease) = self.page_location_with_budget(page, budget)?.into_parts();
            (lease, location)
        };
        Ok(PdfPage::new(
            PdfObject::PdfReference(PdfReference::from_id(location.page_id)),
            &self.ctx,
        ))
    }
}

#[cfg(test)]
mod tests {
    use super::PdfDocument;
    use crate::parser::syntax::ParseMode;

    #[test]
    fn budgeted_page_lookup_releases_traversal_and_matches_source_identity() {
        use crate::{budget::OperationBudget, error::PdfError};
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        doc.replace_page_contents(1, b"q Q".to_vec()).unwrap();
        let expected = doc.get_page(1).unwrap().content_program().unwrap();
        let revision = doc.revision();
        let pool = OperationBudget::new(1024 * 1024);
        let neighbor = pool.reserve(11).unwrap();
        let page = doc.get_page_with_budget(1, &pool).unwrap();
        assert_eq!(pool.live_bytes(), 11);
        let current = page.content_program_with_budget(&pool).unwrap();
        assert_eq!(current.program().source_page_id, expected.source_page_id);
        assert_eq!(current.program().source_bytes(), expected.source_bytes());
        assert_eq!(current.program().source_stamp(), expected.source_stamp());
        assert_eq!(
            current.program().stream_object_id(0),
            expected.stream_object_id(0)
        );
        drop(current);
        assert_eq!(pool.live_bytes(), 11);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
        let blocked = OperationBudget::new(0);
        assert!(matches!(
            doc.get_page_with_budget(1, &blocked),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(blocked.live_bytes(), 0);
        assert!(matches!(
            doc.get_page_with_budget(2, &pool),
            Err(PdfError::PageNotExist)
        ));
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(doc.revision(), revision);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _page = doc.get_page_with_budget(1, &pool).unwrap();
            panic!("budgeted page lookup unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(
            doc.get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            expected.source_bytes()
        );
    }

    #[test]
    fn explicit_creation_versions_work_in_legacy_and_owned_facades() {
        use super::save_options::PdfVersion;
        for version in [
            PdfVersion::V1_0,
            PdfVersion::V1_1,
            PdfVersion::V1_2,
            PdfVersion::V1_3,
            PdfVersion::V1_4,
            PdfVersion::V1_5,
            PdfVersion::V1_6,
            PdfVersion::V1_7,
            PdfVersion::V2_0,
        ] {
            let doc = PdfDocument::new_with_version(version).unwrap();
            assert_eq!(doc.version(), version.as_str());
            doc.add_page(0).unwrap();
            let bytes = doc.save_to_bytes().unwrap();
            assert!(bytes.starts_with(format!("%PDF-{}", version.as_str()).as_bytes()));
            let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            assert_eq!(reopened.effective_version().unwrap(), version);
            let owned = super::Document::new_with_version(version).unwrap();
            assert_eq!(owned.version(), version.as_str());
            assert_eq!(owned.effective_version().unwrap(), version);
        }
        assert_eq!(PdfDocument::new().unwrap().version(), "1.7");
    }

    #[test]
    fn test_from_bytes_with_strict_mode_rejects_missing_binary_comment() {
        let pdf = b"%PDF-1.7\n1 0 obj\n<<>>\nendobj\nxref\n0 1\n0000000000 65535 f\ntrailer\n<< /Size 1 >>\nstartxref\n20\n%%EOF\n";
        assert!(PdfDocument::from_bytes_with_mode(pdf.to_vec(), ParseMode::Strict).is_err());
    }
}

impl<R: Seek + Read> PdfDocument<R> {
    fn root_pages(&self) -> PdfResult<(ObjectId, PdfDict)> {
        let trailer = self.ctx.trailer();
        let root_ref = trailer
            .get("Root")
            .and_then(PdfObject::as_reference)
            .ok_or(PdfError::DocumentError(
                "document Root must be an indirect reference".to_string(),
            ))?;
        let root_dict =
            self.resolve_object(root_ref.id())?
                .into_dict()
                .ok_or(PdfError::DocumentError(
                    "document Root is not a dict".to_string(),
                ))?;
        let pages_ref = root_dict
            .get("Pages")
            .cloned()
            .and_then(PdfObject::into_reference)
            .ok_or(PdfError::DocumentError(
                "document Pages root must be an indirect reference".to_string(),
            ))?;
        let pages_id = pages_ref.id();
        let pages_dict =
            self.resolve_object(pages_id)?
                .into_dict()
                .ok_or(PdfError::DocumentError(
                    "Pages root is not a dict".to_string(),
                ))?;
        Ok((pages_id, pages_dict))
    }

    fn create_page(
        &self,
        parent_id: ObjectId,
        builder: crate::page::page_builder::PageBuilder,
    ) -> PdfResult<ObjectId> {
        let contents_id = self.try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(PdfDict::default(), builder.content)?,
        ))?;
        let mut page = new_page_dict(parent_id, contents_id)?;
        page.try_insert(
            "MediaBox".into(),
            PdfObject::PdfArray(rect_to_array(builder.media_box)?),
        )?;
        if let Some(crop) = builder.crop_box {
            page.try_insert("CropBox".into(), PdfObject::PdfArray(rect_to_array(crop)?))?;
        }
        page.try_insert(
            "Rotate".into(),
            PdfObject::PdfNumber(PdfNumber::new(builder.rotation.rem_euclid(360) as f32)),
        )?;
        page.try_insert("Resources".into(), PdfObject::PdfDict(builder.resources))?;
        if let Some(unit) = builder.user_unit {
            page.try_insert(
                "UserUnit".into(),
                PdfObject::PdfNumber(PdfNumber::new(unit)),
            )?;
        }
        self.try_insert_object(PdfObject::PdfDict(page))
    }

    fn page_insertion_point(&self, pagenum: u32) -> PdfResult<PageInsertionPoint> {
        let entries = self.collect_page_entries()?;
        if entries.is_empty() {
            let (root_id, _) = self.root_pages()?;
            return Ok(PageInsertionPoint {
                parent_id: root_id,
                kid_index: 0,
                ancestor_ids: vec![root_id],
            });
        }

        if (pagenum as usize) < entries.len() {
            let target = &entries[pagenum as usize];
            return Ok(PageInsertionPoint {
                parent_id: target.parent_id,
                kid_index: target.kid_index,
                ancestor_ids: target.ancestor_ids.clone(),
            });
        }

        let target = entries
            .last()
            .ok_or(PdfError::DocumentError("page tree is empty".to_string()))?;
        Ok(PageInsertionPoint {
            parent_id: target.parent_id,
            kid_index: target.kid_index + 1,
            ancestor_ids: target.ancestor_ids.clone(),
        })
    }

    fn page_location(&self, pagenum: u32) -> PdfResult<PageTreeEntry> {
        let entries = self.collect_page_entries()?;
        entries
            .into_iter()
            .nth(pagenum as usize)
            .ok_or(PdfError::PageNotExist)
    }

    fn page_location_with_budget(
        &self,
        page: u32,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<page_tree::BudgetedPageTreeEntry> {
        page_tree::PageTree::page_at_with_budget(&self.ctx, page, budget)
    }

    fn collect_page_entries(&self) -> PdfResult<Vec<PageTreeEntry>> {
        page_tree::PageTree::entries(&self.ctx)
    }

    fn update_pages_kids(
        &self,
        pages_id: ObjectId,
        kid_index: usize,
        kid: PdfObject,
        remove: bool,
    ) -> PdfResult<()> {
        let mut pages_dict =
            self.resolve_object(pages_id)?
                .into_dict()
                .ok_or(PdfError::DocumentError(
                    "Pages node is not a dict".to_string(),
                ))?;
        let mut kids = pages_dict
            .get("Kids")
            .cloned()
            .and_then(PdfObject::into_array)
            .ok_or(PdfError::DocumentError(
                "Pages node Kids must be an array".to_string(),
            ))?;
        if remove {
            let _ = kids.remove(kid_index).ok_or(PdfError::PageNotExist)?;
        } else {
            kids.insert(kid_index, kid)?;
        }
        pages_dict.insert("Kids".to_string(), PdfObject::PdfArray(kids));
        self.update_object(pages_id, PdfObject::PdfDict(pages_dict))
    }

    fn adjust_page_counts(&self, ancestor_ids: &[ObjectId], delta: i32) -> PdfResult<()> {
        for object_id in ancestor_ids {
            let mut pages_dict =
                self.resolve_object(*object_id)?
                    .into_dict()
                    .ok_or(PdfError::DocumentError(
                        "Pages node is not a dict".to_string(),
                    ))?;
            let current = pages_dict.get("Count").and_then(PdfObject::as_i32).ok_or(
                PdfError::DocumentError("Pages node Count must be a number".to_string()),
            )?;
            let next = current + delta;
            if next < 0 {
                return Err(PdfError::DocumentError(
                    "page tree Count cannot be negative".to_string(),
                ));
            }
            pages_dict.insert(
                "Count".to_string(),
                PdfObject::PdfNumber(PdfNumber::new(next as f32)),
            );
            self.update_object(*object_id, PdfObject::PdfDict(pages_dict))?;
        }
        Ok(())
    }
}

#[derive(Debug, Clone)]
struct PageInsertionPoint {
    parent_id: ObjectId,
    kid_index: usize,
    ancestor_ids: Vec<ObjectId>,
}

fn new_page_dict(parent_id: ObjectId, contents_id: ObjectId) -> PdfResult<PdfDict> {
    let mut page = PdfDict::default();
    page.try_insert(
        "Type".to_string(),
        PdfObject::PdfName(PdfName::new("Page".to_string())),
    )?;
    page.try_insert(
        "Parent".to_string(),
        PdfObject::PdfReference(PdfReference::from_id(parent_id)),
    )?;
    page.try_insert(
        "Contents".to_string(),
        PdfObject::PdfReference(PdfReference::from_id(contents_id)),
    )?;
    Ok(page)
}

fn rect_to_array(rect: Rectangle) -> PdfResult<PdfArray> {
    let mut array = PdfArray::default();
    array.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(rect.llx())))?;
    array.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(rect.lly())))?;
    array.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(rect.urx())))?;
    array.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(rect.ury())))?;
    Ok(array)
}

#[cfg(test)]
mod builder_tests {
    use super::*;
    use crate::page::page_builder::PageBuilder;

    #[test]
    fn page_content_limit_accepts_exact_bytes_and_rejects_before_allocation() {
        use crate::parser::options::{Limits, ParseOptions};
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_raw_stream_bytes: 4,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        assert!(matches!(
            doc.add_page_with_builder(0, PageBuilder::new().content(b"12345".to_vec())),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(doc.try_page_count().unwrap(), 0);
        doc.add_page_with_builder(0, PageBuilder::new().content(b"q Q\n".to_vec()))
            .unwrap();
        let entries = doc.collect_page_entries().unwrap();
        assert_eq!(entries[0].page_id, ObjectId::new(4, 0));
        let content = doc
            .ctx
            .resolve(entries[0].page_dict.get("Contents").unwrap())
            .unwrap();
        assert_eq!(content.as_stream().unwrap().raw_data(), b"q Q\n");
    }

    #[test]
    fn configured_page_preserves_boxes_rotation_resources_and_content() {
        let doc = PdfDocument::new().unwrap();
        let mut resources = PdfDict::default();
        resources
            .try_insert("Custom".into(), PdfObject::PdfNull)
            .unwrap();
        doc.add_page_with_builder(
            0,
            PageBuilder::new()
                .media_box(Rectangle::new(10.0, 20.0, 310.0, 420.0))
                .crop_box(Rectangle::new(30.0, 40.0, 280.0, 380.0))
                .rotation(-90)
                .resources(resources)
                .content(b"1 2 m 3 4 l S".to_vec()),
        )
        .unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        let entries = reopened.collect_page_entries().unwrap();
        let page = &entries[0].page_dict;
        assert_eq!(
            page.get("Rotate").unwrap().as_number().unwrap().lexeme(),
            "270"
        );
        for (name, values) in [
            ("MediaBox", [10.0, 20.0, 310.0, 420.0]),
            ("CropBox", [30.0, 40.0, 280.0, 380.0]),
        ] {
            let expected =
                rect_to_array(Rectangle::new(values[0], values[1], values[2], values[3])).unwrap();
            assert_eq!(
                format!("{:?}", page.get(name).unwrap()),
                format!("{:?}", PdfObject::PdfArray(expected))
            );
        }
        assert!(
            reopened
                .get_page(0)
                .unwrap()
                .resource()
                .unwrap()
                .get("Custom")
                .is_some()
        );
        let content = reopened.ctx.resolve(page.get("Contents").unwrap()).unwrap();
        assert_eq!(
            content
                .as_stream()
                .unwrap()
                .decode_data(&reopened.ctx)
                .unwrap(),
            b"1 2 m 3 4 l S"
        );
    }

    #[test]
    fn invalid_page_configuration_does_not_allocate_objects() {
        let doc = PdfDocument::new().unwrap();
        for builder in [
            PageBuilder::new().size(0.0, 100.0),
            PageBuilder::new().size(f32::NAN, 10.0),
            PageBuilder::new().rotation(45),
            PageBuilder::new().crop_box(Rectangle::new(1000.0, 1000.0, 1100.0, 1100.0)),
        ] {
            assert!(doc.add_page_with_builder(0, builder).is_err());
            assert_eq!(doc.try_page_count().unwrap(), 0);
        }
        assert_eq!(
            doc.try_insert_object(PdfObject::PdfNull).unwrap(),
            ObjectId::new(3, 0)
        );
    }
}
