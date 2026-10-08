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

/// Character-code byte position qualified by one original leaf invocation.
/// Borrowing the draw source avoids allocating/cloning an invocation chain.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NestedTextCodePosition<'a> {
    pub source: &'a NestedDrawSource,
    /// TJ item index (including numeric items); None for Tj/quotes.
    pub array_item: Option<usize>,
    pub offset: usize,
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
    fn prepared_form_copies_outlive_chain_iterator_and_clean_unwind() {
        use crate::{
            budget::OperationBudget,
            objects::{pdf_dict::PdfDict, pdf_name::PdfName, PdfObject},
        };
        let pool = OperationBudget::new(65536);
        let neighbor = pool.reserve(7).unwrap();
        let mut original = PdfDict::default();
        original.insert(
            "Subtype".into(),
            PdfObject::PdfName(PdfName::new("Form".into())),
        );
        original.insert(
            "ResourceName".into(),
            PdfObject::PdfName(PdfName::new("kept".repeat(32))),
        );
        let construct = || {
            let mut slot_lease = pool.reserve(std::mem::size_of::<PreparedForm>()).unwrap();
            let mut forms = Vec::new();
            forms.try_reserve_exact(1).unwrap();
            slot_lease
                .resize(forms.capacity() * std::mem::size_of::<PreparedForm>())
                .unwrap();
            let (dictionary, mut copies_lease) =
                crate::objects::clone_dict_with_budget(&original, &pool, Limits::default(), 0)
                    .unwrap();
            let (resource_scope, mut resource_lease) =
                crate::objects::clone_dict_with_budget(&original, &pool, Limits::default(), 0)
                    .unwrap();
            copies_lease.absorb(&mut resource_lease).unwrap();
            forms.push(PreparedForm {
                dictionary,
                resource_scope,
                program: PreparedProgram::Plain(
                    ContentProgram::from_decoded_bytes(b"q Q".to_vec(), Default::default())
                        .unwrap(),
                ),
                copies_lease: Some(copies_lease),
            });
            BudgetedFormChain {
                forms,
                _lease: slot_lease,
            }
        };
        let mut iterator = construct().into_iter();
        let form = iterator.next_back().unwrap();
        let retained = form.copies_lease.as_ref().unwrap().bytes();
        assert!(retained > 0);
        assert!(pool.live_bytes() > retained + 7);
        drop(iterator);
        assert_eq!(pool.live_bytes(), retained + 7);
        assert_eq!(form.dictionary, original);
        assert_eq!(form.resource_scope, original);
        drop(form);
        assert_eq!(pool.live_bytes(), 7);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut iterator = construct().into_iter();
            let _form = iterator.next_back().unwrap();
            drop(iterator);
            panic!("prepared form copy unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 7);
        drop(construct());
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn prepared_chain_slot_owner_survives_reverse_iteration_and_unwind() {
        use crate::budget::OperationBudget;
        let pool = OperationBudget::new(65536);
        let neighbor = pool.reserve(7).unwrap();
        let construct = || {
            let mut lease = pool
                .reserve(2 * std::mem::size_of::<PreparedForm>())
                .unwrap();
            let mut forms = Vec::new();
            forms.try_reserve_exact(2).unwrap();
            lease
                .resize(forms.capacity() * std::mem::size_of::<PreparedForm>())
                .unwrap();
            for _ in 0..2 {
                forms.push(PreparedForm {
                    dictionary: Default::default(),
                    resource_scope: Default::default(),
                    program: PreparedProgram::Plain(
                        ContentProgram::from_decoded_bytes(b"q Q".to_vec(), Default::default())
                            .unwrap(),
                    ),
                    copies_lease: None,
                });
            }
            BudgetedFormChain {
                forms,
                _lease: lease,
            }
        };
        let chain = construct();
        let bytes = chain._lease.bytes();
        assert_eq!(pool.live_bytes(), 7 + bytes);
        let mut iterator = chain.into_iter();
        assert_eq!(iterator.len(), 2);
        drop(iterator.next_back().unwrap());
        assert_eq!(iterator.len(), 1);
        assert_eq!(pool.live_bytes(), 7 + bytes);
        drop(iterator.next().unwrap());
        assert_eq!(iterator.len(), 0);
        // An exhausted IntoIter still owns its allocation until dropped.
        assert_eq!(pool.live_bytes(), 7 + bytes);
        drop(iterator);
        assert_eq!(pool.live_bytes(), 7);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut iterator = construct().into_iter();
            drop(iterator.next_back());
            panic!("prepared chain iterator unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 7);
        drop(construct());
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
    }

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
    pub copies_lease: Option<crate::budget::MemoryLease>,
}

pub(crate) struct BudgetedFormChain {
    forms: Vec<PreparedForm>,
    _lease: crate::budget::MemoryLease,
}

impl std::ops::Deref for BudgetedFormChain {
    type Target = [PreparedForm];
    fn deref(&self) -> &[PreparedForm] {
        &self.forms
    }
}

pub(crate) struct BudgetedFormIter {
    forms: std::vec::IntoIter<PreparedForm>,
    _lease: crate::budget::MemoryLease,
}

impl Iterator for BudgetedFormIter {
    type Item = PreparedForm;
    fn next(&mut self) -> Option<Self::Item> {
        self.forms.next()
    }
    fn size_hint(&self) -> (usize, Option<usize>) {
        self.forms.size_hint()
    }
}

impl DoubleEndedIterator for BudgetedFormIter {
    fn next_back(&mut self) -> Option<Self::Item> {
        self.forms.next_back()
    }
}

impl ExactSizeIterator for BudgetedFormIter {
    fn len(&self) -> usize {
        self.forms.len()
    }
}

impl IntoIterator for BudgetedFormChain {
    type Item = PreparedForm;
    type IntoIter = BudgetedFormIter;
    fn into_iter(self) -> Self::IntoIter {
        BudgetedFormIter {
            forms: self.forms.into_iter(),
            _lease: self._lease,
        }
    }
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

impl AsRef<ContentProgram> for PreparedProgram {
    fn as_ref(&self) -> &ContentProgram {
        self
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
            .map(|(forms, _)| forms)
    }

    pub(crate) fn prepare_form_chain_with_budget(
        &self,
        root: &ContentProgram,
        source: &NestedDrawSource,
        budget: &crate::budget::OperationBudget,
    ) -> crate::error::PdfResult<BudgetedFormChain> {
        let (forms, lease) = self.prepare_form_chain_inner(root, source, Some(budget))?;
        Ok(BudgetedFormChain {
            forms,
            _lease: lease.unwrap(),
        })
    }

    fn prepare_form_chain_inner(
        &self,
        root: &ContentProgram,
        source: &NestedDrawSource,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> crate::error::PdfResult<(Vec<PreparedForm>, Option<crate::budget::MemoryLease>)> {
        use crate::{error::PdfError, objects::PdfObject};
        if !self.source_provenance().matches_program(root) {
            return Err(PdfError::PageResourceError(
                "Nested source requires current page snapshot".into(),
            ));
        }
        let limits = self.ctx.limits();
        source.validate_limits(&limits)?;
        let mut _page_scope_lease = None;
        let page_scope = if let Some(budget) = budget {
            let (scope, lease) = self.resource_with_budget(budget)?;
            _page_scope_lease = Some(lease);
            scope
        } else {
            self.resource()?
        };
        let chain_bytes = source
            .invocations
            .len()
            .checked_mul(std::mem::size_of::<PreparedForm>())
            .ok_or_else(|| PdfError::ResourceLimit("Nested chain slots".into()))?;
        let mut chain_lease = budget.map(|pool| pool.reserve(chain_bytes)).transpose()?;
        let mut chain: Vec<PreparedForm> = Vec::new();
        chain
            .try_reserve_exact(source.invocations.len())
            .map_err(|_| PdfError::ResourceLimit("Nested prepared chain allocation".into()))?;
        if let Some(lease) = chain_lease.as_mut() {
            lease.resize(
                chain
                    .capacity()
                    .checked_mul(std::mem::size_of::<PreparedForm>())
                    .ok_or_else(|| PdfError::ResourceLimit("Nested chain capacity".into()))?,
            )?;
        }
        let mut decoded = root.source_bytes().len();
        let mut operations = root.operations().len();
        let seen_bytes = source
            .invocations
            .len()
            .checked_mul(std::mem::size_of::<ObjectId>())
            .ok_or_else(|| PdfError::ResourceLimit("Nested cycle slots".into()))?;
        let mut seen_lease = budget.map(|pool| pool.reserve(seen_bytes)).transpose()?;
        let mut seen = Vec::new();
        seen.try_reserve_exact(source.invocations.len())
            .map_err(|_| PdfError::ResourceLimit("Nested source cycle path allocation".into()))?;
        if let Some(lease) = seen_lease.as_mut() {
            lease.resize(
                seen.capacity()
                    .checked_mul(std::mem::size_of::<ObjectId>())
                    .ok_or_else(|| PdfError::ResourceLimit("Nested cycle capacity".into()))?,
            )?;
        }
        for call in &source.invocations {
            let scope = chain
                .last()
                .map_or(&page_scope, |form| &form.resource_scope);
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
            let entries_value = scope.get("XObject").ok_or_else(|| {
                PdfError::PageResourceError("Nested XObject scope missing".into())
            })?;
            let mut _entries_lease = None;
            let entries = if let Some(budget) = budget {
                let (object, lease) = self.ctx.resolve_owned_with_budget(entries_value, budget)?;
                _entries_lease = Some(lease);
                object
            } else {
                self.ctx.resolve(entries_value)?
            };
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
            let mut _object_lease = None;
            let object = if let Some(budget) = budget {
                let (object, lease) = self.ctx.resolve_owned_with_budget(entry, budget)?;
                _object_lease = Some(lease);
                object
            } else {
                self.ctx.resolve(entry)?
            };
            let mut _preflight_lease = None;
            let form = if let Some(budget) = budget {
                let (form, lease) =
                    super::pdf_form::PdfForm::try_new_with_budget(&object, self.ctx, budget)?;
                _preflight_lease = Some(lease);
                form
            } else {
                super::pdf_form::PdfForm::try_new(&object, self.ctx)?
            };
            let effective_scope = form.resources.as_ref().unwrap_or(&page_scope);
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
            let mut copies_lease = budget.map(|pool| pool.reserve(0)).transpose()?;
            let mut copy_dictionary = |dict: &crate::objects::pdf_dict::PdfDict| {
                if let Some(budget) = budget {
                    let (dict, mut lease) =
                        crate::objects::clone_dict_with_budget(dict, budget, limits, 0)?;
                    copies_lease.as_mut().unwrap().absorb(&mut lease)?;
                    Ok(dict)
                } else {
                    Ok::<_, PdfError>(dict.clone())
                }
            };
            let dictionary = copy_dictionary(stream.dict())?;
            let resource_scope = copy_dictionary(effective_scope)?;
            chain.push(PreparedForm {
                dictionary,
                resource_scope,
                program,
                copies_lease,
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
        Ok((chain, chain_lease))
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
