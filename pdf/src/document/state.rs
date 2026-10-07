use crate::{
    document::{identity::DocumentIdentity, store::DocumentStore},
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};
use std::cell::{Cell, RefCell};

/// Mutable document state shared by source-backed and newly created documents.
/// Source parsing and immutable source-object caches stay in PDFContext.
#[derive(Debug)]
pub(crate) struct DocumentState {
    pub(crate) source_version: String,
    pub(crate) identity: DocumentIdentity,
    pub(crate) store: RefCell<DocumentStore>,
    pub(crate) trailer: RefCell<PdfDict>,
    pub(crate) revision: Cell<u64>,
    pub(crate) commit_revision: Cell<u64>,
    dirty: Cell<bool>,
    pub(crate) transaction_depth: Cell<u64>,
}

impl DocumentState {
    pub(crate) fn new(trailer: PdfDict, next_object_number: u32, source_version: String) -> Self {
        Self {
            source_version,
            identity: DocumentIdentity::new(),
            store: RefCell::new(DocumentStore::new(next_object_number)),
            trailer: RefCell::new(trailer),
            revision: Cell::new(0),
            commit_revision: Cell::new(0),
            dirty: Cell::new(false),
            transaction_depth: Cell::new(0),
        }
    }
    pub(crate) fn transaction<T>(&self, edit: impl FnOnce() -> PdfResult<T>) -> PdfResult<T> {
        self.transaction_inner(None, edit)
    }

    pub(crate) fn transaction_with_budget<T>(
        &self,
        budget: &crate::budget::OperationBudget,
        limits: crate::parser::options::Limits,
        edit: impl FnOnce() -> PdfResult<T>,
    ) -> PdfResult<T> {
        self.transaction_inner(Some((budget, limits)), edit)
    }

    fn transaction_inner<T>(
        &self,
        budget: Option<(
            &crate::budget::OperationBudget,
            crate::parser::options::Limits,
        )>,
        edit: impl FnOnce() -> PdfResult<T>,
    ) -> PdfResult<T> {
        self.ensure_commit_capacity()?;
        let depth = self
            .transaction_depth
            .get()
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("transaction depth overflow".into()))?;
        self.revision
            .get()
            .checked_add(depth)
            .ok_or_else(|| PdfError::ResourceLimit("document revision exhausted".into()))?;
        // Declared before the guard: quota outlives snapshot disposal/restoration.
        let mut snapshot_lease = None;
        let (trailer_snapshot, snapshot) = if let Some((budget, limits)) = budget {
            let (trailer, mut lease) =
                crate::objects::clone_dict_with_budget(&self.trailer.borrow(), budget, limits, 0)?;
            let (store, mut store_lease) = self
                .store
                .borrow()
                .clone_payloads_with_budget(budget, limits)?;
            lease.absorb(&mut store_lease)?;
            snapshot_lease = Some(lease);
            (trailer, store)
        } else {
            (self.trailer.borrow().clone(), self.store.borrow().clone())
        };
        let _snapshot_lease = snapshot_lease;
        let mut rollback = StoreRollback {
            commit_revision: &self.commit_revision,
            dirty: &self.dirty,
            dirty_snapshot: self.dirty.get(),
            revision: &self.revision,
            transaction_depth: &self.transaction_depth,
            trailer: &self.trailer,
            trailer_snapshot: Some(trailer_snapshot),
            store: &self.store,
            snapshot: Some(snapshot),
        };
        self.transaction_depth.set(depth);
        let result = edit()?;
        rollback.snapshot = None;
        rollback.trailer_snapshot = None;
        Ok(result)
    }

    pub(crate) fn next_revision(&self) -> PdfResult<u64> {
        self.ensure_commit_capacity()?;
        let next = self
            .revision
            .get()
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("document revision exhausted".into()))?;
        next.checked_add(self.transaction_depth.get())
            .ok_or_else(|| PdfError::ResourceLimit("document revision exhausted".into()))?;
        Ok(next)
    }

    fn ensure_commit_capacity(&self) -> PdfResult<()> {
        self.commit_revision
            .get()
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("document commit revision exhausted".into()))?;
        Ok(())
    }

    /// Called only after a successful write checked by next_revision.
    pub(crate) fn did_mutate(&self, revision: u64) {
        self.revision.set(revision);
        if self.transaction_depth.get() == 0 {
            self.commit_revision.set(self.commit_revision.get() + 1);
        } else {
            self.dirty.set(true);
        }
    }
}

struct StoreRollback<'a> {
    commit_revision: &'a Cell<u64>,
    dirty: &'a Cell<bool>,
    dirty_snapshot: bool,
    revision: &'a Cell<u64>,
    transaction_depth: &'a Cell<u64>,
    trailer: &'a RefCell<PdfDict>,
    trailer_snapshot: Option<PdfDict>,
    store: &'a RefCell<DocumentStore>,
    snapshot: Option<DocumentStore>,
}

impl Drop for StoreRollback<'_> {
    fn drop(&mut self) {
        let restoring = self.snapshot.is_some();
        if let Some(snapshot) = self.trailer_snapshot.take() {
            *self.trailer.borrow_mut() = snapshot;
        }
        if let Some(snapshot) = self.snapshot.take() {
            *self.store.borrow_mut() = snapshot;
        }
        if restoring {
            // Each active savepoint reserves one rollback revision in advance.
            self.revision.set(self.revision.get() + 1);
            self.dirty.set(self.dirty_snapshot);
        } else if self.transaction_depth.get() == 1 && self.dirty.get() {
            // Outermost transaction reserved this commit before any writes.
            self.commit_revision.set(self.commit_revision.get() + 1);
            self.dirty.set(false);
        }
        self.transaction_depth.set(self.transaction_depth.get() - 1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::{object_id::ObjectId, PdfObject};

    #[test]
    fn nested_savepoints_restore_each_payload_and_retain_outer_quota() {
        use crate::{
            budget::OperationBudget, objects::pdf_number::PdfNumber, parser::options::Limits,
        };
        let state = DocumentState::new(PdfDict::default(), 7, "1.7".into());
        let id = ObjectId::new(3, 0);
        state
            .store
            .borrow_mut()
            .update_object(id, PdfObject::PdfNumber(PdfNumber::new(123.0)))
            .unwrap();
        let pool = OperationBudget::new(6);
        let limits = Limits::default();
        let result: PdfResult<()> = state.transaction_with_budget(&pool, limits, || {
            assert_eq!(pool.live_bytes(), 3);
            let next = state.next_revision()?;
            state
                .store
                .borrow_mut()
                .update_object(id, PdfObject::PdfNumber(PdfNumber::new(456.0)))?;
            state.did_mutate(next);
            let inner: PdfResult<()> = state.transaction_with_budget(&pool, limits, || {
                assert_eq!(pool.live_bytes(), 6);
                let next = state.next_revision()?;
                state.store.borrow_mut().delete_object(id)?;
                state.did_mutate(next);
                Err(PdfError::DocumentError("inner rollback".into()))
            });
            assert!(inner.is_err());
            assert_eq!(pool.live_bytes(), 3);
            assert_eq!(state.transaction_depth.get(), 1);
            match state.store.borrow().entry(id).unwrap() {
                crate::document::store::StoreEntry::Upsert(PdfObject::PdfNumber(number)) => {
                    assert_eq!(number.lexeme(), "456")
                }
                _ => panic!("inner snapshot not restored"),
            }
            Err(PdfError::DocumentError("outer rollback".into()))
        });
        assert!(result.is_err());
        assert_eq!(pool.peak_bytes(), 6);
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(state.transaction_depth.get(), 0);
        assert_eq!(state.commit_revision.get(), 0);
        match state.store.borrow().entry(id).unwrap() {
            crate::document::store::StoreEntry::Upsert(PdfObject::PdfNumber(number)) => {
                assert_eq!(number.lexeme(), "123")
            }
            _ => panic!("outer snapshot not restored"),
        }
    }

    #[test]
    fn budgeted_snapshot_rejects_before_edit_and_releases_on_nested_rollback() {
        use crate::{
            budget::OperationBudget, objects::pdf_number::PdfNumber, parser::options::Limits,
        };
        let state = DocumentState::new(PdfDict::default(), 7, "1.7".into());
        let id = ObjectId::new(3, 0);
        state
            .store
            .borrow_mut()
            .update_object(id, PdfObject::PdfNumber(PdfNumber::new(123.0)))
            .unwrap();
        state
            .store
            .borrow_mut()
            .delete_object(ObjectId::new(4, 0))
            .unwrap();
        let limits = Limits::default();
        let pool = OperationBudget::new(3);
        let neighbor = pool.reserve(1).unwrap();
        let mut called = false;
        assert!(state
            .transaction_with_budget(&pool, limits, || {
                called = true;
                Ok(())
            })
            .is_err());
        assert!(!called);
        assert_eq!(state.revision.get(), 0);
        assert_eq!(state.transaction_depth.get(), 0);
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        state
            .transaction_with_budget(&pool, limits, || {
                assert_eq!(pool.live_bytes(), 3);
                assert!(state
                    .transaction_with_budget(&pool, limits, || Ok(()))
                    .is_err());
                assert_eq!(state.transaction_depth.get(), 1);
                Ok(())
            })
            .unwrap();
        assert_eq!(pool.live_bytes(), 0);
        let result: PdfResult<()> = state.transaction_with_budget(&pool, limits, || {
            state.store.borrow_mut().delete_object(id)?;
            Err(PdfError::DocumentError("rollback".into()))
        });
        assert!(result.is_err());
        assert!(matches!(
            state.store.borrow().entry(id),
            Some(crate::document::store::StoreEntry::Upsert(_))
        ));
        assert_eq!(pool.live_bytes(), 0);
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: PdfResult<()> = state.transaction_with_budget(&pool, limits, || {
                state.store.borrow_mut().delete_object(id)?;
                panic!("snapshot unwind test");
            });
        }));
        assert!(panic.is_err());
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(state.transaction_depth.get(), 0);
        assert!(matches!(
            state.store.borrow().entry(id),
            Some(crate::document::store::StoreEntry::Upsert(_))
        ));
    }

    #[test]
    fn exhausted_commit_counter_rejects_before_editing() {
        let state = DocumentState::new(PdfDict::default(), 1, "1.7".into());
        state.commit_revision.set(u64::MAX);
        assert!(matches!(
            state.next_revision(),
            Err(PdfError::ResourceLimit(_))
        ));
        let mut called = false;
        let result = state.transaction(|| {
            called = true;
            Ok(())
        });
        assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        assert!(!called);
        assert_eq!(state.revision.get(), 0);
        assert_eq!(state.transaction_depth.get(), 0);
    }

    #[test]
    fn standalone_transaction_restores_store_and_trailer_without_a_reader() {
        let state = DocumentState::new(PdfDict::default(), 7, "1.4".into());
        let identity = state.identity.clone();
        let result: PdfResult<()> = state.transaction(|| {
            let next = state.next_revision()?;
            let id = state
                .store
                .borrow_mut()
                .try_insert_object(PdfObject::PdfNull)?;
            assert_eq!(id, ObjectId::new(7, 0));
            state
                .trailer
                .borrow_mut()
                .try_insert("Custom".into(), PdfObject::PdfNull)?;
            state.did_mutate(next);
            Err(PdfError::DocumentError("restore state".into()))
        });
        assert!(result.is_err());
        assert!(state.store.borrow().entries().is_empty());
        assert!(state.trailer.borrow().get("Custom").is_none());
        assert_eq!(state.source_version, "1.4");
        assert_eq!(state.identity, identity);
        assert_eq!(state.revision.get(), 2);
        assert_eq!(state.transaction_depth.get(), 0);
        assert_eq!(
            state
                .store
                .borrow_mut()
                .try_insert_object(PdfObject::PdfNull)
                .unwrap(),
            ObjectId::new(7, 0)
        );
    }
}
