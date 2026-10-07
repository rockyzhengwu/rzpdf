use std::collections::HashMap;

use crate::error::{PdfError, PdfResult};
use crate::objects::{object_id::ObjectId, PdfObject};

#[derive(Debug, Clone)]
pub enum StoreEntry {
    Upsert(PdfObject),
    Deleted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_delete_and_restore_reuse_one_entry_without_advancing_ids() {
        let mut store = DocumentStore::new(10);
        let id = ObjectId::new(3, 2);
        store.update_object(id, PdfObject::PdfNull).unwrap();
        let capacity = store.entries.capacity();
        assert!(matches!(
            store.entry(id),
            Some(StoreEntry::Upsert(PdfObject::PdfNull))
        ));
        store.delete_object(id).unwrap();
        store.delete_object(id).unwrap();
        assert!(matches!(store.entry(id), Some(StoreEntry::Deleted)));
        store.update_object(id, PdfObject::PdfNull).unwrap();
        assert!(matches!(
            store.entry(id),
            Some(StoreEntry::Upsert(PdfObject::PdfNull))
        ));
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.entries.capacity(), capacity);
        assert_eq!(store.next_obj_num, 10);
        let other = ObjectId::new(4, 0);
        store.delete_object(other).unwrap();
        assert!(matches!(store.entry(other), Some(StoreEntry::Deleted)));
        assert_eq!(store.entries.len(), 2);
    }

    #[test]
    fn allocation_rejects_reserved_collision_and_exhaustion_without_changes() {
        for start in [0, u32::MAX] {
            let mut store = DocumentStore::new(start);
            for _ in 0..2 {
                assert!(store.try_insert_object(PdfObject::PdfNull).is_err());
                assert_eq!(store.next_obj_num, start);
                assert!(store.entries.is_empty());
            }
        }
        let mut store = DocumentStore::new(7);
        let conflicting = ObjectId::new(7, 1);
        store
            .update_object(conflicting, PdfObject::PdfNull)
            .unwrap();
        assert!(matches!(
            store.try_insert_object(PdfObject::PdfNull),
            Err(PdfError::InvalidReference(_))
        ));
        assert_eq!(store.next_obj_num, 7);
        assert_eq!(store.entries.len(), 1);
        assert!(store.entry(conflicting).is_some());
    }

    #[test]
    fn allocation_preserves_last_valid_number_and_does_not_wrap() {
        let mut store = DocumentStore::new(u32::MAX - 1);
        let id = store.try_insert_object(PdfObject::PdfNull).unwrap();
        assert_eq!(id, ObjectId::new(u32::MAX - 1, 0));
        assert!(store.try_insert_object(PdfObject::PdfNull).is_err());
        assert_eq!(store.entries.len(), 1);
        assert_eq!(store.next_obj_num, u32::MAX);
    }
}

#[derive(Debug, Clone)]
pub struct DocumentStore {
    entries: HashMap<ObjectId, StoreEntry>,
    next_obj_num: u32,
}

impl DocumentStore {
    /// Track recursively owned snapshot payloads, not HashMap bucket allocation.
    pub(crate) fn clone_payloads_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        if self.entries.len() > limits.max_objects {
            return Err(PdfError::ResourceLimit("snapshot object count".into()));
        }
        let mut lease = budget.reserve(0)?;
        let mut entries = HashMap::new();
        entries
            .try_reserve(self.entries.len())
            .map_err(|_| PdfError::ResourceLimit("snapshot store allocation".into()))?;
        for (&id, entry) in &self.entries {
            let copy = match entry {
                StoreEntry::Deleted => StoreEntry::Deleted,
                StoreEntry::Upsert(object) => {
                    let (object, mut child) =
                        crate::objects::clone_object_with_budget(object, budget, limits, 0)?;
                    lease.absorb(&mut child)?;
                    StoreEntry::Upsert(object)
                }
            };
            entries.insert(id, copy);
        }
        Ok((
            Self {
                entries,
                next_obj_num: self.next_obj_num,
            },
            lease,
        ))
    }

    pub fn new(next_obj_num: u32) -> Self {
        Self {
            entries: HashMap::new(),
            next_obj_num,
        }
    }

    /// Compatibility entry point; use `try_insert_object` to handle allocation errors.
    pub fn insert_object(&mut self, object: PdfObject) -> ObjectId {
        self.try_insert_object(object)
            .expect("object allocation failed; use try_insert_object")
    }

    pub fn try_insert_object(&mut self, object: PdfObject) -> PdfResult<ObjectId> {
        let next = self
            .next_obj_num
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("object number exhausted".into()))?;
        if self.next_obj_num == 0
            || self
                .entries
                .keys()
                .any(|id| id.obj_num() == self.next_obj_num)
        {
            return Err(PdfError::InvalidReference(
                "object number is reserved or already allocated".into(),
            ));
        }
        self.entries
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("object store allocation failed".into()))?;
        let object_id = ObjectId::new(self.next_obj_num, 0);
        self.entries.insert(object_id, StoreEntry::Upsert(object));
        self.next_obj_num = next;
        Ok(object_id)
    }

    fn reserve_entry(&mut self, object_id: ObjectId) -> PdfResult<()> {
        if !self.entries.contains_key(&object_id) {
            self.entries
                .try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("object store allocation failed".into()))?;
        }
        Ok(())
    }

    pub fn update_object(&mut self, object_id: ObjectId, object: PdfObject) -> PdfResult<()> {
        self.reserve_entry(object_id)?;
        self.entries.insert(object_id, StoreEntry::Upsert(object));
        Ok(())
    }

    pub fn delete_object(&mut self, object_id: ObjectId) -> PdfResult<()> {
        self.reserve_entry(object_id)?;
        self.entries.insert(object_id, StoreEntry::Deleted);
        Ok(())
    }

    pub fn entry(&self, object_id: ObjectId) -> Option<&StoreEntry> {
        self.entries.get(&object_id)
    }

    pub fn entries(&self) -> &HashMap<ObjectId, StoreEntry> {
        &self.entries
    }
}
