//! Full-save-only ephemeral layout. Never edits the document's object store.
use super::xref::{RECORD_BYTES, XrefRecord};
use super::*;
use crate::{
    budget::{MemoryLease, OperationBudget},
    document::save_options::SaveLayout,
    objects::{pdf_dict::BudgetedDictBuilder, pdf_number::PdfNumber},
    parser::options::Limits,
};

enum Entries {
    Classic(Vec<(u64, u32, u8)>),
    Stream {
        id: ObjectId,
        entries: Vec<XrefRecord>,
    },
}

pub(super) struct FullLayoutPlan {
    entries: Entries,
    _lease: Option<MemoryLease>,
    size: u32,
}

impl FullLayoutPlan {
    pub(super) fn new(
        layout: SaveLayout,
        classic_size: u32,
        trailer: &PdfDict,
        source: &crate::document::cross_ref_table::CrossRefTable,
        limits: Limits,
        budget: Option<&OperationBudget>,
    ) -> PdfResult<Self> {
        if layout == SaveLayout::Classic {
            let (mut entries, lease) =
                allocate_xref_entries_inner(classic_size, limits.max_objects, budget)?;
            if let Some(zero) = entries.first_mut() {
                *zero = (0, 65535, b'f');
            }
            return Ok(Self {
                entries: Entries::Classic(entries),
                _lease: lease,
                size: classic_size,
            });
        }
        // Avoid source-reserved object numbers even when a sparse table's Size
        // is greater than its highest materialized object number.
        let source_size = trailer
            .get("Size")
            .map(|v| {
                v.as_number()
                    .ok_or_else(|| {
                        PdfError::ObjectError("xref source Size must be an integer".into())
                    })?
                    .as_u32_checked()
            })
            .transpose()?
            .unwrap_or(0);
        let source_end = source
            .objects()
            .keys()
            .max()
            .copied()
            .map(|n| {
                n.checked_add(1)
                    .ok_or_else(|| PdfError::ResourceLimit("source object number exhausted".into()))
            })
            .transpose()?
            .unwrap_or(1);
        let number = classic_size.max(source_size).max(source_end).max(1);
        let size = number
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("xref-stream Size overflow".into()))?;
        let count = usize::try_from(size)
            .map_err(|_| PdfError::ResourceLimit("xref-stream Size".into()))?;
        if count > limits.max_objects {
            return Err(PdfError::ResourceLimit("xref-stream entries".into()));
        }
        count
            .checked_mul(RECORD_BYTES)
            .filter(|n| *n <= limits.max_raw_stream_bytes && *n <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("xref-stream payload bytes".into()))?;
        let allocation = count
            .checked_mul(std::mem::size_of::<XrefRecord>())
            .ok_or_else(|| PdfError::ResourceLimit("xref-stream allocation".into()))?;
        let mut lease = budget.map(|b| b.reserve(allocation)).transpose()?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("xref-stream allocation".into()))?;
        if let Some(lease) = lease.as_mut() {
            lease.resize(entries.capacity() * std::mem::size_of::<XrefRecord>())?;
        }
        entries.resize(
            count,
            XrefRecord::Free {
                next: 0,
                generation: 0,
            },
        );
        entries[0] = XrefRecord::Free {
            next: 0,
            generation: 65535,
        };
        for (&number, info) in source.objects() {
            if matches!(
                info.state(),
                crate::document::cross_ref_table::ObjectState::Free
            ) {
                let generation = u16::try_from(info.gennum()).map_err(|_| {
                    PdfError::ObjectError("source free generation exceeds 65535".into())
                })?;
                if number == 0 && generation != 65535 {
                    return Err(PdfError::ObjectError(
                        "invalid reserved object zero generation".into(),
                    ));
                }
                entries[number as usize] = XrefRecord::Free {
                    next: 0,
                    generation,
                };
            }
        }
        Ok(Self {
            entries: Entries::Stream {
                id: ObjectId::new(number, 0),
                entries,
            },
            _lease: lease,
            size,
        })
    }

    pub(super) fn size(&self) -> u32 {
        self.size
    }

    // Allocate only in this output plan. No document allocator/overlay writes.
    pub(super) fn allocate_container(
        &mut self,
        limits: Limits,
        budget: &OperationBudget,
    ) -> PdfResult<ObjectId> {
        let Entries::Stream { entries, .. } = &mut self.entries else {
            return Err(PdfError::ObjectError(
                "object streams require stream xref".into(),
            ));
        };
        let size = self
            .size
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("object-stream Size overflow".into()))?;
        let count = size as usize;
        if count > limits.max_objects
            || count
                .checked_mul(RECORD_BYTES)
                .is_none_or(|n| n > limits.max_raw_stream_bytes || n > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit(
                "object-stream xref entries/payload".into(),
            ));
        }
        let bytes = count
            .checked_mul(std::mem::size_of::<XrefRecord>())
            .ok_or_else(|| PdfError::ResourceLimit("object-stream xref allocation".into()))?;
        // Charge both arrays during reallocation. Failure leaves this plan intact.
        let mut lease = budget.reserve(bytes)?;
        let mut replacement = Vec::new();
        replacement
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("object-stream xref allocation".into()))?;
        lease.resize(replacement.capacity() * std::mem::size_of::<XrefRecord>())?;
        replacement.extend_from_slice(entries);
        replacement.push(XrefRecord::Free {
            next: 0,
            generation: 0,
        });
        *entries = replacement;
        self._lease = Some(lease);
        let id = ObjectId::new(self.size, 0);
        self.size = size;
        Ok(id)
    }

    pub(super) fn record_compressed(
        &mut self,
        id: ObjectId,
        container: ObjectId,
        index: u32,
    ) -> PdfResult<()> {
        let Entries::Stream { entries, .. } = &mut self.entries else {
            return Err(PdfError::ObjectError(
                "compressed entry requires stream xref".into(),
            ));
        };
        if id.obj_num() == 0
            || id.gen_num() != 0
            || container.gen_num() != 0
            || container.obj_num() == 0
            || container.obj_num() >= self.size
        {
            return Err(PdfError::InvalidReference(
                "invalid compressed object identity".into(),
            ));
        }
        let slot = entries
            .get_mut(id.obj_num() as usize)
            .ok_or_else(|| PdfError::InvalidReference("compressed object outside layout".into()))?;
        if !matches!(slot, XrefRecord::Free { .. }) {
            return Err(PdfError::ObjectError(
                "duplicate compressed output object".into(),
            ));
        }
        *slot = XrefRecord::Compressed {
            container: container.obj_num(),
            index,
        };
        Ok(())
    }

    pub(super) fn record_deleted(&mut self, id: ObjectId) -> PdfResult<()> {
        if let Entries::Stream { entries, .. } = &mut self.entries {
            let generation = u16::try_from(id.gen_num())
                .map_err(|_| PdfError::ObjectError("deleted generation exceeds 65535".into()))?
                .saturating_add(1);
            let slot = entries
                .get_mut(id.obj_num() as usize)
                .ok_or_else(|| PdfError::ObjectError("deleted object outside layout".into()))?;
            *slot = XrefRecord::Free {
                next: 0,
                generation,
            };
        }
        Ok(())
    }
    pub(super) fn is_stream(&self) -> bool {
        matches!(self.entries, Entries::Stream { .. })
    }

    pub(super) fn record(&mut self, id: ObjectId, offset: u64) -> PdfResult<()> {
        match &mut self.entries {
            Entries::Classic(entries) => {
                entries[id.obj_num() as usize] = (offset, id.gen_num(), b'n')
            }
            Entries::Stream { entries, .. } => {
                if id.obj_num() == 0 {
                    return Err(PdfError::InvalidReference("object zero is reserved".into()));
                }
                let generation = u16::try_from(id.gen_num())
                    .map_err(|_| PdfError::ObjectError("xref generation exceeds 65535".into()))?;
                let slot = entries
                    .get_mut(id.obj_num() as usize)
                    .ok_or_else(|| PdfError::ObjectError("xref record outside layout".into()))?;
                if !matches!(slot, XrefRecord::Free { .. }) {
                    return Err(PdfError::ObjectError(
                        "duplicate output object number".into(),
                    ));
                }
                *slot = XrefRecord::Uncompressed { offset, generation };
            }
        }
        Ok(())
    }

    pub(super) fn classic_entries(&self) -> PdfResult<&[(u64, u32, u8)]> {
        match &self.entries {
            Entries::Classic(entries) => Ok(entries),
            _ => Err(PdfError::ObjectError("not a classic layout".into())),
        }
    }

    // A dangling input reference must not accidentally become a reference to
    // the newly allocated structural object. Validation does not resolve it.
    pub(super) fn validate_references(
        &self,
        object: &PdfObject,
        max_depth: usize,
        depth: usize,
    ) -> PdfResult<()> {
        let Entries::Stream { id, .. } = &self.entries else {
            return Ok(());
        };
        if depth > max_depth {
            return Err(PdfError::ResourceLimit(
                "xref layout reference depth".into(),
            ));
        }
        match object {
            PdfObject::PdfReference(reference)
                if reference.id().gen_num() == 0
                    && reference.id().obj_num() >= id.obj_num()
                    && reference.id().obj_num() < self.size =>
            {
                return Err(PdfError::InvalidReference(
                    "input reference aliases new layout object".into(),
                ));
            }
            PdfObject::PdfArray(array) => {
                for value in array {
                    self.validate_references(value, max_depth, depth + 1)?;
                }
            }
            PdfObject::PdfDict(dict) => {
                if dict.contains_key("Linearized") {
                    return Err(PdfError::UnsupportedFeature(
                        "xref-stream full save cannot retain a linearization declaration".into(),
                    ));
                }
                for (_, value) in dict {
                    self.validate_references(value, max_depth, depth + 1)?;
                }
            }
            PdfObject::PdfStream(stream) => {
                if stream.dict().contains_key("Linearized") {
                    return Err(PdfError::UnsupportedFeature(
                        "xref-stream full save cannot retain a linearization declaration".into(),
                    ));
                }
                for (_, value) in stream.dict() {
                    self.validate_references(value, max_depth, depth + 1)?;
                }
            }
            _ => {}
        }
        Ok(())
    }

    pub(super) fn validate_trailer_references(
        &self,
        trailer: &PdfDict,
        depth: usize,
        encryption: Option<ObjectId>,
    ) -> PdfResult<()> {
        for (key, value) in trailer {
            if key == "Encrypt"
                && encryption.is_some()
                && value.as_reference().map(|r| r.id()) == encryption
            {
                continue;
            }
            self.validate_references(value, depth, 1)?;
        }
        Ok(())
    }

    pub(super) fn write_stream(
        &mut self,
        mut output: &mut impl OutputBuffer,
        original: &PdfDict,
        limits: Limits,
        limit: usize,
    ) -> PdfResult<()> {
        let Entries::Stream { id, entries } = &mut self.entries else {
            return Err(PdfError::ObjectError("not a stream layout".into()));
        };
        let offset = output.len() as u64;
        entries[id.obj_num() as usize] = XrefRecord::Uncompressed {
            offset,
            generation: 0,
        };
        let mut next = 0;
        for (number, entry) in entries.iter_mut().enumerate().rev() {
            if let XrefRecord::Free { generation, .. } = *entry {
                *entry = XrefRecord::Free { next, generation };
                next = number as u32;
            }
        }
        let length = entries
            .len()
            .checked_mul(RECORD_BYTES)
            .ok_or_else(|| PdfError::ResourceLimit("xref-stream length".into()))?;
        let budget = output.budget().unwrap_or_else(|| limits.operation_budget());
        // Cover small temporary descriptor values in addition to owned builder
        // keys and cloned source trailer values, which retain separate leases.
        let _descriptor = budget.reserve(4096)?;
        let mut builder = BudgetedDictBuilder::new(&budget, limits.max_objects)?;
        for (key, value) in original {
            if retained_trailer_key(key)
                && !matches!(
                    key.as_str(),
                    "Size" | "DL" | "F" | "FFilter" | "FDecodeParms"
                )
            {
                add(&mut builder, key, value, &budget, limits)?;
            }
        }
        let number = |n: u64| -> PdfResult<PdfObject> {
            Ok(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                n.to_string().as_bytes(),
            )?))
        };
        add(
            &mut builder,
            "Type",
            &PdfObject::PdfName(PdfName::new("XRef".into())),
            &budget,
            limits,
        )?;
        add(
            &mut builder,
            "Size",
            &number(u64::from(self.size))?,
            &budget,
            limits,
        )?;
        add(
            &mut builder,
            "Length",
            &number(length as u64)?,
            &budget,
            limits,
        )?;
        add(
            &mut builder,
            "W",
            &PdfObject::PdfArray(PdfArray::from_items(vec![
                number(1)?,
                number(8)?,
                number(4)?,
            ])),
            &budget,
            limits,
        )?;
        add(
            &mut builder,
            "Index",
            &PdfObject::PdfArray(PdfArray::from_items(vec![
                number(0)?,
                number(u64::from(self.size))?,
            ])),
            &budget,
            limits,
        )?;
        let (_dict_lease, dict) = {
            let (dict, lease) = builder.finish()?;
            (lease, dict)
        };
        validate_serialization_nodes_with_budget(
            dict.into_iter().map(|(_, v)| (v, 2)),
            1,
            limits,
            Some(&budget),
        )?;
        append_formatted_output(&mut output, format_args!("{} 0 obj\n", id.obj_num()), limit)?;
        write_dict_limited(&mut output, &dict, limit)?;
        append_output(&mut output, b"\nstream\n", limit)?;
        for entry in entries {
            append_output(&mut output, &entry.encode()?, limit)?;
        }
        append_output(&mut output, b"\nendstream\nendobj\n", limit)
    }
}

fn add(
    builder: &mut BudgetedDictBuilder,
    key: &str,
    value: &PdfObject,
    budget: &OperationBudget,
    limits: Limits,
) -> PdfResult<()> {
    if key.len() > limits.max_token_bytes {
        return Err(PdfError::ResourceLimit("xref trailer key bytes".into()));
    }
    let (key, key_lease) = crate::budget::copy_string(key, budget)?;
    let (value, lease) = crate::objects::clone_object_with_budget(value, budget, limits, 1)?;
    builder.insert(key, value, key_lease, lease)
}
