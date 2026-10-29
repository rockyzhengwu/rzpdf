//! Shared accounting for explicitly tracked live bytes in one operation.
//! This is not an allocator: callers must reserve before allocation and keep
//! the lease with its owner. Untracked Rust/native allocations are excluded.
use crate::error::{PdfError, PdfResult};
use std::{cell::Cell, rc::Rc};

pub(crate) mod table_allocator;

#[derive(Debug)]
struct State {
    maximum: usize,
    live: Cell<usize>,
    peak: Cell<usize>,
}

/// Clones share a single live-byte pool, not independent copies of its limit.
/// Like the document state, this handle is single-threaded.
#[derive(Debug, Clone)]
pub struct OperationBudget {
    state: Rc<State>,
}

impl OperationBudget {
    pub(crate) fn shares_pool(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.state, &other.state)
    }

    pub fn new(maximum: usize) -> Self {
        Self {
            state: Rc::new(State {
                maximum,
                live: Cell::new(0),
                peak: Cell::new(0),
            }),
        }
    }

    pub fn live_bytes(&self) -> usize {
        self.state.live.get()
    }
    pub fn maximum_bytes(&self) -> usize {
        self.state.maximum
    }

    /// Highest successfully reserved live-byte total in this shared pool.
    pub fn peak_bytes(&self) -> usize {
        self.state.peak.get()
    }

    /// Reserve before allocating. On failure, the pool remains unchanged.
    pub fn reserve(&self, bytes: usize) -> PdfResult<MemoryLease> {
        let mut lease = MemoryLease {
            state: self.state.clone(),
            bytes: 0,
        };
        lease.resize(bytes)?;
        Ok(lease)
    }
}

/// Non-Clone ownership token. Move it with the allocation; a separate cloned
/// allocation needs a separate lease. Dropping releases only this reservation.
#[derive(Debug)]
pub struct MemoryLease {
    state: Rc<State>,
    bytes: usize,
}

impl MemoryLease {
    pub(crate) fn belongs_to(&self, budget: &OperationBudget) -> bool {
        Rc::ptr_eq(&self.state, &budget.state)
    }

    pub fn bytes(&self) -> usize {
        self.bytes
    }

    /// Transfer another reservation in the same pool without changing live
    /// bytes or briefly releasing quota. Cross-pool failure leaves both intact.
    pub(crate) fn absorb(&mut self, other: &mut Self) -> PdfResult<()> {
        if !Rc::ptr_eq(&self.state, &other.state) {
            return Err(PdfError::ResourceLimit(
                "cannot merge leases from different operations".into(),
            ));
        }
        let bytes = self
            .bytes
            .checked_add(other.bytes)
            .ok_or_else(|| PdfError::ResourceLimit("merged lease size".into()))?;
        self.bytes = bytes;
        other.bytes = 0;
        Ok(())
    }

    /// Change a reservation atomically. For reallocations whose old and new
    /// buffers coexist, reserve the new buffer separately, then drop the old
    /// lease; resizing alone does not account for that temporary overlap.
    pub fn resize(&mut self, bytes: usize) -> PdfResult<()> {
        let live = self.state.live.get();
        let total = if bytes >= self.bytes {
            live.checked_add(bytes - self.bytes)
                .filter(|&total| total <= self.state.maximum)
                .ok_or_else(|| PdfError::ResourceLimit("operation live bytes".into()))?
        } else {
            live - (self.bytes - bytes)
        };
        self.state.live.set(total);
        self.state.peak.set(self.state.peak.get().max(total));
        self.bytes = bytes;
        Ok(())
    }
}

impl Drop for MemoryLease {
    fn drop(&mut self) {
        self.state.live.set(self.state.live.get() - self.bytes);
    }
}

/// A byte buffer whose capacity remains charged while it is owned.
/// No conversion to an untracked Vec is provided. Cloning requires a separate
/// reservation in the same pool, including overlap with the original buffer.
#[derive(Debug)]
pub struct BudgetedBytes {
    data: Vec<u8>,
    lease: MemoryLease,
}

impl BudgetedBytes {
    pub(crate) fn operation_budget(&self) -> OperationBudget {
        OperationBudget {
            state: self.lease.state.clone(),
        }
    }

    /// Reserve a new buffer while the old allocation remains charged.
    pub(crate) fn try_reserve_capacity(&mut self, capacity: usize) -> PdfResult<()> {
        if capacity <= self.data.capacity() {
            return Ok(());
        }
        let budget = self.operation_budget();
        let mut lease = budget.reserve(capacity)?;
        let mut data = Vec::new();
        data.try_reserve_exact(capacity)
            .map_err(|_| PdfError::ResourceLimit("budgeted byte allocation".into()))?;
        lease.resize(data.capacity())?;
        data.extend_from_slice(&self.data);
        *self = Self { data, lease };
        Ok(())
    }
    /// Internal ownership transfer: the receiving owner must keep the lease
    /// beside the allocation. This is deliberately not a public Vec export.
    pub(crate) fn into_reserved_parts(self) -> (Vec<u8>, MemoryLease) {
        (self.data, self.lease)
    }

    /// Read into a separately reserved, owned buffer. Read errors release the
    /// allocation and lease, but do not roll back the underlying reader.
    pub(crate) fn read_exact_from(
        budget: &OperationBudget,
        reader: &mut impl std::io::Read,
        length: usize,
    ) -> PdfResult<Self> {
        let mut lease = budget.reserve(length)?;
        let mut data = Vec::new();
        data.try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("budgeted input allocation".into()))?;
        lease.resize(data.capacity())?;
        data.resize(length, 0);
        reader.read_exact(&mut data)?;
        Ok(Self { data, lease })
    }

    /// Transfer an already reserved allocation without copying its payload.
    /// The caller must have reserved before allocating; capacity reconciliation
    /// here cannot retroactively protect the allocation's instantaneous peak.
    pub(crate) fn from_reserved_vec(data: Vec<u8>, mut lease: MemoryLease) -> PdfResult<Self> {
        lease.resize(data.capacity())?;
        Ok(Self { data, lease })
    }

    pub(crate) fn zeroed(budget: &OperationBudget, length: usize) -> PdfResult<Self> {
        let mut lease = budget.reserve(length)?;
        let mut data = Vec::new();
        data.try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("budgeted byte allocation".into()))?;
        lease.resize(data.capacity())?;
        data.resize(length, 0);
        Ok(Self { data, lease })
    }
    pub(crate) fn as_mut_slice(&mut self) -> &mut [u8] {
        &mut self.data
    }

    pub fn copy_from_slice(budget: &OperationBudget, source: &[u8]) -> PdfResult<Self> {
        let mut lease = budget.reserve(source.len())?;
        let mut data = Vec::new();
        data.try_reserve_exact(source.len())
            .map_err(|_| PdfError::ResourceLimit("budgeted byte allocation".into()))?;
        // Vec promises at least the requested capacity. Reconcile allocator
        // rounding before copying/retaining the buffer; failure drops both.
        lease.resize(data.capacity())?;
        data.extend_from_slice(source);
        Ok(Self { data, lease })
    }

    pub fn as_slice(&self) -> &[u8] {
        &self.data
    }
    pub fn capacity(&self) -> usize {
        self.data.capacity()
    }

    pub fn try_clone(&self) -> PdfResult<Self> {
        let budget = OperationBudget {
            state: self.lease.state.clone(),
        };
        Self::copy_from_slice(&budget, &self.data)
    }

    /// Append with failure atomicity. Growth allocates a separately charged
    /// buffer while the old capacity remains reserved; only then replaces it.
    pub fn try_extend_from_slice(&mut self, suffix: &[u8]) -> PdfResult<()> {
        let length = self
            .data
            .len()
            .checked_add(suffix.len())
            .ok_or_else(|| PdfError::ResourceLimit("budgeted byte length".into()))?;
        if length <= self.data.capacity() {
            self.data.extend_from_slice(suffix);
            return Ok(());
        }
        let budget = OperationBudget {
            state: self.lease.state.clone(),
        };
        let mut lease = budget.reserve(length)?;
        let mut data = Vec::new();
        data.try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("budgeted byte allocation".into()))?;
        lease.resize(data.capacity())?;
        data.extend_from_slice(&self.data);
        data.extend_from_slice(suffix);
        *self = Self { data, lease };
        Ok(())
    }
}

impl AsRef<[u8]> for BudgetedBytes {
    fn as_ref(&self) -> &[u8] {
        self.as_slice()
    }
}

pub(crate) fn copy_string(
    value: &str,
    budget: &OperationBudget,
) -> PdfResult<(String, MemoryLease)> {
    let mut lease = budget.reserve(value.len())?;
    let mut copy = String::new();
    copy.try_reserve_exact(value.len())
        .map_err(|_| PdfError::ResourceLimit("string clone allocation".into()))?;
    lease.resize(copy.capacity())?;
    copy.push_str(value);
    Ok((copy, lease))
}

#[cfg(test)]
mod tests {
    #[test]
    fn reserved_parts_transfer_preserves_allocation_and_shared_pool() {
        let pool = super::OperationBudget::new(64);
        let shared = pool.clone();
        let foreign = super::OperationBudget::new(64);
        let mut bytes = super::BudgetedBytes::copy_from_slice(&pool, b"data").unwrap();
        bytes.try_reserve_capacity(16).unwrap();
        let address = bytes.as_slice().as_ptr();
        let capacity = bytes.capacity();
        let (lease, data) = {
            let (data, lease) = bytes.into_reserved_parts();
            (lease, data)
        };
        assert_eq!(pool.live_bytes(), capacity);
        assert_eq!(data.as_ptr(), address);
        assert_eq!(lease.bytes(), capacity);
        assert!(lease.belongs_to(&shared));
        assert!(!lease.belongs_to(&foreign));
        let owner = super::BudgetedBytes::from_reserved_vec(data, lease).unwrap();
        assert_eq!(owner.as_slice().as_ptr(), address);
        assert_eq!(owner.as_slice(), b"data");
        assert_eq!(pool.live_bytes(), capacity);
        let peak = pool.peak_bytes();
        drop(owner);
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(pool.peak_bytes(), peak);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let bytes = super::BudgetedBytes::copy_from_slice(&pool, b"data").unwrap();
            let (_lease, _data) = {
                let (data, lease) = bytes.into_reserved_parts();
                (lease, data)
            };
            panic!("transferred parts unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(foreign.live_bytes(), 0);
    }

    #[test]
    fn explicit_capacity_growth_counts_overlap_and_preserves_rejected_buffer() {
        let pool = super::OperationBudget::new(24);
        let mut bytes = super::BudgetedBytes::copy_from_slice(&pool, b"abc").unwrap();
        bytes.try_reserve_capacity(8).unwrap();
        assert_eq!(pool.live_bytes(), 8);
        assert_eq!(bytes.as_slice(), b"abc");
        bytes.try_reserve_capacity(16).unwrap();
        assert_eq!(pool.peak_bytes(), 24);
        assert_eq!(pool.live_bytes(), 16);
        drop(bytes);
        assert_eq!(pool.live_bytes(), 0);
        let small = super::OperationBudget::new(23);
        let mut bytes = super::BudgetedBytes::copy_from_slice(&small, b"abc").unwrap();
        bytes.try_reserve_capacity(8).unwrap();
        assert!(bytes.try_reserve_capacity(16).is_err());
        assert_eq!(bytes.capacity(), 8);
        assert_eq!(bytes.as_slice(), b"abc");
        assert_eq!(small.live_bytes(), 8);
        drop(bytes);
        assert_eq!(small.live_bytes(), 0);
    }
    #[test]
    fn shared_peak_tracks_successful_overlap_without_reset_on_release() {
        let budget = OperationBudget::new(8);
        let shared = budget.clone();
        assert_eq!(budget.peak_bytes(), 0);
        let mut first = budget.reserve(3).unwrap();
        let second = shared.reserve(4).unwrap();
        assert_eq!(shared.peak_bytes(), 7);
        assert!(first.resize(5).is_err());
        assert_eq!(budget.peak_bytes(), 7);
        assert_eq!(budget.live_bytes(), 7);
        drop(second);
        first.resize(8).unwrap();
        assert_eq!(shared.peak_bytes(), 8);
        drop(first);
        assert_eq!(shared.live_bytes(), 0);
        assert_eq!(shared.peak_bytes(), 8);
    }

    use super::*;

    #[test]
    fn lease_absorption_transfers_ownership_without_releasing_quota() {
        let budget = OperationBudget::new(5);
        let mut owner = budget.reserve(2).unwrap();
        let mut temporary = budget.reserve(3).unwrap();
        owner.absorb(&mut temporary).unwrap();
        assert_eq!(owner.bytes(), 5);
        assert_eq!(temporary.bytes(), 0);
        assert_eq!(budget.live_bytes(), 5);
        assert!(budget.reserve(1).is_err());
        drop(temporary);
        assert_eq!(budget.live_bytes(), 5);
        let foreign = OperationBudget::new(3);
        let mut foreign_lease = foreign.reserve(3).unwrap();
        assert!(owner.absorb(&mut foreign_lease).is_err());
        assert_eq!(owner.bytes(), 5);
        assert_eq!(foreign_lease.bytes(), 3);
        assert_eq!(budget.live_bytes(), 5);
        assert_eq!(foreign.live_bytes(), 3);
        drop(owner);
        assert_eq!(budget.live_bytes(), 0);
        drop(foreign_lease);
        assert_eq!(foreign.live_bytes(), 0);
    }

    #[test]
    fn reserved_vector_transfer_keeps_owner_and_capacity_together() {
        let budget = OperationBudget::new(3);
        let lease = budget.reserve(3).unwrap();
        let data = b"abc".to_vec();
        let pointer = data.as_ptr();
        let bytes = BudgetedBytes::from_reserved_vec(data, lease).unwrap();
        assert_eq!(bytes.as_slice().as_ptr(), pointer);
        assert_eq!(budget.live_bytes(), bytes.capacity());
        assert!(budget.reserve(1).is_err());
        drop(bytes);
        assert_eq!(budget.live_bytes(), 0);

        let lease = budget.reserve(0).unwrap();
        assert!(BudgetedBytes::from_reserved_vec(b"abcd".to_vec(), lease).is_err());
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn byte_growth_charges_overlap_and_is_atomic_on_failure() {
        // Final six-byte capacity fits, but old three + new six does not.
        let budget = OperationBudget::new(8);
        let mut bytes = BudgetedBytes::copy_from_slice(&budget, b"abc").unwrap();
        let original_capacity = bytes.capacity();
        assert!(bytes.try_extend_from_slice(b"def").is_err());
        assert_eq!(bytes.as_slice(), b"abc");
        assert_eq!(bytes.capacity(), original_capacity);
        assert_eq!(budget.live_bytes(), original_capacity);
        bytes.try_extend_from_slice(b"").unwrap();
        assert_eq!(budget.live_bytes(), original_capacity);
        drop(bytes);
        assert_eq!(budget.live_bytes(), 0);

        let budget = OperationBudget::new(9);
        let mut bytes = BudgetedBytes::copy_from_slice(&budget, b"abc").unwrap();
        bytes.try_extend_from_slice(b"def").unwrap();
        assert_eq!(bytes.as_slice(), b"abcdef");
        assert_eq!(budget.live_bytes(), bytes.capacity());
        // Growth failure with another live owner must preserve both owners.
        let neighbor = BudgetedBytes::copy_from_slice(&budget, b"x").unwrap();
        let retained = budget.live_bytes();
        assert!(bytes.try_extend_from_slice(b"g").is_err());
        assert_eq!(bytes.as_slice(), b"abcdef");
        assert_eq!(neighbor.as_slice(), b"x");
        assert_eq!(budget.live_bytes(), retained);
        drop(bytes);
        assert_eq!(budget.live_bytes(), neighbor.capacity());
        drop(neighbor);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn owned_bytes_keep_capacity_charged_across_return_and_move() {
        fn create(budget: &OperationBudget) -> PdfResult<BudgetedBytes> {
            BudgetedBytes::copy_from_slice(budget, b"abc")
        }
        let budget = OperationBudget::new(10);
        let bytes = create(&budget).unwrap();
        assert_eq!(budget.live_bytes(), bytes.capacity());
        let moved = bytes;
        assert_eq!(moved.as_slice(), b"abc");
        assert_eq!(budget.live_bytes(), moved.capacity());
        drop(moved);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn byte_clone_accounts_for_overlap_and_failed_copy_releases_reservation() {
        let budget = OperationBudget::new(5);
        let first = BudgetedBytes::copy_from_slice(&budget, b"abc").unwrap();
        let retained = budget.live_bytes();
        assert!(first.try_clone().is_err());
        assert_eq!(budget.live_bytes(), retained);
        assert!(BudgetedBytes::copy_from_slice(&budget, b"xyz").is_err());
        assert_eq!(budget.live_bytes(), retained);
        assert_eq!(first.as_slice(), b"abc");
        drop(first);
        assert_eq!(budget.live_bytes(), 0);
        let enough = OperationBudget::new(10);
        let first = BudgetedBytes::copy_from_slice(&enough, b"abc").unwrap();
        let second = first.try_clone().unwrap();
        assert_eq!(enough.live_bytes(), first.capacity() + second.capacity());
        drop(first);
        assert_eq!(enough.live_bytes(), second.capacity());
        drop(second);
        assert_eq!(enough.live_bytes(), 0);
    }

    #[test]
    fn shared_pool_enforces_combined_limit_and_recovers_after_drop() {
        let budget = OperationBudget::new(10);
        let other = budget.clone();
        let first = budget.reserve(6).unwrap();
        assert!(other.reserve(5).is_err());
        assert_eq!(budget.live_bytes(), 6);
        let second = other.reserve(4).unwrap();
        assert_eq!(budget.live_bytes(), 10);
        drop(first);
        assert_eq!(other.live_bytes(), 4);
        drop(second);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn budgeted_input_failure_preserves_neighbor_and_releases_temporary_bytes() {
        use std::io::{self, Read};
        struct FailingReader {
            calls: usize,
            panic_on_failure: bool,
        }
        impl Read for FailingReader {
            fn read(&mut self, out: &mut [u8]) -> io::Result<usize> {
                self.calls += 1;
                if self.calls == 1 {
                    out[0] = b'x';
                    return Ok(1);
                }
                if self.panic_on_failure {
                    panic!("injected input unwind");
                }
                Err(io::Error::other("injected partial input failure"))
            }
        }
        let pool = OperationBudget::new(8);
        let neighbor = BudgetedBytes::copy_from_slice(&pool, b"keep").unwrap();
        let retained = pool.live_bytes();
        let mut rejected = FailingReader {
            calls: 0,
            panic_on_failure: false,
        };
        assert!(BudgetedBytes::read_exact_from(&pool, &mut rejected, 5).is_err());
        assert_eq!(rejected.calls, 0);
        assert_eq!(pool.live_bytes(), retained);
        for panic_on_failure in [false, true] {
            let mut reader = FailingReader {
                calls: 0,
                panic_on_failure,
            };
            let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                BudgetedBytes::read_exact_from(&pool, &mut reader, 4)
            }));
            if panic_on_failure {
                assert!(result.is_err());
            } else {
                assert!(result.unwrap().is_err());
            }
            assert_eq!(reader.calls, 2);
            assert_eq!(pool.live_bytes(), retained);
            assert_eq!(neighbor.as_slice(), b"keep");
        }
        let output = BudgetedBytes::read_exact_from(&pool, &mut &b"okay"[..], 4).unwrap();
        assert_eq!(output.as_slice(), b"okay");
        assert_eq!(pool.live_bytes(), retained + output.capacity());
        drop(output);
        assert_eq!(pool.live_bytes(), retained);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn resizing_is_atomic_and_moved_lease_retains_accounting() {
        let budget = OperationBudget::new(10);
        let mut lease = budget.reserve(6).unwrap();
        let neighbor = budget.reserve(2).unwrap();
        assert!(lease.resize(9).is_err());
        assert_eq!((lease.bytes(), budget.live_bytes()), (6, 8));
        lease.resize(8).unwrap();
        assert_eq!(budget.live_bytes(), 10);
        lease.resize(3).unwrap();
        let moved = lease;
        assert_eq!(budget.live_bytes(), 5);
        drop(neighbor);
        assert_eq!(budget.live_bytes(), 3);
        drop(moved);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn unwind_releases_temporary_lease_but_preserves_live_owner() {
        let budget = OperationBudget::new(10);
        let retained = budget.reserve(3).unwrap();
        let failure = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _temporary = budget.reserve(7).unwrap();
            assert_eq!(budget.live_bytes(), 10);
            panic!("injected unwind");
        }));
        assert!(failure.is_err());
        assert_eq!(budget.live_bytes(), 3);
        drop(retained);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn zero_and_overflow_limits_do_not_corrupt_state() {
        let zero = OperationBudget::new(0);
        let empty = zero.reserve(0).unwrap();
        assert!(zero.reserve(1).is_err());
        drop(empty);
        assert_eq!(zero.live_bytes(), 0);
        let budget = OperationBudget::new(usize::MAX);
        let mut full = budget.reserve(usize::MAX).unwrap();
        assert!(budget.reserve(1).is_err());
        assert_eq!(budget.live_bytes(), usize::MAX);
        full.resize(0).unwrap();
        assert_eq!(budget.live_bytes(), 0);
        drop(budget);
        // The lease itself keeps the pool alive until the owner is dropped.
        assert_eq!(full.bytes(), 0);
    }
}
