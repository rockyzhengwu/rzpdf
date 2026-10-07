//! Internal single-table allocator: two allocation slots cover old/new table
//! overlap without an untracked heap map for allocation bookkeeping.
//! The allocator's own inline control storage and Rc budget state are excluded.
use super::{MemoryLease, OperationBudget};
use allocator_api2::alloc::{AllocError, Allocator, Global, Layout};
use std::{cell::{Cell, RefCell}, ptr::NonNull};

#[derive(Debug)]
pub(crate) struct TableAllocator {
    budget: RefCell<Option<OperationBudget>>,
    depth: Cell<usize>,
    allocations: RefCell<[Option<(NonNull<u8>, Option<MemoryLease>)>; 2]>,
}

impl TableAllocator {
    pub(crate) fn new(budget: &OperationBudget) -> Self {
        Self { budget: RefCell::new(Some(budget.clone())), depth: Cell::new(1), allocations: RefCell::new([None, None]) }
    }

    pub(crate) fn persistent() -> Self {
        Self { budget: RefCell::new(None), depth: Cell::new(0), allocations: RefCell::new([None, None]) }
    }

    /// Existing persistent allocations are not retroactively charged. New
    /// allocations remain charged through all same-pool nested scopes.
    pub(crate) fn enter(&self, budget: &OperationBudget) -> crate::error::PdfResult<()> {
        let next = self.depth.get().checked_add(1)
            .ok_or_else(|| crate::error::PdfError::ResourceLimit("table budget depth".into()))?;
        let mut current = self.budget.borrow_mut();
        if current.as_ref().is_some_and(|current| !current.shares_pool(budget)) {
            return Err(crate::error::PdfError::ResourceLimit("table budget pool mismatch".into()));
        }
        if current.is_none() { *current = Some(budget.clone()); }
        self.depth.set(next);
        Ok(())
    }

    /// Call after transaction snapshots have been restored/discarded. Outermost
    /// exit transfers retained table storage to persistent document memory.
    pub(crate) fn leave(&self) {
        let depth = self.depth.get().checked_sub(1).expect("active table budget scope");
        self.depth.set(depth);
        if depth == 0 {
            for (_, lease) in self.allocations.borrow_mut().iter_mut().flatten() {
                drop(lease.take());
            }
            self.budget.borrow_mut().take();
        }
    }
}

impl Clone for TableAllocator {
    fn clone(&self) -> Self {
        // Plain document snapshots own independent persistent allocations.
        // Budgeted transactions must use the fallible array snapshot instead.
        Self::persistent()
    }
}

// SAFETY: Each successful allocation is delegated to Global using the exact
// caller layout. Nonzero allocations retain a distinct lease until Global
// deallocation, and no allocation is returned on failed reservation. Zero-sized
// requests use Global's normal dangling allocation without consuming a slot.
unsafe impl Allocator for TableAllocator {
    fn allocate(&self, layout: Layout) -> Result<NonNull<[u8]>, AllocError> {
        if layout.size() == 0 { return Global.allocate(layout); }
        let mut allocations = self.allocations.borrow_mut();
        let slot = allocations.iter_mut().find(|slot| slot.is_none()).ok_or(AllocError)?;
        let lease = self.budget.borrow().as_ref()
            .map(|budget| budget.reserve(layout.size()).map_err(|_| AllocError))
            .transpose()?;
        let allocation = Global.allocate(layout)?;
        *slot = Some((allocation.cast(), lease));
        Ok(allocation)
    }

    unsafe fn deallocate(&self, ptr: NonNull<u8>, layout: Layout) {
        // SAFETY: Allocator callers must supply a live allocation from this
        // allocator with its original layout; Global owns its backing storage.
        unsafe { Global.deallocate(ptr, layout); }
        if layout.size() != 0 {
            let mut allocations = self.allocations.borrow_mut();
            let slot = allocations.iter_mut()
                .find(|slot| slot.as_ref().is_some_and(|(address, _)| *address == ptr))
                .expect("table allocation must retain its lease");
            // Return quota only after the allocation has actually been freed.
            drop(slot.take());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::hash::BuildHasherDefault;
    type Table = hashbrown::HashMap<u32, u32, BuildHasherDefault<std::collections::hash_map::DefaultHasher>, TableAllocator>;

    #[test]
    fn table_allocator_charges_real_layout_and_growth_overlap() {
        let probe = OperationBudget::new(1024 * 1024);
        let mut table = Table::with_hasher_in(Default::default(), TableAllocator::new(&probe));
        table.try_reserve(3).unwrap();
        let first = probe.live_bytes();
        assert!(first > table.capacity() * std::mem::size_of::<(u32, u32)>());
        for key in 0..3 { table.insert(key, key); }
        table.try_reserve(100).unwrap();
        let second = probe.live_bytes();
        assert_eq!(probe.peak_bytes(), first + second);
        drop(table);
        assert_eq!(probe.live_bytes(), 0);

        let pool = OperationBudget::new(first + second - 1);
        let mut table = Table::with_hasher_in(Default::default(), TableAllocator::new(&pool));
        table.try_reserve(3).unwrap();
        for key in 0..3 { table.insert(key, key); }
        let capacity = table.capacity();
        assert!(table.try_reserve(100).is_err());
        assert_eq!(table.capacity(), capacity);
        assert_eq!(table.len(), 3);
        assert_eq!(table.get(&2), Some(&2));
        assert_eq!(pool.live_bytes(), first);
        drop(table);
        assert_eq!(pool.live_bytes(), 0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut table = Table::with_hasher_in(Default::default(), TableAllocator::new(&pool));
            table.try_reserve(3).unwrap();
            panic!("table owner unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn table_allocator_nested_scopes_reject_other_pools_and_transfer_at_outer_exit() {
        let pool = OperationBudget::new(4096);
        let mut table = Table::with_hasher_in(Default::default(), TableAllocator::persistent());
        table.try_reserve(3).unwrap();
        table.insert(1, 10);
        assert_eq!(pool.live_bytes(), 0);
        table.allocator().enter(&pool).unwrap();
        table.allocator().enter(&pool.clone()).unwrap();
        let other = OperationBudget::new(4096);
        assert!(table.allocator().enter(&other).is_err());
        assert_eq!(other.live_bytes(), 0);
        table.try_reserve(20).unwrap();
        let retained = pool.live_bytes();
        assert!(retained > 0);
        table.allocator().leave();
        assert_eq!(pool.live_bytes(), retained);
        table.allocator().leave();
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(table.get(&1), Some(&10));
        table.allocator().enter(&other).unwrap();
        table.try_reserve(100).unwrap();
        assert!(other.live_bytes() > 0);
        drop(table);
        assert_eq!(other.live_bytes(), 0);
    }
}
