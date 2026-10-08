//! Pending multi-invocation span planning. Not yet wired into document editing.
//! This arena merges original invocation prefixes, never partially commits them.
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    page::{
        content_program::{BudgetedContentProgram, ContentProgram, OperationReplacement},
        source_provenance::{FormInvocation, NestedDrawSource},
    },
    parser::options::{Limits, ParseOptions},
};

#[derive(Debug, Clone, Copy)]
pub(super) struct SpanNode {
    pub parent: Option<usize>,
    pub invocation: Option<FormInvocation>,
}

/// Root is node zero. Leaves correspond to input sources, including repeated
/// leaves containing different text endpoints. Inputs themselves are borrowed
/// and excluded from this owner's allocation accounting.
pub(super) struct SpanInvocationTree {
    nodes: Vec<SpanNode>,
    leaves: Vec<usize>,
    _nodes_lease: MemoryLease,
    _leaves_lease: MemoryLease,
}

pub(super) struct SpanChildren {
    indices: Vec<usize>,
    _lease: MemoryLease,
}

struct SpanTable<T> {
    items: Vec<T>,
    _lease: MemoryLease,
}

impl<T> SpanTable<T> {
    fn new(count: usize, budget: &OperationBudget) -> PdfResult<Self> {
        let (items, lease) = table(count, budget)?;
        Ok(Self {
            items,
            _lease: lease,
        })
    }
}

pub(super) struct SpanParentEdit {
    program: BudgetedContentProgram,
    _fragments: SpanTable<BudgetedBytes>,
}

impl AsRef<ContentProgram> for SpanParentEdit {
    fn as_ref(&self) -> &ContentProgram {
        self.program.as_ref()
    }
}

impl SpanChildren {
    pub fn indices(&self) -> &[usize] {
        &self.indices
    }
}

/// Retain every node result (including its own heap owners) through publication.
/// This is a preparation arena, not a document transaction or a committed edit.
pub(super) struct SpanBuild<T> {
    results: Vec<Option<T>>,
    _lease: MemoryLease,
}

impl<T> SpanBuild<T> {
    pub fn root(&self) -> &T {
        self.results[0].as_ref().unwrap()
    }
}

fn table<T>(count: usize, budget: &OperationBudget) -> PdfResult<(Vec<T>, MemoryLease)> {
    let bytes = count
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| PdfError::ResourceLimit("span tree allocation overflow".into()))?;
    let mut lease = budget.reserve(bytes)?;
    let mut items = Vec::new();
    items
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("span tree allocation failed".into()))?;
    lease.resize(
        items
            .capacity()
            .checked_mul(std::mem::size_of::<T>())
            .ok_or_else(|| PdfError::ResourceLimit("span tree capacity overflow".into()))?,
    )?;
    Ok((items, lease))
}

impl SpanInvocationTree {
    /// Rebuild all selected child Do operations in one original parent program.
    /// The callback must prepare bindings, not publish changes to a document.
    pub fn rebuild_parent<F>(
        &self,
        parent: usize,
        original: &ContentProgram,
        options: ParseOptions,
        budget: &OperationBudget,
        mut bind: F,
    ) -> PdfResult<SpanParentEdit>
    where
        F: FnMut(usize) -> PdfResult<BudgetedBytes>,
    {
        let children = self.children_in_source_order(parent, budget)?;
        // Validate all original call sites before any binding preparation.
        for &child in children.indices() {
            let call = self.nodes[child].invocation.unwrap();
            let operation = &original
                .operations()
                .get(call.operation_index)
                .ok_or_else(|| PdfError::ObjectError("span parent call missing".into()))?
                .operator;
            if operation.name() != "Do"
                || operation.num_operands() != 1
                || operation.byte_offset() != call.operator_byte_offset
            {
                return Err(PdfError::ObjectError("span parent call mismatch".into()));
            }
            operation.get_as_name(0)?;
        }
        let mut fragments = SpanTable::new(children.indices().len(), budget)?;
        for &child in children.indices() {
            fragments.items.push(bind(child)?);
        }
        let mut replacements = SpanTable::new(children.indices().len(), budget)?;
        for (&child, fragment) in children.indices().iter().zip(&fragments.items) {
            replacements.items.push(OperationReplacement {
                index: self.nodes[child].invocation.unwrap().operation_index,
                source: fragment.as_slice(),
                operation_count: 1,
            });
        }
        let program = original.with_replaced_operation_sequences_with_budget(
            &replacements.items,
            options,
            budget,
        )?;
        for &child in children.indices() {
            let operation = &program.as_ref().operations()
                [self.nodes[child].invocation.unwrap().operation_index]
                .operator;
            if operation.name() != "Do" || operation.num_operands() != 1 {
                return Err(PdfError::ObjectError(
                    "span binding must rebuild one Do".into(),
                ));
            }
            operation.get_as_name(0)?;
        }
        drop(replacements);
        Ok(SpanParentEdit {
            program,
            _fragments: fragments,
        })
    }

    pub fn bottom_up(
        &self,
    ) -> impl DoubleEndedIterator<Item = (usize, &SpanNode)> + ExactSizeIterator {
        // Every parent is inserted before its children; reverse arena order is
        // topological, but must not be mistaken for original sibling order.
        self.nodes.iter().enumerate().rev()
    }

    pub fn children_in_source_order(
        &self,
        parent: usize,
        budget: &OperationBudget,
    ) -> PdfResult<SpanChildren> {
        if parent >= self.nodes.len() {
            return Err(PdfError::ObjectError("span parent out of range".into()));
        }
        let count = self
            .nodes
            .iter()
            .filter(|node| node.parent == Some(parent))
            .count();
        let (mut indices, lease) = table(count, budget)?;
        indices.extend(
            self.nodes
                .iter()
                .enumerate()
                .filter_map(|(index, node)| (node.parent == Some(parent)).then_some(index)),
        );
        indices
            .sort_unstable_by_key(|&index| self.nodes[index].invocation.unwrap().operation_index);
        Ok(SpanChildren {
            indices,
            _lease: lease,
        })
    }

    pub fn build_bottom_up<T, F>(
        &self,
        budget: &OperationBudget,
        mut build: F,
    ) -> PdfResult<SpanBuild<T>>
    where
        F: FnMut(usize, &SpanNode, &[Option<T>]) -> PdfResult<T>,
    {
        let (results, lease) = table(self.nodes.len(), budget)?;
        let mut built = SpanBuild {
            results,
            _lease: lease,
        };
        built.results.resize_with(self.nodes.len(), || None);
        for (index, node) in self.bottom_up() {
            built.results[index] = Some(build(index, node, &built.results)?);
        }
        Ok(built)
    }

    pub fn nodes(&self) -> &[SpanNode] {
        &self.nodes
    }

    fn leaf(&self, source: usize) -> PdfResult<usize> {
        self.leaves
            .get(source)
            .copied()
            .ok_or_else(|| PdfError::ObjectError("span source slot out of range".into()))
    }

    fn depth(&self, mut node: usize) -> usize {
        let mut depth = 0;
        while let Some(parent) = self.nodes[node].parent {
            depth += 1;
            node = parent;
        }
        depth
    }

    pub fn lowest_common_ancestor(
        &self,
        left_source: usize,
        right_source: usize,
    ) -> PdfResult<usize> {
        let mut left = self.leaf(left_source)?;
        let mut right = self.leaf(right_source)?;
        let mut left_depth = self.depth(left);
        let mut right_depth = self.depth(right);
        while left_depth > right_depth {
            left = self.nodes[left].parent.unwrap();
            left_depth -= 1;
        }
        while right_depth > left_depth {
            right = self.nodes[right].parent.unwrap();
            right_depth -= 1;
        }
        while left != right {
            left = self.nodes[left].parent.unwrap();
            right = self.nodes[right].parent.unwrap();
        }
        Ok(left)
    }

    /// Compare original expanded source order, not object IDs or arena order.
    /// Position tuples are (input source slot, leaf operation index).
    pub fn compare_positions(
        &self,
        left: (usize, usize),
        right: (usize, usize),
    ) -> PdfResult<std::cmp::Ordering> {
        let left_leaf = self.leaf(left.0)?;
        let right_leaf = self.leaf(right.0)?;
        if left_leaf == right_leaf {
            return Ok(left.1.cmp(&right.1));
        }
        let ancestor = self.lowest_common_ancestor(left.0, right.0)?;
        let key = |mut leaf: usize, operation: usize| {
            if leaf == ancestor {
                return operation;
            }
            while self.nodes[leaf].parent != Some(ancestor) {
                leaf = self.nodes[leaf].parent.unwrap();
            }
            self.nodes[leaf].invocation.unwrap().operation_index
        };
        let order = key(left_leaf, left.1).cmp(&key(right_leaf, right.1));
        if order == std::cmp::Ordering::Equal {
            return Err(PdfError::ObjectError(
                "text position conflicts with Form call site".into(),
            ));
        }
        Ok(order)
    }

    pub fn new(
        sources: &[&NestedDrawSource],
        limits: &Limits,
        budget: &OperationBudget,
    ) -> PdfResult<Self> {
        if sources.len() > limits.max_objects {
            return Err(PdfError::ResourceLimit("span source count".into()));
        }
        for source in sources {
            source.validate_limits(limits)?;
        }
        // Count actual distinct prefixes before allocating. A sum of path
        // lengths would incorrectly reject heavily shared invocation trees.
        let mut count = 1usize;
        for (index, source) in sources.iter().enumerate() {
            for depth in 0..source.invocations.len() {
                let prefix = &source.invocations[..=depth];
                if !sources[..index]
                    .iter()
                    .any(|earlier| earlier.invocations.get(..=depth) == Some(prefix))
                {
                    count = count.checked_add(1).ok_or_else(|| {
                        PdfError::ResourceLimit("span node count overflow".into())
                    })?;
                    if count > limits.max_objects {
                        return Err(PdfError::ResourceLimit("span node count".into()));
                    }
                }
            }
        }
        if count > limits.max_objects {
            return Err(PdfError::ResourceLimit("span node count".into()));
        }
        let (mut nodes, nodes_lease) = table(count, budget)?;
        let (mut leaves, leaves_lease) = table(sources.len(), budget)?;
        nodes.push(SpanNode {
            parent: None,
            invocation: None,
        });
        for source in sources {
            let mut parent = 0;
            for invocation in &source.invocations {
                let existing = nodes.iter().position(|node| {
                    node.parent == Some(parent)
                        && node
                            .invocation
                            .is_some_and(|call| call.operation_index == invocation.operation_index)
                });
                parent = if let Some(index) = existing {
                    if nodes[index].invocation != Some(*invocation) {
                        return Err(PdfError::ObjectError(
                            "conflicting original span invocation".into(),
                        ));
                    }
                    index
                } else {
                    if nodes.len() >= count {
                        return Err(PdfError::ObjectError("span prefix count mismatch".into()));
                    }
                    let index = nodes.len();
                    nodes.push(SpanNode {
                        parent: Some(parent),
                        invocation: Some(*invocation),
                    });
                    index
                };
            }
            leaves.push(parent);
        }
        Ok(Self {
            nodes,
            leaves,
            _nodes_lease: nodes_lease,
            _leaves_lease: leaves_lease,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn source(indices: &[usize]) -> NestedDrawSource {
        NestedDrawSource {
            invocations: indices
                .iter()
                .map(|&index| FormInvocation {
                    form_id: None,
                    operation_index: index,
                    operator_byte_offset: Some(index as u64),
                })
                .collect(),
            operation_index: 0,
            operator_byte_offset: Some(0),
            geometry_operation_indices: Vec::new(),
        }
    }

    #[test]
    fn shared_prefixes_and_repeated_leaves_own_only_actual_metadata() {
        let first = source(&[2, 3]);
        let sibling = source(&[2, 8]);
        let other = source(&[7, 3]);
        let pool = OperationBudget::new(1024 * 1024);
        let limits = Limits {
            max_objects: 6,
            ..Default::default()
        };
        let tree =
            SpanInvocationTree::new(&[&first, &sibling, &other, &first], &limits, &pool).unwrap();
        assert_eq!(tree.nodes.len(), 6);
        assert_eq!(tree.leaves, [2, 3, 5, 2]);
        assert_eq!(
            pool.live_bytes(),
            tree.nodes.capacity() * std::mem::size_of::<SpanNode>()
                + tree.leaves.capacity() * std::mem::size_of::<usize>()
        );
        drop(tree);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn conflicting_paths_and_quota_failure_release_all_owned_tables() {
        let first = source(&[2, 3]);
        let mut conflict = source(&[2, 3]);
        conflict.invocations[1].operator_byte_offset = Some(999);
        let pool = OperationBudget::new(1024 * 1024);
        assert!(matches!(
            SpanInvocationTree::new(&[&first, &conflict], &Default::default(), &pool),
            Err(PdfError::ObjectError(_))
        ));
        assert_eq!(pool.live_bytes(), 0);
        let tiny = OperationBudget::new(1);
        assert!(matches!(
            SpanInvocationTree::new(&[&first], &Default::default(), &tiny),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(tiny.live_bytes(), 0);
        let second_table_failure = OperationBudget::new(
            3 * std::mem::size_of::<SpanNode>() + std::mem::size_of::<usize>() - 1,
        );
        assert!(matches!(
            SpanInvocationTree::new(&[&first], &Default::default(), &second_table_failure),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(second_table_failure.live_bytes(), 0);
    }

    #[test]
    fn ancestor_and_sibling_order_use_original_call_sites() {
        use std::cmp::Ordering::{Greater, Less};
        let first = source(&[2, 3]);
        let ancestor = source(&[2]);
        let sibling = source(&[2, 8]);
        let other = source(&[7, 3]);
        let pool = OperationBudget::new(1024 * 1024);
        let tree = SpanInvocationTree::new(
            &[&first, &ancestor, &sibling, &other, &first],
            &Default::default(),
            &pool,
        )
        .unwrap();
        assert_eq!(tree.lowest_common_ancestor(0, 2).unwrap(), 1);
        assert_eq!(tree.lowest_common_ancestor(0, 3).unwrap(), 0);
        assert_eq!(tree.lowest_common_ancestor(0, 4).unwrap(), 2);
        assert_eq!(tree.compare_positions((0, 99), (2, 0)).unwrap(), Less);
        assert_eq!(tree.compare_positions((0, 99), (3, 0)).unwrap(), Less);
        assert_eq!(tree.compare_positions((1, 1), (0, 99)).unwrap(), Less);
        assert_eq!(tree.compare_positions((1, 4), (0, 0)).unwrap(), Greater);
        assert_eq!(tree.compare_positions((0, 0), (1, 4)).unwrap(), Less);
        assert_eq!(tree.compare_positions((0, 8), (4, 5)).unwrap(), Greater);
        assert!(tree.compare_positions((1, 3), (0, 0)).is_err());
        assert!(tree.lowest_common_ancestor(99, 0).is_err());
        assert_eq!(tree.nodes().len(), 6);
        drop(tree);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn bottom_up_results_preserve_children_and_release_late_errors_and_unwind() {
        use crate::budget::BudgetedBytes;
        let first = source(&[7, 3]);
        let second = source(&[2, 8]);
        let third = source(&[2, 3]);
        let pool = OperationBudget::new(1024 * 1024);
        let tree = SpanInvocationTree::new(&[&first, &second, &third], &Default::default(), &pool)
            .unwrap();
        let baseline = pool.live_bytes();
        let children = tree.children_in_source_order(0, &pool).unwrap();
        assert_eq!(children.indices(), [3, 1]);
        drop(children);
        assert_eq!(pool.live_bytes(), baseline);
        let merge = |index: usize, _: &SpanNode, results: &[Option<BudgetedBytes>]| {
            let children = tree.children_in_source_order(index, &pool)?;
            for &child in children.indices() {
                assert!(results[child].is_some());
            }
            BudgetedBytes::copy_from_slice(&pool, &[index as u8])
        };
        let error = tree.build_bottom_up(&pool, |index, node, results| {
            if index == 1 {
                return Err(PdfError::DocumentError("late span branch".into()));
            }
            merge(index, node, results)
        });
        assert!(
            matches!(error, Err(PdfError::DocumentError(ref message)) if message == "late span branch")
        );
        assert_eq!(pool.live_bytes(), baseline);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = tree.build_bottom_up(&pool, |index, node, results| {
                if index == 1 {
                    panic!("span build unwind");
                }
                merge(index, node, results)
            });
        }));
        assert_eq!(
            caught.unwrap_err().downcast_ref::<&str>().copied(),
            Some("span build unwind")
        );
        assert_eq!(pool.live_bytes(), baseline);
        let built = tree.build_bottom_up(&pool, merge).unwrap();
        assert_eq!(built.root().as_slice(), [0]);
        assert_eq!(
            built
                .results
                .iter()
                .filter(|result| result.is_some())
                .count(),
            6
        );
        assert!(pool.live_bytes() > baseline);
        drop(built);
        assert_eq!(pool.live_bytes(), baseline);
        drop(tree);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn failed_build_keeps_metadata_charged_during_result_destructors() {
        struct Probe {
            pool: OperationBudget,
            floor: usize,
        }
        impl Drop for Probe {
            fn drop(&mut self) {
                assert!(self.pool.live_bytes() >= self.floor);
            }
        }
        let source = source(&[2, 3]);
        let pool = OperationBudget::new(1024 * 1024);
        let tree = SpanInvocationTree::new(&[&source], &Default::default(), &pool).unwrap();
        let baseline = pool.live_bytes();
        let floor = baseline + tree.nodes.len() * std::mem::size_of::<Option<Probe>>();
        let error = tree.build_bottom_up(&pool, |index, _, _| {
            if index == 0 {
                return Err(PdfError::DocumentError("root preparation rejected".into()));
            }
            Ok(Probe {
                pool: pool.clone(),
                floor,
            })
        });
        assert!(
            matches!(error, Err(PdfError::DocumentError(ref message)) if message == "root preparation rejected")
        );
        assert_eq!(pool.live_bytes(), baseline);
        drop(tree);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn parent_rebuild_merges_siblings_without_partial_source_changes() {
        let original = ContentProgram::from_decoded_bytes(
            b"/OldA Do 17 mystery /OldB Do".to_vec(),
            Default::default(),
        )
        .unwrap();
        let mut first = source(&[2]);
        first.invocations[0].operator_byte_offset = original.operations()[2].operator.byte_offset();
        let mut second = source(&[0]);
        second.invocations[0].operator_byte_offset =
            original.operations()[0].operator.byte_offset();
        let pool = OperationBudget::new(1024 * 1024);
        let tree = SpanInvocationTree::new(&[&first, &second], &Default::default(), &pool).unwrap();
        let baseline = pool.live_bytes();
        let bind = |child: usize| {
            BudgetedBytes::copy_from_slice(
                &pool,
                if tree.nodes[child].invocation.unwrap().operation_index == 0 {
                    b"/NewA Do"
                } else {
                    b"/NewB Do"
                },
            )
        };
        let failed = tree.rebuild_parent(0, &original, Default::default(), &pool, |child| {
            if tree.nodes[child].invocation.unwrap().operation_index == 2 {
                return Err(PdfError::DocumentError(
                    "second span binding rejected".into(),
                ));
            }
            bind(child)
        });
        assert!(
            matches!(failed, Err(PdfError::DocumentError(ref message)) if message == "second span binding rejected")
        );
        assert_eq!(pool.live_bytes(), baseline);
        let edited = tree
            .rebuild_parent(0, &original, Default::default(), &pool, bind)
            .unwrap();
        let program = edited.as_ref();
        assert_eq!(
            program.operations()[0]
                .operator
                .get_as_name(0)
                .unwrap()
                .name(),
            "NewA"
        );
        assert_eq!(
            program.operations()[2]
                .operator
                .get_as_name(0)
                .unwrap()
                .name(),
            "NewB"
        );
        assert_eq!(program.operations()[1].operator.name(), "mystery");
        assert_eq!(
            program.operations()[1].operator.get_as_f32(0).unwrap(),
            17.0
        );
        assert_eq!(original.source_bytes(), b"/OldA Do 17 mystery /OldB Do");
        assert!(pool.live_bytes() > baseline);
        drop(edited);
        assert_eq!(pool.live_bytes(), baseline);
        assert!(
            tree.rebuild_parent(0, &original, Default::default(), &pool, |_| {
                BudgetedBytes::copy_from_slice(&pool, b"q")
            })
            .is_err()
        );
        assert_eq!(pool.live_bytes(), baseline);
    }
}
