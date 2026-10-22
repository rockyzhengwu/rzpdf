//! Transactional static original-GID subset editing. Broader event selection
//! is handled separately; this entry does not reshape or fit replacement text.
use super::{
    PdfDocument,
    embedded_font::{EmbeddedSubsetFontObjects, TrueTypeEmbeddingOptions},
};
use crate::{
    budget::BudgetedBytes,
    error::{PdfError, PdfResult},
    font::truetype::{
        subset::{plan_subset_string, plan_subset_text_array_range, validate_subset_source},
        true_type_program::TrueTypeProgram,
    },
    page::{
        content_builder::ContentBuilder,
        content_program::{BudgetedContentProgram, ContentProgram, OperationReplacement},
    },
};
use std::{
    io::{Read, Seek},
    ops::Range,
};

pub struct SubsetTextEditOptions<'a> {
    /// Caller-supplied licensed full source, not a previously reduced subset.
    pub source: &'a [u8],
    pub embedding: TrueTypeEmbeddingOptions,
    /// Caller must choose a document-unique six-uppercase-letter tag.
    pub subset_tag: [u8; 6],
    pub omit_layout_and_auxiliary: bool,
    /// Compensate new-minus-removed advance before the retained suffix.
    pub preserve_advance: bool,
}

struct PreparedSubsetEdit {
    program: BudgetedContentProgram,
    _fragment: BudgetedBytes,
    _font: EmbeddedSubsetFontObjects,
    _binding: Option<crate::budget::MemoryLease>,
}

/// Original-index content ready for a caller's single transactional publication.
/// Registration mutates local resources; the caller must authenticate the source
/// first and keep the encompassing transaction open until all fragments commit.
pub(super) struct SubsetTextFragment {
    pub(super) index: usize,
    pub(super) fragment: BudgetedBytes,
    pub(super) operation_count: usize,
    pub(super) input_chars: usize,
    pub(super) output_chars: usize,
    pub(super) _font: EmbeddedSubsetFontObjects,
    pub(super) _binding: Option<crate::budget::MemoryLease>,
}

struct PreparedSubsetRangeEdit {
    plan: super::text_event_edit::TextEventEditPlan,
    _font: EmbeddedSubsetFontObjects,
    _binding: Option<crate::budget::MemoryLease>,
}
impl AsRef<ContentProgram> for PreparedSubsetRangeEdit {
    fn as_ref(&self) -> &ContentProgram {
        self.plan.as_ref()
    }
}
impl AsRef<ContentProgram> for PreparedSubsetEdit {
    fn as_ref(&self) -> &ContentProgram {
        self.program.as_ref()
    }
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Replace original text-show events within one selected leaf Form.
    /// The source anchors the start event; intervening child Form calls are
    /// preserved, not included in this leaf-local selection.
    #[expect(
        clippy::too_many_arguments,
        reason = "keep source identity, text geometry and operation budget explicit"
    )]
    pub fn replace_nested_text_event_range_with_subset_and_budget(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &crate::page::source_provenance::NestedDrawSource,
        range: Range<crate::page::content_program::TextCodePosition>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        source.validate_limits(&self.ctx.limits())?;
        if range.start.operation != source.operation_index {
            return Err(PdfError::ObjectError(
                "nested subset event range must anchor its original start".into(),
            ));
        }
        self.commit_nested_prepared_with_budget(
            page,
            root,
            source,
            || {
                let current_page = self.get_page_with_budget(page, budget)?;
                let mut chain =
                    current_page.prepare_form_chain_with_budget(root, source, budget)?;
                let leaf = chain
                    .last()
                    .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?;
                let (page_scope, _page_scope_lease) = current_page.resource_with_budget(budget)?;
                let (mut scope, mut scope_lease) = crate::objects::clone_dict_with_budget(
                    &leaf.resource_scope,
                    budget,
                    self.ctx.limits(),
                    0,
                )?;
                let mut edited = self.build_scoped_subset_event_range(
                    page,
                    &leaf.program,
                    range,
                    text,
                    options,
                    Some(&mut scope),
                    budget,
                    |operation| {
                        // Reconstruct each original event's inherited state without
                        // cloning a budgeted font or reading the newly bound scope.
                        let mut state = Default::default();
                        for (index, call) in source.invocations.iter().enumerate() {
                            let (program, resources) = if index == 0 {
                                (root, &page_scope)
                            } else {
                                (&*chain[index - 1].program, &chain[index - 1].resource_scope)
                            };
                            state = current_page.text_state_before_operation_in_scope_with_budget(
                                program,
                                call.operation_index,
                                resources,
                                state,
                                budget,
                            )?;
                        }
                        current_page
                            .text_state_before_operation_in_scope_with_budget(
                                &leaf.program,
                                operation,
                                &leaf.resource_scope,
                                state,
                                budget,
                            )?
                            .into_metrics()
                    },
                )?;
                edited
                    ._binding
                    .as_mut()
                    .ok_or_else(|| {
                        PdfError::PageResourceError("Nested subset binding absent".into())
                    })?
                    .absorb(&mut scope_lease)?;
                chain.last_mut().unwrap().resource_scope = scope;
                Ok((chain, edited))
            },
            budget,
        )
    }

    /// Replace a range in one nested text-show event (including multiple TJ
    /// strings), copying only the selected invocation chain. Source anchors
    /// the event; byte-code endpoints refer to the original leaf program.
    #[expect(
        clippy::too_many_arguments,
        reason = "keep source identity, text geometry and operation budget explicit"
    )]
    pub fn replace_nested_text_show_range_with_subset_and_budget(
        &self,
        page: u32,
        root: &ContentProgram,
        source: &crate::page::source_provenance::NestedDrawSource,
        range: Range<crate::page::content_program::TextCodePosition>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        source.validate_limits(&self.ctx.limits())?;
        if range.start.operation != source.operation_index
            || range.end.operation != source.operation_index
        {
            return Err(PdfError::ObjectError(
                "nested subset show range must anchor one original event".into(),
            ));
        }
        self.commit_nested_prepared_with_budget(
            page,
            root,
            source,
            || {
                let current_page = self.get_page_with_budget(page, budget)?;
                let mut chain =
                    current_page.prepare_form_chain_with_budget(root, source, budget)?;
                let leaf = chain
                    .last()
                    .ok_or_else(|| PdfError::PageResourceError("Nested leaf absent".into()))?;
                super::text_event_edit::endpoint(&leaf.program, range.start)?;
                super::text_event_edit::endpoint(&leaf.program, range.end)?;
                if range.start > range.end {
                    return Err(PdfError::ObjectError(
                        "nested subset show range is reversed".into(),
                    ));
                }
                let (page_scope, _page_scope_lease) = current_page.resource_with_budget(budget)?;
                let mut state = Default::default();
                for (index, call) in source.invocations.iter().enumerate() {
                    let (program, scope) = if index == 0 {
                        (root, &page_scope)
                    } else {
                        (&*chain[index - 1].program, &chain[index - 1].resource_scope)
                    };
                    state = current_page.text_state_before_operation_in_scope_with_budget(
                        program,
                        call.operation_index,
                        scope,
                        state,
                        budget,
                    )?;
                }
                let state = current_page
                    .text_state_before_operation_in_scope_with_budget(
                        &leaf.program,
                        source.operation_index,
                        &leaf.resource_scope,
                        state,
                        budget,
                    )?
                    .into_metrics()?;
                let leaf = chain.last_mut().unwrap();
                let prepared = self.build_scoped_subset_text_fragment(
                    page,
                    &leaf.program,
                    source.operation_index,
                    range.start.array_item,
                    range.end.array_item.map(|item| (item, range.end.offset)),
                    range.start.offset..range.end.offset,
                    text,
                    options,
                    state,
                    Some(&mut leaf.resource_scope),
                    budget,
                )?;
                let program = leaf.program.with_replaced_operation_sequences_with_budget(
                    &[OperationReplacement {
                        index: prepared.index,
                        source: prepared.fragment.as_slice(),
                        operation_count: prepared.operation_count,
                    }],
                    self.ctx.options(),
                    budget,
                )?;
                let edited = PreparedSubsetEdit {
                    program,
                    _fragment: prepared.fragment,
                    _font: prepared._font,
                    _binding: prepared._binding,
                };
                Ok((chain, edited))
            },
            budget,
        )
    }

    /// Replace a top-level range in original text-show order. Only the start
    /// event registers a new subset; later deletions use their own existing
    /// fonts. Non-text state and numeric TJ spacing remain in source order.
    pub fn replace_page_text_event_range_with_subset_and_budget(
        &self,
        page: u32,
        expected: &ContentProgram,
        range: Range<crate::page::content_program::TextCodePosition>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        self.edit_transaction(|doc| {
            doc.commit_page_content_edit_with_budget(page, expected, budget, || {
                let current_page = doc.get_page_with_budget(page, budget)?;
                doc.build_scoped_subset_event_range(
                    page,
                    expected,
                    range,
                    text,
                    options,
                    None,
                    budget,
                    |index| {
                        current_page.text_metrics_for_operation_with_budget(expected, index, budget)
                    },
                )
            })
        })
    }

    /// Authenticated callers supply original-scope metrics and keep the outer
    /// transaction open. This planner does not publish content or refresh indices.
    #[expect(
        clippy::too_many_arguments,
        reason = "keep source identity, text geometry and operation budget explicit"
    )]
    fn build_scoped_subset_event_range<F>(
        &self,
        page: u32,
        expected: &ContentProgram,
        range: Range<crate::page::content_program::TextCodePosition>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        scope: Option<&mut crate::objects::pdf_dict::PdfDict>,
        budget: &crate::budget::OperationBudget,
        mut resolve_metrics: F,
    ) -> PdfResult<PreparedSubsetRangeEdit>
    where
        F: FnMut(usize) -> PdfResult<crate::page::font_binding::BudgetedTextMetrics>,
    {
        let doc = self;
        use super::text_event_edit::{TextEventFragments, endpoint, full_event_range};
        endpoint(expected, range.start)?;
        endpoint(expected, range.end)?;
        if range.start > range.end {
            return Err(PdfError::ObjectError("text event range is reversed".into()));
        }
        let limits = doc.ctx.limits();
        let mut bytes = 0usize;
        let mut next = None;
        for index in range.start.operation..=range.end.operation {
            let op = &expected.operations()[index].operator;
            if !matches!(op.name(), "Tj" | "TJ" | "'" | "\"") {
                continue;
            }
            let full = full_event_range(expected, index)?;
            if index > range.start.operation && next.is_none() {
                next = full.as_ref().map(|full| full.start);
            }
            let mut charge = |length: usize| -> PdfResult<()> {
                bytes = bytes
                    .checked_add(length)
                    .filter(|&n| n <= limits.max_decoded_bytes)
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("subset event input code bytes".into())
                    })?;
                Ok(())
            };
            if op.name() == "TJ" {
                let array = op.get_as_array(0)?;
                if array.len() > limits.max_objects {
                    return Err(PdfError::ResourceLimit("text array items".into()));
                }
                for item in array.into_iter() {
                    if let Some(string) = item.as_string() {
                        charge(string.bytes().len())?;
                    }
                }
            } else {
                charge(
                    op.get_as_string(if op.name() == "\"" { 2 } else { 0 })?
                        .bytes()
                        .len(),
                )?;
            }
        }
        // Resolve and serialize all later original-font events before
        // the start event registration changes page resources/revision.
        let tail = if let Some(start) = next {
            Some(doc.build_text_event_fragments(
                expected,
                start..range.end,
                "",
                options.preserve_advance,
                budget,
                &mut resolve_metrics,
            )?)
        } else {
            None
        };
        let state = resolve_metrics(range.start.operation)?;
        let end = if range.start.operation == range.end.operation {
            range.end
        } else {
            full_event_range(expected, range.start.operation)?
                .ok_or_else(|| PdfError::ObjectError("subset start requires a string".into()))?
                .end
        };
        let first = doc.build_scoped_subset_text_fragment(
            page,
            expected,
            range.start.operation,
            range.start.array_item,
            end.array_item.map(|item| (item, end.offset)),
            range.start.offset..end.offset,
            text,
            options,
            state,
            scope,
            budget,
        )?;
        let head = TextEventFragments::single(
            first.index,
            first.fragment,
            first.operation_count,
            first.input_chars,
            first.output_chars,
            budget,
        )?;
        let plan = doc.finish_text_event_fragments(expected, &mut [Some(head), tail], budget)?;
        Ok(PreparedSubsetRangeEdit {
            plan,
            _font: first._font,
            _binding: first._binding,
        })
    }

    /// Replace a byte-code range in one top-level Tj, ' or ", generating a new local
    /// subset for retained prefix, replacement and suffix. Original font and
    /// non-target pages remain unchanged. Optional compensation keeps the
    /// original suffix/next-show advance; this does not support TJ/Form selection.
    pub fn replace_page_text_string_with_subset(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        range: Range<usize>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
    ) -> PdfResult<()> {
        let budget = self.ctx.operation_budget();
        self.replace_page_text_string_with_subset_and_budget(
            page, expected, operation, range, text, options, &budget,
        )
    }

    /// Same edit with a caller-owned pool for allocation coexistence and retries.
    /// Borrowed snapshots/source and existing parser allocations are not charged.
    #[expect(
        clippy::too_many_arguments,
        reason = "keep source identity, text geometry and operation budget explicit"
    )]
    pub fn replace_page_text_string_with_subset_and_budget(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        range: Range<usize>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        self.replace_subset_string_inner(
            page, expected, operation, None, None, range, text, options, budget,
        )
    }

    /// Replace a decoded byte-code range in one TJ string. Original numeric
    /// adjustments, other strings and shared font objects remain unchanged.
    /// This is the single-item entry; cross-item selections are planned separately.
    #[expect(
        clippy::too_many_arguments,
        reason = "keep source identity, text geometry and operation budget explicit"
    )]
    pub fn replace_page_text_array_item_with_subset_and_budget(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        item: usize,
        range: Range<usize>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        self.replace_subset_string_inner(
            page,
            expected,
            operation,
            Some(item),
            None,
            range,
            text,
            options,
            budget,
        )
    }

    /// Replace a range across string items of one TJ, retaining every original
    /// numeric adjustment. Endpoints include numeric slots in their item index.
    #[expect(
        clippy::too_many_arguments,
        reason = "keep source identity, text geometry and operation budget explicit"
    )]
    pub fn replace_page_text_array_range_with_subset_and_budget(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        range: Range<(usize, usize)>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        self.replace_subset_string_inner(
            page,
            expected,
            operation,
            Some(range.start.0),
            Some(range.end),
            range.start.1..range.end.1,
            text,
            options,
            budget,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "keep source identity, text geometry and operation budget explicit"
    )]
    fn replace_subset_string_inner(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        array_item: Option<usize>,
        array_end: Option<(usize, usize)>,
        range: Range<usize>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<()> {
        self.edit_transaction(|doc| {
            doc.commit_page_content_edit_with_budget(page, expected, budget, || {
                let current_page = doc.get_page_with_budget(page, budget)?;
                let state = current_page
                    .text_metrics_for_operation_with_budget(expected, operation, budget)?;
                let prepared = doc.build_subset_text_fragment(
                    page, expected, operation, array_item, array_end, range, text, options, state,
                    budget,
                )?;
                let program = expected.with_replaced_operation_sequences_with_budget(
                    &[OperationReplacement {
                        index: prepared.index,
                        source: prepared.fragment.as_slice(),
                        operation_count: prepared.operation_count,
                    }],
                    doc.ctx.options(),
                    budget,
                )?;
                Ok(PreparedSubsetEdit {
                    program,
                    _fragment: prepared.fragment,
                    _font: prepared._font,
                    _binding: prepared._binding,
                })
            })
        })
    }

    /// Does not commit content or refresh snapshots. Effective metrics must come
    /// from the authenticated original program, before resource registration.
    #[expect(
        clippy::too_many_arguments,
        reason = "keep source identity, text geometry and operation budget explicit"
    )]
    pub(super) fn build_subset_text_fragment(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        array_item: Option<usize>,
        array_end: Option<(usize, usize)>,
        range: Range<usize>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        state: crate::page::font_binding::BudgetedTextMetrics,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<SubsetTextFragment> {
        self.build_scoped_subset_text_fragment(
            page, expected, operation, array_item, array_end, range, text, options, state, None,
            budget,
        )
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "keep source identity, text geometry and operation budget explicit"
    )]
    pub(super) fn build_scoped_subset_text_fragment(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        array_item: Option<usize>,
        array_end: Option<(usize, usize)>,
        range: Range<usize>,
        text: &str,
        options: SubsetTextEditOptions<'_>,
        state: crate::page::font_binding::BudgetedTextMetrics,
        scope: Option<&mut crate::objects::pdf_dict::PdfDict>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<SubsetTextFragment> {
        let doc = self;
        let limits = doc.ctx.limits();
        let target = &expected
            .operations()
            .get(operation)
            .ok_or_else(|| PdfError::ObjectError("Subset text operation out of range".into()))?
            .operator;
        let original_codes = match (array_item, target.name(), target.num_operands()) {
            (None, "Tj" | "'", 1) => target.get_as_string(0)?.bytes(),
            (None, "\"", 3) => target.get_as_string(2)?.bytes(),
            (Some(item), "TJ", 1) => target
                .get_as_array(0)?
                .get(item)
                .and_then(|object| object.as_string())
                .ok_or_else(|| {
                    PdfError::ObjectError("Subset TJ endpoint requires a string".into())
                })?
                .bytes(),
            _ => {
                return Err(PdfError::UnsupportedFeature(
                    "Subset string entry requires a matching text show".into(),
                ));
            }
        };
        let vertical = state.font.writting_mode() == crate::font::WritingMode::Vertical;
        let embedded = state.font.fontfile().ok_or_else(|| {
            PdfError::FontError("Subset edit requires an embedded TrueType font".into())
        })?;
        validate_subset_source(embedded, options.source, limits)?;
        let source = TrueTypeProgram::try_new(options.source)?;
        let array_range =
            array_item.map(|item| (item, range.start)..array_end.unwrap_or((item, range.end)));
        let planned = if let Some(array_range) = &array_range {
            plan_subset_text_array_range(
                &state.font,
                &source,
                target.get_as_array(0)?,
                array_range.clone(),
                text,
                limits,
                budget,
            )?
        } else {
            plan_subset_string(
                &state.font,
                &source,
                original_codes,
                range.clone(),
                text,
                limits,
                budget,
            )?
        };
        let planned_text = std::str::from_utf8(planned.as_slice())
            .map_err(|_| PdfError::FontError("Invalid planned Unicode".into()))?;
        let mut input_chars = 0usize;
        let mut count = |codes: &[u8]| -> PdfResult<()> {
            input_chars = input_chars
                .checked_add(state.font.visit_chars_limited(
                    codes,
                    limits.max_text_chars.saturating_sub(input_chars),
                    |_| Ok(()),
                )?)
                .filter(|&n| n <= limits.max_text_chars)
                .ok_or_else(|| PdfError::ResourceLimit("subset input characters".into()))?;
            Ok(())
        };
        if array_range.is_some() {
            for item in target.get_as_array(0)?.into_iter() {
                if let Some(string) = item.as_string() {
                    count(string.bytes())?;
                }
            }
        } else {
            count(original_codes)?;
        }
        let output_chars = planned_text.chars().count();
        // Snapshot validation above precedes registration. Both this
        // nested registration and content publication roll back together.
        let registered = doc
            .resources()
            .embed_truetype_subset_font_objects_with_mode(
                if scope.is_some() { None } else { Some(page) },
                options.source,
                planned_text,
                options.embedding,
                options.subset_tag,
                options.omit_layout_and_auxiliary,
                budget,
                vertical,
            )?;
        let codes = registered
            .font
            .encoding()
            .encode_text_with_budget(text, budget)?;
        let binding = scope
            .map(|scope| {
                doc.bind_local_font_with_budget(
                    scope,
                    registered.font.object_handle().object_id(),
                    budget,
                )
            })
            .transpose()?;
        let font_selection = ContentBuilder::font_selection_with_budget(
            binding
                .as_ref()
                .map(|(name, _)| name.as_str())
                .or_else(|| registered.font.resource_name())
                .ok_or_else(|| PdfError::PageResourceError("Subset font binding absent".into()))?,
            state.metrics.font_size,
            limits,
            budget,
        )?;
        let mut quote = std::io::Cursor::new([0u8; 128]);
        let extra_operations = match target.name() {
            "'" => {
                use std::io::Write;
                quote
                    .write_all(b"T*\n")
                    .map_err(|_| PdfError::ObjectError("Quote normalization overflow".into()))?;
                1
            }
            "\"" => {
                use std::io::Write;
                write!(
                    &mut quote,
                    "{} Tw\n{} Tc\nT*\n",
                    state.metrics.word_space, state.metrics.char_space
                )
                .map_err(|_| PdfError::ObjectError("Quote normalization overflow".into()))?;
                3
            }
            _ => 0,
        };
        let quote_length = quote.position() as usize;
        let replacement_state = if options.preserve_advance {
            let reference = crate::objects::PdfObject::PdfReference(
                crate::objects::pdf_reference::PdfReference::from_id(
                    registered.font.object_handle().object_id(),
                ),
            );
            let (object, _object_lease) = doc.ctx.resolve_owned_with_budget(&reference, budget)?;
            let (font, lease) = crate::font::pdf_font::Font::try_new_with_budget(
                object.as_dict().ok_or_else(|| {
                    PdfError::FontError("Registered font is not a dictionary".into())
                })?,
                &doc.ctx,
                budget,
            )?;
            Some(state.with_replacement_font(font, lease))
        } else {
            None
        };
        let (prefix, suffix) = if let Some(array_range) = &array_range {
            let array = target.get_as_array(0)?;
            // Only old-font codes enter the existing replacement engine.
            // The new-font bytes are shown separately at the start point.
            let replacements = super::content_edit::budgeted_text_array_range_replacements(
                budget,
                &state.font,
                array,
                array_range.clone(),
                b"",
                limits,
            )?;
            let borrowed =
                super::content_edit::BudgetedTextItems::new(budget, replacements.as_slice())?;
            let adjustments = if let Some(replacement) = &replacement_state {
                let first = std::cell::Cell::new(true);
                Some(super::content_edit::budgeted_text_array_range_adjustments(
                    budget,
                    array,
                    array_range,
                    b"",
                    borrowed.as_slice(),
                    |removed, _| {
                        state.replacement_adjustment_to(
                            removed,
                            replacement,
                            if first.replace(false) {
                                codes.as_slice()
                            } else {
                                b""
                            },
                            limits,
                        )
                    },
                )?)
            } else {
                None
            };
            let rebuilt = if let Some(adjustments) = &adjustments {
                expected.with_adjusted_text_array_items_codes_with_budget(
                    operation,
                    borrowed.as_slice(),
                    adjustments.as_slice(),
                    doc.ctx.options(),
                    budget,
                )?
            } else {
                expected.with_replaced_text_array_items_codes_with_budget(
                    operation,
                    borrowed.as_slice(),
                    doc.ctx.options(),
                    budget,
                )?
            };
            // BeforeSuffix serializes the first retained prefix as its
            // original item. Split just after it, before its adjustment.
            rebuilt
                .as_ref()
                .text_array_split_string_fragments_with_budget(
                    operation,
                    array_range.start.0,
                    array_range.start.1..array_range.start.1,
                    doc.ctx.options(),
                    budget,
                )?
        } else {
            (
                ContentBuilder::encoded_text_with_budget(
                    &original_codes[..range.start],
                    limits,
                    budget,
                )?,
                ContentBuilder::encoded_text_with_budget(
                    &original_codes[range.end..],
                    limits,
                    budget,
                )?,
            )
        };
        let show = if let (None, Some(replacement)) = (&array_range, &replacement_state) {
            let adjustment = state.replacement_adjustment_to(
                &original_codes[range.clone()],
                replacement,
                codes.as_slice(),
                limits,
            )?;
            ContentBuilder::adjusted_encoded_text_with_budget(
                codes.as_slice(),
                adjustment,
                limits,
                budget,
            )?
        } else {
            ContentBuilder::encoded_text_with_budget(codes.as_slice(), limits, budget)?
        };
        let length = font_selection
            .as_slice()
            .len()
            .checked_add(show.as_slice().len())
            .and_then(|length| length.checked_add(prefix.as_slice().len()))
            .and_then(|length| length.checked_add(suffix.as_slice().len()))
            .and_then(|length| length.checked_add(4))
            .and_then(|length| length.checked_add(quote_length))
            .filter(|&length| {
                length <= limits.max_raw_stream_bytes && length <= limits.max_decoded_bytes
            })
            .ok_or_else(|| PdfError::ResourceLimit("subset edit fragment bytes".into()))?;
        let mut lease = budget.reserve(length)?;
        let mut fragment = Vec::new();
        fragment
            .try_reserve_exact(length)
            .map_err(|_| PdfError::ResourceLimit("subset edit fragment allocation".into()))?;
        lease.resize(fragment.capacity())?;
        fragment.extend_from_slice(&quote.get_ref()[..quote_length]);
        fragment.extend_from_slice(prefix.as_slice());
        fragment.extend_from_slice(b"q\n");
        fragment.extend_from_slice(font_selection.as_slice());
        fragment.extend_from_slice(show.as_slice());
        fragment.extend_from_slice(b"Q\n");
        fragment.extend_from_slice(suffix.as_slice());
        let fragment = BudgetedBytes::from_reserved_vec(fragment, lease)?;
        Ok(SubsetTextFragment {
            index: operation,
            fragment,
            operation_count: 6 + extra_operations,
            input_chars,
            output_chars,
            _font: registered,
            _binding: binding.map(|(_, lease)| lease),
        })
    }
}

#[cfg(test)]
mod metric_lifecycle_tests {
    use super::*;
    use crate::budget::{MemoryLease, OperationBudget};

    struct SaturatedEdit {
        prepared: PreparedSubsetEdit,
        _remaining: MemoryLease,
    }
    impl AsRef<ContentProgram> for SaturatedEdit {
        fn as_ref(&self) -> &ContentProgram {
            self.prepared.as_ref()
        }
    }

    #[test]
    fn prepared_subset_metric_lease_coexists_with_failed_content_publication() {
        let source = crate::font::truetype::subset::tests::fixture();
        let metadata = || TrueTypeEmbeddingOptions {
            postscript_name: "Demo".into(),
            cap_height: 60.0,
            stem_v: 80.0,
            serif: false,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let original_pool = OperationBudget::new(16 * 1024 * 1024);
        let original = doc
            .resources()
            .embed_truetype_subset_font(
                0,
                &source,
                "A",
                metadata(),
                *b"BASEAA",
                false,
                &original_pool,
            )
            .unwrap();
        doc.replace_page_contents(
            0,
            format!(
                "BT /{} 24 Tf 48 760 Td <0001> Tj ET",
                original.font.resource_name,
            )
            .into_bytes(),
        )
        .unwrap();
        let before = doc.save_to_bytes().unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let pool = OperationBudget::new(16 * 1024 * 1024);
        let neighbor = pool.reserve(7).unwrap();
        let prepared_seen = std::cell::Cell::new(false);
        let result = doc.edit_transaction(|doc| {
            doc.commit_page_content_edit_with_budget(0, &snapshot, &pool, || {
                let page = doc.get_page_with_budget(0, &pool)?;
                let state = page.text_metrics_for_operation_with_budget(&snapshot, 3, &pool)?;
                let fragment = doc.build_subset_text_fragment(
                    0,
                    &snapshot,
                    3,
                    None,
                    None,
                    0..2,
                    "B",
                    SubsetTextEditOptions {
                        source: &source,
                        embedding: metadata(),
                        subset_tag: *b"FAILAA",
                        omit_layout_and_auxiliary: false,
                        preserve_advance: true,
                    },
                    state,
                    &pool,
                )?;
                let program = snapshot.with_replaced_operation_sequences_with_budget(
                    &[OperationReplacement {
                        index: fragment.index,
                        source: fragment.fragment.as_slice(),
                        operation_count: fragment.operation_count,
                    }],
                    doc.ctx.options(),
                    &pool,
                )?;
                let prepared = PreparedSubsetEdit {
                    program,
                    _fragment: fragment.fragment,
                    _font: fragment._font,
                    _binding: fragment._binding,
                };
                assert!(prepared._font.font.metric_bytes() > 0);
                assert!(pool.live_bytes() >= 7 + prepared._font.font.metric_bytes());
                let remaining = pool.reserve(pool.maximum_bytes() - pool.live_bytes())?;
                prepared_seen.set(true);
                Ok(SaturatedEdit {
                    prepared,
                    _remaining: remaining,
                })
            })
        });
        assert!(
            prepared_seen.get(),
            "must reach publication, not fail in preparation"
        );
        assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        assert_eq!(pool.live_bytes(), 7);
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        // The rollback invalidated the old snapshot. Retry with a fresh one.
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        doc.replace_page_text_string_with_subset_and_budget(
            0,
            &snapshot,
            3,
            0..2,
            "B",
            SubsetTextEditOptions {
                source: &source,
                embedding: metadata(),
                subset_tag: *b"GOODAA",
                omit_layout_and_auxiliary: false,
                preserve_advance: true,
            },
            &pool,
        )
        .unwrap();
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        drop(original);
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(original_pool.live_bytes(), 0);
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(
            reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order()
                .unwrap()[0]
                .unicode
                .as_deref(),
            Some("B")
        );
    }
}
