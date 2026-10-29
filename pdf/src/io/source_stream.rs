//! Immutable source locations and bounded, leased raw-stream residency.
use crate::{
    budget::{MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
};
use std::{cell::RefCell, collections::VecDeque, rc::Rc};

/// Opaque provenance: offsets from another parser cannot be read accidentally.
#[derive(Clone, Debug)]
pub(crate) struct SourceId(Rc<()>);
impl SourceId {
    pub(crate) fn new() -> Self {
        Self(Rc::new(()))
    }
    pub(crate) fn matches(&self, other: &Self) -> bool {
        Rc::ptr_eq(&self.0, &other.0)
    }
}
#[derive(Clone, Debug)]
pub(crate) struct SourceRange {
    pub source: SourceId,
    pub start: u64,
    pub length: usize,
}
impl SourceRange {
    pub(crate) fn matches(&self, other: &Self) -> bool {
        self.source.matches(&other.source)
            && self.start == other.start
            && self.length == other.length
    }
}

pub(crate) struct RawAllocation {
    pub bytes: Vec<u8>,
    pub _lease: MemoryLease,
    pub _transient: Option<TransientLease>,
}

impl std::fmt::Debug for RawAllocation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawAllocation")
            .field("bytes", &self.bytes.len())
            .finish()
    }
}
#[derive(Debug, Default)]
struct TransientState {
    live: std::cell::Cell<usize>,
    peak: std::cell::Cell<usize>,
}
#[derive(Debug)]
pub(crate) struct TransientLease {
    state: Rc<TransientState>,
    bytes: usize,
}
impl Drop for TransientLease {
    fn drop(&mut self) {
        self.state.live.set(self.state.live.get() - self.bytes);
    }
}

/// A read pins the allocation until this owner is dropped. Source input views
/// borrow the document's immutable input instead and need no second reservation.
pub struct RawStreamData<'a>(pub(crate) RawRead<'a>);
#[derive(Debug)]
pub(crate) enum RawRead<'a> {
    Borrowed(&'a [u8]),
    Leased(Rc<RawAllocation>),
}
impl std::fmt::Debug for RawStreamData<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("RawStreamData")
            .field("bytes", &self.len())
            .finish()
    }
}
impl std::ops::Deref for RawStreamData<'_> {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match &self.0 {
            RawRead::Borrowed(bytes) => bytes,
            RawRead::Leased(data) => &data.bytes,
        }
    }
}
impl AsRef<[u8]> for RawStreamData<'_> {
    fn as_ref(&self) -> &[u8] {
        self
    }
}

/// Owned source buffers only. Shared input and document edits are separate.
#[derive(Debug)]
pub(crate) struct RawStreamCache {
    pool: OperationBudget,
    transient: Rc<TransientState>,
    entries: RefCell<VecDeque<(SourceRange, Rc<RawAllocation>)>>,
    hits: std::cell::Cell<u64>,
    misses: std::cell::Cell<u64>,
    evictions: std::cell::Cell<u64>,
}
#[derive(Clone, Copy, Debug, Default)]
pub struct SourceStreamStatistics {
    pub source_bytes: usize,
    pub resident_bytes: usize,
    pub pinned_bytes: usize,
    pub peak_resident_bytes: usize,
    pub transient_bytes: usize,
    pub peak_transient_bytes: usize,
    pub limit_bytes: usize,
    pub entries: usize,
    pub hits: u64,
    pub misses: u64,
    pub evictions: u64,
}
impl RawStreamCache {
    pub(crate) fn new(maximum: usize) -> Self {
        Self {
            pool: OperationBudget::new(maximum),
            transient: Rc::new(TransientState::default()),
            entries: RefCell::new(VecDeque::new()),
            hits: 0.into(),
            misses: 0.into(),
            evictions: 0.into(),
        }
    }
    pub(crate) fn get_or_read(
        &self,
        range: &SourceRange,
        read: impl FnOnce(&OperationBudget) -> PdfResult<RawAllocation>,
    ) -> PdfResult<Rc<RawAllocation>> {
        let mut entries = self.entries.borrow_mut();
        if let Some(index) = entries.iter().position(|(key, _)| key.matches(range)) {
            let entry = entries.remove(index).unwrap();
            let result = entry.1.clone();
            entries.push_back(entry);
            self.hits.set(self.hits.get().saturating_add(1));
            return Ok(result);
        }
        self.misses.set(self.misses.get().saturating_add(1));
        // Only unpinned entries can release a reservation. A lease retained by
        // a serializer/decoder continues to count even after an explicit clear.
        while self.pool.live_bytes().saturating_add(range.length) > self.pool.maximum_bytes()
            || entries.len() >= 256
        {
            let Some(index) = entries
                .iter()
                .position(|(_, data)| Rc::strong_count(data) == 1)
            else {
                break;
            };
            entries.remove(index);
            self.evictions.set(self.evictions.get().saturating_add(1));
        }
        let actual = self.pool.live_bytes().saturating_add(range.length);
        if entries.len() >= 256 {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "raw_stream_cache_entries",
                stage: "source_read",
                limit: 256,
                actual: entries.len() as u64 + 1,
            });
        }
        if actual > self.pool.maximum_bytes() {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "raw_stream_residency",
                stage: "source_read",
                limit: self.pool.maximum_bytes() as u64,
                actual: actual as u64,
            });
        }
        drop(entries);
        let data = Rc::new(read(&self.pool)?);
        self.entries
            .borrow_mut()
            .push_back((range.clone(), data.clone()));
        Ok(data)
    }
    pub(crate) fn track_transient(&self, mut data: RawAllocation) -> RawAllocation {
        let bytes = data.bytes.capacity();
        let live = self
            .transient
            .live
            .get()
            .checked_add(bytes)
            .expect("live owned allocations fit address space");
        self.transient.live.set(live);
        self.transient.peak.set(self.transient.peak.get().max(live));
        data._transient = Some(TransientLease {
            state: self.transient.clone(),
            bytes,
        });
        data
    }
    pub(crate) fn clear(&self) {
        self.entries.borrow_mut().clear();
    }
    pub(crate) fn statistics(&self) -> SourceStreamStatistics {
        let entries = self.entries.borrow();
        let unpinned: usize = entries
            .iter()
            .filter(|(_, value)| Rc::strong_count(value) == 1)
            .map(|(_, value)| value.bytes.capacity())
            .sum();
        SourceStreamStatistics {
            resident_bytes: self.pool.live_bytes(),
            pinned_bytes: self.pool.live_bytes().saturating_sub(unpinned),
            peak_resident_bytes: self.pool.peak_bytes(),
            transient_bytes: self.transient.live.get(),
            peak_transient_bytes: self.transient.peak.get(),
            limit_bytes: self.pool.maximum_bytes(),
            entries: entries.len(),
            hits: self.hits.get(),
            misses: self.misses.get(),
            evictions: self.evictions.get(),
            ..Default::default()
        }
    }
}
