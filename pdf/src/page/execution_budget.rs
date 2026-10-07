//! Shared logical work budget for one page's content invocation tree.
use crate::{
    error::{PdfError, PdfResult},
    parser::options::Limits,
};
use std::{cell::Cell, rc::Rc};

#[derive(Debug, Default)]
struct Work {
    operations: Cell<usize>,
    decoded_bytes: Cell<usize>,
    path_segments: Cell<usize>,
    text_chars: Cell<usize>,
}

#[derive(Debug, Clone)]
pub(crate) struct ExecutionBudget {
    work: Rc<Work>,
    depth: usize,
}

impl ExecutionBudget {
    pub fn new() -> Self {
        Self {
            work: Rc::new(Work::default()),
            depth: 0,
        }
    }

    pub fn child(&self, limits: &Limits) -> PdfResult<Self> {
        let depth = self
            .depth
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("Form depth overflow".into()))?;
        if depth > limits.max_depth {
            return Err(PdfError::ResourceLimit("Form invocation depth".into()));
        }
        Ok(Self {
            work: self.work.clone(),
            depth,
        })
    }

    pub fn operation(&self, limits: &Limits) -> PdfResult<()> {
        Self::charge(
            &self.work.operations,
            1,
            limits.max_content_operations,
            "content invocation operations",
        )
    }

    pub fn path_segments(&self, count: usize, limits: &Limits) -> PdfResult<()> {
        Self::charge(
            &self.work.path_segments,
            count,
            limits.max_path_segments,
            "content invocation path segments",
        )
    }

    pub fn text_chars(&self, count: usize, limits: &Limits) -> PdfResult<()> {
        Self::charge(
            &self.work.text_chars,
            count,
            limits.max_text_chars,
            "content invocation text characters",
        )
    }

    pub fn remaining_text_chars(&self, limits: &Limits) -> usize {
        limits
            .max_text_chars
            .saturating_sub(self.work.text_chars.get())
    }

    /// Repeated invocations are charged separately; not a live-memory counter.
    pub fn decoded(&self, bytes: usize, limits: &Limits) -> PdfResult<()> {
        Self::charge(
            &self.work.decoded_bytes,
            bytes,
            limits.max_decoded_bytes,
            "content invocation decoded work",
        )
    }

    fn charge(
        counter: &Cell<usize>,
        added: usize,
        maximum: usize,
        description: &str,
    ) -> PdfResult<()> {
        let total = counter
            .get()
            .checked_add(added)
            .filter(|v| *v <= maximum)
            .ok_or_else(|| PdfError::ResourceLimit(description.into()))?;
        counter.set(total);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn sibling_work_is_shared_but_depth_is_branch_local() {
        let limits = Limits {
            max_content_operations: 3,
            max_decoded_bytes: 10,
            max_depth: 2,
            ..Limits::default()
        };
        let root = ExecutionBudget::new();
        let child = root.child(&limits).unwrap();
        let sibling = root.child(&limits).unwrap();
        root.operation(&limits).unwrap();
        child.operation(&limits).unwrap();
        sibling.operation(&limits).unwrap();
        assert!(root.operation(&limits).is_err());
        assert_eq!(root.work.operations.get(), 3);
        child.decoded(6, &limits).unwrap();
        assert!(sibling.decoded(5, &limits).is_err());
        sibling.decoded(4, &limits).unwrap();
        assert_eq!(root.work.decoded_bytes.get(), 10);
        let limits = Limits {
            max_path_segments: 5,
            max_text_chars: 3,
            ..limits
        };
        child.path_segments(3, &limits).unwrap();
        sibling.path_segments(2, &limits).unwrap();
        assert!(root.path_segments(1, &limits).is_err());
        assert_eq!(root.work.path_segments.get(), 5);
        child.text_chars(2, &limits).unwrap();
        assert_eq!(sibling.remaining_text_chars(&limits), 1);
        assert!(sibling.text_chars(2, &limits).is_err());
        sibling.text_chars(1, &limits).unwrap();
        assert_eq!(root.remaining_text_chars(&limits), 0);
        assert!(child.child(&limits).unwrap().child(&limits).is_err());
        assert!(root.child(&limits).is_ok());
        assert!(root.decoded(usize::MAX, &limits).is_err());
        assert_eq!(root.work.decoded_bytes.get(), 10);
    }
}
