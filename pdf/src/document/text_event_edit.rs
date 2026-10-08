use super::{PdfDocument, content_edit};
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    objects::PdfObject,
    page::{
        content_program::{
            BudgetedContentProgram, ContentProgram, OperationReplacement, TextCodePosition,
        },
        font_binding::BudgetedTextMetrics,
    },
};

type PreparedEvent = (
    usize,
    BudgetedBytes,
    usize,
    BudgetedTextMetrics,
    BudgetedBytes,
);

/// Own generated content and every actual font/code/fragment allocation until
/// the caller's transaction has consumed the result.
pub(super) struct TextEventEditPlan {
    program: BudgetedContentProgram,
    _events: OwnedTable<PreparedEvent>,
}

impl AsRef<ContentProgram> for TextEventEditPlan {
    fn as_ref(&self) -> &ContentProgram {
        self.program.as_ref()
    }
}
use std::{
    io::{Read, Seek},
    ops::Range,
};

/// Metadata and its real allocation remain under the same owner.
struct OwnedTable<T> {
    items: Vec<T>,
    _lease: MemoryLease,
}

impl<T> OwnedTable<T> {
    fn new(budget: &OperationBudget, count: usize) -> PdfResult<Self> {
        let size = std::mem::size_of::<T>();
        let mut lease = budget.reserve(
            count
                .checked_mul(size)
                .ok_or_else(|| PdfError::ResourceLimit("text event metadata".into()))?,
        )?;
        let mut items = Vec::new();
        items
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("text event table allocation".into()))?;
        lease.resize(
            items
                .capacity()
                .checked_mul(size)
                .ok_or_else(|| PdfError::ResourceLimit("text event metadata".into()))?,
        )?;
        Ok(Self {
            items,
            _lease: lease,
        })
    }
}

fn is_text(name: &str) -> bool {
    matches!(name, "Tj" | "TJ" | "'" | "\"")
}

fn endpoint(program: &ContentProgram, position: TextCodePosition) -> PdfResult<()> {
    let target = &program
        .operations()
        .get(position.operation)
        .ok_or_else(|| PdfError::ObjectError("text event endpoint out of range".into()))?
        .operator;
    let bytes = match (target.name(), target.num_operands(), position.array_item) {
        ("Tj" | "'", 1, None) => target.get_as_string(0)?.bytes(),
        ("\"", 3, None) => {
            target.get_as_f32(0)?;
            target.get_as_f32(1)?;
            target.get_as_string(2)?.bytes()
        }
        ("TJ", 1, Some(item)) => target
            .get_as_array(0)?
            .get(item)
            .and_then(PdfObject::as_string)
            .ok_or_else(|| {
                PdfError::ObjectError("text event endpoint requires a TJ string".into())
            })?
            .bytes(),
        _ => {
            return Err(PdfError::ObjectError(
                "text event endpoint operator/item mismatch".into(),
            ));
        }
    };
    if position.offset > bytes.len() {
        return Err(PdfError::ObjectError(
            "text event endpoint offset out of range".into(),
        ));
    }
    Ok(())
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Replace a top-level text-event range without compensating removed advance.
    /// New text uses the start event's existing font; following text moves by the
    /// new-minus-removed advance unless intervening matrix/line operations reset it.
    pub fn replace_page_text_event_range(
        &self,
        page: u32,
        expected: &ContentProgram,
        range: Range<TextCodePosition>,
        text: &str,
    ) -> PdfResult<()> {
        self.replace_page_text_event_range_inner(page, expected, range, text, false)
    }

    /// Replace a decoded-code range across top-level text-show events in source
    /// order, including intervening font/spacing/matrix/line state changes.
    /// Unicode is encoded only into the start event's effective existing font.
    /// Every removed segment is compensated using its own original metrics,
    /// before any retained suffix. Non-text operations and TJ numbers are kept.
    /// This is not search, shaping, layout fitting or a cross-Form selection.
    pub fn replace_page_text_event_range_preserving_advance(
        &self,
        page: u32,
        expected: &ContentProgram,
        range: Range<TextCodePosition>,
        text: &str,
    ) -> PdfResult<()> {
        self.replace_page_text_event_range_inner(page, expected, range, text, true)
    }

    fn replace_page_text_event_range_inner(
        &self,
        page: u32,
        expected: &ContentProgram,
        range: Range<TextCodePosition>,
        text: &str,
        preserve_advance: bool,
    ) -> PdfResult<()> {
        let budget = self.ctx.limits().operation_budget();
        self.commit_page_content_edit_with_budget(page, expected, &budget, || {
            let current_page = self.get_page_with_budget(page, &budget)?;
            self.build_text_event_edit_plan(
                expected,
                range,
                text,
                preserve_advance,
                &budget,
                |index| {
                    current_page.text_metrics_for_operation_with_budget(expected, index, &budget)
                },
            )
        })
    }

    /// Scoped callers provide original effective metrics without committing
    /// individual events or changing the source indices between replacements.
    pub(super) fn build_text_event_edit_plan<F>(
        &self,
        expected: &ContentProgram,
        range: Range<TextCodePosition>,
        text: &str,
        preserve_advance: bool,
        budget: &OperationBudget,
        mut resolve_metrics: F,
    ) -> PdfResult<TextEventEditPlan>
    where
        F: FnMut(usize) -> PdfResult<BudgetedTextMetrics>,
    {
        let limits = self.ctx.limits();
        endpoint(expected, range.start)?;
        endpoint(expected, range.end)?;
        if range.start > range.end {
            return Err(PdfError::ObjectError("text event range is reversed".into()));
        }
        let count = expected.operations()[range.start.operation..=range.end.operation]
            .iter()
            .filter(|op| is_text(op.operator.name()))
            .count();
        if count > limits.max_objects {
            return Err(PdfError::ResourceLimit("text event count".into()));
        }
        let mut prepared = OwnedTable::new(&budget, count)?;
        let mut input_chars = 0usize;
        let mut output_chars = 0usize;
        for index in range.start.operation..=range.end.operation {
            let target = &expected.operations()[index].operator;
            if !is_text(target.name()) {
                continue;
            }
            // An empty/numeric-only intervening TJ has no strings to remove.
            if target.name() == "TJ" && target.num_operands() != 1 {
                return Err(PdfError::ObjectError("invalid TJ event".into()));
            }
            if target.name() == "TJ" && target.get_as_array(0)?.len() > limits.max_objects {
                return Err(PdfError::ResourceLimit("text array items".into()));
            }
            if target.name() == "TJ"
                && !target
                    .get_as_array(0)?
                    .into_iter()
                    .any(|item| item.as_string().is_some())
            {
                if target
                    .get_as_array(0)?
                    .into_iter()
                    .any(|item| item.as_number().is_none())
                {
                    return Err(PdfError::ObjectError(
                        "TJ item must be string or number".into(),
                    ));
                }
                continue;
            }
            let state = resolve_metrics(index)?;
            let codes = state.font.encode_text_with_budget(
                if index == range.start.operation {
                    text
                } else {
                    ""
                },
                limits,
                &budget,
            )?;
            let count_chars = |bytes: &[u8], total: &mut usize| -> PdfResult<()> {
                *total = total
                    .checked_add(state.font.visit_chars_limited(
                        bytes,
                        limits.max_text_chars.saturating_sub(*total),
                        |_| Ok(()),
                    )?)
                    .filter(|&count| count <= limits.max_text_chars)
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("aggregate text event characters".into())
                    })?;
                Ok(())
            };
            let (fragment, operation_count) = if target.name() == "TJ" {
                if target.num_operands() != 1 {
                    return Err(PdfError::ObjectError("invalid TJ event".into()));
                }
                let array = target.get_as_array(0)?;
                let first = array
                    .into_iter()
                    .position(|item| item.as_string().is_some())
                    .unwrap();
                let last = array
                    .into_iter()
                    .enumerate()
                    .filter_map(|(index, item)| item.as_string().map(|_| index))
                    .last()
                    .unwrap();
                let start = if index == range.start.operation {
                    (range.start.array_item.unwrap(), range.start.offset)
                } else {
                    (first, 0)
                };
                let end = if index == range.end.operation {
                    (range.end.array_item.unwrap(), range.end.offset)
                } else {
                    (
                        last,
                        array.get(last).unwrap().as_string().unwrap().bytes().len(),
                    )
                };
                let item_range = start..end;
                content_edit::validate_text_array_range(array, &item_range, limits)?;
                let changed = (start.0..=end.0)
                    .filter(|&item| array.get(item).and_then(PdfObject::as_string).is_some())
                    .count();
                if preserve_advance
                    && changed
                        .checked_mul(2)
                        .and_then(|count| count.checked_add(array.len()))
                        .is_none_or(|count| count > limits.max_objects)
                {
                    return Err(PdfError::ResourceLimit("adjusted text array items".into()));
                }
                let replacements = content_edit::budgeted_text_array_range_replacements(
                    &budget,
                    &state.font,
                    array,
                    item_range.clone(),
                    codes.as_slice(),
                    limits,
                )?;
                let borrowed =
                    content_edit::BudgetedTextItems::new(&budget, replacements.as_slice())?;
                let adjustments = if preserve_advance {
                    Some(content_edit::budgeted_text_array_range_adjustments(
                        &budget,
                        array,
                        &item_range,
                        codes.as_slice(),
                        borrowed.as_slice(),
                        |old, new| state.replacement_adjustment(old, new, limits),
                    )?)
                } else {
                    None
                };
                let mut next = 0;
                for (item, object) in array.into_iter().enumerate() {
                    if let Some(string) = object.as_string() {
                        count_chars(string.bytes(), &mut input_chars)?;
                        let bytes = if borrowed
                            .as_slice()
                            .get(next)
                            .is_some_and(|&(index, _)| index == item)
                        {
                            let bytes = borrowed.as_slice()[next].1;
                            next += 1;
                            bytes
                        } else {
                            string.bytes()
                        };
                        count_chars(bytes, &mut output_chars)?;
                    }
                }
                (
                    if let Some(adjustments) = adjustments {
                        expected.adjusted_text_array_items_fragment_with_budget(
                            index,
                            borrowed.as_slice(),
                            adjustments.as_slice(),
                            self.ctx.options(),
                            &budget,
                        )?
                    } else {
                        expected.text_array_items_fragment_with_budget(
                            index,
                            borrowed.as_slice(),
                            self.ctx.options(),
                            &budget,
                        )?
                    },
                    1,
                )
            } else {
                let old = match (target.name(), target.num_operands()) {
                    ("Tj" | "'", 1) => target.get_as_string(0)?.bytes(),
                    ("\"", 3) => target.get_as_string(2)?.bytes(),
                    _ => {
                        return Err(PdfError::ObjectError("invalid single-string event".into()));
                    }
                };
                let start = if index == range.start.operation {
                    range.start.offset
                } else {
                    0
                };
                let end = if index == range.end.operation {
                    range.end.offset
                } else {
                    old.len()
                };
                let size = old
                    .len()
                    .checked_sub(end - start)
                    .and_then(|size| size.checked_add(codes.as_slice().len()))
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("text event replacement bytes".into())
                    })?;
                let lease = budget.reserve(size)?;
                let replacement = content_edit::text_range_replacement(
                    &state.font,
                    old,
                    start..end,
                    codes.as_slice(),
                    limits,
                )?;
                let replacement =
                    crate::budget::BudgetedBytes::from_reserved_vec(replacement, lease)?;
                count_chars(old, &mut input_chars)?;
                count_chars(replacement.as_slice(), &mut output_chars)?;
                if preserve_advance {
                    let adjustment =
                        state.replacement_adjustment(&old[start..end], codes.as_slice(), limits)?;
                    expected.adjusted_text_range_fragment_with_budget(
                        index,
                        replacement.as_slice(),
                        start
                            .checked_add(codes.as_slice().len())
                            .ok_or_else(|| PdfError::ResourceLimit("text suffix offset".into()))?,
                        adjustment,
                        self.ctx.options(),
                        &budget,
                    )?
                } else {
                    (
                        expected.text_codes_fragment_with_budget(
                            index,
                            replacement.as_slice(),
                            self.ctx.options(),
                            &budget,
                        )?,
                        1,
                    )
                }
            };
            // Retain each actual font/encoding owner and generated fragment
            // across final parsing and transaction commit, never bare-clone.
            prepared
                .items
                .push((index, fragment, operation_count, state, codes));
        }
        let mut borrowed = OwnedTable::new(&budget, prepared.items.len())?;
        for (index, fragment, count, _, _) in &prepared.items {
            borrowed.items.push(OperationReplacement {
                index: *index,
                source: fragment.as_slice(),
                operation_count: *count,
            });
        }
        let program = expected.with_replaced_operation_sequences_with_budget(
            &borrowed.items,
            self.ctx.options(),
            &budget,
        )?;
        drop(borrowed);
        Ok(TextEventEditPlan {
            program,
            _events: prepared,
        })
    }
}
