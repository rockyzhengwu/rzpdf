//! A full rewrite invalidates linearization offsets. Remove only recognized,
//! source-owned structures, without changing document state or collecting
//! unrelated objects. Incremental saving deliberately does not use this plan.
use super::*;
use crate::budget::OperationBudget;
use crate::document::cross_ref_table::ObjectState;

#[derive(Default)]
pub(super) struct Removal {
    // One parameter dictionary and at most two hint streams (Annex F).
    ids: [Option<ObjectId>; 3],
}

fn unsupported() -> PdfError {
    PdfError::UnsupportedFeature(
        "full save cannot safely remove modified, malformed or referenced linearization structures"
            .into(),
    )
}

impl Removal {
    pub(super) fn removed(&self, id: ObjectId) -> bool {
        self.ids.contains(&Some(id))
    }

    pub(super) fn inspect<R: std::io::Read + std::io::Seek>(
        doc: &PdfDocument<R>,
        ids: &[ObjectId],
        budget: &OperationBudget,
    ) -> PdfResult<Self> {
        let mut result = Self::default();
        let source = doc.ctx().source_xref();
        let Some(first) = source
            .objects()
            .values()
            .filter(|info| matches!(info.state(), ObjectState::Normal))
            .min_by_key(|info| info.offset())
        else {
            return Ok(result);
        };
        let (object, _lease) = doc
            .ctx()
            .resolve_source_owned_with_budget(first.object_id(), budget)?;
        let Some(dict) = object.as_dict().filter(|d| d.contains_key("Linearized")) else {
            return Ok(result);
        };
        // Unknown fields may carry application data; do not discard them.
        if dict.into_iter().any(|(key, _)| {
            !matches!(
                key.as_str(),
                "Linearized" | "L" | "H" | "O" | "E" | "N" | "T" | "P"
            )
        }) || dict
            .get("Linearized")
            .and_then(PdfObject::as_number)
            .is_none_or(|n| n.value() != 1.0)
        {
            return Err(unsupported());
        }
        for key in ["L", "O", "E", "N", "T"] {
            if dict
                .get(key)
                .and_then(PdfObject::as_number)
                .and_then(|n| n.as_u64_checked().ok())
                .is_none_or(|n| n == 0)
            {
                return Err(unsupported());
            }
        }
        if dict.get("P").is_some_and(|value| {
            value
                .as_number()
                .and_then(|n| n.as_u64_checked().ok())
                .is_none()
        }) {
            return Err(unsupported());
        }
        let hints = dict
            .get("H")
            .and_then(PdfObject::as_array)
            .ok_or_else(unsupported)?;
        if !matches!(hints.len(), 2 | 4) {
            return Err(unsupported());
        }
        result.ids[0] = Some(first.object_id());
        for index in 0..hints.len() / 2 {
            let integer = |i: usize| {
                hints
                    .get(i)
                    .and_then(PdfObject::as_number)
                    .and_then(|n| n.as_u64_checked().ok())
                    .ok_or_else(unsupported)
            };
            let offset = integer(index * 2)?;
            let length = integer(index * 2 + 1)?;
            let end = offset
                .checked_add(length)
                .filter(|_| length != 0)
                .ok_or_else(unsupported)?;
            let hint = source
                .objects()
                .values()
                .find(|info| matches!(info.state(), ObjectState::Normal) && info.offset() == offset)
                .ok_or_else(unsupported)?;
            if result.removed(hint.object_id()) {
                return Err(unsupported());
            }
            let object_end =
                doc.ctx()
                    .source_object_end_with_budget(offset, hint.object_id(), budget)?;
            if end < object_end
                || source.objects().values().any(|info| {
                    matches!(info.state(), ObjectState::Normal)
                        && info.offset() > offset
                        && info.offset() < end
                })
            {
                return Err(unsupported());
            }
            let (object, _lease) = doc
                .ctx()
                .resolve_source_owned_with_budget(hint.object_id(), budget)?;
            let stream = object.as_stream().ok_or_else(unsupported)?;
            if stream.dict().into_iter().any(|(key, _)| {
                !matches!(
                    key.as_str(),
                    "S" | "O" | "Length" | "Filter" | "DecodeParms" | "DL"
                )
            }) || (index == 0 && stream.dict().get("S").is_none())
            {
                return Err(unsupported());
            }
            result.ids[index + 1] = Some(hint.object_id());
        }
        // A changed hint stream/dictionary may now contain user data. Refuse
        // rather than quietly discarding an overlay edit to a structural object.
        for id in result.ids.into_iter().flatten() {
            let (source, _source_lease) = doc.ctx().resolve_source_owned_with_budget(id, budget)?;
            if let Some((effective, _lease)) = doc.ctx().object_for_write_with_budget(id, budget)?
                && effective != source
            {
                return Err(unsupported());
            }
        }
        for &id in ids {
            if result.removed(id) {
                continue;
            }
            if let Some((object, _lease)) = doc.ctx().object_for_write_with_budget(id, budget)? {
                result.validate_references(&object, doc.ctx().limits().max_depth)?;
            }
        }
        for (key, value) in doc.ctx().trailer().into_iter() {
            if retained_trailer_key(key) {
                result.validate_references(value, doc.ctx().limits().max_depth)?;
            }
        }
        Ok(result)
    }

    fn validate_references(&self, object: &PdfObject, depth: usize) -> PdfResult<()> {
        super::object_stream::visit_references(object, depth, 0, &mut |id| {
            if self.removed(id) {
                Err(unsupported())
            } else {
                Ok(())
            }
        })
    }
}
