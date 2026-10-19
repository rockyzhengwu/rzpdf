//! Selected PDF 2.0 Catalog/page associated files. No filesystem or action runtime.
//!
//! Recovery is deliberately not an authoring capability:
//! ```compile_fail
//! use pdf::{document::{recovery::RecoveredDocument, associated_files::AssociatedFileDefinition}, budget::OperationBudget};
//! fn cannot_create(d: &RecoveredDocument, def: &AssociatedFileDefinition, b: &OperationBudget) {
//!     d.create_associated_file(def, b"data", b).unwrap();
//! }
//! ```
//! ```compile_fail
//! use pdf::{document::{recovery::RecoveredDocument, associated_files::AssociatedFileOwner, identity::ObjectHandle}, budget::OperationBudget};
//! fn cannot_bind(d: &RecoveredDocument, target: &AssociatedFileOwner, file: &ObjectHandle, b: &OperationBudget) {
//!     d.bind_associated_file(target, file, b).unwrap();
//! }
//! ```
use super::{
    PdfDocument,
    embedded_file::invalid,
    identity::{DocumentStamp, ObjectHandle},
    save_options::PdfVersion,
};
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_reference::PdfReference, pdf_stream::PdfStream, pdf_string::PdfString,
    },
};
use std::io::{Read, Seek};

const MAX_FILES: usize = 1024;
const MAX_TEXT: usize = 64 * 1024;
const MAX_CREATE_BYTES: usize = 64 * 1024 * 1024;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AssociatedFileRelationship {
    Source,
    Data,
    Alternative,
    Supplement,
    Unspecified,
}
impl AssociatedFileRelationship {
    pub fn name(self) -> &'static str {
        match self {
            Self::Source => "Source",
            Self::Data => "Data",
            Self::Alternative => "Alternative",
            Self::Supplement => "Supplement",
            Self::Unspecified => "Unspecified",
        }
    }
}

#[derive(Debug, Clone)]
pub struct AssociatedFileDefinition {
    pub name: String,
    pub mime_type: String,
    pub relationship: AssociatedFileRelationship,
    pub description: Option<String>,
}
impl AssociatedFileDefinition {
    pub fn new(
        name: impl Into<String>,
        mime_type: impl Into<String>,
        relationship: AssociatedFileRelationship,
    ) -> Self {
        Self {
            name: name.into(),
            mime_type: mime_type.into(),
            relationship,
            description: None,
        }
    }
}

/// Exact current owner target. Reacquire after any revision change.
#[derive(Debug, Clone)]
pub struct AssociatedFileOwner {
    stamp: DocumentStamp,
    page: Option<(u32, ObjectHandle)>,
}
impl AssociatedFileOwner {
    pub fn stamp(&self) -> &DocumentStamp {
        &self.stamp
    }
    pub fn page(&self) -> Option<&ObjectHandle> {
        self.page.as_ref().map(|(_, handle)| handle)
    }
}

#[derive(Debug)]
pub struct AssociatedFileRecord {
    source: PdfObject,
    filespec: Option<ObjectHandle>,
    name: String,
    description: Option<String>,
    relationship: String,
    mime_type: Option<String>,
    embedded: bool,
    size: Option<u64>,
    modification_date: Option<String>,
    _lease: MemoryLease,
}
impl AssociatedFileRecord {
    pub fn filespec(&self) -> Option<&ObjectHandle> {
        self.filespec.as_ref()
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn description(&self) -> Option<&str> {
        self.description.as_deref()
    }
    /// Original name, including unsupported/custom relationships.
    pub fn relationship(&self) -> &str {
        &self.relationship
    }
    pub fn mime_type(&self) -> Option<&str> {
        self.mime_type.as_deref()
    }
    pub fn is_embedded(&self) -> bool {
        self.embedded
    }
    pub fn size(&self) -> Option<u64> {
        self.size
    }
    /// Original PDF date text, not a timezone-normalized filesystem date.
    pub fn modification_date(&self) -> Option<&str> {
        self.modification_date.as_deref()
    }
}

#[derive(Debug)]
pub struct AssociatedFiles {
    owner: AssociatedFileOwner,
    files: Vec<AssociatedFileRecord>,
    _lease: MemoryLease,
}
impl AssociatedFiles {
    pub fn owner(&self) -> &AssociatedFileOwner {
        &self.owner
    }
    pub fn stamp(&self) -> &DocumentStamp {
        &self.owner.stamp
    }
    pub fn files(&self) -> &[AssociatedFileRecord] {
        &self.files
    }
}

fn name(value: &str) -> PdfObject {
    PdfObject::PdfName(PdfName::new(value.into()))
}
fn reference(handle: &ObjectHandle) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(handle.object_id()))
}
fn valid_mime(value: &str) -> bool {
    let Some((a, b)) = value.split_once('/') else {
        return false;
    };
    value.len() <= 255
        && [a, b].into_iter().all(|part| {
            !part.is_empty()
                && part
                    .bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"!$&^_.+-".contains(&b))
        })
}

impl<R: Read + Seek> PdfDocument<R> {
    pub fn associated_file_owner(
        &self,
        page: Option<u32>,
        budget: &OperationBudget,
    ) -> PdfResult<AssociatedFileOwner> {
        let page = match page {
            None => None,
            Some(index) => {
                let (entry, _lease) = self.page_location_with_budget(index, budget)?.into_parts();
                Some((
                    index,
                    ObjectHandle {
                        document: self.identity(),
                        id: entry.page_id,
                    },
                ))
            }
        };
        Ok(AssociatedFileOwner {
            stamp: self.stamp(),
            page,
        })
    }

    fn af_owner_dict(
        &self,
        owner: &AssociatedFileOwner,
        budget: &OperationBudget,
    ) -> PdfResult<(PdfDict, MemoryLease)> {
        if !self.is_stamp_current(&owner.stamp) {
            return Err(invalid("foreign or stale owner"));
        }
        if let Some((index, handle)) = &owner.page {
            let (entry, lease) = self.page_location_with_budget(*index, budget)?.into_parts();
            if self.validate_handle(handle)? != entry.page_id {
                return Err(invalid("page owner changed"));
            }
            return Ok((entry.page_dict, lease));
        }
        let trailer = self.ctx.trailer();
        let root = trailer.get("Root").ok_or_else(|| invalid("Root missing"))?;
        self.file_dict(root, budget)
    }

    fn af_text(
        &self,
        value: &PdfObject,
        budget: &OperationBudget,
        output: &mut MemoryLease,
    ) -> PdfResult<String> {
        let (value, _source_lease) = self.file_resolve(value, budget)?;
        let s = value
            .as_string()
            .ok_or_else(|| invalid("text string expected"))?;
        if s.bytes().len() > MAX_TEXT.min(self.ctx.limits().max_token_bytes) {
            return Err(PdfError::ResourceLimit("AF text bytes".into()));
        }
        let mut lease = budget.reserve(
            s.bytes()
                .len()
                // UTF-8 expansion plus String's geometric/minimum capacity.
                .checked_mul(8)
                .and_then(|n| n.checked_add(8))
                .ok_or_else(|| invalid("text size overflow"))?,
        )?;
        let text = s.text_content_with_limits(&self.ctx.limits())?;
        lease.resize(text.capacity())?;
        output.absorb(&mut lease)?;
        Ok(text)
    }

    fn af_name(
        &self,
        value: &PdfObject,
        budget: &OperationBudget,
        output: &mut MemoryLease,
    ) -> PdfResult<String> {
        let (value, _lease) = self.file_resolve(value, budget)?;
        let n = value.as_name().ok_or_else(|| invalid("name expected"))?;
        // Never turn lossy PDF-name bytes into a different relationship name.
        let text = std::str::from_utf8(n.bytes()).map_err(|_| invalid("non UTF-8 AF name"))?;
        if text.len() > MAX_TEXT {
            return Err(PdfError::ResourceLimit("AF name bytes".into()));
        }
        let (text, mut lease) = crate::budget::copy_string(text, budget)?;
        output.absorb(&mut lease)?;
        Ok(text)
    }

    fn af_spec(
        &self,
        value: &PdfObject,
        budget: &OperationBudget,
    ) -> PdfResult<(PdfDict, MemoryLease)> {
        let (dict, lease) = self.file_dict(value, budget)?;
        if let Some(kind) = dict.get("Type") {
            let (kind, _lease) = self.file_resolve(kind, budget)?;
            if kind.as_name().map(|n| n.name()) != Some("Filespec") {
                return Err(invalid("Filespec Type"));
            }
        } else if dict.contains_key("EF") {
            return Err(invalid("Filespec Type required with EF"));
        }
        // Validate both filename fields, not just the preferred display name.
        for key in ["F", "UF"] {
            if let Some(value) = dict.get(key) {
                let mut temporary = budget.reserve(0)?;
                self.af_text(value, budget, &mut temporary)?;
            }
        }
        Ok((dict, lease))
    }

    fn af_record(
        &self,
        input: &PdfObject,
        budget: &OperationBudget,
    ) -> PdfResult<AssociatedFileRecord> {
        let (source, mut lease) =
            crate::objects::clone_object_with_budget(input, budget, self.ctx.limits(), 0)?;
        let filespec = source.as_reference().map(|r| ObjectHandle {
            document: self.identity(),
            id: r.id(),
        });
        let (spec, _spec_lease) = self.af_spec(&source, budget)?;
        let file_name = spec
            .get("UF")
            .or_else(|| spec.get("F"))
            .ok_or_else(|| invalid("filename missing"))?;
        let file_name = self.af_text(file_name, budget, &mut lease)?;
        let description = spec
            .get("Desc")
            .map(|v| self.af_text(v, budget, &mut lease))
            .transpose()?;
        let relationship = self.af_name(
            spec.get("AFRelationship")
                .ok_or_else(|| invalid("AFRelationship missing"))?,
            budget,
            &mut lease,
        )?;
        let mut result = AssociatedFileRecord {
            source,
            filespec,
            name: file_name,
            description,
            relationship,
            mime_type: None,
            embedded: false,
            size: None,
            modification_date: None,
            _lease: lease,
        };
        if let Some((stream, _stream_lease)) = self.file_stream(&spec, budget)? {
            result.embedded = true;
            let mime = self.af_name(
                stream
                    .dict()
                    .get("Subtype")
                    .ok_or_else(|| invalid("AF MIME missing"))?,
                budget,
                &mut result._lease,
            )?;
            if !valid_mime(&mime) {
                return Err(PdfError::UnsupportedFeature("AF MIME profile".into()));
            }
            result.mime_type = Some(mime);
            if let Some(params) = stream.dict().get("Params") {
                let (params, _params_lease) = self.file_dict(params, budget)?;
                if let Some(size) = params.get("Size") {
                    let (size, _size_lease) = self.file_resolve(size, budget)?;
                    result.size = Some(
                        size.as_number()
                            .ok_or_else(|| invalid("Size integer expected"))?
                            .as_u64_checked()?,
                    );
                }
                result.modification_date = params
                    .get("ModDate")
                    .map(|v| self.af_text(v, budget, &mut result._lease))
                    .transpose()?;
            }
        }
        Ok(result)
    }

    /// Read only the selected owner; no payload decoding or external I/O.
    pub fn associated_files(
        &self,
        owner: &AssociatedFileOwner,
        budget: &OperationBudget,
    ) -> PdfResult<AssociatedFiles> {
        let (dict, _owner_lease) = self.af_owner_dict(owner, budget)?;
        let mut result = AssociatedFiles {
            owner: owner.clone(),
            files: Vec::new(),
            _lease: budget.reserve(0)?,
        };
        let Some(value) = dict.get("AF") else {
            return Ok(result);
        };
        let (value, _array_lease) = self.file_resolve(value, budget)?;
        if matches!(value, PdfObject::PdfNull) {
            return Ok(result);
        }
        let items = value
            .as_array()
            .ok_or_else(|| invalid("AF must be an array"))?;
        if items.len() > MAX_FILES.min(self.ctx.limits().max_objects) {
            return Err(PdfError::ResourceLimit("AF entries".into()));
        }
        result._lease.resize(
            items
                .len()
                .checked_mul(std::mem::size_of::<AssociatedFileRecord>())
                .ok_or_else(|| invalid("list size overflow"))?,
        )?;
        result
            .files
            .try_reserve_exact(items.len())
            .map_err(|_| PdfError::ResourceLimit("AF list allocation".into()))?;
        result._lease.resize(
            result
                .files
                .capacity()
                .checked_mul(std::mem::size_of::<AssociatedFileRecord>())
                .ok_or_else(|| invalid("list capacity overflow"))?,
        )?;
        for input in items {
            if let Some(id) = input.as_reference().map(|r| r.id())
                && result
                    .files
                    .iter()
                    .any(|r| r.filespec.as_ref().is_some_and(|h| h.id == id))
            {
                return Err(invalid("duplicate AF identity"));
            }
            result.files.push(self.af_record(input, budget)?);
        }
        Ok(result)
    }

    pub fn associated_file_data(
        &self,
        snapshot: &AssociatedFiles,
        index: usize,
        budget: &OperationBudget,
    ) -> PdfResult<BudgetedBytes> {
        if !self.is_stamp_current(&snapshot.owner.stamp) {
            return Err(invalid("foreign or stale AF snapshot"));
        }
        let record = snapshot
            .files
            .get(index)
            .ok_or_else(|| invalid("AF index out of range"))?;
        let (spec, _spec_lease) = self.af_spec(&record.source, budget)?;
        let (stream, _stream_lease) = self.file_stream(&spec, budget)?.ok_or_else(|| {
            PdfError::UnsupportedFeature("external associated file has no embedded payload".into())
        })?;
        let data = stream.decode_data_with_budget(self.ctx(), budget)?;
        if record
            .size
            .is_some_and(|size| size != data.as_slice().len() as u64)
        {
            return Err(invalid("embedded file Size mismatch"));
        }
        Ok(data)
    }

    fn af_write_guard(&self, budget: &OperationBudget) -> PdfResult<()> {
        self.ctx.require_unsigned_source_with_budget(budget)?;
        self.ctx.require_content_edit_permission()?;
        if self.effective_version_with_budget(budget)? < PdfVersion::V2_0 {
            return Err(PdfError::UnsupportedFeature(
                "AF authoring requires explicit PDF 2.0 upgrade and reopen".into(),
            ));
        }
        Ok(())
    }

    /// Link one validated embedded Filespec. Returns false for an exact no-op.
    /// No name-tree registration or shared Filespec mutation is performed.
    pub fn bind_associated_file(
        &self,
        owner: &AssociatedFileOwner,
        filespec: &ObjectHandle,
        budget: &OperationBudget,
    ) -> PdfResult<bool> {
        self.af_write_guard(budget)?;
        self.validate_handle(filespec)?;
        let (dict, _owner_lease) = self.af_owner_dict(owner, budget)?;
        let record = self.af_record(&reference(filespec), budget)?;
        if !record.embedded
            || !["Source", "Data", "Alternative", "Supplement", "Unspecified"]
                .contains(&record.relationship.as_str())
        {
            return Err(PdfError::UnsupportedFeature(
                "binding external/unsupported AF relationship".into(),
            ));
        }
        let (spec, _spec_lease) = self.af_spec(&reference(filespec), budget)?;
        if ["FS", "RF", "EP", "CI", "ID", "V", "DOS", "Mac", "Unix"]
            .iter()
            .any(|key| spec.contains_key(key))
        {
            return Err(PdfError::UnsupportedFeature(
                "binding extended Filespec".into(),
            ));
        }
        let (stream, _stream_lease) = self
            .file_stream(&spec, budget)?
            .ok_or_else(|| invalid("embedded AF required"))?;
        if stream.dict().contains_key("Params") && record.modification_date.is_none() {
            return Err(invalid("binding Params requires ModDate"));
        }
        let list = self.associated_files(owner, budget)?;
        if list
            .files
            .iter()
            .any(|r| r.filespec.as_ref() == Some(filespec))
        {
            return Ok(false);
        }
        if list.files.len() >= MAX_FILES.min(self.ctx.limits().max_objects) {
            return Err(PdfError::ResourceLimit("AF entries".into()));
        }
        self.af_replace_links(owner, dict, &list, Some(filespec), None, budget)?;
        Ok(true)
    }

    /// Unlink this Filespec from one owner. Does not erase data or history.
    pub fn unbind_associated_file(
        &self,
        owner: &AssociatedFileOwner,
        filespec: &ObjectHandle,
        budget: &OperationBudget,
    ) -> PdfResult<bool> {
        self.af_write_guard(budget)?;
        self.validate_handle(filespec)?;
        let (dict, _owner_lease) = self.af_owner_dict(owner, budget)?;
        let list = self.associated_files(owner, budget)?;
        if !list
            .files
            .iter()
            .any(|r| r.filespec.as_ref() == Some(filespec))
        {
            return Ok(false);
        }
        self.af_replace_links(owner, dict, &list, None, Some(filespec), budget)?;
        Ok(true)
    }

    fn af_replace_links(
        &self,
        owner: &AssociatedFileOwner,
        mut dict: PdfDict,
        list: &AssociatedFiles,
        add: Option<&ObjectHandle>,
        remove: Option<&ObjectHandle>,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        let count = list
            .files
            .len()
            .checked_add(usize::from(add.is_some()))
            .ok_or_else(|| invalid("AF entries overflow"))?;
        let mut lease = budget.reserve(
            count
                .checked_mul(std::mem::size_of::<PdfObject>())
                .and_then(|n| n.checked_mul(2))
                .and_then(|n| n.checked_add(64 * 1024))
                .ok_or_else(|| invalid("AF patch size overflow"))?,
        )?;
        let mut array = PdfArray::default();
        for record in &list.files {
            if remove.is_some() && record.filespec.as_ref() == remove {
                continue;
            }
            let (copy, mut copy_lease) = crate::objects::clone_object_with_budget(
                &record.source,
                budget,
                self.ctx.limits(),
                0,
            )?;
            array.try_add_obj(copy)?;
            lease.absorb(&mut copy_lease)?;
        }
        if let Some(file) = add {
            array.try_add_obj(reference(file))?;
        }
        let id = if let Some((_, page)) = &owner.page {
            self.validate_handle(page)?
        } else {
            self.ctx
                .trailer()
                .get("Root")
                .and_then(PdfObject::as_reference)
                .ok_or_else(|| invalid("indirect Root required"))?
                .id()
        };
        if array.is_empty() {
            dict.remove("AF");
            self.edit_transaction_with_budget(budget, |doc| {
                doc.update_object(id, PdfObject::PdfDict(dict))
            })
        } else {
            let (dict, _patch_lease) = dict.with_replaced_entry_with_budget(
                "AF",
                &PdfObject::PdfArray(array),
                budget,
                self.ctx.limits(),
            )?;
            self.edit_transaction_with_budget(budget, |doc| {
                doc.update_object(id, PdfObject::PdfDict(dict))
            })
        }
    }

    fn af_definition(
        &self,
        definition: &AssociatedFileDefinition,
        budget: &OperationBudget,
    ) -> PdfResult<MemoryLease> {
        let cap = MAX_TEXT.min(self.ctx.limits().max_token_bytes);
        if definition.name.is_empty() || definition.name.chars().any(|c| c.is_control()) {
            return Err(invalid("empty or control-character filename"));
        }
        let desc = definition.description.as_deref().unwrap_or("");
        let encoded_size = |text: &str| {
            text.encode_utf16()
                .count()
                .checked_mul(2)
                .and_then(|n| n.checked_add(2))
        };
        if definition.name.len() > cap
            || desc.len() > cap
            || encoded_size(&definition.name).is_none_or(|n| n > cap)
            || (definition.description.is_some() && encoded_size(desc).is_none_or(|n| n > cap))
        {
            return Err(PdfError::ResourceLimit("AF definition text bytes".into()));
        }
        if !valid_mime(&definition.mime_type) {
            return Err(invalid("unsupported MIME syntax"));
        }
        let text = definition
            .name
            .len()
            .checked_add(desc.len())
            .and_then(|v| v.checked_add(definition.mime_type.len()))
            .and_then(|v| v.checked_mul(16))
            .and_then(|v| v.checked_add(64 * 1024))
            .ok_or_else(|| invalid("definition size overflow"))?;
        budget.reserve(text)
    }

    fn af_payload_limit(&self, size: usize) -> PdfResult<()> {
        if size
            > MAX_CREATE_BYTES
                .min(self.ctx.limits().max_decoded_bytes)
                .min(self.ctx.limits().max_raw_stream_bytes)
        {
            return Err(PdfError::ResourceLimit("AF created payload bytes".into()));
        }
        Ok(())
    }

    // Validate generated tokens, table sizes and direct depth before publishing.
    // The temporary source has scratch/payload reservations; its checked copy
    // has a separate lease, including transient overlap.
    fn af_insert(&self, object: PdfObject, budget: &OperationBudget) -> PdfResult<ObjectHandle> {
        let (checked, _lease) =
            crate::objects::clone_object_with_budget(&object, budget, self.ctx.limits(), 0)?;
        let id = self.try_insert_object(checked)?;
        Ok(ObjectHandle {
            document: self.identity(),
            id,
        })
    }

    fn af_new_spec(
        &self,
        mut spec: PdfDict,
        definition: &AssociatedFileDefinition,
        stream: ObjectHandle,
    ) -> PdfResult<PdfDict> {
        spec.try_insert("Type".into(), name("Filespec"))?;
        let fallback = if definition.name.bytes().all(|c| (0x20..0x7f).contains(&c)) {
            definition.name.as_bytes()
        } else {
            b"attachment.bin"
        };
        spec.try_insert(
            "F".into(),
            PdfObject::PdfString(PdfString::new(fallback.to_vec(), false)),
        )?;
        spec.try_insert(
            "UF".into(),
            PdfObject::PdfString(PdfString::from_text_with_limits(
                &definition.name,
                &self.ctx.limits(),
            )?),
        )?;
        spec.try_insert(
            "AFRelationship".into(),
            name(definition.relationship.name()),
        )?;
        if let Some(desc) = &definition.description {
            spec.try_insert(
                "Desc".into(),
                PdfObject::PdfString(PdfString::from_text_with_limits(desc, &self.ctx.limits())?),
            )?;
        } else {
            spec.remove("Desc");
        }
        let mut ef = PdfDict::default();
        ef.try_insert("F".into(), reference(&stream))?;
        ef.try_insert("UF".into(), reference(&stream))?;
        spec.try_insert("EF".into(), PdfObject::PdfDict(ef))?;
        Ok(spec)
    }

    pub fn create_associated_file(
        &self,
        definition: &AssociatedFileDefinition,
        data: &[u8],
        budget: &OperationBudget,
    ) -> PdfResult<ObjectHandle> {
        self.af_write_guard(budget)?;
        let _scratch = self.af_definition(definition, budget)?;
        self.af_payload_limit(data.len())?;
        let (data, _data_lease) =
            BudgetedBytes::copy_from_slice(budget, data)?.into_reserved_parts();
        self.edit_transaction_with_budget(budget, |doc| {
            let mut dict = PdfDict::default();
            dict.try_insert("Type".into(), name("EmbeddedFile"))?;
            dict.try_insert("Subtype".into(), name(&definition.mime_type))?;
            let stream = doc.af_insert(
                PdfObject::PdfStream(PdfStream::from_decoded_data(dict, data)?),
                budget,
            )?;
            let spec = doc.af_new_spec(PdfDict::default(), definition, stream)?;
            doc.af_insert(PdfObject::PdfDict(spec), budget)
        })
    }

    pub fn clone_associated_file(
        &self,
        filespec: &ObjectHandle,
        definition: &AssociatedFileDefinition,
        budget: &OperationBudget,
    ) -> PdfResult<ObjectHandle> {
        self.af_write_guard(budget)?;
        self.validate_handle(filespec)?;
        let _scratch = self.af_definition(definition, budget)?;
        let (spec, _spec_lease) = self.af_spec(&reference(filespec), budget)?;
        for key in ["FS", "RF", "EP", "CI", "ID", "V", "DOS", "Mac", "Unix"] {
            if spec.contains_key(key) {
                return Err(PdfError::UnsupportedFeature(format!(
                    "cloning Filespec with {key}"
                )));
            }
        }
        let (stream, _stream_lease) = self
            .file_stream(&spec, budget)?
            .ok_or_else(|| PdfError::UnsupportedFeature("cloning external Filespec".into()))?;
        let data = stream.decode_data_with_budget(self.ctx(), budget)?;
        self.af_payload_limit(data.as_slice().len())?;
        if let Some(params) = stream.dict().get("Params") {
            let (params, _params_lease) = self.file_dict(params, budget)?;
            let mut lease = budget.reserve(0)?;
            self.af_text(
                params
                    .get("ModDate")
                    .ok_or_else(|| invalid("cloned Params requires ModDate"))?,
                budget,
                &mut lease,
            )?;
            if let Some(size) = params.get("Size") {
                let (size, _lease) = self.file_resolve(size, budget)?;
                if size
                    .as_number()
                    .ok_or_else(|| invalid("Size integer"))?
                    .as_u64_checked()?
                    != data.as_slice().len() as u64
                {
                    return Err(invalid("cloned Size mismatch"));
                }
            }
        }
        let (data, _data_lease) = data.into_reserved_parts();
        self.edit_transaction_with_budget(budget, |doc| {
            let (mut stream_dict, _dict_lease) =
                crate::objects::clone_dict_with_budget(stream.dict(), budget, doc.ctx.limits(), 0)?;
            stream_dict.try_insert("Subtype".into(), name(&definition.mime_type))?;
            let (stream, _rebuilt_lease) = PdfStream::from_decoded_data_with_budget(
                &stream_dict,
                data,
                budget,
                doc.ctx.limits(),
            )?;
            let stream = doc.af_insert(PdfObject::PdfStream(stream), budget)?;
            let new_spec = doc.af_new_spec(spec, definition, stream)?;
            doc.af_insert(PdfObject::PdfDict(new_spec), budget)
        })
    }
}
