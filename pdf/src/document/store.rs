use crate::budget::table_allocator::TableAllocator;
use std::collections::hash_map::RandomState;

type ObjectTable = hashbrown::HashMap<ObjectId, StoreEntry, RandomState, TableAllocator>;

/// Read-only object-store view. The backing allocation strategy is private.
/// Unlike the former standard HashMap reference, this does not permit naming
/// a concrete hash-table type; keys/get/iter retain their borrowing semantics.
#[derive(Clone, Copy)]
pub struct StoreEntries<'a> {
    table: &'a ObjectTable,
}

impl<'a> StoreEntries<'a> {
    pub fn len(&self) -> usize {
        self.table.len()
    }
    pub fn is_empty(&self) -> bool {
        self.table.is_empty()
    }
    pub fn capacity(&self) -> usize {
        self.table.capacity()
    }
    pub fn get(&self, id: &ObjectId) -> Option<&'a StoreEntry> {
        self.table.get(id)
    }
    pub fn contains_key(&self, id: &ObjectId) -> bool {
        self.table.contains_key(id)
    }
    pub fn keys(&self) -> impl Iterator<Item = &'a ObjectId> + 'a {
        self.table.keys()
    }
    pub fn values(&self) -> impl Iterator<Item = &'a StoreEntry> + 'a {
        self.table.values()
    }
    pub fn iter(&self) -> impl Iterator<Item = (&'a ObjectId, &'a StoreEntry)> + 'a {
        self.table.iter()
    }
}

use crate::error::{PdfError, PdfResult};
use crate::objects::{PdfObject, object_id::ObjectId};

#[derive(Debug, Clone)]
pub enum StoreEntry {
    Upsert(PdfObject),
    Deleted,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn budgeted_snapshot_counts_empty_payload_slots_and_reuses_restore_buckets() {
        use crate::{budget::OperationBudget, parser::options::Limits};
        let mut store = DocumentStore::new(10);
        let first = ObjectId::new(1, 0);
        let second = ObjectId::new(2, 0);
        store.delete_object(first).unwrap();
        store.delete_object(second).unwrap();
        let bytes = 2 * std::mem::size_of::<(ObjectId, StoreEntry)>();
        let small = OperationBudget::new(bytes - 1);
        assert!(matches!(
            store.snapshot_with_budget(&small, Limits::default()),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(small.live_bytes(), 0);
        assert_eq!(store.entries.len(), 2);
        let pool = OperationBudget::new(bytes);
        let (snapshot, lease) = store
            .snapshot_with_budget(&pool, Limits::default())
            .unwrap();
        assert_eq!(pool.live_bytes(), bytes);
        assert_eq!(
            lease.bytes(),
            snapshot.entries.capacity() * std::mem::size_of::<(ObjectId, StoreEntry)>()
        );
        store.update_object(first, PdfObject::PdfNull).unwrap();
        for _ in 0..32 {
            store.try_insert_object(PdfObject::PdfNull).unwrap();
        }
        let capacity = store.entries.capacity();
        store.restore_snapshot(snapshot);
        assert_eq!(store.entries.capacity(), capacity);
        assert_eq!(store.entries.len(), 2);
        assert!(matches!(store.entry(first), Some(StoreEntry::Deleted)));
        assert!(matches!(store.entry(second), Some(StoreEntry::Deleted)));
        assert_eq!(store.next_obj_num, 10);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let owned = store
                .snapshot_with_budget(&pool, Limits::default())
                .unwrap();
            let _owned = owned;
            panic!("snapshot array unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        assert!(
            store
                .snapshot_with_budget(
                    &pool,
                    Limits {
                        max_objects: 1,
                        ..Default::default()
                    }
                )
                .is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
    }

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
    entries: ObjectTable,
    next_obj_num: u32,
}

/// Contiguous rollback data. The transaction retains its separate lease until
/// this table is discarded or moved back into existing store buckets.
pub(crate) struct StoreSnapshot {
    entries: Vec<(ObjectId, StoreEntry)>,
    next_obj_num: u32,
}

impl StoreSnapshot {
    pub(crate) fn into_entries(self) -> Vec<(ObjectId, StoreEntry)> {
        self.entries
    }
}

impl DocumentStore {
    pub(crate) fn enter_table_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        self.entries.allocator().enter(budget)
    }

    pub(crate) fn leave_table_budget(&self) {
        self.entries.allocator().leave();
    }

    /// Snapshot table slots and recursively owned payloads reserve before
    /// allocation; no additional HashMap buckets are created.
    pub(crate) fn snapshot_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<(StoreSnapshot, crate::budget::MemoryLease)> {
        if self.entries.len() > limits.max_objects {
            return Err(PdfError::ResourceLimit("snapshot object count".into()));
        }
        let mut lease = budget.reserve(
            self.entries
                .len()
                .checked_mul(std::mem::size_of::<(ObjectId, StoreEntry)>())
                .ok_or_else(|| PdfError::ResourceLimit("snapshot store table size".into()))?,
        )?;
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(self.entries.len())
            .map_err(|_| PdfError::ResourceLimit("snapshot store allocation".into()))?;
        lease.resize(
            entries
                .capacity()
                .checked_mul(std::mem::size_of::<(ObjectId, StoreEntry)>())
                .ok_or_else(|| PdfError::ResourceLimit("snapshot store table capacity".into()))?,
        )?;
        for (&id, entry) in &self.entries {
            let copy = match entry {
                StoreEntry::Deleted => StoreEntry::Deleted,
                StoreEntry::Upsert(object) => {
                    let (mut child, object) = {
                        let (object, child) =
                            crate::objects::clone_object_with_budget(object, budget, limits, 0)?;
                        (child, object)
                    };
                    lease.absorb(&mut child)?;
                    StoreEntry::Upsert(object)
                }
            };
            entries.push((id, copy));
        }
        Ok((
            StoreSnapshot {
                entries,
                next_obj_num: self.next_obj_num,
            },
            lease,
        ))
    }

    pub(crate) fn restore_snapshot(&mut self, snapshot: StoreSnapshot) {
        // Store writes never remove keys or shrink buckets (deletion is a
        // tombstone). Nested rollback preserves at least its captured length,
        // so this restore needs no allocation, including mixed transactions.
        debug_assert!(self.entries.capacity() >= snapshot.entries.len());
        self.entries.clear();
        for (id, entry) in snapshot.entries {
            self.entries.insert(id, entry);
        }
        self.next_obj_num = snapshot.next_obj_num;
    }

    pub fn new(next_obj_num: u32) -> Self {
        Self {
            entries: ObjectTable::with_hasher_in(RandomState::new(), TableAllocator::persistent()),
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

    pub fn entries(&self) -> StoreEntries<'_> {
        StoreEntries {
            table: &self.entries,
        }
    }
}
