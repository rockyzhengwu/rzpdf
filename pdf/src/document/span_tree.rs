//! Internal multi-invocation planning and single-transaction publication.
//! Range expansion and inherited-scope planning validate original sources before
//! the public span entry publishes prepared edits in one transaction.
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

/// Original event descriptor retained through later unique-node preparation.
/// Empty paths identify top-level events occurring between nested endpoints.
pub(super) struct SpanSelectedEvent {
    path: SpanTable<FormInvocation>,
    operation: usize,
    byte_offset: Option<u64>,
    codes: Option<std::ops::Range<crate::page::content_program::TextCodePosition>>,
    first: bool,
}

pub(super) struct SpanSelection {
    events: SpanTable<SpanSelectedEvent>,
    stamp: Option<super::identity::DocumentStamp>,
    page_id: Option<crate::objects::object_id::ObjectId>,
}

/// Borrows both owners so node indices cannot outlive their selection/tree.
pub(super) struct SpanMappedSelection<'a> {
    selection: &'a SpanSelection,
    tree: &'a SpanInvocationTree,
    nodes: SpanTable<usize>,
}

struct SpanEventGroups<'m, 's> {
    mapped: &'m SpanMappedSelection<'s>,
    offsets: SpanTable<usize>,
    events: SpanTable<usize>,
}

impl<'s> SpanMappedSelection<'s> {
    fn group_events(&self, budget: &OperationBudget) -> PdfResult<SpanEventGroups<'_, 's>> {
        let count = self.tree.nodes.len();
        if self.nodes.items.len() != self.selection.events.items.len() {
            return Err(PdfError::ObjectError(
                "span event mapping length mismatch".into(),
            ));
        }
        let slots = count
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("span grouping overflow".into()))?;
        let mut offsets: SpanTable<usize> = SpanTable::new(slots, budget)?;
        offsets.items.resize(slots, 0);
        for &node in &self.nodes.items {
            if node >= count {
                return Err(PdfError::ObjectError(
                    "span mapped node out of range".into(),
                ));
            }
            offsets.items[node + 1] += 1;
        }
        for node in 0..count {
            offsets.items[node + 1] += offsets.items[node];
        }
        let mut cursors = SpanTable::new(count, budget)?;
        cursors.items.extend_from_slice(&offsets.items[..count]);
        let mut events = SpanTable::new(self.nodes.items.len(), budget)?;
        events.items.resize(self.nodes.items.len(), 0);
        for (index, &node) in self.nodes.items.iter().enumerate() {
            events.items[cursors.items[node]] = index;
            cursors.items[node] += 1;
        }
        for node in 0..count {
            for pair in events.items[offsets.items[node]..offsets.items[node + 1]].windows(2) {
                if self.selection.events.items[pair[0]].operation
                    >= self.selection.events.items[pair[1]].operation
                {
                    return Err(PdfError::ObjectError(
                        "span node events are not in original order".into(),
                    ));
                }
            }
        }
        Ok(SpanEventGroups {
            mapped: self,
            offsets,
            events,
        })
    }
}

impl SpanEventGroups<'_, '_> {
    fn for_node(&self, node: usize) -> PdfResult<&[usize]> {
        if node >= self.mapped.tree.nodes.len() {
            return Err(PdfError::ObjectError("span group node out of range".into()));
        }
        Ok(&self.events.items[self.offsets.items[node]..self.offsets.items[node + 1]])
    }
}

impl SpanSelection {
    fn map_to_tree<'a>(
        &'a self,
        tree: &'a SpanInvocationTree,
        budget: &OperationBudget,
    ) -> PdfResult<SpanMappedSelection<'a>> {
        let mut nodes = SpanTable::new(self.events.items.len(), budget)?;
        for (index, event) in self.events.items.iter().enumerate() {
            // from_selection already records one leaf per event. Authenticate
            // the complete chain backwards before reusing it; matching counts
            // or leaf IDs alone do not establish provenance. Other trees keep
            // the general lookup semantics below.
            if tree.leaves.len() == self.events.items.len() {
                if let Some(&leaf) = tree.leaves.get(index) {
                    let mut candidate = leaf;
                    let mut matches = true;
                    for invocation in event.path.items.iter().rev() {
                        match tree.nodes.get(candidate) {
                            Some(node) if node.invocation.as_ref() == Some(invocation) => {
                                if let Some(parent) = node.parent { candidate = parent; }
                                else { matches = false; break; }
                            }
                            _ => { matches = false; break; }
                        }
                    }
                    if matches && candidate == 0 && tree.nodes.first().is_some_and(|root|
                        root.parent.is_none() && root.invocation.is_none()) {
                        nodes.items.push(leaf);
                        continue;
                    }
                }
            }
            let mut parent = 0;
            for invocation in &event.path.items {
                parent = tree
                    .nodes
                    .iter()
                    .position(|node| {
                        node.parent == Some(parent) && node.invocation.as_ref() == Some(invocation)
                    })
                    .ok_or_else(|| {
                        PdfError::ObjectError("selected invocation absent from tree".into())
                    })?;
            }
            nodes.items.push(parent);
        }
        Ok(SpanMappedSelection {
            selection: self,
            tree,
            nodes,
        })
    }
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

/// Fully materialized, unpublished Form stream and its preparation owners.
/// Keeping the intermediate dictionaries avoids detaching their budget leases.
pub(super) struct SpanFormOutput<P> {
    pub stream: crate::objects::pdf_stream::PdfStream,
    retained: SpanFormRetention<P>,
}

/// Must outlive insertion/publication of the separated stream. Its lease covers
/// the stream payload/dictionary as well as retained preparation allocations.
pub(super) struct SpanFormRetention<P> {
    _program: P,
    _resources: crate::objects::PdfObject,
    _dictionary: crate::objects::pdf_dict::PdfDict,
    _form: crate::page::source_provenance::PreparedForm,
    _lease: MemoryLease,
}

impl<P: AsRef<ContentProgram>> SpanFormOutput<P> {
    pub fn new(
        mut form: crate::page::source_provenance::PreparedForm,
        program: P,
        budget: &OperationBudget,
        limits: Limits,
    ) -> PdfResult<Self> {
        let bytes = program.as_ref().source_bytes().len();
        if bytes > limits.max_raw_stream_bytes || bytes > limits.max_decoded_bytes {
            return Err(PdfError::ResourceLimit(
                "span prepared Form stream bytes".into(),
            ));
        }
        let resources =
            crate::objects::PdfObject::PdfDict(std::mem::take(&mut form.resource_scope));
        let (dictionary, mut lease) = form.dictionary.with_replaced_entry_with_budget(
            "Resources",
            &resources,
            budget,
            limits,
        )?;
        let (data, mut payload_lease) =
            BudgetedBytes::copy_from_slice(budget, program.as_ref().source_bytes())?
                .into_reserved_parts();
        lease.absorb(&mut payload_lease)?;
        let (stream, mut stream_lease) =
            crate::objects::pdf_stream::PdfStream::from_decoded_data_with_budget(
                &dictionary,
                data,
                budget,
                limits,
            )?;
        lease.absorb(&mut stream_lease)?;
        Ok(Self {
            stream,
            retained: SpanFormRetention {
                _program: program,
                _form: form,
                _resources: resources,
                _dictionary: dictionary,
                _lease: lease,
            },
        })
    }

    pub fn into_parts(self) -> (crate::objects::pdf_stream::PdfStream, SpanFormRetention<P>) {
        (self.stream, self.retained)
    }
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

/// One validated dictionary/scope/program owner per original invocation node.
/// Root stays absent because page publication uses the original page snapshot.
pub(super) struct SpanPreparedForms<'a> {
    forms: SpanTable<Option<crate::page::source_provenance::PreparedForm>>,
    tree: &'a SpanInvocationTree,
}

impl SpanPreparedForms<'_> {
    pub fn get(&self, node: usize) -> Option<&crate::page::source_provenance::PreparedForm> {
        self.forms.items.get(node).and_then(Option::as_ref)
    }
}

impl<T> SpanBuild<T> {
    pub fn root(&self) -> &T {
        self.results[0].as_ref().unwrap()
    }
}

enum SpanNodeProgram<'a> {
    Rebuilt(SpanParentEdit),
    // The caller's plan table stays alive through the entire transaction.
    Planned(&'a super::text_event_edit::TextEventEditPlan),
    OwnedPlanned(super::text_event_edit::TextEventEditPlan),
}

enum SpanPublicationPlans<'a> {
    Borrowed(&'a [Option<super::text_event_edit::TextEventEditPlan>]),
    Owned(SpanTable<Option<super::text_event_edit::TextEventEditPlan>>),
}

impl<'a> SpanPublicationPlans<'a> {
    fn as_slice(&self) -> &[Option<super::text_event_edit::TextEventEditPlan>] {
        match self {
            Self::Borrowed(plans) => plans,
            Self::Owned(plans) => &plans.items,
        }
    }

    fn take(&mut self, index: usize) -> Option<SpanNodeProgram<'a>> {
        match self {
            Self::Borrowed(plans) => plans[index].as_ref().map(SpanNodeProgram::Planned),
            Self::Owned(plans) => plans.items[index].take().map(SpanNodeProgram::OwnedPlanned),
        }
    }
}

impl AsRef<ContentProgram> for SpanNodeProgram<'_> {
    fn as_ref(&self) -> &ContentProgram {
        match self {
            Self::Rebuilt(program) => program.as_ref(),
            Self::Planned(plan) => plan.as_ref(),
            Self::OwnedPlanned(plan) => plan.as_ref(),
        }
    }
}

struct SpanPublishedNode<'a> {
    id: Option<crate::objects::object_id::ObjectId>,
    root_program: Option<SpanNodeProgram<'a>>,
    _retained: Option<SpanFormRetention<SpanNodeProgram<'a>>>,
    _bindings: MemoryLease,
}

struct SpanSubsetPublication<'a> {
    published: SpanBuild<SpanPublishedNode<'a>>,
    _font: super::embedded_font::EmbeddedSubsetFontObjects,
    _binding: Option<MemoryLease>,
    _scope: MemoryLease,
}

impl AsRef<ContentProgram> for SpanSubsetPublication<'_> {
    fn as_ref(&self) -> &ContentProgram { self.published.as_ref() }
}

fn check_plan_character_totals(
    plans: &[Option<super::text_event_edit::TextEventEditPlan>],
    limit: usize,
) -> PdfResult<(usize, usize)> {
    let (mut input, mut output) = (0usize, 0usize);
    for plan in plans.iter().flatten() {
        let (old, new) = plan.character_counts();
        input = input
            .checked_add(old)
            .filter(|&count| count <= limit)
            .ok_or_else(|| PdfError::ResourceLimit("span aggregate input characters".into()))?;
        output = output
            .checked_add(new)
            .filter(|&count| count <= limit)
            .ok_or_else(|| PdfError::ResourceLimit("span aggregate output characters".into()))?;
    }
    Ok((input, output))
}

impl AsRef<ContentProgram> for SpanBuild<SpanPublishedNode<'_>> {
    fn as_ref(&self) -> &ContentProgram {
        self.root().root_program.as_ref().unwrap().as_ref()
    }
}

impl<R: std::io::Read + std::io::Seek> super::PdfDocument<R> {
    /// Replace an expanded original selection, including intervening Form calls.
    /// Only the insertion event gets a new subset; all deletions use their own
    /// original fonts. Selected invocation copies are published once.
    pub fn replace_nested_text_span_with_subset_and_budget(
        &self,
        page: u32,
        root: &ContentProgram,
        range: std::ops::Range<crate::page::source_provenance::NestedTextCodePosition<'_>>,
        text: &str,
        options: super::subset_text_edit::SubsetTextEditOptions<'_>,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        let selection = self.collect_expanded_text_span(page, root, range, budget)?;
        let tree = SpanInvocationTree::from_selection(&selection, &self.ctx.limits(), budget)?;
        let mapped = selection.map_to_tree(&tree, budget)?;
        let groups = mapped.group_events(budget)?;
        self.edit_transaction_with_budget(budget, |doc| {
            doc.commit_page_content_edit_inner(page, root, Some(budget), || {
                let mut prepared = doc.prepare_selected_span(page, root, &mapped, budget)?;
                let first_index = selection.events.items.iter().position(|event| event.first)
                    .ok_or_else(|| PdfError::ObjectError("subset span insertion absent".into()))?;
                let first_node = mapped.nodes.items[first_index];
                let first = &selection.events.items[first_index];
                let codes = first.codes.as_ref().ok_or_else(|| {
                    PdfError::ObjectError("subset span insertion requires a string".into())
                })?;
                let mut donors = SpanTable::new(tree.nodes.len(), budget)?;
                for node in 0..tree.nodes.len() {
                    let events = groups.for_node(node)?;
                    let mut node_donors = SpanTable::new(events.len(), budget)?;
                    for &index in events {
                        let event = &selection.events.items[index];
                        let fragment = if index == first_index { None } else {
                            event.codes.as_ref().map(|range| {
                                let program = if node == 0 { root } else { &*prepared.get(node).unwrap().program };
                                doc.build_text_event_fragments(program, range.clone(), "", options.preserve_advance, budget, |operation| {
                                    doc.span_text_metrics_for_operation(page, root, &tree, &prepared, node, operation, budget)
                                })
                            }).transpose()?
                        };
                        node_donors.items.push(fragment);
                    }
                    donors.items.push(node_donors);
                }
                // Every old-font metric is resolved before registration changes
                // document revisions. No stale snapshot is refreshed afterward.
                let metrics = doc.span_text_metrics_for_operation(
                    page, root, &tree, &prepared, first_node, first.operation, budget,
                )?;
                let (mut scope, scope_lease) = if first_node == 0 {
                    (None, budget.reserve(0)?)
                } else {
                    let (scope, lease) = crate::objects::clone_dict_with_budget(
                        &prepared.get(first_node).unwrap().resource_scope, budget, doc.ctx.limits(), 0,
                    )?;
                    (Some(scope), lease)
                };
                let program = if first_node == 0 { root } else { &*prepared.get(first_node).unwrap().program };
                let fragment = doc.build_scoped_subset_text_fragment(
                    page, program, first.operation, codes.start.array_item,
                    codes.end.array_item.map(|item| (item, codes.end.offset)),
                    codes.start.offset..codes.end.offset, text, options, metrics, scope.as_mut(), budget,
                )?;
                let head = super::text_event_edit::TextEventFragments::single(
                    fragment.index, fragment.fragment, fragment.operation_count,
                    fragment.input_chars, fragment.output_chars, budget,
                )?;
                let slot = groups.for_node(first_node)?.iter().position(|&index| index == first_index).unwrap();
                donors.items[first_node].items[slot] = Some(head);
                let mut plans = SpanTable::new(tree.nodes.len(), budget)?;
                for node in 0..tree.nodes.len() {
                    let program = if node == 0 { root } else { &*prepared.get(node).unwrap().program };
                    let donor = &mut donors.items[node];
                    plans.items.push(if donor.items.iter().any(Option::is_some) {
                        Some(doc.finish_text_event_fragments(program, &mut donor.items, budget)?)
                    } else { None });
                }
                if let Some(scope) = scope {
                    prepared.forms.items[first_node].as_mut().unwrap().resource_scope = scope;
                }
                let published = doc.build_prepared_span_publication_from_plans(
                    page, root, &tree, prepared, SpanPublicationPlans::Owned(plans), budget,
                )?;
                Ok(SpanSubsetPublication {
                    published, _font: fragment._font, _binding: fragment._binding, _scope: scope_lease,
                })
            })
        })
    }

    fn build_selected_span_plans(
        &self,
        page: u32,
        root: &ContentProgram,
        groups: &SpanEventGroups<'_, '_>,
        prepared: &SpanPreparedForms<'_>,
        text: &str,
        preserve_advance: bool,
        budget: &OperationBudget,
    ) -> PdfResult<SpanTable<Option<super::text_event_edit::TextEventEditPlan>>> {
        let mapped = groups.mapped;
        self.validate_span_selection(page, root, mapped, prepared, budget)?;
        let mut plans = SpanTable::new(mapped.tree.nodes.len(), budget)?;
        plans.items.resize_with(mapped.tree.nodes.len(), || None);
        for node in 0..mapped.tree.nodes.len() {
            let events = groups.for_node(node)?;
            if events.is_empty() {
                continue;
            }
            let program = if node == 0 {
                root
            } else {
                &*prepared
                    .get(node)
                    .ok_or_else(|| PdfError::ObjectError("selected Form not prepared".into()))?
                    .program
            };
            let mut donors = SpanTable::new(events.len(), budget)?;
            for &event in events {
                let event = &mapped.selection.events.items[event];
                let plan = event
                    .codes
                    .as_ref()
                    .map(|codes| {
                        self.build_text_event_fragments(
                            program,
                            codes.clone(),
                            if event.first { text } else { "" },
                            preserve_advance,
                            budget,
                            |index| {
                                self.span_text_metrics_for_operation(
                                    page,
                                    root,
                                    mapped.tree,
                                    prepared,
                                    node,
                                    index,
                                    budget,
                                )
                            },
                        )
                    })
                    .transpose()?;
                donors.items.push(plan);
            }
            let populated = donors.items.iter().filter(|plan| plan.is_some()).count();
            if populated > 0 {
                plans.items[node] =
                    Some(self.finish_text_event_fragments(program, &mut donors.items, budget)?);
            }
            check_plan_character_totals(&plans.items, self.ctx.limits().max_text_chars)?;
        }
        Ok(plans)
    }

    pub(super) fn replace_selected_text_span(
        &self,
        page: u32,
        root: &ContentProgram,
        range: std::ops::Range<crate::page::source_provenance::NestedTextCodePosition<'_>>,
        text: &str,
        preserve_advance: bool,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        let selection = self.collect_expanded_text_span(page, root, range, budget)?;
        let tree = SpanInvocationTree::from_selection(&selection, &self.ctx.limits(), budget)?;
        let mapped = selection.map_to_tree(&tree, budget)?;
        let prepared = self.prepare_selected_span(page, root, &mapped, budget)?;
        let groups = mapped.group_events(budget)?;
        let plans = self.build_selected_span_plans(
            page,
            root,
            &groups,
            &prepared,
            text,
            preserve_advance,
            budget,
        )?;
        self.commit_prepared_span(page, root, &tree, prepared, &plans.items, budget)
    }

    fn prepare_selected_span<'a>(
        &self,
        page: u32,
        root: &ContentProgram,
        mapped: &SpanMappedSelection<'a>,
        budget: &OperationBudget,
    ) -> PdfResult<SpanPreparedForms<'a>> {
        if mapped.selection.stamp.as_ref() != root.source_stamp()
            || mapped.selection.page_id != root.source_page_id
        {
            return Err(PdfError::ObjectError(
                "span selection owner/source mismatch".into(),
            ));
        }
        let original_page = self.get_page_with_budget(page, budget)?;
        if !original_page.source_provenance().matches_program(root) {
            return Err(PdfError::PageResourceError(
                "span requires current page snapshot".into(),
            ));
        }
        let prepared = self.prepare_span_nodes(&original_page, root, mapped.tree, budget)?;
        self.validate_span_selection(page, root, mapped, &prepared, budget)?;
        Ok(prepared)
    }

    fn validate_span_selection(
        &self,
        page: u32,
        root: &ContentProgram,
        mapped: &SpanMappedSelection<'_>,
        prepared: &SpanPreparedForms<'_>,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        if !std::ptr::eq(mapped.tree, prepared.tree)
            || mapped.selection.stamp.as_ref() != root.source_stamp()
            || mapped.selection.page_id != root.source_page_id
            || mapped.nodes.items.len() != mapped.selection.events.items.len()
        {
            return Err(PdfError::ObjectError(
                "span selection owner/source mismatch".into(),
            ));
        }
        let page = self.get_page_with_budget(page, budget)?;
        if !page.source_provenance().matches_program(root) {
            return Err(PdfError::PageResourceError(
                "span requires current page snapshot".into(),
            ));
        }
        for (event, &node) in mapped
            .selection
            .events
            .items
            .iter()
            .zip(&mapped.nodes.items)
        {
            let program = if node == 0 {
                root
            } else {
                &*prepared
                    .get(node)
                    .ok_or_else(|| PdfError::ObjectError("selected Form not prepared".into()))?
                    .program
            };
            let op = program
                .operations()
                .get(event.operation)
                .ok_or_else(|| PdfError::ObjectError("selected event index missing".into()))?;
            if op.operator.byte_offset() != event.byte_offset {
                return Err(PdfError::ObjectError(
                    "selected event byte position changed".into(),
                ));
            }
            let full = super::text_event_edit::full_event_range(program, event.operation)?;
            match (&event.codes, full) {
                (None, None) => (),
                (Some(codes), Some(full))
                    if codes.start >= full.start
                        && codes.end <= full.end
                        && codes.start <= codes.end =>
                {
                    super::text_event_edit::endpoint(program, codes.start)?;
                    super::text_event_edit::endpoint(program, codes.end)?;
                }
                _ => return Err(PdfError::ObjectError("selected code range changed".into())),
            }
        }
        Ok(())
    }

    /// Retain selected original descriptors, charging every path and table
    /// capacity. Programs remain in the authenticated snapshot/prepared tree,
    /// rather than being cloned once for every selected event.
    pub(super) fn collect_expanded_text_span(
        &self,
        page: u32,
        root: &ContentProgram,
        range: std::ops::Range<crate::page::source_provenance::NestedTextCodePosition<'_>>,
        budget: &OperationBudget,
    ) -> PdfResult<SpanSelection> {
        let mut count = 0usize;
        self.visit_expanded_text_code_ranges(
            page,
            root,
            range.clone(),
            budget,
            |_, _, _, _, _| {
                count = count
                    .checked_add(1)
                    .filter(|&n| n <= self.ctx.limits().max_objects)
                    .ok_or_else(|| PdfError::ResourceLimit("span selected events".into()))?;
                Ok(())
            },
        )?;
        let mut selection = SpanSelection {
            events: SpanTable::new(count, budget)?,
            stamp: root.source_stamp().cloned(),
            page_id: root.source_page_id,
        };
        self.visit_expanded_text_code_ranges(
            page,
            root,
            range,
            budget,
            |path, program, operation, codes, first| {
                if selection.events.items.len() >= count {
                    return Err(PdfError::ObjectError(
                        "span selection changed during collection".into(),
                    ));
                }
                let mut owned_path = SpanTable::new(path.len(), budget)?;
                owned_path.items.extend_from_slice(path);
                selection.events.items.push(SpanSelectedEvent {
                    path: owned_path,
                    operation,
                    byte_offset: program.operations()[operation].operator.byte_offset(),
                    codes,
                    first,
                });
                Ok(())
            },
        )?;
        if selection.events.items.len() != count {
            return Err(PdfError::ObjectError(
                "span selected event count changed".into(),
            ));
        }
        Ok(selection)
    }

    /// Expose per-event lexical ranges clipped to the original endpoints.
    /// None preserves a numeric-only/empty TJ; Some(empty) is a valid insertion
    /// or deletion boundary in an actual string. Font validation follows later.
    pub(super) fn visit_expanded_text_code_ranges<F>(
        &self,
        page: u32,
        root: &ContentProgram,
        range: std::ops::Range<crate::page::source_provenance::NestedTextCodePosition<'_>>,
        budget: &OperationBudget,
        mut visit: F,
    ) -> PdfResult<()>
    where
        F: FnMut(
            &[FormInvocation],
            &ContentProgram,
            usize,
            Option<std::ops::Range<crate::page::content_program::TextCodePosition>>,
            bool,
        ) -> PdfResult<()>,
    {
        let position = |endpoint: crate::page::source_provenance::NestedTextCodePosition<'_>| {
            crate::page::content_program::TextCodePosition {
                operation: endpoint.source.operation_index,
                array_item: endpoint.array_item,
                offset: endpoint.offset,
            }
        };
        let start = position(range.start);
        let end = position(range.end);
        self.visit_expanded_text_span(
            page,
            root,
            range,
            budget,
            |path, program, index, first, last| {
                let mut codes = super::text_event_edit::full_event_range(program, index)?;
                if let Some(codes) = codes.as_mut() {
                    if first {
                        codes.start = start;
                    }
                    if last {
                        codes.end = end;
                    }
                    super::text_event_edit::endpoint(program, codes.start)?;
                    super::text_event_edit::endpoint(program, codes.end)?;
                    if codes.start > codes.end {
                        return Err(PdfError::ObjectError(
                            "clipped text event is reversed".into(),
                        ));
                    }
                } else if first || last {
                    return Err(PdfError::ObjectError(
                        "span endpoint requires a string".into(),
                    ));
                }
                visit(path, program, index, codes, first)
            },
        )
    }

    /// Validate both lexical endpoints before exposing the inclusive event span.
    /// Byte offsets are validated here; font-specific code boundaries remain the
    /// edit planner's responsibility. This does not publish or copy any streams.
    pub(super) fn visit_expanded_text_span<F>(
        &self,
        page: u32,
        root: &ContentProgram,
        range: std::ops::Range<crate::page::source_provenance::NestedTextCodePosition<'_>>,
        budget: &OperationBudget,
        mut visit: F,
    ) -> PdfResult<()>
    where
        F: FnMut(&[FormInvocation], &ContentProgram, usize, bool, bool) -> PdfResult<()>,
    {
        let endpoints = [range.start, range.end];
        for endpoint in endpoints {
            // Expanded selections can start/end in the page root. Do not
            // relax the nonempty-chain contract of ordinary nested edits.
            if !endpoint.source.invocations.is_empty() {
                endpoint.source.validate_limits(&self.ctx.limits())?;
            }
            if !endpoint.source.geometry_operation_indices.is_empty() {
                return Err(PdfError::ObjectError(
                    "span endpoint has path geometry".into(),
                ));
            }
        }
        let mut ordinals = [None, None];
        let mut ordinal = 0;
        self.visit_expanded_text_events(page, root, budget, |path, program, index| {
            for (slot, endpoint) in endpoints.iter().enumerate() {
                if path == endpoint.source.invocations && index == endpoint.source.operation_index {
                    if program.operations()[index].operator.byte_offset()
                        != endpoint.source.operator_byte_offset
                    {
                        return Err(PdfError::ObjectError(
                            "span endpoint byte position mismatch".into(),
                        ));
                    }
                    super::text_event_edit::endpoint(
                        program,
                        crate::page::content_program::TextCodePosition {
                            operation: index,
                            array_item: endpoint.array_item,
                            offset: endpoint.offset,
                        },
                    )?;
                    ordinals[slot] = Some(ordinal);
                }
            }
            ordinal += 1;
            Ok(())
        })?;
        let start =
            ordinals[0].ok_or_else(|| PdfError::ObjectError("span start not found".into()))?;
        let end = ordinals[1].ok_or_else(|| PdfError::ObjectError("span end not found".into()))?;
        if start > end
            || (start == end
                && (range.start.array_item, range.start.offset)
                    > (range.end.array_item, range.end.offset))
        {
            return Err(PdfError::ObjectError(
                "expanded text span is reversed".into(),
            ));
        }
        ordinal = 0;
        // Two bounded passes avoid allocating an event/path copy for every show.
        // Callbacks may have external side effects; those are not rolled back.
        self.visit_expanded_text_events(page, root, budget, |path, program, index| {
            let current = ordinal;
            ordinal += 1;
            if (start..=end).contains(&current) {
                visit(path, program, index, current == start, current == end)?;
            }
            Ok(())
        })
    }

    /// Visit every lexical text-show in expanded original call order, including
    /// shows producing no character events. Callback references are ephemeral.
    pub(super) fn visit_expanded_text_events<F>(
        &self,
        page: u32,
        root: &ContentProgram,
        budget: &OperationBudget,
        mut visit: F,
    ) -> PdfResult<()>
    where
        F: FnMut(&[FormInvocation], &ContentProgram, usize) -> PdfResult<()>,
    {
        let page = self.get_page_with_budget(page, budget)?;
        if !page.source_provenance().matches_program(root) {
            return Err(PdfError::PageResourceError(
                "span requires current page snapshot".into(),
            ));
        }
        let limits = self.ctx.limits();
        let (scope, _scope_lease) = page.resource_with_budget(budget)?;
        let mut path = SpanTable::new(limits.max_depth.min(limits.max_objects), budget)?;
        // Each frame owns its prepared stream and resource leases. No references
        // into the stack survive a push, and dropping the stack also unwinds an
        // interrupted callback without recursive Rust calls.
        struct Frame {
            form: crate::page::source_provenance::PreparedForm,
            next: usize,
        }
        let mut frames: SpanTable<Frame> =
            SpanTable::new(limits.max_depth.min(limits.max_objects), budget)?;
        let mut root_next = 0;
        let mut work = (0usize, 0usize, 0usize, 0usize);
        fn charge(
            program: &ContentProgram,
            work: &mut (usize, usize, usize, usize),
            limits: Limits,
        ) -> PdfResult<()> {
            work.0 = work
                .0
                .checked_add(1)
                .filter(|&n| n <= limits.max_objects)
                .ok_or_else(|| PdfError::ResourceLimit("span expanded nodes".into()))?;
            work.1 = work
                .1
                .checked_add(program.source_bytes().len())
                .filter(|&n| n <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("span expanded decoded work".into()))?;
            work.2 = work
                .2
                .checked_add(program.operations().len())
                .filter(|&n| n <= limits.max_content_operations)
                .ok_or_else(|| PdfError::ResourceLimit("span expanded operations".into()))?;
            Ok(())
        }
        charge(root, &mut work, limits)?;
        loop {
            let (program, current_scope, next) = match frames.items.last_mut() {
                Some(frame) => (
                    &*frame.form.program,
                    &frame.form.resource_scope,
                    &mut frame.next,
                ),
                None => (root, &scope, &mut root_next),
            };
            if *next == program.operations().len() {
                if frames.items.pop().is_none() {
                    break;
                }
                path.items.pop();
                continue;
            }
            let index = *next;
            *next += 1;
            let operation = &program.operations()[index];
            let op = &operation.operator;
            if matches!(op.name(), "Tj" | "TJ" | "'" | "\"") {
                match (op.name(), op.num_operands()) {
                    ("Tj" | "'", 1) => {
                        op.get_as_string(0)?;
                    }
                    ("\"", 3) => {
                        op.get_as_f32(0)?;
                        op.get_as_f32(1)?;
                        op.get_as_string(2)?;
                    }
                    ("TJ", 1) => {
                        for item in op.get_as_array(0)? {
                            if !matches!(
                                item,
                                crate::objects::PdfObject::PdfString(_)
                                    | crate::objects::PdfObject::PdfNumber(_)
                            ) {
                                return Err(PdfError::ObjectError("span TJ item type".into()));
                            }
                        }
                    }
                    _ => return Err(PdfError::ObjectError("span text operand count".into())),
                }
                work.3 = work
                    .3
                    .checked_add(1)
                    .filter(|&n| n <= limits.max_objects)
                    .ok_or_else(|| PdfError::ResourceLimit("span expanded text events".into()))?;
                visit(&path.items, program, index)?;
            } else if op.name() == "Do" {
                if let Some(call) =
                    page.form_call_at_operation_with_budget(program, index, current_scope, budget)?
                {
                    if path.items.len() >= limits.max_depth.min(limits.max_objects) {
                        return Err(PdfError::ResourceLimit("span expanded depth".into()));
                    }
                    if call.form_id.is_some()
                        && path
                            .items
                            .iter()
                            .any(|parent| parent.form_id == call.form_id)
                    {
                        return Err(PdfError::PageResourceError("Nested Form cycle".into()));
                    }
                    let form = page.prepare_form_call_with_budget(
                        program,
                        current_scope,
                        &scope,
                        &call,
                        budget,
                    )?;
                    charge(&form.program, &mut work, limits)?;
                    path.items.push(call);
                    frames.items.push(Frame { form, next: 0 });
                }
            }
        }
        Ok(())
    }

    /// Reconstruct metrics from original caller prefixes, without cloning fonts.
    /// Ancestor lookup rescans the path; no linear-time complexity is claimed.
    pub(super) fn span_text_metrics_for_operation(
        &self,
        page: u32,
        root: &ContentProgram,
        tree: &SpanInvocationTree,
        prepared: &SpanPreparedForms<'_>,
        node: usize,
        target: usize,
        budget: &OperationBudget,
    ) -> PdfResult<crate::page::font_binding::BudgetedTextMetrics> {
        if !std::ptr::eq(tree, prepared.tree) || node >= tree.nodes.len() {
            return Err(PdfError::ObjectError(
                "span metrics tree/node mismatch".into(),
            ));
        }
        let page = self.get_page_with_budget(page, budget)?;
        if !page.source_provenance().matches_program(root) {
            return Err(PdfError::PageResourceError(
                "span requires current page snapshot".into(),
            ));
        }
        let (page_scope, _scope_lease) = page.resource_with_budget(budget)?;
        let mut state = Default::default();
        let depth = tree.depth(node);
        for level in 1..=depth {
            let mut child = node;
            for _ in level..depth {
                child = tree.nodes[child].parent.unwrap();
            }
            let parent = tree.nodes[child].parent.unwrap();
            let (program, scope) = if parent == 0 {
                (root, &page_scope)
            } else {
                let form = prepared
                    .get(parent)
                    .ok_or_else(|| PdfError::ObjectError("span metrics parent absent".into()))?;
                (&*form.program, &form.resource_scope)
            };
            state = page.text_state_before_operation_in_scope_with_budget(
                program,
                tree.nodes[child].invocation.unwrap().operation_index,
                scope,
                state,
                budget,
            )?;
        }
        let (program, scope) = if node == 0 {
            (root, &page_scope)
        } else {
            let form = prepared
                .get(node)
                .ok_or_else(|| PdfError::ObjectError("span metrics leaf absent".into()))?;
            (&*form.program, &form.resource_scope)
        };
        page.text_state_before_operation_in_scope_with_budget(
            program, target, scope, state, budget,
        )?
        .into_metrics()
    }

    /// Prepare selected text-source nodes once per invocation, not once per leaf.
    pub(super) fn prepare_span_forms<'a>(
        &self,
        page: u32,
        root: &ContentProgram,
        tree: &'a SpanInvocationTree,
        sources: &[&NestedDrawSource],
        budget: &OperationBudget,
    ) -> PdfResult<SpanPreparedForms<'a>> {
        let page = self.get_page_with_budget(page, budget)?;
        if !page.source_provenance().matches_program(root) {
            return Err(PdfError::PageResourceError(
                "span requires current page snapshot".into(),
            ));
        }
        if sources.len() != tree.leaves.len() {
            return Err(PdfError::ObjectError(
                "span source table size mismatch".into(),
            ));
        }
        let limits = self.ctx.limits();
        for (slot, source) in sources.iter().enumerate() {
            source.validate_limits(&limits)?;
            if !source.geometry_operation_indices.is_empty() {
                return Err(PdfError::UnsupportedFeature(
                    "span text source has path geometry".into(),
                ));
            }
            let mut node = tree.leaf(slot)?;
            for call in source.invocations.iter().rev() {
                if tree.nodes[node].invocation.as_ref() != Some(call) {
                    return Err(PdfError::ObjectError(
                        "span original source path mismatch".into(),
                    ));
                }
                node = tree.nodes[node]
                    .parent
                    .ok_or_else(|| PdfError::ObjectError("span source depth mismatch".into()))?;
            }
            if node != 0 {
                return Err(PdfError::ObjectError("span source depth mismatch".into()));
            }
        }
        let prepared = self.prepare_span_nodes(&page, root, tree, budget)?;
        for (slot, source) in sources.iter().enumerate() {
            let form = prepared.get(tree.leaf(slot)?).unwrap();
            let op = form
                .program
                .operations()
                .get(source.operation_index)
                .ok_or_else(|| PdfError::PageResourceError("Nested leaf index missing".into()))?;
            if op.operator.byte_offset() != source.operator_byte_offset {
                return Err(PdfError::PageResourceError(
                    "Nested leaf byte position mismatch".into(),
                ));
            }
            if !matches!(op.operator.name(), "Tj" | "TJ" | "'" | "\"") {
                return Err(PdfError::UnsupportedFeature(
                    "span endpoint is not text".into(),
                ));
            }
        }
        Ok(prepared)
    }

    fn prepare_span_nodes<'a>(
        &self,
        page: &crate::page::PdfPage<'_, R>,
        root: &ContentProgram,
        tree: &'a SpanInvocationTree,
        budget: &OperationBudget,
    ) -> PdfResult<SpanPreparedForms<'a>> {
        let limits = self.ctx.limits();
        let (page_scope, _scope_lease) = page.resource_with_budget(budget)?;
        let mut prepared = SpanPreparedForms {
            tree,
            forms: SpanTable::new(tree.nodes.len(), budget)?,
        };
        prepared.forms.items.resize_with(tree.nodes.len(), || None);
        let mut bytes = root.source_bytes().len();
        let mut operations = root.operations().len();
        if bytes > limits.max_decoded_bytes || operations > limits.max_content_operations {
            return Err(PdfError::ResourceLimit("span original root work".into()));
        }
        for (index, node) in tree.nodes.iter().enumerate().skip(1) {
            let call = node.invocation.as_ref().unwrap();
            let parent = node.parent.unwrap();
            let mut ancestor = parent;
            while ancestor != 0 {
                if call.form_id.is_some()
                    && tree.nodes[ancestor].invocation.unwrap().form_id == call.form_id
                {
                    return Err(PdfError::PageResourceError("Nested Form cycle".into()));
                }
                ancestor = tree.nodes[ancestor].parent.unwrap();
            }
            let (original, scope) = if parent == 0 {
                (root, &page_scope)
            } else {
                let form = prepared
                    .get(parent)
                    .ok_or_else(|| PdfError::ObjectError("span parent not prepared".into()))?;
                (&*form.program, &form.resource_scope)
            };
            let form =
                page.prepare_form_call_with_budget(original, scope, &page_scope, call, budget)?;
            bytes = bytes
                .checked_add(form.program.source_bytes().len())
                .filter(|&count| count <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("span original decoded work".into()))?;
            operations = operations
                .checked_add(form.program.operations().len())
                .filter(|&count| count <= limits.max_content_operations)
                .ok_or_else(|| PdfError::ResourceLimit("span original operations".into()))?;
            prepared.forms.items[index] = Some(form);
        }
        Ok(prepared)
    }

    /// Internal multi-branch publication; callers must prepare every selected
    /// text plan and its original scope before entering this single transaction.
    pub(super) fn commit_prepared_span(
        &self,
        page: u32,
        root: &ContentProgram,
        tree: &SpanInvocationTree,
        prepared: SpanPreparedForms<'_>,
        plans: &[Option<super::text_event_edit::TextEventEditPlan>],
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        if plans.len() != tree.nodes.len() {
            return Err(PdfError::ObjectError(
                "span text plan table size mismatch".into(),
            ));
        }
        check_plan_character_totals(plans, self.ctx.limits().max_text_chars)?;
        self.edit_transaction_with_budget(budget, |doc| {
            doc.commit_page_content_edit_inner(page, root, Some(budget), || {
                doc.build_prepared_span_publication(page, root, tree, prepared, plans, budget)
            })
        })
    }

    /// Called only inside an authenticated page-edit transaction. Separating
    /// publication lets subset callers register fonts after authentication and
    /// retain their font/resource owners across this same bottom-up build.
    fn build_prepared_span_publication<'a>(
        &self,
        page: u32,
        root: &'a ContentProgram,
        tree: &SpanInvocationTree,
        prepared: SpanPreparedForms<'_>,
        plans: &'a [Option<super::text_event_edit::TextEventEditPlan>],
        budget: &OperationBudget,
    ) -> PdfResult<SpanBuild<SpanPublishedNode<'a>>> {
        self.build_prepared_span_publication_from_plans(
            page, root, tree, prepared, SpanPublicationPlans::Borrowed(plans), budget,
        )
    }

    fn build_prepared_span_publication_from_plans<'a>(
        &self,
        page: u32,
        root: &'a ContentProgram,
        tree: &SpanInvocationTree,
        prepared: SpanPreparedForms<'_>,
        mut plans: SpanPublicationPlans<'a>,
        budget: &OperationBudget,
    ) -> PdfResult<SpanBuild<SpanPublishedNode<'a>>> {
                let doc = self;
                if plans.as_slice().len() != tree.nodes.len() {
                    return Err(PdfError::ObjectError("span text plan table size mismatch".into()));
                }
                check_plan_character_totals(plans.as_slice(), self.ctx.limits().max_text_chars)?;
                let mut bytes = 0usize;
                tree.build_prepared_bottom_up::<SpanPublishedNode, _>(
                    prepared,
                    budget,
                    |index, _, mut form, results| {
                        let mut bindings = budget.reserve(0)?;
                        let (original, mut scope) = match form.as_mut() {
                            Some(form) => (&*form.program, Some(&mut form.resource_scope)),
                            None => (root, None),
                        };
                        let bind = |child: usize| {
                            let id = results[child]
                                .as_ref()
                                .and_then(|node| node.id)
                                .ok_or_else(|| {
                                    PdfError::ObjectError("span child not published".into())
                                })?;
                            let (name, mut lease) = if let Some(scope) = scope.as_deref_mut() {
                                doc.bind_local_xobject_with_budget(scope, id, budget)?
                            } else {
                                doc.resources()
                                    .register_form_with_budget(page, id, budget)?
                            };
                            bindings.absorb(&mut lease)?;
                            super::nested_edit::form_do_with_budget(&name, doc.ctx.limits(), budget)
                        };
                        let program = if let Some(plan) = plans.take(index) {
                            if !tree.nodes.iter().any(|node| node.parent == Some(index)) {
                                // No child bindings can alter original indices.
                                // Reuse the validated plan while its byte/fragment
                                // owners and leases remain in the caller table.
                                plan
                            } else {
                                let text_plan = match &plan {
                                    SpanNodeProgram::Planned(plan) => *plan,
                                    SpanNodeProgram::OwnedPlanned(plan) => plan,
                                    SpanNodeProgram::Rebuilt(_) => unreachable!("plan table contains only text plans"),
                                };
                                SpanNodeProgram::Rebuilt(tree.rebuild_parent_with_text_plan(
                                    index,
                                    original,
                                    text_plan,
                                    doc.ctx.options(),
                                    budget,
                                    bind,
                                )?)
                            }
                        } else {
                            SpanNodeProgram::Rebuilt(tree.rebuild_parent(
                                index, original, doc.ctx.options(), budget, bind,
                            )?)
                        };
                        bytes = bytes
                            .checked_add(program.as_ref().source_bytes().len())
                            .filter(|&total| total <= doc.ctx.limits().max_decoded_bytes)
                            .ok_or_else(|| {
                                PdfError::ResourceLimit("span aggregate edited bytes".into())
                            })?;
                        if let Some(form) = form {
                            let output =
                                SpanFormOutput::new(form, program, budget, doc.ctx.limits())?;
                            let (stream, retained) = output.into_parts();
                            let id = doc
                                .try_insert_object(crate::objects::PdfObject::PdfStream(stream))?;
                            Ok(SpanPublishedNode {
                                id: Some(id),
                                root_program: None,
                                _retained: Some(retained),
                                _bindings: bindings,
                            })
                        } else {
                            Ok(SpanPublishedNode {
                                id: None,
                                root_program: Some(program),
                                _retained: None,
                                _bindings: bindings,
                            })
                        }
                    },
                )
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
    /// Consume validated form owners in topological order. The callback receives
    /// ownership (including dictionary leases) and must retain it in its result
    /// if the prepared node is needed through later publication. Root has no Form.
    pub fn build_prepared_bottom_up<T, F>(
        &self,
        mut prepared: SpanPreparedForms<'_>,
        budget: &OperationBudget,
        mut build: F,
    ) -> PdfResult<SpanBuild<T>>
    where
        F: FnMut(
            usize,
            &SpanNode,
            Option<crate::page::source_provenance::PreparedForm>,
            &[Option<T>],
        ) -> PdfResult<T>,
    {
        if !std::ptr::eq(prepared.tree, self) {
            return Err(PdfError::ObjectError("span prepared tree mismatch".into()));
        }
        if prepared.forms.items.len() != self.nodes.len() {
            return Err(PdfError::ObjectError(
                "span prepared table size mismatch".into(),
            ));
        }
        self.build_bottom_up(budget, |index, node, results| {
            let form = prepared.forms.items[index].take();
            if (index == 0) != form.is_none() {
                return Err(PdfError::ObjectError("span prepared node absent".into()));
            }
            build(index, node, form, results)
        })
    }

    /// Validate each selected source using the page's real chain preparer, then
    /// move its owners into unique nodes. Shared ancestors are retained once,
    /// although the existing chain preparer temporarily copies shared prefixes.
    pub fn prepare_forms<F>(
        &self,
        budget: &OperationBudget,
        mut prepare: F,
    ) -> PdfResult<SpanPreparedForms<'_>>
    where
        F: FnMut(usize) -> PdfResult<crate::page::source_provenance::BudgetedFormChain>,
    {
        let mut prepared = SpanPreparedForms {
            forms: SpanTable::new(self.nodes.len(), budget)?,
            tree: self,
        };
        prepared.forms.items.resize_with(self.nodes.len(), || None);
        for (slot, &leaf) in self.leaves.iter().enumerate() {
            let chain = prepare(slot)?;
            let depth = self.depth(leaf);
            if chain.len() != depth {
                return Err(PdfError::ObjectError(
                    "span prepared chain depth mismatch".into(),
                ));
            }
            let mut node = leaf;
            for form in chain.into_iter().rev() {
                if let Some(existing) = prepared.forms.items[node].as_ref() {
                    if existing.program.source_bytes() != form.program.source_bytes()
                        || existing.dictionary != form.dictionary
                        || existing.resource_scope != form.resource_scope
                    {
                        return Err(PdfError::ObjectError(
                            "span prepared program mismatch".into(),
                        ));
                    }
                } else {
                    prepared.forms.items[node] = Some(form);
                }
                node = self.nodes[node].parent.unwrap();
            }
        }
        Ok(prepared)
    }

    /// Borrow real text-plan fragments; never merge an already rewritten program
    /// using indices from the original parent. The caller retains the plan owner.
    pub fn rebuild_parent_with_text_plan<F>(
        &self,
        parent: usize,
        original: &ContentProgram,
        plan: &super::text_event_edit::TextEventEditPlan,
        options: ParseOptions,
        budget: &OperationBudget,
        bind: F,
    ) -> PdfResult<SpanParentEdit>
    where
        F: FnMut(usize) -> PdfResult<BudgetedBytes>,
    {
        let fragments = plan.original_replacements();
        let mut borrowed = SpanTable::new(fragments.len(), budget)?;
        borrowed.items.extend(fragments);
        self.rebuild_parent_with_replacements(
            parent,
            original,
            &borrowed.items,
            options,
            budget,
            bind,
        )
    }

    /// Rebuild all selected child Do operations in one original parent program.
    /// The callback must prepare bindings, not publish changes to a document.
    pub fn rebuild_parent<F>(
        &self,
        parent: usize,
        original: &ContentProgram,
        options: ParseOptions,
        budget: &OperationBudget,
        bind: F,
    ) -> PdfResult<SpanParentEdit>
    where
        F: FnMut(usize) -> PdfResult<BudgetedBytes>,
    {
        self.rebuild_parent_with_replacements(parent, original, &[], options, budget, bind)
    }

    /// Merge local edits and child bindings against the same original indices.
    /// Borrowed local fragments must remain alive through this preparation call.
    pub fn rebuild_parent_with_replacements<F>(
        &self,
        parent: usize,
        original: &ContentProgram,
        local: &[OperationReplacement<'_>],
        options: ParseOptions,
        budget: &OperationBudget,
        mut bind: F,
    ) -> PdfResult<SpanParentEdit>
    where
        F: FnMut(usize) -> PdfResult<BudgetedBytes>,
    {
        let children = self.children_in_source_order(parent, budget)?;
        let count = children
            .indices()
            .len()
            .checked_add(local.len())
            .ok_or_else(|| PdfError::ResourceLimit("span replacement count overflow".into()))?;
        if count > options.limits.max_objects {
            return Err(PdfError::ResourceLimit("span replacement count".into()));
        }
        for (position, edit) in local.iter().enumerate() {
            if edit.index >= original.operations().len()
                || (position > 0 && local[position - 1].index >= edit.index)
                || children.indices().iter().any(|&child| {
                    self.nodes[child].invocation.unwrap().operation_index == edit.index
                })
            {
                return Err(PdfError::ObjectError(
                    "span local replacement conflict".into(),
                ));
            }
        }
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
        let mut replacements = SpanTable::new(count, budget)?;
        for edit in local {
            replacements.items.push(OperationReplacement {
                index: edit.index,
                source: edit.source,
                operation_count: edit.operation_count,
            });
        }
        for (&child, fragment) in children.indices().iter().zip(&fragments.items) {
            replacements.items.push(OperationReplacement {
                index: self.nodes[child].invocation.unwrap().operation_index,
                source: fragment.as_slice(),
                operation_count: 1,
            });
        }
        replacements.items.sort_unstable_by_key(|edit| edit.index);
        let program = original.with_replaced_operation_sequences_with_budget(
            &replacements.items,
            options,
            budget,
        )?;
        for &child in children.indices() {
            let original_index = self.nodes[child].invocation.unwrap().operation_index;
            let mut final_index = original_index;
            for edit in replacements
                .items
                .iter()
                .take_while(|edit| edit.index < original_index)
            {
                final_index = final_index
                    .checked_sub(1)
                    .and_then(|index| index.checked_add(edit.operation_count))
                    .ok_or_else(|| PdfError::ResourceLimit("span rebuilt index overflow".into()))?;
            }
            let operation = &program.as_ref().operations()[final_index].operator;
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
        Self::from_paths(
            sources.len(),
            |index| sources[index].invocations.as_slice(),
            limits,
            budget,
        )
    }

    fn from_selection(
        selection: &SpanSelection,
        limits: &Limits,
        budget: &OperationBudget,
    ) -> PdfResult<Self> {
        Self::from_paths(
            selection.events.items.len(),
            |index| selection.events.items[index].path.items.as_slice(),
            limits,
            budget,
        )
    }

    fn from_paths<'a, F>(
        source_count: usize,
        path: F,
        limits: &Limits,
        budget: &OperationBudget,
    ) -> PdfResult<Self>
    where
        F: Fn(usize) -> &'a [FormInvocation],
    {
        if source_count > limits.max_objects {
            return Err(PdfError::ResourceLimit("span source count".into()));
        }
        for index in 0..source_count {
            if path(index).len() > limits.max_depth.min(limits.max_objects) {
                return Err(PdfError::ResourceLimit("span invocation depth".into()));
            }
        }
        if limits.max_objects == 0 {
            return Err(PdfError::ResourceLimit("span node count".into()));
        }
        // Index each unique call site once. Compare the complete provenance on
        // a hit: equal operation indices must not conceal identity/offset changes.
        // The allocator charges real hash-table layouts including growth overlap.
        type PrefixTable = hashbrown::HashMap<
            (usize, usize), (usize, FormInvocation),
            std::collections::hash_map::RandomState,
            crate::budget::table_allocator::TableAllocator,
        >;
        let mut prefixes = PrefixTable::with_hasher_in(
            Default::default(), crate::budget::table_allocator::TableAllocator::new(budget),
        );
        let (mut leaves, leaves_lease) = table(source_count, budget)?;
        let mut previous: Option<(&[FormInvocation], usize)> = None;
        for index in 0..source_count {
            let calls = path(index);
            // Expanded events commonly share consecutive invocation paths.
            // Borrow the caller's complete provenance; no new cache allocation
            // and no identity/byte-offset checks omitted on this shortcut.
            if let Some((original, leaf)) = previous {
                if original == calls {
                    leaves.push(leaf);
                    continue;
                }
            }
            let mut parent = 0;
            for invocation in calls {
                let key = (parent, invocation.operation_index);
                parent = if let Some(&(node, original)) = prefixes.get(&key) {
                    if original != *invocation {
                        return Err(PdfError::ObjectError(
                            "conflicting original span invocation".into(),
                        ));
                    }
                    node
                } else {
                    let node = prefixes.len().checked_add(1).ok_or_else(|| {
                        PdfError::ResourceLimit("span node count overflow".into())
                    })?;
                    if node >= limits.max_objects {
                        return Err(PdfError::ResourceLimit("span node count".into()));
                    }
                    prefixes.try_reserve(1).map_err(|_| {
                        PdfError::ResourceLimit("span prefix table allocation".into())
                    })?;
                    prefixes.insert(key, (node, *invocation));
                    node
                };
            }
            leaves.push(parent);
            previous = Some((calls, parent));
        }
        let (mut nodes, nodes_lease) = table(prefixes.len() + 1, budget)?;
        nodes.resize(prefixes.len() + 1, SpanNode { parent: None, invocation: None });
        for (&(parent, _), &(node, invocation)) in &prefixes {
            nodes[node] = SpanNode { parent: Some(parent), invocation: Some(invocation) };
        }
        // Stable first-seen node IDs preserve original leaves/topological order;
        // hash iteration does not determine arena order. Release scratch storage.
        drop(prefixes);
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
    #[test]
    fn owned_publication_plan_keeps_program_budget_after_table_is_dropped() {
        let doc = super::super::PdfDocument::new().unwrap();
        let pool = OperationBudget::new(1024 * 1024);
        let original = ContentProgram::from_decoded_bytes(
            b"(A) Tj".to_vec(), doc.ctx.options(),
        ).unwrap();
        let fragment = super::super::text_event_edit::TextEventFragments::single(
            0, crate::budget::BudgetedBytes::copy_from_slice(&pool, b"(B) Tj").unwrap(),
            1, 1, 1, &pool,
        ).unwrap();
        let plan = doc.finish_text_event_fragments(
            &original, &mut [Some(fragment)], &pool,
        ).unwrap();
        let mut table = SpanTable::new(1, &pool).unwrap();
        table.items.push(Some(plan));
        let mut plans = SpanPublicationPlans::Owned(table);
        assert_eq!(check_plan_character_totals(plans.as_slice(), 1).unwrap(), (1, 1));
        let program = plans.take(0).unwrap();
        assert!(matches!(program, SpanNodeProgram::OwnedPlanned(_)));
        assert!(plans.take(0).is_none());
        drop(plans);
        assert!(pool.live_bytes() > 0);
        assert_eq!(program.as_ref().source_bytes(), b"\n(B) Tj\n");
        drop(program);
        assert_eq!(pool.live_bytes(), 0);
    }

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
    fn selected_mapping_leaf_hints_do_not_replace_provenance_checks() {
        let owners = OperationBudget::new(1024 * 1024);
        let mut selection = SpanSelection {
            events: SpanTable::new(2, &owners).unwrap(), stamp: None, page_id: None,
        };
        for index in 0..2 {
            let original = source(&[1, index + 2]);
            let mut path = SpanTable::new(2, &owners).unwrap();
            path.items.extend_from_slice(&original.invocations);
            selection.events.items.push(SpanSelectedEvent {
                path, operation: index, byte_offset: Some(0), codes: None, first: index == 0,
            });
        }
        for mutation in 0..4 {
            let mut tree = SpanInvocationTree::from_selection(&selection, &Default::default(), &owners).unwrap();
            let expected = tree.leaves.clone();
            match mutation {
                0 => tree.leaves.swap(0, 1),
                1 => tree.leaves[0] = usize::MAX,
                2 => tree.nodes[expected[0]].invocation.as_mut().unwrap().operator_byte_offset = Some(999),
                3 => tree.nodes[expected[0]].parent = Some(expected[0]),
                _ => unreachable!(),
            }
            let pool = OperationBudget::new(4096);
            let neighbor = pool.reserve(7).unwrap();
            let result = selection.map_to_tree(&tree, &pool);
            if mutation < 2 {
                assert_eq!(result.as_ref().unwrap().nodes.items, expected);
            } else {
                assert!(matches!(result, Err(PdfError::ObjectError(ref message))
                    if message == "selected invocation absent from tree"));
            }
            drop(result);
            assert_eq!(pool.live_bytes(), 7);
            drop(neighbor);
            assert_eq!(pool.live_bytes(), 0);
        }
        drop(selection);
        assert_eq!(owners.live_bytes(), 0);
    }

    #[test]
    fn selected_mapping_wide_and_repeated_measurement_baseline() {
        for width in [1usize, 128, 1024] {
            let owners = OperationBudget::new(16 * 1024 * 1024);
            let count = width * 8;
            let mut selection = SpanSelection {
                events: SpanTable::new(count, &owners).unwrap(), stamp: None, page_id: None,
            };
            for index in 0..count {
                let original = source(&[1, index % width + 2]);
                let mut path = SpanTable::new(2, &owners).unwrap();
                path.items.extend_from_slice(&original.invocations);
                selection.events.items.push(SpanSelectedEvent {
                    path, operation: index, byte_offset: Some(index as u64), codes: None,
                    first: index == 0,
                });
            }
            let tree = SpanInvocationTree::from_selection(&selection, &Default::default(), &owners).unwrap();
            assert_eq!(tree.nodes.len(), width + 2);
            let retained = owners.live_bytes();
            let pool = OperationBudget::new(1024 * 1024);
            let neighbor = pool.reserve(7).unwrap();
            let started = std::time::Instant::now();
            let mapped = selection.map_to_tree(&tree, &pool).unwrap();
            let elapsed = started.elapsed();
            assert_eq!(mapped.nodes.items, tree.leaves);
            let actual = pool.live_bytes() - 7;
            assert!(actual >= count * std::mem::size_of::<usize>());
            println!("mapping-only width={width} events={count} elapsed_us={} tracked_peak={} retained={actual}",
                elapsed.as_micros(), pool.peak_bytes());
            drop(mapped);
            assert_eq!(pool.live_bytes(), 7);
            assert_eq!(owners.live_bytes(), retained);
            let tight = OperationBudget::new(actual);
            let overlap = tight.reserve(1).unwrap();
            assert!(selection.map_to_tree(&tree, &tight).is_err());
            assert_eq!(tight.live_bytes(), 1);
            drop(overlap);
            drop(selection.map_to_tree(&tree, &tight).unwrap());
            assert_eq!(tight.live_bytes(), 0);
            drop(neighbor);
            assert_eq!(pool.live_bytes(), 0);
            drop(tree);
            drop(selection);
            assert_eq!(owners.live_bytes(), 0);
        }
    }

    #[test]
    fn invocation_tree_repeated_and_wide_path_measurement_baseline() {
        for wide in [false, true] {
            for count in [16, 128, 512] {
                // Input provenance is held by the caller and deliberately not
                // included in this tree-only operation budget or elapsed time.
                let sources: Vec<_> = (0..count)
                    .map(|index| source(&[1, if wide { index + 2 } else { 2 }]))
                    .collect();
                let borrowed: Vec<_> = sources.iter().collect();
                let limits = Limits { max_objects: count + 2, ..Default::default() };
                let pool = OperationBudget::new(1024 * 1024);
                let started = std::time::Instant::now();
                let tree = SpanInvocationTree::new(&borrowed, &limits, &pool).unwrap();
                let elapsed = started.elapsed();
                assert_eq!(tree.nodes.len(), if wide { count + 2 } else { 3 });
                assert_eq!(tree.leaves.len(), count);
                for (index, &leaf) in tree.leaves.iter().enumerate() {
                    assert_eq!(tree.nodes[leaf].parent, Some(1));
                    assert_eq!(tree.nodes[leaf].invocation.unwrap().operation_index,
                        if wide { index + 2 } else { 2 });
                }
                println!("tree-only wide={wide} sources={count} nodes={} elapsed_us={} tracked_peak={}",
                    tree.nodes.len(), elapsed.as_micros(), pool.peak_bytes());
                drop(tree);
                assert_eq!(pool.live_bytes(), 0);
                if wide {
                    let rejected = SpanInvocationTree::new(&borrowed,
                        &Limits { max_objects: count + 1, ..limits }, &pool);
                    assert!(matches!(rejected,
                        Err(PdfError::ResourceLimit(ref message)) if message == "span node count"));
                    assert_eq!(pool.live_bytes(), 0);
                    drop(SpanInvocationTree::new(&borrowed, &limits, &pool).unwrap());
                    assert_eq!(pool.live_bytes(), 0);
                }
            }
        }
    }

    #[test]
    fn selected_tree_keeps_root_events_and_intermediate_invocations() {
        let pool = OperationBudget::new(1024 * 1024);
        let sources = [
            source(&[2, 3]),
            source(&[]),
            source(&[2, 8]),
            source(&[7, 3]),
        ];
        let mut selection = SpanSelection {
            events: SpanTable::new(4, &pool).unwrap(),
            stamp: None,
            page_id: None,
        };
        for (index, source) in sources.iter().enumerate() {
            let mut path = SpanTable::new(source.invocations.len(), &pool).unwrap();
            path.items.extend_from_slice(&source.invocations);
            selection.events.items.push(SpanSelectedEvent {
                path,
                operation: 0,
                byte_offset: Some(0),
                codes: None,
                first: index == 0,
            });
        }
        let limits = Limits {
            max_objects: 6,
            ..Default::default()
        };
        let tree = SpanInvocationTree::from_selection(&selection, &limits, &pool).unwrap();
        assert_eq!(tree.nodes.len(), 6);
        assert_eq!(tree.leaves, [2, 0, 3, 5]);
        let mapped = selection.map_to_tree(&tree, &pool).unwrap();
        assert_eq!(mapped.nodes.items, tree.leaves);
        drop(mapped);
        let retained = pool.live_bytes();
        let too_few = Limits {
            max_objects: 5,
            ..limits
        };
        assert!(SpanInvocationTree::from_selection(&selection, &too_few, &pool).is_err());
        assert_eq!(pool.live_bytes(), retained);
        drop(tree);
        drop(selection);
        assert_eq!(pool.live_bytes(), 0);
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
        // Include the scratch index/output overlap rather than assuming the
        // old two-Vec allocation order. Probe the real layout on this platform.
        let probe = OperationBudget::new(1024 * 1024);
        drop(SpanInvocationTree::new(&[&first], &Default::default(), &probe).unwrap());
        assert_eq!(probe.live_bytes(), 0);
        let peak = probe.peak_bytes();
        let exact = OperationBudget::new(peak + 7);
        let neighbor = exact.reserve(7).unwrap();
        let tree = SpanInvocationTree::new(&[&first], &Default::default(), &exact).unwrap();
        assert_eq!(exact.peak_bytes(), peak + 7);
        drop(tree);
        assert_eq!(exact.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(exact.live_bytes(), 0);
        let second_table_failure = OperationBudget::new(peak + 6);
        let neighbor = second_table_failure.reserve(7).unwrap();
        assert!(matches!(
            SpanInvocationTree::new(&[&first], &Default::default(), &second_table_failure),
            Err(PdfError::ResourceLimit(ref message)) if message == "operation live bytes"
        ));
        assert_eq!(second_table_failure.live_bytes(), 7);
        drop(neighbor);
        drop(SpanInvocationTree::new(&[&first], &Default::default(), &second_table_failure).unwrap());
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
    fn expanded_empty_events_enforce_exact_event_limit_and_release_budget() {
        use crate::document::PdfDocument;
        for count in [16, 17] {
            let doc = PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_objects: 16,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
            doc.add_page(0).unwrap();
            doc.replace_page_contents(0, b"[] TJ ".repeat(count))
                .unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let budget = OperationBudget::new(4 * 1024 * 1024);
            let mut visited = 0;
            let result = doc.visit_expanded_text_events(0, &root, &budget, |path, _, _| {
                assert!(path.is_empty());
                visited += 1;
                Ok(())
            });
            if count == 16 {
                result.unwrap();
            } else {
                assert!(matches!(result, Err(PdfError::ResourceLimit(ref message))
                    if message == "span expanded text events"));
            }
            assert_eq!(visited, 16);
            assert_eq!(budget.live_bytes(), 0);
            // Traversal is read-only even when its callback ran before rejection.
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                root.source_bytes()
            );
        }
    }

    #[test]
    fn expanded_events_walk_deep_forms_on_small_thread_stack() {
        std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(|| {
                use crate::{
                    document::{PdfDocument, resource_manager::ResourceKind},
                    objects::{
                        PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference,
                        pdf_stream::PdfStream,
                    },
                };
                let depth = 512;
                for max_depth in [depth - 1, depth] {
                    let doc = PdfDocument::new_with_options(ParseOptions {
                        limits: Limits {
                            max_depth,
                            ..Default::default()
                        },
                        ..Default::default()
                    })
                    .unwrap();
                    doc.add_page(0).unwrap();
                    let template = crate::parser::syntax::SyntaxParser::new(
                        crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                            b"<< /Type /XObject /Subtype /Form /BBox [0 0 1 1] >>".to_vec(),
                        ))
                        .unwrap(),
                    )
                    .get_object()
                    .unwrap()
                    .into_dict()
                    .unwrap();
                    let mut child = None;
                    for _ in 0..depth {
                        let mut dictionary = template.clone();
                        let bytes = if let Some(id) = child {
                            let mut xobjects = PdfDict::default();
                            xobjects.insert(
                                "Child".into(),
                                PdfObject::PdfReference(PdfReference::from_id(id)),
                            );
                            let mut resources = PdfDict::default();
                            resources.insert("XObject".into(), PdfObject::PdfDict(xobjects));
                            dictionary.insert("Resources".into(), PdfObject::PdfDict(resources));
                            b"/Child Do".to_vec()
                        } else {
                            b"BT [] TJ [17] TJ ET".to_vec()
                        };
                        child = Some(
                            doc.try_insert_object(PdfObject::PdfStream(
                                PdfStream::from_decoded_data(dictionary, bytes).unwrap(),
                            ))
                            .unwrap(),
                        );
                    }
                    let name = doc
                        .resources()
                        .register_existing(0, ResourceKind::Form, child.unwrap())
                        .unwrap();
                    doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
                        .unwrap();
                    let root = doc.get_page(0).unwrap().content_program().unwrap();
                    let budget = OperationBudget::new(64 * 1024 * 1024);
                    let mut events = 0;
                    let result = doc.visit_expanded_text_events(0, &root, &budget, |path, _, _| {
                        assert_eq!(path.len(), depth);
                        events += 1;
                        Ok(())
                    });
                    if max_depth == depth {
                        result.unwrap();
                        assert_eq!(events, 2);
                    } else {
                        assert!(matches!(result, Err(PdfError::ResourceLimit(ref message))
                        if message == "span expanded depth"));
                        assert_eq!(events, 0);
                    }
                    assert_eq!(budget.live_bytes(), 0);
                }
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn selected_span_many_events_measures_growth_and_releases_budget() {
        use crate::{
            document::{PdfDocument, resource_manager::{ResourceKind, StandardFont}},
            objects::{PdfObject, pdf_stream::PdfStream},
            page::source_provenance::NestedTextCodePosition,
        };
        for preserve in [false, true] {
            let mut previous_peak = 0;
            for count in [8, 64, 256] {
                let doc = PdfDocument::new().unwrap();
                doc.add_page(0).unwrap();
                let font = doc.resources().register_standard_font(0, StandardFont::Courier).unwrap();
                let dictionary = crate::parser::syntax::SyntaxParser::new(
                    crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                        b"<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>".to_vec(),
                    )).unwrap(),
                ).get_object().unwrap().into_dict().unwrap();
                let mut body = b"BT ".to_vec();
                body.extend(b"(A) Tj ".repeat(count));
                body.extend_from_slice(b"ET");
                let leaf = doc.try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(dictionary, body).unwrap(),
                )).unwrap();
                let name = doc.resources().register_existing(0, ResourceKind::Form, leaf).unwrap();
                doc.replace_page_contents(0,
                    format!("BT /{font} 12 Tf ET /{name} Do /{name} Do BT (Z) Tj ET").into_bytes(),
                ).unwrap();
                doc.copy_page(0, 1).unwrap();
                let mut page = doc.get_page(0).unwrap();
                let root = page.content_program().unwrap();
                let original = page.extract_characters_paint_order().unwrap();
                assert_eq!(original.len(), count * 2 + 1);
                let start = NestedTextCodePosition {
                    source: original[0].nested_source.as_ref().unwrap(),
                    array_item: None, offset: 0,
                };
                let end = NestedTextCodePosition {
                    source: original[count * 2 - 1].nested_source.as_ref().unwrap(),
                    array_item: None, offset: 1,
                };
                if count == 256 {
                    let saved = doc.save_to_bytes().unwrap();
                    let limited = OperationBudget::new(512 * 1024);
                    assert!(matches!(
                        doc.replace_selected_text_span(0, &root, start..end, "X", preserve, &limited),
                        Err(PdfError::ResourceLimit(_))
                    ));
                    assert_eq!(limited.live_bytes(), 0);
                    assert!(limited.peak_bytes() <= 512 * 1024);
                    assert_eq!(doc.save_to_bytes().unwrap(), saved);
                }
                let budget = OperationBudget::new(16 * 1024 * 1024);
                doc.replace_selected_text_span(0, &root, start..end, "X", preserve, &budget).unwrap();
                assert_eq!(budget.live_bytes(), 0);
                assert!(budget.peak_bytes() > previous_peak);
                previous_peak = budget.peak_bytes();
                println!("many-event span per_call={count} preserve={preserve}: tracked peak={} bytes",
                    budget.peak_bytes());
                let reopened = PdfDocument::from_bytes_with_mode(
                    doc.save_to_bytes().unwrap(), crate::parser::syntax::ParseMode::Strict,
                ).unwrap();
                let edited = reopened.get_page(0).unwrap().extract_characters_paint_order().unwrap();
                assert_eq!(edited.iter().filter_map(|ch| ch.unicode.as_deref()).collect::<String>(), "XZ");
                assert_eq!(reopened.get_page(1).unwrap().extract_characters_paint_order().unwrap()
                    .iter().filter_map(|ch| ch.unicode.as_deref()).collect::<String>(),
                    format!("{}Z", "A".repeat(count * 2)));
                let old = original.last().unwrap();
                let new = edited.last().unwrap();
                assert!((old.position.x() - new.position.x()).abs() < 0.002);
                assert!((old.position.y() - new.position.y()).abs() < 0.002);
            }
        }
    }

    #[test]
    fn selected_span_prepares_root_text_between_shared_form_calls() {
        use crate::{
            document::{
                PdfDocument,
                resource_manager::{ResourceKind, StandardFont},
            },
            objects::{PdfObject, pdf_stream::PdfStream},
            page::source_provenance::NestedTextCodePosition,
        };
        for (preserve, shared_form) in [(false, true), (true, true), (false, false), (true, false)]
        {
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Courier)
                .unwrap();
            let dictionary = crate::parser::syntax::SyntaxParser::new(
                crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                    b"<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>".to_vec(),
                ))
                .unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap();
            let id = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(
                        dictionary,
                        b"BT (AB) Tj [] TJ ET q 0 0 4 5 re f Q BT (C) Tj ET".to_vec(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, id)
                .unwrap();
            let second_name = if shared_form {
                name.clone()
            } else {
                let second_id = doc
                    .try_insert_object(doc.resolve_object(id).unwrap())
                    .unwrap();
                assert_ne!(id, second_id);
                doc.resources()
                    .register_existing(0, ResourceKind::Form, second_id)
                    .unwrap()
            };
            let image_name = doc
                .resources()
                .embed_rgb_image(0, 1, 1, vec![20, 40, 60])
                .unwrap();
            let content = format!(
                "BT /{font} 12 Tf ET /{name} Do q 1 2 3 4 re S Q q 6 0 0 7 8 9 cm /{image_name} Do Q BT (M) Tj ET /{second_name} Do"
            );
            doc.replace_page_contents(0, content.clone().into_bytes())
                .unwrap();
            doc.copy_page(0, 1).unwrap();
            let mut page = doc.get_page(0).unwrap();
            let root = page.content_program().unwrap();
            let chars = page.extract_characters_paint_order().unwrap();
            let original_images = page.extract_images_paint_order().unwrap();
            assert_eq!(original_images.len(), 1);
            let matrix_values = |matrix: &crate::geom::matrix::Matrix| {
                [
                    matrix.a(),
                    matrix.b(),
                    matrix.c(),
                    matrix.d(),
                    matrix.e(),
                    matrix.f(),
                ]
            };
            assert_eq!(chars.len(), 7);
            let start = NestedTextCodePosition {
                source: chars[0].nested_source.as_ref().unwrap(),
                array_item: None,
                offset: 0,
            };
            let end = NestedTextCodePosition {
                source: chars[5].nested_source.as_ref().unwrap(),
                array_item: None,
                offset: 1,
            };
            let budget = OperationBudget::new(8 * 1024 * 1024);
            let selection = doc
                .collect_expanded_text_span(0, &root, start..end, &budget)
                .unwrap();
            let tree = SpanInvocationTree::from_selection(&selection, &Default::default(), &budget)
                .unwrap();
            assert_eq!(tree.leaves, [1, 1, 1, 0, 2]);
            let mapped = selection.map_to_tree(&tree, &budget).unwrap();
            let retained = budget.live_bytes();
            let grouped = mapped.group_events(&budget).unwrap();
            assert_eq!(grouped.for_node(0).unwrap(), [3]);
            assert_eq!(grouped.for_node(1).unwrap(), [0, 1, 2]);
            assert_eq!(grouped.for_node(2).unwrap(), [4]);
            assert!(grouped.for_node(3).is_err());
            assert!(budget.live_bytes() > retained);
            drop(grouped);
            assert_eq!(budget.live_bytes(), retained);
            let prepared = doc
                .prepare_selected_span(0, &root, &mapped, &budget)
                .unwrap();
            assert!(prepared.get(0).is_none());
            assert!(prepared.get(1).is_some() && prepared.get(2).is_some());
            assert_eq!(selection.events.items[3].path.items.len(), 0);
            assert!(selection.events.items[3].codes.is_some());
            drop(prepared);
            let retained = budget.live_bytes();
            // Fail before publication, then reuse the original authenticated endpoints.
            let before_failure = doc.save_to_bytes().unwrap();
            assert!(
                doc.replace_selected_text_span(0, &root, end..start, "X", preserve, &budget)
                    .is_err()
            );
            assert_eq!(budget.live_bytes(), retained);
            assert_eq!(doc.save_to_bytes().unwrap(), before_failure);
            assert!(
                doc.replace_selected_text_span(0, &root, start..end, "😀", preserve, &budget)
                    .is_err()
            );
            assert_eq!(budget.live_bytes(), retained);
            assert_eq!(doc.save_to_bytes().unwrap(), before_failure);
            let exhausted = OperationBudget::new(0);
            assert!(
                doc.replace_selected_text_span(0, &root, start..end, "X", preserve, &exhausted)
                    .is_err()
            );
            assert_eq!(exhausted.live_bytes(), 0);
            assert_eq!(doc.save_to_bytes().unwrap(), before_failure);
            // Isolate the operation's peak from earlier selection probes and failures.
            let edit_budget = OperationBudget::new(8 * 1024 * 1024);
            doc.replace_selected_text_span(0, &root, start..end, "X", preserve, &edit_budget)
                .unwrap();
            assert_eq!(edit_budget.live_bytes(), 0);
            assert!(edit_budget.peak_bytes() > 0);
            eprintln!(
                "mixed root/two-call span preserve={preserve} shared_form={shared_form}: tracked peak={} bytes",
                edit_budget.peak_bytes()
            );
            assert_eq!(budget.live_bytes(), retained);
            let mut edited_page = doc.get_page(0).unwrap();
            let edited_root = edited_page.content_program().unwrap();
            assert!(
                edited_root
                    .source_bytes()
                    .windows(b"q 1 2 3 4 re S Q".len())
                    .any(|bytes| bytes == b"q 1 2 3 4 re S Q")
            );
            let edited_chars = edited_page.extract_characters_paint_order().unwrap();
            let edited_leaf = edited_page
                .nested_content_program(
                    &edited_root,
                    edited_chars[0].nested_source.as_ref().unwrap(),
                )
                .unwrap();
            assert!(
                edited_leaf
                    .source_bytes()
                    .windows(b"q 0 0 4 5 re f Q".len())
                    .any(|bytes| bytes == b"q 0 0 4 5 re f Q")
            );
            for page_index in [0, 1] {
                let images = doc
                    .get_page(page_index)
                    .unwrap()
                    .extract_images_paint_order()
                    .unwrap();
                assert_eq!(images.len(), 1);
                assert_eq!(
                    matrix_values(&images[0].matrix),
                    matrix_values(&original_images[0].matrix)
                );
                assert_eq!(images[0].image.data(), original_images[0].image.data());
                assert_eq!(images[0].image.pixel_dimensions(), (1, 1));
            }
            for (page, expected) in [(0, "XBC"), (1, "ABCMABC")] {
                let text: String = doc
                    .get_page(page)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap()
                    .iter()
                    .filter_map(|character| character.unicode.as_deref())
                    .collect();
                assert_eq!(text, expected);
            }
            if preserve {
                let after = doc
                    .get_page(0)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap();
                for (old, new) in [(5, 1), (6, 2)] {
                    assert!((chars[old].position.x() - after[new].position.x()).abs() < 0.002);
                    assert!((chars[old].position.y() - after[new].position.y()).abs() < 0.002);
                }
            }
            let reopened = PdfDocument::from_bytes_with_mode(
                doc.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            for (page, expected) in [(0, "XBC"), (1, "ABCMABC")] {
                let text: String = reopened
                    .get_page(page)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap()
                    .iter()
                    .filter_map(|character| character.unicode.as_deref())
                    .collect();
                assert_eq!(text, expected);
                let images = reopened
                    .get_page(page)
                    .unwrap()
                    .extract_images_paint_order()
                    .unwrap();
                assert_eq!(images.len(), 1);
                assert_eq!(
                    matrix_values(&images[0].matrix),
                    matrix_values(&original_images[0].matrix)
                );
                assert_eq!(images[0].image.data(), original_images[0].image.data());
                assert_eq!(images[0].image.pixel_dimensions(), (1, 1));
            }
            let retained = budget.live_bytes();
            // Same operation positions and strings, but a newer document revision.
            doc.replace_page_contents(0, format!("{content}\n% changed revision").into_bytes())
                .unwrap();
            let fresh = doc.get_page(0).unwrap().content_program().unwrap();
            assert!(
                doc.prepare_selected_span(0, &fresh, &mapped, &budget)
                    .is_err()
            );
            assert_eq!(budget.live_bytes(), retained);
            drop(mapped);
            drop(tree);
            drop(selection);
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn shared_ancestor_publication_edits_two_leaves_in_both_modes() {
        use crate::{
            document::{
                PdfDocument,
                resource_manager::{ResourceKind, StandardFont},
            },
            objects::{
                PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream,
            },
            page::content_program::TextCodePosition,
        };
        let dictionary = || {
            crate::parser::syntax::SyntaxParser::new(
                crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                    b"<< /Type /XObject /Subtype /Form /BBox [0 0 200 200] >>".to_vec(),
                ))
                .unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap()
        };
        for preserve in [false, true] {
            let doc = PdfDocument::new().unwrap();
            doc.add_page(0).unwrap();
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Courier)
                .unwrap();
            let mut leaf_dictionary = dictionary();
            leaf_dictionary.insert(
                "Resources".into(),
                PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
            );
            let leaf_bytes = b"BT (AB) Tj [] TJ [17] TJ (C) Tj ET".to_vec();
            let leaf = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(leaf_dictionary, leaf_bytes.clone()).unwrap(),
                ))
                .unwrap();
            let mut xobjects = PdfDict::default();
            xobjects.insert(
                "Leaf".into(),
                PdfObject::PdfReference(PdfReference::from_id(leaf)),
            );
            let mut scope = PdfDict::default();
            scope.insert("XObject".into(), PdfObject::PdfDict(xobjects));
            let mut outer_dictionary = dictionary();
            outer_dictionary.insert("Resources".into(), PdfObject::PdfDict(scope));
            let outer_bytes = b"/Leaf Do /Leaf Do".to_vec();
            let outer = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(outer_dictionary, outer_bytes.clone()).unwrap(),
                ))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, outer)
                .unwrap();
            doc.replace_page_contents(
                0,
                format!("BT /{font} 12 Tf 2 Tc 3 Tw -75 Tz ET /{name} Do /{name} Do").into_bytes(),
            )
            .unwrap();
            doc.copy_page(0, 1).unwrap();
            let mut page = doc.get_page(0).unwrap();
            let root = page.content_program().unwrap();
            let before = page.extract_characters_paint_order().unwrap();
            assert_eq!(before.len(), 12);
            let sources = [
                before[0].nested_source.as_ref().unwrap(),
                before[3].nested_source.as_ref().unwrap(),
            ];
            let pool = OperationBudget::new(4 * 1024 * 1024);
            let mut events = Vec::new();
            doc.visit_expanded_text_events(0, &root, &pool, |path, program, index| {
                events.push((
                    path.to_vec(),
                    index,
                    program.operations()[index].operator.name().to_owned(),
                ));
                Ok(())
            })
            .unwrap();
            assert_eq!(events.len(), 16);
            for (call, group) in events.chunks_exact(4).enumerate() {
                assert_eq!(
                    group.iter().map(|event| event.1).collect::<Vec<_>>(),
                    [1, 2, 3, 4]
                );
                assert_eq!(
                    group
                        .iter()
                        .map(|event| event.2.as_str())
                        .collect::<Vec<_>>(),
                    ["Tj", "TJ", "TJ", "Tj"]
                );
                for event in group {
                    assert_eq!(
                        event.0,
                        before[call * 3].nested_source.as_ref().unwrap().invocations
                    );
                }
            }
            assert_eq!(pool.live_bytes(), 0);
            let start = crate::page::source_provenance::NestedTextCodePosition {
                source: before[0].nested_source.as_ref().unwrap(),
                array_item: None,
                offset: 0,
            };
            let end = crate::page::source_provenance::NestedTextCodePosition {
                source: before[5].nested_source.as_ref().unwrap(),
                array_item: None,
                offset: 1,
            };
            let mut selected = Vec::new();
            doc.visit_expanded_text_span(
                0,
                &root,
                start..end,
                &pool,
                |path, program, index, first, last| {
                    let codes = super::super::text_event_edit::full_event_range(program, index)?;
                    match index {
                        1 | 4 => {
                            let codes = codes.unwrap();
                            assert_eq!(codes.start.operation, index);
                            assert_eq!(codes.start.array_item, None);
                            assert_eq!(codes.start.offset, 0);
                            assert_eq!(codes.end.offset, if index == 1 { 2 } else { 1 });
                        }
                        2 | 3 => assert!(codes.is_none()),
                        _ => panic!("unexpected selected event"),
                    }
                    selected.push((path.to_vec(), index, first, last));
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(selected.len(), 8);
            assert!(selected[0].2 && !selected[0].3);
            assert!(!selected[7].2 && selected[7].3);
            assert!(selected[1..7].iter().all(|event| !event.2 && !event.3));
            for (selected, original) in selected.iter().zip(&events) {
                assert_eq!(selected.0, original.0);
                assert_eq!(selected.1, original.1);
            }
            assert_eq!(pool.live_bytes(), 0);
            let mut callbacks = 0;
            let partial_start =
                crate::page::source_provenance::NestedTextCodePosition { offset: 1, ..start };
            let partial_end =
                crate::page::source_provenance::NestedTextCodePosition { offset: 0, ..end };
            let mut clipped = Vec::new();
            doc.visit_expanded_text_code_ranges(
                0,
                &root,
                partial_start..partial_end,
                &pool,
                |_, _, index, codes, first| {
                    clipped.push((index, codes, first));
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(clipped.len(), 8);
            let first_codes = clipped[0].1.as_ref().unwrap();
            assert_eq!((first_codes.start.offset, first_codes.end.offset), (1, 2));
            let last_codes = clipped[7].1.as_ref().unwrap();
            assert_eq!((last_codes.start.offset, last_codes.end.offset), (0, 0));
            assert!(clipped[0].2);
            assert!(clipped[1..].iter().all(|event| !event.2));
            assert_eq!(clipped.iter().filter(|event| event.1.is_none()).count(), 4);
            let same_end = crate::page::source_provenance::NestedTextCodePosition {
                offset: 2,
                ..partial_start
            };
            let mut same_events = 0;
            doc.visit_expanded_text_code_ranges(
                0,
                &root,
                partial_start..same_end,
                &pool,
                |_, _, _, codes, first| {
                    let codes = codes.unwrap();
                    assert_eq!((codes.start.offset, codes.end.offset), (1, 2));
                    assert!(first);
                    same_events += 1;
                    Ok(())
                },
            )
            .unwrap();
            assert_eq!(same_events, 1);
            assert_eq!(pool.live_bytes(), 0);
            let selection = doc
                .collect_expanded_text_span(0, &root, partial_start..partial_end, &pool)
                .unwrap();
            assert!(pool.live_bytes() > 0);
            assert_eq!(selection.events.items.len(), clipped.len());
            for ((event, clipped), original) in
                selection.events.items.iter().zip(&clipped).zip(&events)
            {
                assert_eq!(event.path.items, original.0);
                assert_eq!(event.operation, clipped.0);
                assert_eq!(event.codes, clipped.1);
                assert_eq!(event.first, clipped.2);
                assert!(event.byte_offset.is_some());
            }
            let selected_tree =
                SpanInvocationTree::from_selection(&selection, &Default::default(), &pool).unwrap();
            assert_eq!(selected_tree.leaves, [2, 2, 2, 2, 3, 3, 3, 3]);
            let retained = pool.live_bytes();
            let mapped = selection.map_to_tree(&selected_tree, &pool).unwrap();
            {
                let prepared = doc.prepare_selected_span(0, &root, &mapped, &pool).unwrap();
                doc.validate_span_selection(0, &root, &mapped, &prepared, &pool)
                    .unwrap();
                let retained = pool.live_bytes();
                let foreign = doc.get_page(1).unwrap().content_program().unwrap();
                assert!(
                    doc.validate_span_selection(1, &foreign, &mapped, &prepared, &pool)
                        .is_err()
                );
                assert_eq!(pool.live_bytes(), retained);
                assert!(
                    doc.validate_span_selection(1, &root, &mapped, &prepared, &pool)
                        .is_err()
                );
                assert_eq!(pool.live_bytes(), retained);
            }
            assert!(std::ptr::eq(mapped.selection, &selection));
            assert!(std::ptr::eq(mapped.tree, &selected_tree));
            assert_eq!(mapped.nodes.items, [2, 2, 2, 2, 3, 3, 3, 3]);
            assert!(pool.live_bytes() > retained);
            drop(mapped);
            assert_eq!(pool.live_bytes(), retained);
            let missing_tree =
                SpanInvocationTree::new(&sources[..1], &Default::default(), &pool).unwrap();
            let retained = pool.live_bytes();
            assert!(selection.map_to_tree(&missing_tree, &pool).is_err());
            assert_eq!(pool.live_bytes(), retained);
            drop(missing_tree);
            drop(selected_tree);
            drop(selection);
            assert_eq!(pool.live_bytes(), 0);
            assert!(
                doc.visit_expanded_text_span(0, &root, end..start, &pool, |_, _, _, _, _| {
                    callbacks += 1;
                    Ok(())
                })
                .is_err()
            );
            let mut wrong_source = end.source.clone();
            wrong_source.operator_byte_offset = Some(u64::MAX);
            let wrong_end = crate::page::source_provenance::NestedTextCodePosition {
                source: &wrong_source,
                ..end
            };
            assert!(
                doc.visit_expanded_text_span(0, &root, start..wrong_end, &pool, |_, _, _, _, _| {
                    callbacks += 1;
                    Ok(())
                })
                .is_err()
            );
            assert_eq!(callbacks, 0);
            assert_eq!(pool.live_bytes(), 0);
            let interrupted = doc.visit_expanded_text_events(0, &root, &pool, |_, _, _| {
                Err(PdfError::ObjectError("callback interrupted".into()))
            });
            assert!(matches!(interrupted, Err(PdfError::ObjectError(_))));
            assert_eq!(pool.live_bytes(), 0);
            let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                let _ = doc.visit_expanded_text_events(0, &root, &pool, |_, _, _| {
                    panic!("callback panic");
                });
            }));
            assert!(panic.is_err());
            assert_eq!(pool.live_bytes(), 0);
            let exhausted = OperationBudget::new(0);
            assert!(
                doc.visit_expanded_text_events(0, &root, &exhausted, |_, _, _| Ok(()))
                    .is_err()
            );
            assert_eq!(exhausted.live_bytes(), 0);
            let tree = SpanInvocationTree::new(&sources, &Default::default(), &pool).unwrap();
            assert_eq!(tree.nodes().len(), 4);
            assert_eq!(tree.lowest_common_ancestor(0, 1).unwrap(), 1);
            let baseline = pool.live_bytes();
            let foreign_root = doc.get_page(1).unwrap().content_program().unwrap();
            assert!(
                matches!(doc.prepare_span_forms(0, &foreign_root, &tree, &sources, &pool),
                Err(PdfError::PageResourceError(ref message)) if message == "span requires current page snapshot")
            );
            assert_eq!(pool.live_bytes(), baseline);
            let mut wrong_leaf = (*sources[1]).clone();
            wrong_leaf.operator_byte_offset = None;
            assert!(
                matches!(doc.prepare_span_forms(0, &root, &tree, &[sources[0], &wrong_leaf], &pool),
                Err(PdfError::PageResourceError(ref message)) if message == "Nested leaf byte position mismatch")
            );
            assert_eq!(pool.live_bytes(), baseline);
            let prepared = doc
                .prepare_span_forms(0, &root, &tree, &sources, &pool)
                .unwrap();
            let mut plans: Vec<_> = (0..tree.nodes().len()).map(|_| None).collect();
            for (slot, text) in [(0, "X"), (1, "")] {
                let node = tree.leaf(slot).unwrap();
                let form = prepared.get(node).unwrap();
                let start = TextCodePosition {
                    operation: sources[slot].operation_index,
                    array_item: None,
                    offset: 0,
                };
                let end = TextCodePosition { offset: 2, ..start };
                plans[node] = Some(
                    doc.build_text_event_edit_plan(
                        &form.program,
                        start..end,
                        text,
                        preserve,
                        &pool,
                        |index| {
                            doc.span_text_metrics_for_operation(
                                0, &root, &tree, &prepared, node, index, &pool,
                            )
                        },
                    )
                    .unwrap(),
                );
            }
            {
                let node = tree.leaf(0).unwrap();
                let form = prepared.get(node).unwrap();
                let retained = pool.live_bytes();
                let mut donors = [None, None];
                for (slot, operation, len, text) in [(0, 1, 2, "X"), (1, 4, 1, "")] {
                    let start = TextCodePosition {
                        operation,
                        array_item: None,
                        offset: 0,
                    };
                    let end = TextCodePosition {
                        offset: len,
                        ..start
                    };
                    donors[slot] = Some(
                        doc.build_text_event_fragments(
                            &form.program,
                            start..end,
                            text,
                            preserve,
                            &pool,
                            |index| {
                                doc.span_text_metrics_for_operation(
                                    0, &root, &tree, &prepared, node, index, &pool,
                                )
                            },
                        )
                        .unwrap(),
                    );
                }
                let donor_bytes = pool.live_bytes();
                donors.swap(0, 1);
                assert!(matches!(
                    doc.finish_text_event_fragments(&form.program, &mut donors, &pool),
                    Err(PdfError::ObjectError(ref message))
                        if message == "merged events are not in original order"
                ));
                assert!(donors.iter().all(Option::is_some));
                assert_eq!(pool.live_bytes(), donor_bytes);
                donors.swap(0, 1);
                let duplicate_start = TextCodePosition {
                    operation: 1,
                    array_item: None,
                    offset: 0,
                };
                let duplicate = doc.build_text_event_fragments(
                    &form.program,
                    duplicate_start..TextCodePosition { offset: 2, ..duplicate_start },
                    "X", preserve, &pool,
                    |index| doc.span_text_metrics_for_operation(
                        0, &root, &tree, &prepared, node, index, &pool,
                    ),
                ).unwrap();
                let mut repeated = [donors[0].take(), Some(duplicate)];
                let repeated_bytes = pool.live_bytes();
                assert!(matches!(
                    doc.finish_text_event_fragments(&form.program, &mut repeated, &pool),
                    Err(PdfError::ObjectError(ref message))
                        if message == "merged events are not in original order"
                ));
                assert!(repeated.iter().all(Option::is_some));
                assert_eq!(pool.live_bytes(), repeated_bytes);
                donors[0] = repeated[0].take();
                drop(repeated);
                assert_eq!(pool.live_bytes(), donor_bytes);
                let exhausted = OperationBudget::new(0);
                assert!(
                    doc.finish_text_event_fragments(&form.program, &mut donors, &exhausted)
                        .is_err()
                );
                assert!(donors.iter().all(Option::is_some));
                assert_eq!(pool.live_bytes(), donor_bytes);
                assert_eq!(exhausted.live_bytes(), 0);
                let merged = doc
                    .finish_text_event_fragments(&form.program, &mut donors, &pool)
                    .unwrap();
                assert!(donors.iter().all(Option::is_none));
                assert_eq!(merged.character_counts(), (3, 1));
                assert_eq!(
                    merged
                        .original_replacements()
                        .map(|event| event.index)
                        .collect::<Vec<_>>(),
                    [1, 4]
                );
                drop(merged);
                assert_eq!(pool.live_bytes(), retained);
            }
            assert_eq!(check_plan_character_totals(&plans, 4).unwrap(), (4, 1));
            let before_rejection = pool.live_bytes();
            assert!(
                matches!(check_plan_character_totals(&plans, 3), Err(PdfError::ResourceLimit(ref message)) if message == "span aggregate input characters")
            );
            assert_eq!(pool.live_bytes(), before_rejection);
            let node = tree.leaf(1).unwrap();
            let form = prepared.get(node).unwrap();
            let start = TextCodePosition {
                operation: sources[1].operation_index,
                array_item: None,
                offset: 0,
            };
            let end = TextCodePosition { offset: 2, ..start };
            let oversized = doc
                .build_text_event_edit_plan(
                    &form.program,
                    start..end,
                    "XXXX",
                    preserve,
                    &pool,
                    |index| {
                        doc.span_text_metrics_for_operation(
                            0, &root, &tree, &prepared, node, index, &pool,
                        )
                    },
                )
                .unwrap();
            let saved = plans[node].replace(oversized);
            assert!(
                matches!(check_plan_character_totals(&plans, 4), Err(PdfError::ResourceLimit(ref message)) if message == "span aggregate output characters")
            );
            let oversized = std::mem::replace(&mut plans[node], saved);
            drop(oversized);
            assert_eq!(pool.live_bytes(), before_rejection);
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                root.source_bytes()
            );
            drop(prepared);
            let plan_baseline = pool.live_bytes();
            let resources_before = doc.get_page(0).unwrap().resource().unwrap();
            let shared_resources_before = doc.get_page(1).unwrap().resource().unwrap();
            let leaf_before = doc.resolve_object(leaf).unwrap();
            let outer_before = doc.resolve_object(outer).unwrap();
            let mut rejected_forms = tree
                .prepare_forms(&pool, |slot| {
                    page.prepare_form_chain_with_budget(&root, sources[slot], &pool)
                })
                .unwrap();
            let ancestor = rejected_forms.forms.items[1].as_mut().unwrap();
            let (dictionary, mut lease) = ancestor
                .dictionary
                .with_replaced_entry_with_budget(
                    "F",
                    &PdfObject::PdfName(crate::objects::pdf_name::PdfName::new(
                        "external-marker".into(),
                    )),
                    &pool,
                    Default::default(),
                )
                .unwrap();
            ancestor.dictionary = dictionary;
            ancestor
                .copies_lease
                .as_mut()
                .unwrap()
                .absorb(&mut lease)
                .unwrap();
            let rejected = doc.commit_prepared_span(0, &root, &tree, rejected_forms, &plans, &pool);
            assert!(
                matches!(rejected, Err(PdfError::UnsupportedFeature(ref message)) if message == "replacing external-file stream data")
            );
            assert_eq!(pool.live_bytes(), plan_baseline);
            assert_eq!(
                doc.get_page(0).unwrap().resource().unwrap(),
                resources_before
            );
            assert_eq!(
                doc.get_page(1).unwrap().resource().unwrap(),
                shared_resources_before
            );
            assert_eq!(doc.resolve_object(leaf).unwrap(), leaf_before);
            assert_eq!(doc.resolve_object(outer).unwrap(), outer_before);
            let rollback_reopened = PdfDocument::from_bytes_with_mode(
                doc.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            for page_index in [0, 1] {
                let text: String = rollback_reopened
                    .get_page(page_index)
                    .unwrap()
                    .extract_characters_paint_order()
                    .unwrap()
                    .iter()
                    .filter_map(|character| character.unicode.as_deref())
                    .collect();
                assert_eq!(text, "ABCABCABCABC");
            }
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                root.source_bytes()
            );
            assert_eq!(
                doc.get_page(1)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                root.source_bytes()
            );
            let fresh_root = doc.get_page(0).unwrap().content_program().unwrap();
            let prepared = doc
                .prepare_span_forms(0, &fresh_root, &tree, &sources, &pool)
                .unwrap();
            doc.commit_prepared_span(0, &fresh_root, &tree, prepared, &plans, &pool)
                .unwrap();
            drop(plans);
            drop(tree);
            assert_eq!(pool.live_bytes(), 0);
            assert_eq!(
                doc.resolve_object(leaf)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .raw_data(),
                leaf_bytes
            );
            assert_eq!(
                doc.resolve_object(outer)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .raw_data(),
                outer_bytes
            );
            let reopened = PdfDocument::from_bytes_with_mode(
                doc.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            let after = reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap();
            let text: String = after
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect();
            assert_eq!(text, "XCCABCABC");
            let shared: String = reopened
                .get_page(1)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap()
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect();
            assert_eq!(shared, "ABCABCABCABC");
            if preserve {
                for (old, new) in [(2, 1), (5, 2)] {
                    assert!((before[old].position.x() - after[new].position.x()).abs() < 0.002);
                    assert!((before[old].position.y() - after[new].position.y()).abs() < 0.002);
                }
            }
            for (old, new) in before[6..].iter().zip(&after[3..]) {
                assert_eq!(old.unicode, new.unicode);
                assert!((old.position.x() - new.position.x()).abs() < 0.002);
                assert!((old.position.y() - new.position.y()).abs() < 0.002);
            }
            // Measure a fresh full operation across all four invocations,
            // excluding the earlier probes and retained manual plans.
            let mut measured_page = rollback_reopened.get_page(0).unwrap();
            let measured_root = measured_page.content_program().unwrap();
            let original_chars = measured_page.extract_characters_paint_order().unwrap();
            let start = crate::page::source_provenance::NestedTextCodePosition {
                source: original_chars[0].nested_source.as_ref().unwrap(),
                array_item: None,
                offset: 1,
            };
            let end = crate::page::source_provenance::NestedTextCodePosition {
                source: original_chars[10].nested_source.as_ref().unwrap(),
                array_item: None,
                offset: 1,
            };
            let measured_budget = OperationBudget::new(8 * 1024 * 1024);
            rollback_reopened.replace_selected_text_span(
                0, &measured_root, start..end, "X", preserve, &measured_budget,
            ).unwrap();
            assert_eq!(measured_budget.live_bytes(), 0);
            assert!(measured_budget.peak_bytes() > 0);
            println!("full four-call span preserve={preserve}: tracked peak={} bytes",
                measured_budget.peak_bytes());
            let measured_reopen = PdfDocument::from_bytes_with_mode(
                rollback_reopened.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            ).unwrap();
            let edited_chars = measured_reopen.get_page(0).unwrap()
                .extract_characters_paint_order().unwrap();
            assert_eq!(edited_chars.iter().filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(), "AXBC");
            assert_eq!(measured_reopen.get_page(1).unwrap()
                .extract_characters_paint_order().unwrap().iter()
                .filter_map(|ch| ch.unicode.as_deref()).collect::<String>(), "ABCABCABCABC");
            if preserve {
                for (old, new) in original_chars[10..].iter().zip(&edited_chars[2..]) {
                    assert!((old.position.x() - new.position.x()).abs() < 0.002);
                    assert!((old.position.y() - new.position.y()).abs() < 0.002);
                }
            }
        }
    }

    #[test]
    fn actual_form_chains_retain_unique_nodes_and_release_late_failure() {
        use crate::{
            document::{
                PdfDocument,
                resource_manager::{ResourceKind, StandardFont},
            },
            objects::{PdfObject, pdf_name::PdfName, pdf_stream::PdfStream},
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        let mut dictionary = crate::parser::syntax::SyntaxParser::new(
            crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                b"<< /Type /XObject /Subtype /Form /BBox [0 0 200 200] >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        dictionary.insert(
            "Subtype".into(),
            PdfObject::PdfName(PdfName::new("Form".into())),
        );
        dictionary.insert(
            "Resources".into(),
            PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
        );
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    dictionary,
                    format!("BT /{font} 12 Tf (AB) Tj ET").into_bytes(),
                )
                .unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
            .unwrap();
        doc.copy_page(0, 1).unwrap();
        let mut page = doc.get_page(0).unwrap();
        let root = page.content_program().unwrap();
        let chars = page.extract_characters_paint_order().unwrap();
        let sources: Vec<_> = [0, 1, 2]
            .into_iter()
            .map(|index| chars[index].nested_source.as_ref().unwrap())
            .collect();
        let pool = OperationBudget::new(1024 * 1024);
        let tree = SpanInvocationTree::new(&sources, &Default::default(), &pool).unwrap();
        assert_eq!(tree.nodes().len(), 3);
        let baseline = pool.live_bytes();
        let failed = tree.prepare_forms(&pool, |slot| {
            if slot == 2 {
                return Err(PdfError::DocumentError("late real chain rejected".into()));
            }
            page.prepare_form_chain_with_budget(&root, sources[slot], &pool)
        });
        assert!(
            matches!(failed, Err(PdfError::DocumentError(ref message)) if message == "late real chain rejected")
        );
        assert_eq!(pool.live_bytes(), baseline);
        let prepared = tree
            .prepare_forms(&pool, |slot| {
                page.prepare_form_chain_with_budget(&root, sources[slot], &pool)
            })
            .unwrap();
        assert!(prepared.get(0).is_none());
        assert!(prepared.get(1).is_some());
        assert!(prepared.get(2).is_some());
        assert!(prepared.get(3).is_none());
        assert_eq!(
            prepared
                .forms
                .items
                .iter()
                .filter(|form| form.is_some())
                .count(),
            2
        );
        assert_eq!(
            prepared.get(1).unwrap().program.source_bytes(),
            prepared.get(2).unwrap().program.source_bytes()
        );
        assert!(pool.live_bytes() > baseline);
        drop(prepared);
        assert_eq!(pool.live_bytes(), baseline);
        let other = SpanInvocationTree::new(&sources, &Default::default(), &pool).unwrap();
        assert_eq!(other.nodes().len(), tree.nodes().len());
        let two_trees = pool.live_bytes();
        let prepared = tree
            .prepare_forms(&pool, |slot| {
                page.prepare_form_chain_with_budget(&root, sources[slot], &pool)
            })
            .unwrap();
        let mut callbacks = 0;
        let rejected = other.build_prepared_bottom_up::<(), _>(prepared, &pool, |_, _, _, _| {
            callbacks += 1;
            Ok(())
        });
        assert!(
            matches!(rejected, Err(PdfError::ObjectError(ref message)) if message == "span prepared tree mismatch")
        );
        assert_eq!(callbacks, 0);
        assert_eq!(pool.live_bytes(), two_trees);
        drop(other);
        assert_eq!(pool.live_bytes(), baseline);
        let prepared = tree
            .prepare_forms(&pool, |slot| {
                page.prepare_form_chain_with_budget(&root, sources[slot], &pool)
            })
            .unwrap();
        let built = tree
            .build_prepared_bottom_up::<Option<crate::page::source_provenance::PreparedForm>, _>(
                prepared,
                &pool,
                |index, _, form, results| {
                    if index == 0 {
                        assert!(results[1].as_ref().unwrap().is_some());
                        assert!(results[2].as_ref().unwrap().is_some());
                        assert!(form.is_none());
                    } else {
                        assert!(form.is_some());
                    }
                    Ok(form)
                },
            )
            .unwrap();
        assert!(built.root().is_none());
        assert!(pool.live_bytes() > baseline);
        drop(built);
        assert_eq!(pool.live_bytes(), baseline);
        let prepared = tree
            .prepare_forms(&pool, |slot| {
                page.prepare_form_chain_with_budget(&root, sources[slot], &pool)
            })
            .unwrap();
        let outputs = tree
            .build_prepared_bottom_up::<Option<SpanFormOutput<SpanParentEdit>>, _>(
                prepared,
                &pool,
                |index, _, form, results| {
                    if let Some(form) = form {
                        let program = tree.rebuild_parent(
                            index,
                            &form.program,
                            Default::default(),
                            &pool,
                            |_| panic!("leaf has no child bindings"),
                        )?;
                        let output = SpanFormOutput::new(form, program, &pool, Default::default())?;
                        assert!(
                            output
                                .stream
                                .dict()
                                .get("Resources")
                                .unwrap()
                                .as_dict()
                                .is_some()
                        );
                        Ok(Some(output))
                    } else {
                        assert!(results[1].as_ref().unwrap().is_some());
                        assert!(results[2].as_ref().unwrap().is_some());
                        Ok(None)
                    }
                },
            )
            .unwrap();
        assert!(outputs.root().is_none());
        assert!(pool.live_bytes() > baseline);
        drop(outputs);
        assert_eq!(pool.live_bytes(), baseline);
        let mut chain = page
            .prepare_form_chain_with_budget(&root, sources[0], &pool)
            .unwrap()
            .into_iter();
        let form = chain.next().unwrap();
        drop(chain);
        let program = tree
            .rebuild_parent(
                tree.leaf(0).unwrap(),
                &form.program,
                Default::default(),
                &pool,
                |_| panic!("leaf has no child bindings"),
            )
            .unwrap();
        let output = SpanFormOutput::new(form, program, &pool, Default::default()).unwrap();
        let materialized = pool.live_bytes();
        let (stream, retained) = output.into_parts();
        assert_eq!(pool.live_bytes(), materialized);
        assert!(stream.dict().get("Resources").is_some());
        drop(stream);
        assert_eq!(pool.live_bytes(), materialized);
        drop(retained);
        assert_eq!(pool.live_bytes(), baseline);
        for raw_limit in [false, true] {
            let mut chain = page
                .prepare_form_chain_with_budget(&root, sources[0], &pool)
                .unwrap()
                .into_iter();
            let form = chain.next().unwrap();
            drop(chain);
            let leaf = tree.leaf(0).unwrap();
            let program = tree
                .rebuild_parent(leaf, &form.program, Default::default(), &pool, |_| {
                    panic!("leaf has no child bindings")
                })
                .unwrap();
            let bytes = program.as_ref().source_bytes().len();
            let mut limits = Limits::default();
            if raw_limit {
                limits.max_raw_stream_bytes = bytes - 1;
            } else {
                limits.max_decoded_bytes = bytes - 1;
            }
            let rejected = SpanFormOutput::new(form, program, &pool, limits);
            assert!(
                matches!(rejected, Err(PdfError::ResourceLimit(ref message)) if message == "span prepared Form stream bytes")
            );
            assert_eq!(pool.live_bytes(), baseline);
        }
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
        let prepare = |snapshot: &ContentProgram| {
            tree.prepare_forms(&pool, |slot| {
                page.prepare_form_chain_with_budget(snapshot, sources[slot], &pool)
            })
            .unwrap()
        };
        let plans: Vec<_> = (0..tree.nodes().len()).map(|_| None).collect();
        let prepared = prepare(&root);
        let failed: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.commit_prepared_span(0, &root, &tree, prepared, &plans, &pool)?;
            Err(PdfError::DocumentError(
                "span publication outer rollback".into(),
            ))
        });
        assert!(
            matches!(failed, Err(PdfError::DocumentError(ref message)) if message == "span publication outer rollback")
        );
        assert_eq!(pool.live_bytes(), baseline);
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
        let fresh_root = doc.get_page(0).unwrap().content_program().unwrap();
        let prepared = prepare(&fresh_root);
        let mut plans = plans;
        for node in [1, 2] {
            let program = &prepared.get(node).unwrap().program;
            let start = crate::page::content_program::TextCodePosition {
                operation: 2,
                array_item: None,
                offset: 0,
            };
            let end = crate::page::content_program::TextCodePosition { offset: 2, ..start };
            plans[node] = Some(
                doc.build_text_event_edit_plan(
                    program,
                    start..end,
                    if node == 1 { "X" } else { "" },
                    false,
                    &pool,
                    |index| page.text_metrics_for_operation_with_budget(program, index, &pool),
                )
                .unwrap(),
            );
        }
        doc.commit_prepared_span(0, &fresh_root, &tree, prepared, &plans, &pool)
            .unwrap();
        drop(plans);
        assert_eq!(pool.live_bytes(), baseline);
        assert_ne!(
            doc.get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
        assert_eq!(
            doc.get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        for page_index in [0, 1] {
            let text: String = reopened
                .get_page(page_index)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap()
                .iter()
                .filter_map(|character| character.unicode.as_deref())
                .collect();
            assert_eq!(text, if page_index == 0 { "X" } else { "ABAB" });
        }
    }

    #[test]
    fn actual_text_plan_merges_with_sibling_calls_without_publication() {
        use crate::{
            document::{PdfDocument, resource_manager::StandardFont},
            page::content_program::TextCodePosition,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{font} 12 Tf 3 2 (AB) \" /OldA Do 17 mystery /OldB Do ET").into_bytes(),
        )
        .unwrap();
        let page = doc.get_page(0).unwrap();
        let original = page.content_program().unwrap();
        let mut first = source(&[3]);
        first.invocations[0].operator_byte_offset = original.operations()[3].operator.byte_offset();
        let mut second = source(&[5]);
        second.invocations[0].operator_byte_offset =
            original.operations()[5].operator.byte_offset();
        let pool = OperationBudget::new(1024 * 1024);
        let tree = SpanInvocationTree::new(&[&first, &second], &Default::default(), &pool).unwrap();
        let baseline = pool.live_bytes();
        for preserve in [false, true] {
            let start = TextCodePosition {
                operation: 2,
                array_item: None,
                offset: 0,
            };
            let end = TextCodePosition { offset: 2, ..start };
            let plan = doc
                .build_text_event_edit_plan(&original, start..end, "X", preserve, &pool, |index| {
                    page.text_metrics_for_operation_with_budget(&original, index, &pool)
                })
                .unwrap();
            let edits: Vec<_> = plan.original_replacements().collect();
            assert_eq!(edits.len(), 1);
            assert_eq!(edits[0].index, 2);
            assert_eq!(edits[0].operation_count, if preserve { 4 } else { 1 });
            drop(edits);
            let plan_baseline = pool.live_bytes();
            let mut bindings = 0;
            let failed = tree.rebuild_parent_with_text_plan(
                0,
                &original,
                &plan,
                Default::default(),
                &pool,
                |_| {
                    bindings += 1;
                    if bindings == 2 {
                        return Err(PdfError::DocumentError(
                            "real plan late binding rejected".into(),
                        ));
                    }
                    BudgetedBytes::copy_from_slice(&pool, b"/New Do")
                },
            );
            assert!(
                matches!(failed, Err(PdfError::DocumentError(ref message)) if message == "real plan late binding rejected")
            );
            assert_eq!(bindings, 2);
            assert_eq!(pool.live_bytes(), plan_baseline);
            assert_eq!(plan.original_replacements().next().unwrap().index, 2);
            let merged = tree
                .rebuild_parent_with_text_plan(
                    0,
                    &original,
                    &plan,
                    Default::default(),
                    &pool,
                    |_| BudgetedBytes::copy_from_slice(&pool, b"/New Do"),
                )
                .unwrap();
            let calls: Vec<_> = merged
                .as_ref()
                .operations()
                .iter()
                .enumerate()
                .filter(|(_, op)| op.operator.name() == "Do")
                .collect();
            assert_eq!(calls.len(), 2);
            assert_eq!(calls[0].0, if preserve { 6 } else { 3 });
            assert_eq!(calls[1].0, if preserve { 8 } else { 5 });
            for (_, op) in calls {
                assert_eq!(op.operator.get_as_name(0).unwrap().name(), "New");
            }
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                original.source_bytes()
            );
            drop(merged);
            assert_eq!(pool.live_bytes(), plan_baseline);
            drop(plan);
            assert_eq!(pool.live_bytes(), baseline);
        }
    }

    #[test]
    fn local_edits_and_child_bindings_share_original_indices() {
        let original = ContentProgram::from_decoded_bytes(
            b"(Old) Tj /OldA Do 17 mystery /OldB Do".to_vec(),
            Default::default(),
        )
        .unwrap();
        let mut first = source(&[1]);
        first.invocations[0].operator_byte_offset = original.operations()[1].operator.byte_offset();
        let mut second = source(&[3]);
        second.invocations[0].operator_byte_offset =
            original.operations()[3].operator.byte_offset();
        let pool = OperationBudget::new(1024 * 1024);
        let tree = SpanInvocationTree::new(&[&first, &second], &Default::default(), &pool).unwrap();
        let baseline = pool.live_bytes();
        for (source, operation_count, call_indices) in
            [(&b"(New) Tj 0 Tc"[..], 2, [2, 4]), (&b""[..], 0, [0, 2])]
        {
            let edited = tree
                .rebuild_parent_with_replacements(
                    0,
                    &original,
                    &[OperationReplacement {
                        index: 0,
                        source,
                        operation_count,
                    }],
                    Default::default(),
                    &pool,
                    |_| BudgetedBytes::copy_from_slice(&pool, b"/New Do"),
                )
                .unwrap();
            for index in call_indices {
                assert_eq!(edited.as_ref().operations()[index].operator.name(), "Do");
                assert_eq!(
                    edited.as_ref().operations()[index]
                        .operator
                        .get_as_name(0)
                        .unwrap()
                        .name(),
                    "New"
                );
            }
            assert_eq!(
                edited.as_ref().operations()[call_indices[0] + 1]
                    .operator
                    .name(),
                "mystery"
            );
            drop(edited);
            assert_eq!(pool.live_bytes(), baseline);
        }
        let mut calls = 0;
        let rejected = tree.rebuild_parent_with_replacements(
            0,
            &original,
            &[OperationReplacement {
                index: 3,
                source: b"q",
                operation_count: 1,
            }],
            Default::default(),
            &pool,
            |_| {
                calls += 1;
                BudgetedBytes::copy_from_slice(&pool, b"/New Do")
            },
        );
        assert!(
            matches!(rejected, Err(PdfError::ObjectError(ref message)) if message == "span local replacement conflict")
        );
        assert_eq!(calls, 0);
        assert_eq!(pool.live_bytes(), baseline);
        assert_eq!(
            original.source_bytes(),
            b"(Old) Tj /OldA Do 17 mystery /OldB Do"
        );
    }

    #[test]
    fn parent_preflight_rejects_late_bad_call_before_binding() {
        let original = ContentProgram::from_decoded_bytes(
            b"/OldA Do 17 mystery /OldB Do".to_vec(),
            Default::default(),
        )
        .unwrap();
        let mut first = source(&[0]);
        first.invocations[0].operator_byte_offset = original.operations()[0].operator.byte_offset();
        let mut second = source(&[2]);
        second.invocations[0].operator_byte_offset = original.operations()[2]
            .operator
            .byte_offset()
            .map(|offset| offset + 1);
        let pool = OperationBudget::new(1024 * 1024);
        let tree = SpanInvocationTree::new(&[&first, &second], &Default::default(), &pool).unwrap();
        let baseline = pool.live_bytes();
        let mut calls = 0;
        let rejected = tree.rebuild_parent(0, &original, Default::default(), &pool, |_| {
            calls += 1;
            BudgetedBytes::copy_from_slice(&pool, b"/New Do")
        });
        assert!(
            matches!(rejected, Err(PdfError::ObjectError(ref message)) if message == "span parent call mismatch")
        );
        assert_eq!(calls, 0);
        assert_eq!(pool.live_bytes(), baseline);
        assert_eq!(original.source_bytes(), b"/OldA Do 17 mystery /OldB Do");
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
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            tree.rebuild_parent(0, &original, Default::default(), &pool, |child| {
                if tree.nodes[child].invocation.unwrap().operation_index == 2 {
                    std::panic::panic_any("second span binding unwind");
                }
                bind(child)
            })
        }));
        let payload = caught.err().expect("binding must unwind");
        assert_eq!(
            payload.downcast_ref::<&str>(),
            Some(&"second span binding unwind")
        );
        assert_eq!(pool.live_bytes(), baseline);
        assert_eq!(original.source_bytes(), b"/OldA Do 17 mystery /OldB Do");
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
