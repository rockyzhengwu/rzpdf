use super::*;
use crate::{
    budget::{MemoryLease, OperationBudget},
    objects::{pdf_dict::BudgetedDictBuilder, pdf_reference::PdfReference},
    parser::options::Limits,
};

pub(super) fn ids(
    original: &PdfDict,
    budget: &OperationBudget,
    limits: Limits,
) -> PdfResult<(PdfObject, MemoryLease)> {
    let mut lease = budget.reserve(2 * std::mem::size_of::<PdfObject>())?;
    let mut values = Vec::new();
    values
        .try_reserve_exact(2)
        .map_err(|_| PdfError::ResourceLimit("file ID array allocation".into()))?;
    lease.resize(values.capacity() * std::mem::size_of::<PdfObject>())?;
    let first = original
        .get("ID")
        .and_then(PdfObject::as_array)
        .filter(|array| array.len() == 2)
        .and_then(|array| array.into_iter().next())
        .filter(|value| {
            value
                .as_string()
                .is_some_and(|string| !string.bytes().is_empty())
        });
    for index in 0..2 {
        let (value, mut child) = if let (0, Some(first)) = (index, first) {
            crate::objects::clone_object_with_budget(first, budget, limits, 0)?
        } else {
            let mut bytes = [0; 16];
            crate::security::r6_random(&mut bytes)?;
            let (data, lease) = crate::budget::BudgetedBytes::copy_from_slice(budget, &bytes)?
                .into_reserved_parts();
            (PdfObject::PdfString(PdfString::new(data, true)), lease)
        };
        lease.absorb(&mut child)?;
        values.push(value);
    }
    Ok((PdfObject::PdfArray(PdfArray::from_items(values)), lease))
}

pub(super) fn trailer(
    original: &PdfDict,
    encryption: Option<ObjectId>,
    limits: Limits,
    budget: &OperationBudget,
) -> PdfResult<(PdfDict, MemoryLease)> {
    let mut builder = BudgetedDictBuilder::new(budget, limits.max_objects)?;
    for (key, value) in original {
        if key == "Encrypt" || (key == "ID" && encryption.is_some()) {
            continue;
        }
        let (key, key_lease) = crate::budget::copy_string(key, budget)?;
        let (value, lease) = crate::objects::clone_object_with_budget(value, budget, limits, 0)?;
        builder.insert(key, value, key_lease, lease)?;
    }
    if let Some(id) = encryption {
        let (key, key_lease) = crate::budget::copy_string("Encrypt", budget)?;
        builder.insert(
            key,
            PdfObject::PdfReference(PdfReference::from_id(id)),
            key_lease,
            budget.reserve(0)?,
        )?;
        let (key, key_lease) = crate::budget::copy_string("ID", budget)?;
        let (value, value_lease) = ids(original, budget, limits)?;
        builder.insert(key, value, key_lease, value_lease)?;
    }
    builder.finish()
}
