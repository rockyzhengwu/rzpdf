//! Explicit append-only serialization. Original bytes are never rewritten.
mod security;
mod stream;
use super::*;
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    document::{
        cross_ref_table::ObjectState, save_options::{IncrementalSaveOptions, IncrementalSaveLayout, PdfVersion}, store::StoreEntry,
    },
    objects::{pdf_dict::BudgetedDictBuilder, pdf_number::PdfNumber},
    parser::options::Limits,
};

fn unsupported(message: &str) -> PdfError {
    PdfError::UnsupportedFeature(message.into())
}

fn structural_stream_type(object: &PdfObject) -> Option<&str> {
    object
        .as_stream()
        .and_then(|stream| stream.dict().get("Type"))
        .and_then(PdfObject::as_name)
        .map(|name| name.name())
        .filter(|name| matches!(*name, "XRef" | "ObjStm"))
}

fn entry(changes: &[(ObjectId, StoreEntry)], id: ObjectId) -> Option<&StoreEntry> {
    changes
        .binary_search_by_key(&(id.obj_num(), id.gen_num()), |(id, _)| {
            (id.obj_num(), id.gen_num())
        })
        .ok()
        .map(|index| &changes[index].1)
}

/// Inspect direct containers only; every indirect source object is visited
/// separately. Cyclic references therefore do not cause recursive traversal.
fn inspect(
    object: &PdfObject,
    changes: &[(ObjectId, StoreEntry)],
    check_deleted: bool,
    limits: Limits,
    depth: usize,
    nodes: &mut usize,
    allow_signature: bool,
    authenticated_encrypt: Option<&PdfDict>,
) -> PdfResult<()> {
    if depth > limits.max_depth || *nodes >= limits.max_objects {
        return Err(PdfError::ResourceLimit(
            "incremental preflight nodes/depth".into(),
        ));
    }
    *nodes += 1;
    let dict = match object {
        PdfObject::PdfDict(dict) => Some(dict),
        PdfObject::PdfStream(stream) => Some(stream.dict()),
        _ => None,
    };
    if let Some(dict) = dict {
        if dict.contains_key("Linearized") {
            return Err(unsupported("incremental save of linearized input"));
        }
        if !allow_signature
            && (dict.contains_key("ByteRange")
                || dict
                    .get("SigFlags")
                    .is_some_and(|value| value.as_u32() != Some(0))
                || dict.contains_key("Perms")
                || ["FT", "Type"].iter().any(|key| {
                    dict.get(key)
                        .and_then(PdfObject::as_name)
                        .is_some_and(|name| name.name() == "Sig")
                }))
        {
            return Err(unsupported(
                "incremental save of signed or signature-bearing input",
            ));
        }
        for (key, child) in dict {
            // A historical plaintext xref stream may carry direct Encrypt.
            // Exempt only the exact authenticated dictionary: its Perms byte
            // string is not a Catalog signature permissions dictionary.
            if key == "Encrypt" && structural_stream_type(object) == Some("XRef")
                && authenticated_encrypt.is_some() && child.as_dict() == authenticated_encrypt {
                continue;
            }
            inspect(
                child,
                changes,
                check_deleted,
                limits,
                depth + 1,
                nodes,
                allow_signature,
                authenticated_encrypt,
            )?;
        }
    } else if let PdfObject::PdfArray(array) = object {
        for child in array {
            inspect(
                child,
                changes,
                check_deleted,
                limits,
                depth + 1,
                nodes,
                allow_signature,
                authenticated_encrypt,
            )?;
        }
    } else if let PdfObject::PdfReference(reference) = object {
        if check_deleted && matches!(entry(changes, reference.id()), Some(StoreEntry::Deleted)) {
            return Err(PdfError::InvalidReference(format!(
                "live reference to deleted object {:?}",
                reference.id()
            )));
        }
    }
    Ok(())
}

fn insert_value(
    builder: &mut BudgetedDictBuilder,
    key: &str,
    value: &PdfObject,
    limits: Limits,
    budget: &OperationBudget,
) -> PdfResult<()> {
    let (key, key_lease) = crate::budget::copy_string(key, budget)?;
    let (value, value_lease) = crate::objects::clone_object_with_budget(value, budget, limits, 0)?;
    builder.insert(key, value, key_lease, value_lease)
}

fn insert_number(
    builder: &mut BudgetedDictBuilder,
    key: &str,
    value: u64,
    budget: &OperationBudget,
) -> PdfResult<()> {
    let (key, key_lease) = crate::budget::copy_string(key, budget)?;
    let formatted = format_stack(format_args!("{value}"))?;
    let (number, value_lease) = crate::budget::copy_string(
        std::str::from_utf8(&formatted.bytes[..formatted.len]).unwrap(),
        budget,
    )?;
    builder.insert(
        key,
        PdfObject::PdfNumber(PdfNumber::from_owned_lexeme(number)?),
        key_lease,
        value_lease,
    )
}

fn trailer(
    current: &PdfDict,
    original: &PdfDict,
    size: u32,
    prev: u64,
    renew_id: bool,
    limits: Limits,
    budget: &OperationBudget,
) -> PdfResult<(PdfDict, MemoryLease)> {
    let mut builder = BudgetedDictBuilder::new(budget, limits.max_objects)?;
    for (key, value) in current {
        if retained_trailer_key(key) && key != "Size" && !(renew_id && key == "ID") {
            insert_value(&mut builder, key, value, limits, budget)?;
        }
    }
    if renew_id {
        let (value, value_lease) = super::protection::ids(original, budget, limits)?;
        let (key, key_lease) = crate::budget::copy_string("ID", budget)?;
        builder.insert(key, value, key_lease, value_lease)?;
    }
    // Explicit null prevents readers inheriting removed optional entries from
    // previous trailers (e.g. removed Info). Old bytes still remain recoverable.
    for (key, _) in original {
        if retained_trailer_key(key) && key != "Size" && !current.contains_key(key) {
            insert_value(&mut builder, key, &PdfObject::PdfNull, limits, budget)?;
        }
    }
    insert_number(&mut builder, "Size", u64::from(size), budget)?;
    insert_number(&mut builder, "Prev", prev, budget)?;
    builder.finish()
}

pub(crate) fn write_incremental_with_layout<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    source: &[u8],
    prev: u64,
    original: &PdfDict,
    options: IncrementalSaveOptions,
    layout: IncrementalSaveLayout,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    write_incremental_inner(document, source, prev, original, options, budget, false, None, layout)
}

// Private capability: only signature preparation uses this on an isolated
// unsigned snapshot. Source inspection is NEVER bypassed.
pub(crate) fn write_signature_incremental<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    source: &[u8],
    prev: u64,
    original: &PdfDict,
    options: IncrementalSaveOptions,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    write_incremental_inner(document, source, prev, original, options, budget, true, None, IncrementalSaveLayout::Classic)
}

pub(crate) fn write_signed_revision_incremental<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>, source: &[u8], prev: u64, original: &PdfDict,
    plan: &crate::signatures::append::SignedRevisionPlan, budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    write_incremental_inner(document, source, prev, original, Default::default(), budget, true, Some(plan), IncrementalSaveLayout::Classic)
}

fn write_incremental_inner<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    source: &[u8],
    prev: u64,
    original: &PdfDict,
    options: IncrementalSaveOptions,
    budget: &OperationBudget,
    allow_signature: bool,
    approval: Option<&crate::signatures::append::SignedRevisionPlan>,
    layout: IncrementalSaveLayout,
) -> PdfResult<BudgetedBytes> {
    let limit = options.max_output_bytes;
    if source.len() > limit
        || prev
            >= u64::try_from(source.len())
                .map_err(|_| PdfError::ResourceLimit("source length".into()))?
    {
        return Err(PdfError::ResourceLimit(
            "incremental source/output bytes".into(),
        ));
    }
    if allow_signature && document.ctx().source_security().is_some() {
        return Err(unsupported("encrypted/signature combination"));
    }
    if !allow_signature {
        document.ctx().require_unsigned_source_with_budget(budget)?;
    }
    let effective = document.effective_version_with_budget(budget)?;
    let original_root = original
        .get("Root")
        .and_then(PdfObject::as_object_id)
        .ok_or_else(|| {
            PdfError::InvalidReference("incremental source Root must be indirect".into())
        })?;
    let limits = document.ctx().limits();
    let authenticated_encrypt = document.ctx().source_security().map(|handler| &handler.dictionary);
    // Leases precede owned values so data is dropped before its accounting.
    let (_changes_lease, mut changes) = {
        if let Some(plan) = approval {
            let (entries, lease) = plan.changes_with_budget(document, source, original, budget)?;
            (lease, entries)
        } else {
            let (snapshot, lease) = document.ctx().save_changes_with_budget(budget)?;
            (lease, snapshot.into_entries())
        }
    };
    changes.sort_unstable_by_key(|(id, _)| (id.obj_num(), id.gen_num()));
    let (_trailer_lease, current) = {
        let (dict, lease) =
            crate::objects::clone_dict_with_budget(&document.ctx().trailer(), budget, limits, 0)?;
        (lease, dict)
    };
    security::validate(document, original, &current, &changes, budget)?;
    let (_ids_lease, ids) = {
        let (ids, lease) = document.ctx().object_ids_with_budget(true, budget)?;
        (lease, ids)
    };
    for id in &ids {
        // The authenticated Encrypt dictionary has a /Perms byte string, not
        // the Catalog's signature permissions dictionary. Its identity and
        // unchanged contents were already validated by security::validate.
        if Some(*id) == document.ctx().encryption_dictionary_id() { continue; }
        // Examine the original even if the caller deleted/overwrote a Sig or
        // Linearized object. Editing the overlay cannot bypass source checks.
        if document
            .ctx()
            .source_xref()
            .lookup_id(*id)
            .is_some_and(|info| !matches!(info.state(), ObjectState::Free))
        {
            let (_lease, object) = {
                let (object, lease) = document
                    .ctx()
                    .resolve_source_owned_with_budget(*id, budget)?;
                (lease, object)
            };
            inspect(&object, &changes, false, limits, 1, &mut 0, approval.is_some(), authenticated_encrypt)?;
            if structural_stream_type(&object).is_some() && entry(&changes, *id).is_some() {
                return Err(unsupported(
                    "incremental edits to structural xref/object-stream containers",
                ));
            }
            if *id == original_root {
                if let Some(version) = object
                    .as_dict()
                    .and_then(|dict| dict.get("Version"))
                    .filter(|value| !matches!(value, PdfObject::PdfNull))
                {
                    let (_version_lease, version) = {
                        let (value, lease) = match version.as_object_id() {
                            Some(id) => document
                                .ctx()
                                .resolve_source_owned_with_budget(id, budget)?,
                            None => crate::objects::clone_object_with_budget(
                                version, budget, limits, 0,
                            )?,
                        };
                        (lease, value)
                    };
                    let version = version
                        .as_name()
                        .and_then(|name| {
                            crate::document::save_options::PdfVersion::parse(name.name())
                        })
                        .ok_or_else(|| unsupported("unknown source Catalog Version"))?;
                    if effective < version {
                        return Err(unsupported("incremental PDF version downgrade"));
                    }
                }
            }
        }
        let mut _lease = None;
        let selected = if approval.is_some() {
            match entry(&changes, *id) {
                Some(StoreEntry::Upsert(object)) => Some(crate::objects::clone_object_with_budget(object, budget, limits, 0)?),
                Some(StoreEntry::Deleted) => return Err(unsupported("approval plan may not delete objects")),
                None => document.ctx().object_for_write_with_budget(*id, budget)?,
            }
        } else { document.ctx().object_for_write_with_budget(*id, budget)? };
        if let Some(object) = selected
            .map(|(object, lease)| {
                _lease = Some(lease);
                object
            })
        {
            if structural_stream_type(&object).is_some() && entry(&changes, *id).is_some() {
                return Err(unsupported(
                    "incremental edits to structural xref/object-stream containers",
                ));
            }
            // Historical xref-stream trailer roots (such as Info) are not
            // current live roots. Prev still reads their original bytes.
            inspect(
                &object,
                &changes,
                structural_stream_type(&object) != Some("XRef"),
                limits,
                1,
                &mut 0,
                allow_signature,
                authenticated_encrypt,
            )?;
        }
    }
    if approval.is_some() {
        for (_, change) in &changes {
            let StoreEntry::Upsert(object) = change else { return Err(unsupported("approval deletion")); };
            inspect(object, &changes, true, limits, 1, &mut 0, true, authenticated_encrypt)?;
        }
    }
    for (key, object) in &current {
        // A direct Encrypt dictionary is legal (e.g. MuPDF output). Its Perms
        // string is authenticated, unchanged, and not a signature transform.
        if key == "Encrypt" && document.ctx().source_security().is_some() { continue; }
        inspect(object, &changes, true, limits, 1, &mut 0, allow_signature, authenticated_encrypt)?;
    }
    let mut output = SerializationOutput {
        bytes: BudgetedBytes::copy_from_slice(budget, &[])?,
        limits,
        capacity_limit: limit,
    };
    append_output(&mut output, source, limit)?;
    if changes.is_empty() && &current == original {
        return Ok(output.bytes);
    }
    let original_size = original
        .get("Size")
        .and_then(PdfObject::as_u32)
        .ok_or_else(|| PdfError::ObjectError("incremental source Size must be u32".into()))?;
    let max = ids.iter().map(|id| id.obj_num()).chain(changes.iter().map(|(id, _)| id.obj_num())).max().unwrap_or(0);
    let mut size = original_size.max(
        max.checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("incremental Size overflow".into()))?,
    );
    // Allocate only after the no-op exit. This output-only object must not
    // capture a dangling reference or mutate the document allocator.
    let stream_id = if layout == IncrementalSaveLayout::XrefStream {
        if effective < PdfVersion::V1_5 {
            return Err(unsupported("xref-stream incremental save requires effective PDF 1.5+"));
        }
        let source_end = document.ctx().source_xref().objects().keys().max().copied()
            .unwrap_or(0).checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("incremental source object number".into()))?;
        let id = ObjectId::new(size.max(source_end).max(1), 0);
        size = id.obj_num().checked_add(1)
            .filter(|n| *n as usize <= limits.max_objects)
            .ok_or_else(|| PdfError::ResourceLimit("incremental xref-stream Size".into()))?;
        for id_to_check in &ids {
            if let Some((object, _lease)) = document.ctx().object_for_write_with_budget(*id_to_check, budget)? {
                stream::validate_references(&object, id, limits.max_depth, 0)?;
            }
        }
        for (_, object) in &current {
            stream::validate_references(object, id, limits.max_depth, 0)?;
        }
        Some(id)
    } else { None };
    let capacity = document
        .ctx()
        .source_xref()
        .objects()
        .len()
        .checked_add(changes.len())
        .and_then(|n| n.checked_add(1 + usize::from(stream_id.is_some())))
        .ok_or_else(|| PdfError::ResourceLimit("incremental xref entries".into()))?;
    let mut xref_lease = budget.reserve(
        capacity
            .checked_mul(std::mem::size_of::<(u32, u64, u32, u8)>())
            .ok_or_else(|| PdfError::ResourceLimit("incremental xref allocation".into()))?,
    )?;
    let mut xref: Vec<(u32, u64, u32, u8)> = Vec::new();
    xref.try_reserve_exact(capacity)
        .map_err(|_| PdfError::ResourceLimit("incremental xref allocation".into()))?;
    xref_lease.resize(xref.capacity() * std::mem::size_of::<(u32, u64, u32, u8)>())?;
    xref.push((0, 0, 65535, b'f'));
    for (&number, info) in document.ctx().source_xref().objects() {
        if stream_id.is_some() && (info.gennum() > 65535 && matches!(info.state(), ObjectState::Free)
            || number == 0 && (!matches!(info.state(), ObjectState::Free) || info.gennum() != 65535)) {
            return Err(PdfError::ObjectError("invalid incremental free generation".into()));
        }
        if number != 0 && matches!(info.state(), ObjectState::Free) {
            xref.push((number, 0, info.gennum(), b'f'));
        }
    }
    append_output(&mut output, b"\n", limit)?;
    for (id, change) in changes {
        if id.obj_num() == 0 || id.gen_num() > 65535 {
            return Err(PdfError::ObjectError(
                "incremental object number/generation".into(),
            ));
        }
        if let Some(info) = document.ctx().source_xref().lookup(&id.obj_num()) {
            if matches!(info.state(), ObjectState::Free)
                || document.ctx().source_xref().lookup_id(id).is_none()
            {
                return Err(PdfError::InvalidReference(
                    "incremental generation/reuse conflict".into(),
                ));
            }
        } else if id.obj_num() < original_size {
            return Err(PdfError::InvalidReference(
                "incremental object number reuse".into(),
            ));
        }
        match change {
            StoreEntry::Deleted => xref.push((
                id.obj_num(),
                0,
                id.gen_num().saturating_add(1).min(65535),
                b'f',
            )),
            StoreEntry::Upsert(object) => {
                validate_serialization_nodes_with_budget(
                    std::iter::once((&object, 1)),
                    0,
                    limits,
                    Some(budget),
                )?;
                feature_versions::validate_feature_versions_with_budget(
                    document,
                    &object,
                    effective,
                    Some(budget),
                )?;
                // The source objects were decrypted once at their original
                // object boundary. Encrypt only the changed standalone output
                // object, including updates originally stored in an ObjStm.
                let mut encrypted_lease = None;
                let object = if let Some(handler) = document.ctx().source_security() {
                    let (object, lease) = handler.transform(document.ctx(), &object, true, budget)?;
                    encrypted_lease = Some(lease);
                    object
                } else { object };
                let offset = u64::try_from(output.len())
                    .map_err(|_| PdfError::ResourceLimit("incremental object offset".into()))?;
                xref.push((id.obj_num(), offset, id.gen_num(), b'n'));
                append_formatted_output(
                    &mut output,
                    format_args!("{} {} obj\n", id.obj_num(), id.gen_num()),
                    limit,
                )?;
                write_object_limited(&mut output, &object, limit)?;
                append_output(&mut output, b"\nendobj\n", limit)?;
                drop(object);
                drop(encrypted_lease);
            }
        }
    }
    let xref_offset = output.len();
    if let Some(id) = stream_id {
        xref.push((id.obj_num(), u64::try_from(xref_offset)
            .map_err(|_| PdfError::ResourceLimit("incremental xref offset".into()))?, 0, b'n'));
    }
    xref.sort_unstable_by_key(|entry| entry.0);
    if xref.windows(2).any(|pair| pair[0].0 == pair[1].0) {
        return Err(PdfError::InvalidReference(
            "duplicate incremental xref number".into(),
        ));
    }
    let mut next_free = 0;
    for (number, offset, _, state) in xref.iter_mut().rev() {
        if *state == b'f' {
            *offset = next_free;
            next_free = u64::from(*number);
        }
    }
    if let Some(id) = stream_id {
        let (_lease, dict) = {
            let (dict, lease) = trailer(&current, original, size, prev,
                document.ctx().source_security().is_some(), limits, budget)?;
            (lease, dict)
        };
        stream::write(&mut output, id, &xref, &dict, limits, limit, budget)?;
        append_formatted_output(&mut output,
            format_args!("\nstartxref\n{xref_offset}\n%%EOF\n"), limit)?;
        return Ok(output.bytes);
    }
    append_output(&mut output, b"xref\n", limit)?;
    let mut start = 0;
    while start < xref.len() {
        let mut end = start + 1;
        while end < xref.len() && xref[end].0 == xref[end - 1].0 + 1 {
            end += 1;
        }
        append_formatted_output(
            &mut output,
            format_args!("{} {}\n", xref[start].0, end - start),
            limit,
        )?;
        for (_, offset, generation, state) in &xref[start..end] {
            if *offset > 9_999_999_999 || *generation > 65535 {
                return Err(PdfError::ResourceLimit(
                    "classic incremental xref field width".into(),
                ));
            }
            append_formatted_output(
                &mut output,
                format_args!("{offset:010} {generation:05} {} \n", *state as char),
                limit,
            )?;
        }
        start = end;
    }
    let (_lease, trailer) = {
        let (dict, lease) = trailer(&current, original, size, prev,
            document.ctx().source_security().is_some(), limits, budget)?;
        (lease, dict)
    };
    validate_serialization_nodes_with_budget(
        trailer.into_iter().map(|(_, value)| (value, 2)),
        1,
        limits,
        Some(budget),
    )?;
    append_output(&mut output, b"trailer\n", limit)?;
    write_dict_limited(&mut output, &trailer, limit)?;
    append_formatted_output(
        &mut output,
        format_args!("\nstartxref\n{xref_offset}\n%%EOF\n"),
        limit,
    )?;
    Ok(output.bytes)
}
