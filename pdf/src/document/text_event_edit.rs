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

type PreparedEvent = (usize, BudgetedBytes, usize);

/// Own generated content and its fragment leases until transaction consumption.
/// Temporary metrics and encoding owners are released after serialization.
pub(super) struct TextEventEditPlan {
    program: BudgetedContentProgram,
    _events: OwnedTable<PreparedEvent>,
    input_chars: usize,
    output_chars: usize,
}

#[cfg(test)]
mod fragment_owner_tests {
    use super::*;

    #[test]
    fn invalid_fragment_parse_preserves_single_and_multiple_donors() {
        for count in [1, 2] {
            let doc = PdfDocument::new().unwrap();
            let expected = ContentProgram::from_decoded_bytes(
                b"(A) Tj (B) Tj".to_vec(), doc.ctx.options(),
            ).unwrap();
            let budget = OperationBudget::new(1024 * 1024);
            let mut donors = Vec::new();
            for index in 0..count {
                let mut events = OwnedTable::new(&budget, 1).unwrap();
                let bytes: &[u8] = if index == 0 { b"(" } else { b"(Y) Tj" };
                events.items.push((index, BudgetedBytes::copy_from_slice(&budget, bytes).unwrap(), 1));
                donors.push(Some(TextEventFragments { events, input_chars: 1, output_chars: 1 }));
            }
            let retained = budget.live_bytes();
            let failure = doc.finish_text_event_fragments(&expected, &mut donors, &budget);
            assert!(failure.is_err());
            assert!(!matches!(failure, Err(PdfError::ResourceLimit(_))),
                "invalid syntax must reach fragment parsing, not fail on budget");
            assert!(donors.iter().all(Option::is_some));
            assert_eq!(budget.live_bytes(), retained);
            assert_eq!(donors[0].as_ref().unwrap().events.items[0].1.as_slice(), b"(");
            donors[0].as_mut().unwrap().events.items[0].1 =
                BudgetedBytes::copy_from_slice(&budget, b"(X) Tj").unwrap();
            let result = doc.finish_text_event_fragments(&expected, &mut donors, &budget).unwrap();
            assert!(donors.iter().all(Option::is_none));
            assert_eq!(result.character_counts(), (count, count));
            assert_eq!(result.original_replacements().len(), count);
            drop(result);
            assert_eq!(budget.live_bytes(), 0);
        }
    }
}

/// Generated original-index fragments without an intermediate full program.
pub(super) struct TextEventFragments {
    events: OwnedTable<PreparedEvent>,
    input_chars: usize,
    output_chars: usize,
}

impl TextEventFragments {
    pub(super) fn single(
        index: usize, fragment: BudgetedBytes, operation_count: usize,
        input_chars: usize, output_chars: usize, budget: &OperationBudget,
    ) -> PdfResult<Self> {
        let mut events = OwnedTable::new(budget, 1)?;
        events.items.push((index, fragment, operation_count));
        Ok(Self { events, input_chars, output_chars })
    }
}

impl AsRef<ContentProgram> for TextEventEditPlan {
    fn as_ref(&self) -> &ContentProgram {
        self.program.as_ref()
    }
}

impl TextEventEditPlan {
    pub(super) fn character_counts(&self) -> (usize, usize) {
        (self.input_chars, self.output_chars)
    }
    /// Borrow owned fragments at original indices. A tree parent can merge them
    /// with child Do edits; no fragment borrows temporary font/encoding data.
    pub(super) fn original_replacements(
        &self,
    ) -> impl ExactSizeIterator<Item = OperationReplacement<'_>> {
        self._events
            .items
            .iter()
            .map(|(index, fragment, count)| OperationReplacement {
                index: *index,
                source: fragment.as_slice(),
                operation_count: *count,
            })
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

/// Full lexical code range of one show. Numeric-only and empty TJ events have
/// no editable strings but must remain in expanded selection order unchanged.
pub(super) fn full_event_range(
    program: &ContentProgram,
    index: usize,
) -> PdfResult<Option<Range<TextCodePosition>>> {
    let op = &program.operations().get(index)
        .ok_or_else(|| PdfError::ObjectError("span event index out of range".into()))?.operator;
    let (first, last, len) = match (op.name(), op.num_operands()) {
        ("Tj" | "'", 1) => (None, None, op.get_as_string(0)?.bytes().len()),
        ("\"", 3) => {
            op.get_as_f32(0)?;
            op.get_as_f32(1)?;
            (None, None, op.get_as_string(2)?.bytes().len())
        }
        ("TJ", 1) => {
            let mut first = None;
            let mut last = None;
            let mut len = 0;
            for (item, object) in op.get_as_array(0)?.into_iter().enumerate() {
                match object {
                    PdfObject::PdfString(string) => {
                        first.get_or_insert(item);
                        last = Some(item);
                        len = string.bytes().len();
                    }
                    PdfObject::PdfNumber(_) => (),
                    _ => return Err(PdfError::ObjectError("span TJ item type".into())),
                }
            }
            if first.is_none() {
                return Ok(None);
            }
            (first, last, len)
        }
        _ => return Err(PdfError::ObjectError("span event operator/operand mismatch".into())),
    };
    Ok(Some(TextCodePosition { operation: index, array_item: first, offset: 0 }
        ..TextCodePosition { operation: index, array_item: last, offset: len }))
}

pub(super) fn endpoint(program: &ContentProgram, position: TextCodePosition) -> PdfResult<()> {
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

#[cfg(test)]
mod span_range_tests {
    use super::*;

    #[test]
    fn full_event_ranges_keep_tj_numeric_slots_and_quote_operands() {
        let program = ContentProgram::from_decoded_bytes(
            b"[(A) 17 () -3 (BC) 9] TJ [] TJ [17 -3] TJ () Tj (D) ' 2 3 (EF) \""
                .to_vec(),
            Default::default(),
        ).unwrap();
        let range = full_event_range(&program, 0).unwrap().unwrap();
        assert_eq!((range.start.array_item, range.start.offset), (Some(0), 0));
        assert_eq!((range.end.array_item, range.end.offset), (Some(4), 2));
        for index in [1, 2] {
            assert!(full_event_range(&program, index).unwrap().is_none());
        }
        for (index, len) in [(3, 0), (4, 1), (5, 2)] {
            let range = full_event_range(&program, index).unwrap().unwrap();
            assert_eq!((range.start.array_item, range.start.offset), (None, 0));
            assert_eq!((range.end.array_item, range.end.offset), (None, len));
            endpoint(&program, range.start).unwrap();
            endpoint(&program, range.end).unwrap();
        }
        assert!(full_event_range(&program, 6).is_err());
    }

    #[test]
    fn full_event_ranges_reject_invalid_shows_without_fabricating_endpoints() {
        for bytes in [b"[true] TJ".as_slice(), b"[17 (A) null] TJ", b"() () Tj",
            b"2 (A) \"", b"2 true (A) \"", b"12 Tf"] {
            let program = ContentProgram::from_decoded_bytes(bytes.to_vec(), Default::default()).unwrap();
            assert!(full_event_range(&program, 0).is_err(), "{bytes:?}");
        }
    }
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
        resolve_metrics: F,
    ) -> PdfResult<TextEventEditPlan>
    where
        F: FnMut(usize) -> PdfResult<BudgetedTextMetrics>,
    {
        let fragments = self.build_text_event_fragments(
            expected, range, text, preserve_advance, budget, resolve_metrics,
        )?;
        self.finish_text_event_fragments(expected, &mut [Some(fragments)], budget)
    }

    pub(super) fn build_text_event_fragments<F>(
        &self,
        expected: &ContentProgram,
        range: Range<TextCodePosition>,
        text: &str,
        preserve_advance: bool,
        budget: &OperationBudget,
        mut resolve_metrics: F,
    ) -> PdfResult<TextEventFragments>
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
            // The serialized fragment owns its bytes. Metrics and encoded codes
            // are no longer borrowed and can drop here, releasing their actual
            // allocations and leases before the next event/final reconstruction.
            prepared
                .items
                .push((index, fragment, operation_count));
        }
        Ok(TextEventFragments {
            events: prepared,
            input_chars,
            output_chars,
        })
    }

    /// Build one complete program per node. Donors remain owned on failure.
    pub(super) fn finish_text_event_fragments(
        &self,
        expected: &ContentProgram,
        donors: &mut [Option<TextEventFragments>],
        budget: &OperationBudget,
    ) -> PdfResult<TextEventEditPlan> {
        let limits = self.ctx.limits();
        let (mut count, mut input_chars, mut output_chars) = (0usize, 0usize, 0usize);
        let mut previous = None;
        for donor in donors.iter().flatten() {
            count = count.checked_add(donor.events.items.len())
                .filter(|&n| n <= limits.max_objects)
                .ok_or_else(|| PdfError::ResourceLimit("merged text event count".into()))?;
            input_chars = input_chars.checked_add(donor.input_chars)
                .filter(|&n| n <= limits.max_text_chars)
                .ok_or_else(|| PdfError::ResourceLimit("merged input text characters".into()))?;
            output_chars = output_chars.checked_add(donor.output_chars)
                .filter(|&n| n <= limits.max_text_chars)
                .ok_or_else(|| PdfError::ResourceLimit("merged output text characters".into()))?;
            for (index, _, _) in &donor.events.items {
                if previous.is_some_and(|previous| previous >= *index) {
                    return Err(PdfError::ObjectError("merged events are not in original order".into()));
                }
                previous = Some(*index);
            }
        }
        let sole = donors.iter().flatten().count() == 1;
        // A sole table can transfer its existing allocation/lease after parsing.
        // For multiple donors reserve the destination before consuming any owner.
        let mut events = if sole { None } else { Some(OwnedTable::new(budget, count)?) };
        let mut borrowed = OwnedTable::new(budget, count)?;
        for donor in donors.iter().flatten() {
            for (index, fragment, count) in &donor.events.items {
                borrowed.items.push(OperationReplacement {
                    index: *index, source: fragment.as_slice(), operation_count: *count,
                });
            }
        }
        let program = expected.with_replaced_operation_sequences_with_budget(
            &borrowed.items, self.ctx.options(), budget,
        )?;
        drop(borrowed);
        for donor in donors {
            if let Some(mut donor) = donor.take() {
                if let Some(events) = &mut events {
                    events.items.append(&mut donor.events.items);
                } else {
                    events = Some(donor.events);
                }
            }
        }
        Ok(TextEventEditPlan {
            program, _events: events.expect("destination allocated or sole donor transferred"),
            input_chars, output_chars,
        })
    }
}
