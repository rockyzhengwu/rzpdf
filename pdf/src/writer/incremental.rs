//! Explicit append-only serialization. Original bytes are never rewritten.
use super::*;
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    document::{
        cross_ref_table::ObjectState, save_options::IncrementalSaveOptions, store::StoreEntry,
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
        for (_, child) in dict {
            inspect(
                child,
                changes,
                check_deleted,
                limits,
                depth + 1,
                nodes,
                allow_signature,
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
    limits: Limits,
    budget: &OperationBudget,
) -> PdfResult<(PdfDict, MemoryLease)> {
    let mut builder = BudgetedDictBuilder::new(budget, limits.max_objects)?;
    for (key, value) in current {
        if retained_trailer_key(key) && key != "Size" {
            insert_value(&mut builder, key, value, limits, budget)?;
        }
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

pub(crate) fn write_incremental<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    source: &[u8],
    prev: u64,
    original: &PdfDict,
    options: IncrementalSaveOptions,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    write_incremental_inner(document, source, prev, original, options, budget, false)
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
    write_incremental_inner(document, source, prev, original, options, budget, true)
}

fn write_incremental_inner<R: std::io::Read + std::io::Seek>(
    document: &PdfDocument<R>,
    source: &[u8],
    prev: u64,
    original: &PdfDict,
    options: IncrementalSaveOptions,
    budget: &OperationBudget,
    allow_signature: bool,
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
    for dict in [original, &*document.ctx().trailer()] {
        if dict
            .get("Encrypt")
            .is_some_and(|obj| !matches!(obj, PdfObject::PdfNull))
        {
            return Err(unsupported("incremental save of encrypted input"));
        }
    }
    let effective = document.effective_version_with_budget(budget)?;
    let original_root = original
        .get("Root")
        .and_then(PdfObject::as_object_id)
        .ok_or_else(|| {
            PdfError::InvalidReference("incremental source Root must be indirect".into())
        })?;
    let limits = document.ctx().limits();
    // Leases precede owned values so data is dropped before its accounting.
    let (_changes_lease, mut changes) = {
        let (snapshot, lease) = document.ctx().save_changes_with_budget(budget)?;
        (lease, snapshot.into_entries())
    };
    changes.sort_unstable_by_key(|(id, _)| (id.obj_num(), id.gen_num()));
    let (_trailer_lease, current) = {
        let (dict, lease) =
            crate::objects::clone_dict_with_budget(&document.ctx().trailer(), budget, limits, 0)?;
        (lease, dict)
    };
    let (_ids_lease, ids) = {
        let (ids, lease) = document.ctx().object_ids_with_budget(true, budget)?;
        (lease, ids)
    };
    for id in &ids {
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
            inspect(&object, &changes, false, limits, 1, &mut 0, false)?;
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
        if let Some(object) = document
            .ctx()
            .object_for_write_with_budget(*id, budget)?
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
            )?;
        }
    }
    for (_, object) in &current {
        inspect(object, &changes, true, limits, 1, &mut 0, allow_signature)?;
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
    let max = ids.iter().map(|id| id.obj_num()).max().unwrap_or(0);
    let size = original_size.max(
        max.checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("incremental Size overflow".into()))?,
    );
    let capacity = document
        .ctx()
        .source_xref()
        .objects()
        .len()
        .checked_add(changes.len())
        .and_then(|n| n.checked_add(1))
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
            }
        }
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
    let xref_offset = output.len();
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
        let (dict, lease) = trailer(&current, original, size, prev, limits, budget)?;
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
