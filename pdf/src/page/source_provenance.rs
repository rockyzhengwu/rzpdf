use super::content_program::ContentProgram;
use crate::{document::identity::DocumentStamp, objects::object_id::ObjectId};

/// One Form call site in its parent's decoded stream, outermost first.
/// Object IDs are document-local; this is not a persistent edit handle.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FormInvocation {
    pub form_id: Option<ObjectId>,
    pub operation_index: usize,
    pub operator_byte_offset: Option<u64>,
}

/// Leaf operation in a decoded Form stream, qualified by its invocation chain.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NestedDrawSource {
    pub invocations: Vec<FormInvocation>,
    pub operation_index: usize,
    pub operator_byte_offset: Option<u64>,
    /// Leaf-stream m/l/c/v/y/re/h indices for path events; empty for text/images.
    pub geometry_operation_indices: Vec<usize>,
}

impl NestedDrawSource {
    pub(crate) fn validate_limits(
        &self,
        limits: &crate::parser::options::Limits,
    ) -> crate::error::PdfResult<()> {
        if self.invocations.is_empty() || self.invocations.len() > limits.max_depth.min(128) {
            return Err(crate::error::PdfError::ResourceLimit(
                "Nested source invocation depth".into(),
            ));
        }
        if self.geometry_operation_indices.len() > limits.max_content_operations {
            return Err(crate::error::PdfError::ResourceLimit(
                "Nested source geometry operations".into(),
            ));
        }
        if self.retained_bytes()? > limits.max_decoded_bytes {
            return Err(crate::error::PdfError::ResourceLimit(
                "Nested source retained bytes".into(),
            ));
        }
        Ok(())
    }

    pub(crate) fn try_copy(&self) -> crate::error::PdfResult<Self> {
        let mut invocations = Vec::new();
        invocations
            .try_reserve_exact(self.invocations.len())
            .map_err(|_| {
                crate::error::PdfError::ResourceLimit("nested source copy allocation".into())
            })?;
        invocations.extend_from_slice(&self.invocations);
        let mut geometry_operation_indices = Vec::new();
        geometry_operation_indices
            .try_reserve_exact(self.geometry_operation_indices.len())
            .map_err(|_| {
                crate::error::PdfError::ResourceLimit("nested geometry source allocation".into())
            })?;
        geometry_operation_indices.extend_from_slice(&self.geometry_operation_indices);
        Ok(Self {
            invocations,
            geometry_operation_indices,
            operation_index: self.operation_index,
            operator_byte_offset: self.operator_byte_offset,
        })
    }
    pub(crate) fn retained_bytes(&self) -> crate::error::PdfResult<usize> {
        self.invocations
            .len()
            .checked_mul(std::mem::size_of::<FormInvocation>())
            .and_then(|b| {
                self.geometry_operation_indices
                    .len()
                    .checked_mul(std::mem::size_of::<usize>())
                    .and_then(|n| b.checked_add(n))
            })
            .ok_or_else(|| {
                crate::error::PdfError::ResourceLimit("nested source bytes overflow".into())
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::options::Limits;

    #[test]
    fn nested_source_limits_check_exact_bytes_operations_and_depth_before_lookup() {
        let source = NestedDrawSource {
            invocations: vec![FormInvocation {
                form_id: None,
                operation_index: 0,
                operator_byte_offset: Some(0),
            }],
            operation_index: 3,
            operator_byte_offset: Some(20),
            geometry_operation_indices: vec![0, 1, 2],
        };
        let bytes = std::mem::size_of::<FormInvocation>() + 3 * std::mem::size_of::<usize>();
        assert_eq!(source.retained_bytes().unwrap(), bytes);
        let limits = Limits {
            max_depth: 1,
            max_content_operations: 3,
            max_decoded_bytes: bytes,
            ..Limits::default()
        };
        source.validate_limits(&limits).unwrap();
        let copied = source.try_copy().unwrap();
        assert_eq!(copied, source);
        for limited in [
            Limits {
                max_depth: 0,
                ..limits
            },
            Limits {
                max_content_operations: 2,
                ..limits
            },
            Limits {
                max_decoded_bytes: bytes - 1,
                ..limits
            },
        ] {
            assert!(matches!(
                source.validate_limits(&limited),
                Err(crate::error::PdfError::ResourceLimit(_))
            ));
        }
        let empty = NestedDrawSource {
            invocations: Vec::new(),
            ..source
        };
        assert!(empty.validate_limits(&Limits::default()).is_err());
    }
}

/// Provenance of top-level interpreted events, not a persistent edit handle.
#[derive(Debug, Clone)]
pub(crate) struct PageSourceProvenance {
    pub stamp: DocumentStamp,
    pub page_id: Option<ObjectId>,
}

impl PageSourceProvenance {
    pub fn matches_program(&self, program: &ContentProgram) -> bool {
        self.page_id.is_some()
            && self.page_id == program.source_page_id
            && program.source_stamp() == Some(&self.stamp)
    }
}

/// Validated ancestors retained for inside-out copy-on-write editing.
/// Dictionaries still contain same-document references; not portable artifacts.
pub(crate) struct PreparedForm {
    pub dictionary: crate::objects::pdf_dict::PdfDict,
    pub resource_scope: crate::objects::pdf_dict::PdfDict,
    pub program: PreparedProgram,
}

pub(crate) enum PreparedProgram {
    Plain(ContentProgram),
    Budgeted(super::content_program::BudgetedContentProgram),
}

impl std::ops::Deref for PreparedProgram {
    type Target = ContentProgram;
    fn deref(&self) -> &ContentProgram {
        match self {
            Self::Plain(program) => program,
            Self::Budgeted(program) => program.program(),
        }
    }
}

impl<R: std::io::Read + std::io::Seek> super::PdfPage<'_, R> {
    /// Verify a current page snapshot and every Form call site, then return the
    /// leaf's decoded program. The result is offline, not permission to edit a
    /// shared Form. Leaf indices must never be applied to the page program.
    pub fn nested_content_program(
        &self,
        root: &ContentProgram,
        source: &NestedDrawSource,
    ) -> crate::error::PdfResult<ContentProgram> {
        self.prepare_form_chain(root, source)?
            .pop()
            .ok_or_else(|| crate::error::PdfError::PageResourceError("Nested leaf absent".into()))
            .and_then(|form| match form.program {
                PreparedProgram::Plain(program) => Ok(program),
                PreparedProgram::Budgeted(_) => Err(crate::error::PdfError::PageResourceError(
                    "Budgeted leaf requires its owner".into(),
                )),
            })
    }

    /// No mutation: check current provenance and retain effective resource scopes
    /// including legacy page fallback. Aggregate decoded work is bounded, but
    /// dictionary clones/capacities are not an operation-wide peak-memory budget.
    pub(crate) fn prepare_form_chain(
        &self,
        root: &ContentProgram,
        source: &NestedDrawSource,
    ) -> crate::error::PdfResult<Vec<PreparedForm>> {
        self.prepare_form_chain_inner(root, source, None)
    }

    pub(crate) fn prepare_form_chain_with_budget(
        &self,
        root: &ContentProgram,
        source: &NestedDrawSource,
        budget: &crate::budget::OperationBudget,
    ) -> crate::error::PdfResult<Vec<PreparedForm>> {
        self.prepare_form_chain_inner(root, source, Some(budget))
    }

    fn prepare_form_chain_inner(
        &self,
        root: &ContentProgram,
        source: &NestedDrawSource,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> crate::error::PdfResult<Vec<PreparedForm>> {
        use crate::{error::PdfError, objects::PdfObject};
        if !self.source_provenance().matches_program(root) {
            return Err(PdfError::PageResourceError(
                "Nested source requires current page snapshot".into(),
            ));
        }
        let limits = self.ctx.limits();
        source.validate_limits(&limits)?;
        let page_scope = self.resource()?;
        let mut scope = page_scope.clone();
        let mut chain: Vec<PreparedForm> = Vec::new();
        chain
            .try_reserve_exact(source.invocations.len())
            .map_err(|_| PdfError::ResourceLimit("Nested prepared chain allocation".into()))?;
        let mut decoded = root.source_bytes().len();
        let mut operations = root.operations().len();
        let mut seen = Vec::new();
        seen.try_reserve_exact(source.invocations.len())
            .map_err(|_| PdfError::ResourceLimit("Nested source cycle path allocation".into()))?;
        for call in &source.invocations {
            let current = chain.last().map_or(root, |form| &form.program);
            let operation = current
                .operations()
                .get(call.operation_index)
                .ok_or_else(|| PdfError::PageResourceError("Nested call index missing".into()))?;
            if operation.operator.name() != "Do"
                || operation.operator.byte_offset() != call.operator_byte_offset
            {
                return Err(PdfError::PageResourceError(
                    "Nested Form call position mismatch".into(),
                ));
            }
            let name = operation.operator.get_as_name(0)?.name();
            let entries = self.ctx.resolve(scope.get("XObject").ok_or_else(|| {
                PdfError::PageResourceError("Nested XObject scope missing".into())
            })?)?;
            let entries = entries.as_dict().ok_or_else(|| {
                PdfError::PageResourceError("Nested XObject scope must be a dictionary".into())
            })?;
            let entry = entries.get(name).ok_or_else(|| {
                PdfError::PageResourceError("Nested Form resource missing".into())
            })?;
            if entry.as_object_id() != call.form_id {
                return Err(PdfError::PageResourceError(
                    "Nested Form identity mismatch".into(),
                ));
            }
            if let Some(id) = call.form_id {
                if seen.contains(&id) {
                    return Err(PdfError::PageResourceError("Nested Form cycle".into()));
                }
                seen.push(id);
            }
            let object = self.ctx.resolve(entry)?;
            let form = super::pdf_form::PdfForm::try_new(&object, self.ctx)?;
            scope = form.resources.unwrap_or_else(|| page_scope.clone());
            let PdfObject::PdfStream(stream) = object else {
                return Err(PdfError::PageResourceError(
                    "Nested Form must be stream".into(),
                ));
            };
            let program = if let Some(budget) = budget {
                PreparedProgram::Budgeted(ContentProgram::from_decoded_bytes_with_budget(
                    stream.decode_data_with_budget(self.ctx, budget)?,
                    self.ctx.options(),
                    budget,
                )?)
            } else {
                PreparedProgram::Plain(ContentProgram::from_decoded_bytes(
                    stream.decode_data(self.ctx)?,
                    self.ctx.options(),
                )?)
            };
            decoded = decoded
                .checked_add(program.source_bytes().len())
                .filter(|n| *n <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("Nested program decoded work".into()))?;
            operations = operations
                .checked_add(program.operations().len())
                .filter(|n| *n <= limits.max_content_operations)
                .ok_or_else(|| PdfError::ResourceLimit("Nested program operations".into()))?;
            chain.push(PreparedForm {
                dictionary: stream.dict().clone(),
                resource_scope: scope.clone(),
                program,
            });
        }
        let leaf = &chain
            .last()
            .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?
            .program;
        let operation = leaf
            .operations()
            .get(source.operation_index)
            .ok_or_else(|| PdfError::PageResourceError("Nested leaf index missing".into()))?;
        if operation.operator.byte_offset() != source.operator_byte_offset {
            return Err(PdfError::PageResourceError(
                "Nested leaf byte position mismatch".into(),
            ));
        }
        let mut previous = None;
        for &index in &source.geometry_operation_indices {
            if index >= source.operation_index || previous.is_some_and(|p| index <= p) {
                return Err(PdfError::PageResourceError(
                    "Nested geometry index order mismatch".into(),
                ));
            }
            let geometry = leaf.operations().get(index).ok_or_else(|| {
                PdfError::PageResourceError("Nested geometry index missing".into())
            })?;
            if !matches!(
                geometry.operator.name(),
                "m" | "l" | "c" | "v" | "y" | "re" | "h"
            ) {
                return Err(PdfError::PageResourceError(
                    "Nested geometry operator mismatch".into(),
                ));
            }
            previous = Some(index);
        }
        Ok(chain)
    }

    pub(crate) fn source_provenance(&self) -> PageSourceProvenance {
        PageSourceProvenance {
            stamp: DocumentStamp {
                document: self.ctx.identity(),
                revision: self.ctx.revision(),
            },
            page_id: self.page_node.as_object_id(),
        }
    }
}
