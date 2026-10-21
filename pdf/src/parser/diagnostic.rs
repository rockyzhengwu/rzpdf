//! File-structure diagnostics, independent of page/content interpretation.
use crate::{
    error::{PdfError, PdfResult},
    objects::object_id::ObjectId,
};
use std::{cell::RefCell, rc::Rc};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum StructureReason {
    CompatibleHeader,
    CompatibleEof,
    CompatibleXrefEntry,
    CompatibleXrefTable,
    CompatibleXrefStreamType,
    CompatibleXrefTrailingBytes,
    DuplicateDictionaryKey,
    StreamBoundarySearch,
    StreamStartWhitespace,
    RebuiltObjectIndex,
    ImageTailRecovery,
    ImageTrailingBytes,
    ContentStreamBoundary,
    RepairedObjectOffset,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StructureDiagnostic {
    pub offset: u64,
    pub object: Option<ObjectId>,
    pub reason: StructureReason,
}

/// Immutable provenance is not a PDF validity or conformance certificate.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[non_exhaustive]
pub enum ReadProvenance {
    Created,
    Parsed(super::syntax::ParseMode),
    RebuiltReadOnly,
}

#[derive(Debug, Clone)]
pub(crate) struct DiagnosticLog(Rc<RefCell<Log>>);
#[derive(Debug)]
struct Log {
    entries: Vec<StructureDiagnostic>,
    maximum: usize,
    budget: crate::budget::OperationBudget,
    lease: Option<crate::budget::MemoryLease>,
}
impl DiagnosticLog {
    pub(crate) fn new(maximum_bytes: usize) -> Self {
        Self(Rc::new(RefCell::new(Log {
            entries: Vec::new(),
            maximum: 1024,
            budget: crate::budget::OperationBudget::new(maximum_bytes.min(1024 * 1024)),
            lease: None,
        })))
    }
    pub(crate) fn record(&self, entry: StructureDiagnostic) -> PdfResult<()> {
        let mut log = self.0.borrow_mut();
        if log.entries.contains(&entry) {
            return Ok(());
        }
        if log.entries.len() >= log.maximum {
            return Err(PdfError::ResourceLimit(
                "file structure diagnostic count".into(),
            ));
        }
        // Fixed-size reasons retain no attacker-controlled message strings.
        if log.entries.len() == log.entries.capacity() {
            let capacity = log
                .entries
                .capacity()
                .saturating_mul(2)
                .max(1)
                .min(log.maximum);
            let mut lease = log
                .budget
                .reserve(capacity * std::mem::size_of::<StructureDiagnostic>())?;
            let mut replacement = Vec::new();
            replacement
                .try_reserve_exact(capacity)
                .map_err(|_| PdfError::ResourceLimit("structure diagnostics".into()))?;
            lease.resize(replacement.capacity() * std::mem::size_of::<StructureDiagnostic>())?;
            replacement.extend_from_slice(&log.entries);
            log.entries = replacement;
            log.lease = Some(lease);
        }
        log.entries.push(entry);
        Ok(())
    }
    pub(crate) fn snapshot(&self) -> PdfResult<Vec<StructureDiagnostic>> {
        let log = self.0.borrow();
        let mut entries = Vec::new();
        entries
            .try_reserve_exact(log.entries.len())
            .map_err(|_| PdfError::ResourceLimit("diagnostic snapshot".into()))?;
        entries.extend_from_slice(&log.entries);
        Ok(entries)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn exact_count_dedup_and_atomic_overflow() {
        let log = DiagnosticLog::new(1024 * 1024);
        for offset in 0..1024 {
            log.record(StructureDiagnostic {
                offset,
                object: None,
                reason: StructureReason::CompatibleEof,
            })
            .unwrap();
        }
        log.record(StructureDiagnostic {
            offset: 0,
            object: None,
            reason: StructureReason::CompatibleEof,
        })
        .unwrap();
        assert!(
            log.record(StructureDiagnostic {
                offset: 1024,
                object: None,
                reason: StructureReason::CompatibleEof
            })
            .is_err()
        );
        assert_eq!(log.snapshot().unwrap().len(), 1024);
    }
    #[test]
    fn byte_cap_includes_reallocation_overlap() {
        let unit = std::mem::size_of::<StructureDiagnostic>();
        for (bytes, okay) in [(unit, true), (unit - 1, false)] {
            let log = DiagnosticLog::new(bytes);
            assert_eq!(
                log.record(StructureDiagnostic {
                    offset: 1,
                    object: None,
                    reason: StructureReason::CompatibleEof
                })
                .is_ok(),
                okay
            );
            assert_eq!(log.snapshot().unwrap().len(), usize::from(okay));
        }
        let log = DiagnosticLog::new(unit * 2);
        log.record(StructureDiagnostic {
            offset: 1,
            object: None,
            reason: StructureReason::CompatibleEof,
        })
        .unwrap();
        assert!(
            log.record(StructureDiagnostic {
                offset: 2,
                object: None,
                reason: StructureReason::CompatibleEof
            })
            .is_err()
        );
        assert_eq!(log.snapshot().unwrap().len(), 1);
    }
}
