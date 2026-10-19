//! Sparse, plaintext incremental xref stream. No new object streams.
use super::*;
use crate::writer::xref::{RECORD_BYTES, XrefRecord};

pub(super) fn validate_references(
    object: &PdfObject,
    structural: ObjectId,
    max_depth: usize,
    depth: usize,
) -> PdfResult<()> {
    if depth > max_depth {
        return Err(PdfError::ResourceLimit(
            "incremental reference depth".into(),
        ));
    }
    match object {
        PdfObject::PdfReference(reference) if reference.id() == structural => {
            return Err(PdfError::InvalidReference(
                "input reference aliases incremental xref stream".into(),
            ));
        }
        PdfObject::PdfArray(array) => {
            for value in array {
                validate_references(value, structural, max_depth, depth + 1)?;
            }
        }
        PdfObject::PdfDict(dict) => {
            for (_, value) in dict {
                validate_references(value, structural, max_depth, depth + 1)?;
            }
        }
        PdfObject::PdfStream(stream) => {
            for (_, value) in stream.dict() {
                validate_references(value, structural, max_depth, depth + 1)?;
            }
        }
        _ => {}
    }
    Ok(())
}

pub(super) fn write(
    output: &mut impl OutputBuffer,
    id: ObjectId,
    records: &[(u32, u64, u32, u8)],
    trailer: &PdfDict,
    limits: Limits,
    limit: usize,
    budget: &OperationBudget,
) -> PdfResult<()> {
    let length = records
        .len()
        .checked_mul(RECORD_BYTES)
        .filter(|n| *n <= limits.max_raw_stream_bytes && *n <= limits.max_decoded_bytes)
        .ok_or_else(|| PdfError::ResourceLimit("incremental xref payload bytes".into()))?;
    // /Index contains only contiguous runs of represented entries. Its backing
    // storage and temporary number descriptors share the operation budget.
    let runs = usize::from(!records.is_empty())
        + records
            .windows(2)
            .filter(|pair| pair[0].0.checked_add(1) != Some(pair[1].0))
            .count();
    let count = runs
        .checked_mul(2)
        .ok_or_else(|| PdfError::ResourceLimit("incremental Index count".into()))?;
    if count > limits.max_objects {
        return Err(PdfError::ResourceLimit("incremental Index count".into()));
    }
    let mut index_lease = budget.reserve(
        count
            .checked_mul(std::mem::size_of::<PdfObject>())
            .ok_or_else(|| PdfError::ResourceLimit("incremental Index allocation".into()))?,
    )?;
    let mut index = Vec::new();
    index
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("incremental Index allocation".into()))?;
    index_lease.resize(index.capacity() * std::mem::size_of::<PdfObject>())?;
    let _descriptors = budget.reserve(
        count
            .checked_mul(32)
            .and_then(|n| n.checked_add(4096))
            .ok_or_else(|| PdfError::ResourceLimit("incremental xref descriptors".into()))?,
    )?;
    let number = |n: u64| -> PdfResult<PdfObject> {
        let text = format_stack(format_args!("{n}"))?;
        Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
            &text.bytes[..text.len],
        )?))
    };
    let mut start = 0;
    while start < records.len() {
        let mut end = start + 1;
        while end < records.len() && records[end - 1].0.checked_add(1) == Some(records[end].0) {
            end += 1;
        }
        index.push(number(u64::from(records[start].0))?);
        index.push(number((end - start) as u64)?);
        start = end;
    }
    let mut builder = BudgetedDictBuilder::new(budget, limits.max_objects)?;
    for (key, value) in trailer {
        // Prev is deliberately retained; XRefStm describes the previous
        // revision's hybrid supplement and must never leak into this one.
        if !matches!(
            key.as_str(),
            "Type"
                | "W"
                | "Index"
                | "Length"
                | "Filter"
                | "DecodeParms"
                | "XRefStm"
                | "DL"
                | "F"
                | "FFilter"
                | "FDecodeParms"
        ) {
            insert_value(&mut builder, key, value, limits, budget)?;
        }
    }
    insert_value(
        &mut builder,
        "Type",
        &PdfObject::PdfName(PdfName::new("XRef".into())),
        limits,
        budget,
    )?;
    insert_number(&mut builder, "Length", length as u64, budget)?;
    insert_value(
        &mut builder,
        "W",
        &PdfObject::PdfArray(PdfArray::from_items(vec![
            number(1)?,
            number(8)?,
            number(4)?,
        ])),
        limits,
        budget,
    )?;
    insert_value(
        &mut builder,
        "Index",
        &PdfObject::PdfArray(PdfArray::from_items(index)),
        limits,
        budget,
    )?;
    let (_dict_lease, dict) = {
        let (dict, lease) = builder.finish()?;
        (lease, dict)
    };
    validate_serialization_nodes_with_budget(
        dict.into_iter().map(|(_, v)| (v, 2)),
        1,
        limits,
        Some(budget),
    )?;
    append_formatted_output(output, format_args!("{} 0 obj\n", id.obj_num()), limit)?;
    write_dict_limited(output, &dict, limit)?;
    append_output(output, b"\nstream\n", limit)?;
    for &(_, field, generation, state) in records {
        let generation = u16::try_from(generation)
            .map_err(|_| PdfError::ObjectError("incremental xref generation".into()))?;
        let record = if state == b'f' {
            XrefRecord::Free {
                next: u32::try_from(field)
                    .map_err(|_| PdfError::ResourceLimit("incremental free link".into()))?,
                generation,
            }
        } else {
            XrefRecord::Uncompressed {
                offset: field,
                generation,
            }
        };
        append_output(output, &record.encode()?, limit)?;
    }
    append_output(output, b"\nendstream\nendobj\n", limit)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn direct_encrypt_exception_requires_xref_context_and_exact_authenticated_dictionary() {
        let mut encryption = PdfDict::default();
        encryption
            .try_insert(
                "Perms".into(),
                PdfObject::PdfString(PdfString::new(vec![1; 16], true)),
            )
            .unwrap();
        for (kind, key, exact, accepted) in [
            ("XRef", "Encrypt", true, true),
            ("XRef", "Encrypt", false, false),
            ("XRef", "Private", true, false),
            ("Metadata", "Encrypt", true, false),
        ] {
            let mut child = encryption.clone();
            if !exact {
                child
                    .try_insert("Private".into(), PdfObject::PdfNull)
                    .unwrap();
            }
            let mut dict = PdfDict::default();
            dict.try_insert("Type".into(), PdfObject::PdfName(PdfName::new(kind.into())))
                .unwrap();
            dict.try_insert(key.into(), PdfObject::PdfDict(child))
                .unwrap();
            let object = PdfObject::PdfStream(PdfStream::new(dict, vec![]));
            let result = inspect(
                &object,
                &[],
                false,
                Limits::default(),
                1,
                &mut 0,
                false,
                Some(&encryption),
            );
            assert_eq!(result.is_ok(), accepted, "{kind} {key} {exact}");
        }
    }
}
