use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{pdf_array::PdfArray, pdf_stream::PdfStream, PdfObject},
    page::content_program::ContentProgram,
};
use std::io::{Read, Seek};

/// Keep metadata allocation and its reservation under the same ownership.
pub(crate) struct BudgetedTextItems<'a> {
    items: Vec<(usize, &'a [u8])>,
    _lease: crate::budget::MemoryLease,
}

impl<'a> BudgetedTextItems<'a> {
    pub(crate) fn new(
        budget: &crate::budget::OperationBudget,
        replacements: &'a [(usize, Vec<u8>)],
    ) -> PdfResult<Self> {
        let size = std::mem::size_of::<(usize, &[u8])>();
        let mut lease = budget.reserve(
            replacements
                .len()
                .checked_mul(size)
                .ok_or_else(|| PdfError::ResourceLimit("TJ borrowed metadata".into()))?,
        )?;
        let mut items = Vec::new();
        items
            .try_reserve_exact(replacements.len())
            .map_err(|_| PdfError::ResourceLimit("TJ replacements allocation".into()))?;
        lease.resize(
            items
                .capacity()
                .checked_mul(size)
                .ok_or_else(|| PdfError::ResourceLimit("TJ borrowed metadata".into()))?,
        )?;
        items.extend(
            replacements
                .iter()
                .map(|(index, codes)| (*index, codes.as_slice())),
        );
        Ok(Self {
            items,
            _lease: lease,
        })
    }

    pub(crate) fn as_slice(&self) -> &[(usize, &'a [u8])] {
        &self.items
    }
}

pub(crate) fn path_parameter_replacement(
    target: &crate::page::operator::Operator,
    values: &[f32],
    limits: crate::parser::options::Limits,
) -> PdfResult<Vec<u8>> {
    let count = match target.name() {
        "m" | "l" => 2,
        "v" | "y" | "re" => 4,
        "c" => 6,
        _ => {
            return Err(PdfError::ObjectError(
                "replacement requires numeric path geometry".into(),
            ))
        }
    };
    if target.num_operands() != count || values.len() != count {
        return Err(PdfError::ObjectError(
            "path parameter count mismatch".into(),
        ));
    }
    let mut replacement = Vec::new();
    for (index, value) in values.iter().enumerate() {
        target.get_as_f32(index)?;
        if !value.is_finite() {
            return Err(PdfError::ObjectError(
                "path parameter must be finite".into(),
            ));
        }
        let number = crate::objects::pdf_number::PdfNumber::new(*value);
        let bytes = number.lexeme().as_bytes();
        crate::objects::pdf_number::PdfNumber::new_from_lexeme(bytes)?;
        crate::filter::check_output_size(
            replacement.len(),
            bytes.len() + 1,
            limits.max_decoded_bytes,
            "path replacement bytes",
        )?;
        replacement
            .try_reserve(bytes.len() + 1)
            .map_err(|_| PdfError::ResourceLimit("path replacement allocation".into()))?;
        replacement.extend_from_slice(bytes);
        replacement.push(b' ');
    }
    crate::filter::check_output_size(
        replacement.len(),
        target.name().len(),
        limits.max_decoded_bytes,
        "path replacement bytes",
    )?;
    replacement
        .try_reserve(target.name().len())
        .map_err(|_| PdfError::ResourceLimit("path replacement allocation".into()))?;
    replacement.extend_from_slice(target.name().as_bytes());
    Ok(replacement)
}

pub(crate) fn text_range_replacement(
    font: &crate::font::pdf_font::Font,
    old: &[u8],
    range: std::ops::Range<usize>,
    codes: &[u8],
    limits: crate::parser::options::Limits,
) -> PdfResult<Vec<u8>> {
    if range.start > range.end || range.end > old.len() {
        return Err(PdfError::ObjectError(
            "text code range out of bounds".into(),
        ));
    }
    let mut offset = 0usize;
    let mut start = range.start == 0;
    let mut end = range.end == 0;
    font.visit_chars_limited(old, limits.max_text_chars, |character| {
        offset = offset
            .checked_add(usize::from(character.length()))
            .filter(|next| *next > offset && *next <= old.len())
            .ok_or_else(|| PdfError::FontError("text character range mismatch".into()))?;
        start |= range.start == offset;
        end |= range.end == offset;
        Ok(())
    })?;
    if offset != old.len() || !start || !end {
        return Err(PdfError::ObjectError(
            "text range splits a character code".into(),
        ));
    }
    let size = old
        .len()
        .checked_sub(range.len())
        .and_then(|size| size.checked_add(codes.len()))
        .filter(|size| *size <= limits.max_decoded_bytes)
        .ok_or_else(|| PdfError::ResourceLimit("text range replacement bytes".into()))?;
    let mut replacement = Vec::new();
    replacement
        .try_reserve_exact(size)
        .map_err(|_| PdfError::ResourceLimit("text range replacement allocation".into()))?;
    replacement.extend_from_slice(&old[..range.start]);
    replacement.extend_from_slice(&codes);
    replacement.extend_from_slice(&old[range.end..]);
    font.visit_chars_limited(&replacement, limits.max_text_chars, |_| Ok(()))?;
    Ok(replacement)
}

/// Endpoints are (TJ array index, decoded code-byte offset), not Unicode indices.
pub(crate) struct BudgetedTextReplacements {
    items: Vec<(usize, Vec<u8>)>,
    _lease: crate::budget::MemoryLease,
}

impl BudgetedTextReplacements {
    pub(crate) fn as_slice(&self) -> &[(usize, Vec<u8>)] {
        &self.items
    }
}

pub(crate) fn budgeted_text_array_range_replacements(
    budget: &crate::budget::OperationBudget,
    font: &crate::font::pdf_font::Font,
    array: &PdfArray,
    range: std::ops::Range<(usize, usize)>,
    codes: &[u8],
    limits: crate::parser::options::Limits,
) -> PdfResult<BudgetedTextReplacements> {
    let mut lease = budget.reserve(0)?;
    let items = text_array_range_replacements_with_reservation(
        font, array, range, codes, limits, &mut lease,
    )?;
    Ok(BudgetedTextReplacements {
        items,
        _lease: lease,
    })
}

pub(crate) struct BudgetedTextAdjustments {
    items: Vec<crate::page::content_program::TextArrayAdjustment>,
    _lease: crate::budget::MemoryLease,
}

impl BudgetedTextAdjustments {
    pub(crate) fn as_slice(&self) -> &[crate::page::content_program::TextArrayAdjustment] {
        &self.items
    }
}

/// Borrow existing range output; no font cloning or concatenated measurement
/// buffers. Each adjustment precedes the original retained suffix.
pub(crate) fn budgeted_text_array_range_adjustments(
    budget: &crate::budget::OperationBudget,
    array: &PdfArray,
    range: &std::ops::Range<(usize, usize)>,
    codes: &[u8],
    replacements: &[(usize, &[u8])],
    measure: impl Fn(&[u8], &[u8]) -> PdfResult<f32>,
) -> PdfResult<BudgetedTextAdjustments> {
    use crate::page::content_program::TextArrayAdjustment;
    let size = std::mem::size_of::<TextArrayAdjustment>();
    let mut lease = budget.reserve(replacements.len().checked_mul(size)
        .ok_or_else(|| PdfError::ResourceLimit("TJ adjustment metadata".into()))?)?;
    let mut items = Vec::new();
    items.try_reserve_exact(replacements.len())
        .map_err(|_| PdfError::ResourceLimit("TJ adjustment allocation".into()))?;
    lease.resize(items.capacity().checked_mul(size)
        .ok_or_else(|| PdfError::ResourceLimit("TJ adjustment metadata".into()))?)?;
    for &(item, _) in replacements {
        let old = array.get(item).and_then(PdfObject::as_string)
            .ok_or_else(|| PdfError::ObjectError("TJ adjustment requires a string item".into()))?.bytes();
        let start = if item == range.start.0 { range.start.1 } else { 0 };
        let end = if item == range.end.0 { range.end.1 } else { old.len() };
        let removed = old.get(start..end)
            .ok_or_else(|| PdfError::ObjectError("TJ adjustment range out of bounds".into()))?;
        let inserted = if item == range.start.0 { codes } else { b"" };
        items.push(TextArrayAdjustment {
            value: measure(removed, inserted)?,
            suffix_offset: start.checked_add(inserted.len())
                .ok_or_else(|| PdfError::ResourceLimit("TJ suffix offset".into()))?,
        });
    }
    Ok(BudgetedTextAdjustments { items, _lease: lease })
}

#[cfg(test)]
fn text_array_range_replacements(
    font: &crate::font::pdf_font::Font,
    array: &PdfArray,
    range: std::ops::Range<(usize, usize)>,
    codes: &[u8],
    limits: crate::parser::options::Limits,
) -> PdfResult<Vec<(usize, Vec<u8>)>> {
    let budget = limits.operation_budget();
    let mut reservation = budget.reserve(0)?;
    text_array_range_replacements_with_reservation(
        font,
        array,
        range,
        codes,
        limits,
        &mut reservation,
    )
}

pub(crate) fn validate_text_array_range(
    array: &PdfArray,
    range: &std::ops::Range<(usize, usize)>,
    limits: crate::parser::options::Limits,
) -> PdfResult<()> {
    if array.len() > limits.max_objects {
        return Err(PdfError::ResourceLimit("TJ array items".into()));
    }
    if range.start > range.end {
        return Err(PdfError::ObjectError(
            "TJ range endpoints out of bounds".into(),
        ));
    }
    for (index, offset) in [range.start, range.end] {
        let string = array
            .get(index)
            .and_then(PdfObject::as_string)
            .ok_or_else(|| PdfError::ObjectError("TJ endpoint requires a string item".into()))?;
        if offset > string.bytes().len() {
            return Err(PdfError::ObjectError(
                "TJ endpoint offset out of bounds".into(),
            ));
        }
    }
    for item in array {
        if item.as_string().is_none() && item.as_number().is_none() {
            return Err(PdfError::ObjectError(
                "TJ item must be string or number".into(),
            ));
        }
    }
    Ok(())
}

fn text_array_range_replacements_with_reservation(
    font: &crate::font::pdf_font::Font,
    array: &PdfArray,
    range: std::ops::Range<(usize, usize)>,
    codes: &[u8],
    limits: crate::parser::options::Limits,
    reservation: &mut crate::budget::MemoryLease,
) -> PdfResult<Vec<(usize, Vec<u8>)>> {
    validate_text_array_range(array, &range, limits)?;
    // Preflight the whole logical output before allocating any replacement
    // string. Per-item limits alone allow earlier buffers to accumulate before
    // a later item exposes that the combined output is too large.
    let mut planned_bytes = 0usize;
    let mut replacement_bytes = 0usize;
    let mut replacement_count = 0usize;
    let mut old_chars = 0usize;
    for (index, item) in array.into_iter().enumerate() {
        let Some(string) = item.as_string() else {
            if item.as_number().is_none() {
                return Err(PdfError::ObjectError(
                    "TJ item must be string or number".into(),
                ));
            }
            continue;
        };
        let mut size = string.bytes().len();
        let characters = font.visit_chars_limited(
            string.bytes(),
            limits.max_text_chars - old_chars,
            |_| Ok(()),
        )?;
        old_chars = old_chars
            .checked_add(characters)
            .filter(|&count| count <= limits.max_text_chars)
            .ok_or_else(|| PdfError::ResourceLimit("TJ input characters".into()))?;
        if index >= range.start.0 && index <= range.end.0 {
            let start = if index == range.start.0 {
                range.start.1
            } else {
                0
            };
            let end = if index == range.end.0 {
                range.end.1
            } else {
                size
            };
            size = size
                .checked_sub(end - start)
                .and_then(|size| {
                    size.checked_add(if index == range.start.0 {
                        codes.len()
                    } else {
                        0
                    })
                })
                .ok_or_else(|| PdfError::ResourceLimit("TJ output code bytes".into()))?;
            replacement_bytes = replacement_bytes
                .checked_add(size)
                .ok_or_else(|| PdfError::ResourceLimit("TJ replacement capacities".into()))?;
            replacement_count += 1;
        }
        planned_bytes = planned_bytes
            .checked_add(size)
            .filter(|&count| count <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("TJ output code bytes".into()))?;
    }
    let entry_size = std::mem::size_of::<(usize, Vec<u8>)>();
    let metadata_bytes = replacement_count
        .checked_mul(entry_size)
        .ok_or_else(|| PdfError::ResourceLimit("TJ replacement metadata".into()))?;
    let mut reserved_bytes = replacement_bytes
        .checked_add(metadata_bytes)
        .ok_or_else(|| PdfError::ResourceLimit("TJ replacement capacities".into()))?;
    reservation.resize(reserved_bytes)?;
    let mut replacements = Vec::new();
    replacements
        .try_reserve_exact(replacement_count)
        .map_err(|_| PdfError::ResourceLimit("TJ replacements allocation".into()))?;
    reserved_bytes = replacement_bytes
        .checked_add(
            replacements
                .capacity()
                .checked_mul(entry_size)
                .ok_or_else(|| PdfError::ResourceLimit("TJ replacement metadata".into()))?,
        )
        .ok_or_else(|| PdfError::ResourceLimit("TJ replacement capacities".into()))?;
    reservation.resize(reserved_bytes)?;
    let mut output_chars = 0usize;
    let mut output_bytes = 0usize;
    for (index, item) in array.into_iter().enumerate() {
        let Some(string) = item.as_string() else {
            if item.as_number().is_none() {
                return Err(PdfError::ObjectError(
                    "TJ item must be string or number".into(),
                ));
            }
            continue;
        };
        let old = string.bytes();
        let replacement = if index >= range.start.0 && index <= range.end.0 {
            let start = if index == range.start.0 {
                range.start.1
            } else {
                0
            };
            let end = if index == range.end.0 {
                range.end.1
            } else {
                old.len()
            };
            Some(text_range_replacement(
                font,
                old,
                start..end,
                if index == range.start.0 { codes } else { b"" },
                limits,
            )?)
        } else {
            None
        };
        let output = replacement.as_deref().unwrap_or(old);
        output_chars = output_chars
            .checked_add(font.visit_chars_limited(
                output,
                limits.max_text_chars - output_chars,
                |_| Ok(()),
            )?)
            .filter(|&count| count <= limits.max_text_chars)
            .ok_or_else(|| PdfError::ResourceLimit("TJ output characters".into()))?;
        output_bytes = output_bytes
            .checked_add(output.len())
            .filter(|&count| count <= limits.max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("TJ output code bytes".into()))?;
        if let Some(replacement) = replacement {
            reserved_bytes = reserved_bytes
                .checked_add(replacement.capacity() - replacement.len())
                .ok_or_else(|| PdfError::ResourceLimit("TJ replacement capacities".into()))?;
            reservation.resize(reserved_bytes)?;
            replacements.push((index, replacement));
        }
    }
    Ok(replacements)
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Atomically replace multiple source operations with caller-built sequences.
    /// Indices/counts refer to this page's current snapshot; unchanged operations
    /// and stream boundaries are verified before copy-on-write commit. Does not
    /// encode Unicode, compute layout or validate arbitrary graphics semantics.
    pub fn replace_page_operation_sequences(
        &self,
        page: u32,
        expected: &ContentProgram,
        replacements: &[crate::page::content_program::OperationReplacement<'_>],
    ) -> PdfResult<()> {
        let budget = self.ctx.limits().operation_budget();
        self.commit_page_content_edit_with_budget(page, expected, &budget, || {
            expected.with_replaced_operation_sequences_with_budget(replacements, self.ctx.options(), &budget)
        })
    }

    /// Replace a TJ range while preserving retained prefixes, suffixes and
    /// later text positions. Each changed string is split around a per-item
    /// adjustment. Original numeric spacing is kept. Refresh item indices after
    /// commit; endpoints are decoded code bytes, not Unicode indices.
    pub fn replace_page_text_array_range_preserving_advance(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        range: std::ops::Range<(usize, usize)>,
        text: &str,
    ) -> PdfResult<()> {
        let budget = self.ctx.limits().operation_budget();
        let mut retained_state = None;
        let mut retained_replacements = None;
        self.commit_page_content_edit_with_budget(page, expected, &budget, || {
            let target = &expected.operations().get(operation).ok_or_else(||
                PdfError::ObjectError("text operation index out of range".into()))?.operator;
            if target.name() != "TJ" || target.num_operands() != 1 {
                return Err(PdfError::ObjectError("array range compensation requires TJ".into()));
            }
            let array = target.get_as_array(0)?;
            validate_text_array_range(array, &range, self.ctx.limits())?;
            let count = (range.start.0..=range.end.0)
                .filter(|&index| array.get(index).and_then(PdfObject::as_string).is_some()).count();
            let added = count.checked_mul(2).and_then(|count| count.checked_add(array.len()))
                .ok_or_else(|| PdfError::ResourceLimit("TJ adjusted array items".into()))?;
            if added > self.ctx.limits().max_objects {
                return Err(PdfError::ResourceLimit("TJ adjusted array items".into()));
            }
            let state = self.get_page_with_budget(page, &budget)?
                .text_metrics_for_operation_with_budget(expected, operation, &budget)?;
            let state = retained_state.insert(state);
            let codes = state.font.encode_text_with_budget(text, self.ctx.limits(), &budget)?;
            let replacements = budgeted_text_array_range_replacements(
                &budget, &state.font, array, range.clone(), codes.as_slice(), self.ctx.limits(),
            )?;
            let replacements = retained_replacements.insert(replacements);
            let borrowed = BudgetedTextItems::new(&budget, replacements.as_slice())?;
            let adjustments = budgeted_text_array_range_adjustments(
                &budget, array, &range, codes.as_slice(), borrowed.as_slice(),
                |old, new| state.replacement_adjustment(old, new, self.ctx.limits()),
            )?;
            expected.with_adjusted_text_array_items_codes_with_budget(
                operation, borrowed.as_slice(), adjustments.as_slice(), self.ctx.options(), &budget,
            )
        })
    }

    /// Replace a range spanning TJ string items. Endpoints are array indices and
    /// decoded code-byte offsets. Insert into the start item; retain numbers,
    /// empty intermediate strings, and the end suffix. No advance compensation.
    pub fn replace_page_text_array_range(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        range: std::ops::Range<(usize, usize)>,
        text: &str,
    ) -> PdfResult<()> {
        let budget = self.ctx.limits().operation_budget();
        let mut retained_replacements = None;
        let mut retained_font = None;
        self.commit_page_content_edit_with_budget(page, expected, &budget, || {
            let target = &expected
                .operations()
                .get(operation)
                .ok_or_else(|| PdfError::ObjectError("text operation index out of range".into()))?
                .operator;
            if target.name() != "TJ" || target.num_operands() != 1 {
                return Err(PdfError::ObjectError("array range requires TJ".into()));
            }
            validate_text_array_range(target.get_as_array(0)?, &range, self.ctx.limits())?;
            let current_page = self.get_page_with_budget(page, &budget)?;
            let font =
                current_page.font_for_text_operation_with_budget(expected, operation, &budget)?;
            let font = retained_font.insert(font);
            let codes = font.encode_text_with_budget(text, self.ctx.limits(), &budget)?;
            let replacements = budgeted_text_array_range_replacements(
                &budget,
                font,
                target.get_as_array(0)?,
                range,
                codes.as_slice(),
                self.ctx.limits(),
            )?;
            let replacements = retained_replacements.insert(replacements);
            let borrowed = BudgetedTextItems::new(&budget, replacements.as_slice())?;
            expected.with_replaced_text_array_items_codes_with_budget(
                operation,
                borrowed.as_slice(),
                self.ctx.options(),
                &budget,
            )
        })
    }

    /// Clear strings in one top-level text-show event, preserving its operator.
    /// Quotes retain line/spacing effects and TJ retains numeric adjustments.
    /// Removed glyph advances are not compensated; this is not secure erasure.
    pub fn remove_page_text_draw(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            expected.with_cleared_text_draw(operation, self.ctx.options())
        })
    }

    /// Stop painting one path but still end/discard it at the same location.
    /// Geometry and pending clipping remain; replaces painting with n, not erasure.
    pub fn remove_page_path_draw(
        &self,
        page: u32,
        expected: &ContentProgram,
        painting_operation: usize,
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            let target = expected
                .operations()
                .get(painting_operation)
                .ok_or_else(|| {
                    PdfError::ObjectError("path painting operation index out of range".into())
                })?;
            if !matches!(
                target.operator.name(),
                "S" | "s" | "f" | "F" | "f*" | "B" | "B*" | "b" | "b*"
            ) || target.operator.num_operands() != 0
            {
                return Err(PdfError::ObjectError(
                    "path removal requires a painting operation".into(),
                ));
            }
            expected.with_replaced_operation(painting_operation, b"n", self.ctx.options())
        })
    }

    /// Logically remove one top-level image Do without deleting its resource.
    /// Other draws and shared pages remain unchanged; not secure data erasure.
    pub fn remove_page_image_draw(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            let target = expected.operations().get(operation).ok_or_else(|| {
                PdfError::ObjectError("image operation index out of range".into())
            })?;
            if target.operator.inline_image().is_some() {
                return expected.without_operation(operation, self.ctx.options());
            }
            if target.operator.name() != "Do" || target.operator.num_operands() != 1 {
                return Err(PdfError::ObjectError(
                    "image removal requires a named Do".into(),
                ));
            }
            let resource = target
                .named_resource_use()?
                .ok_or_else(|| PdfError::ObjectError("image resource missing".into()))?;
            self.get_page(page)?.extract_image_source(resource.name)?;
            expected.without_operation(operation, self.ctx.options())
        })
    }

    /// Remove one top-level Form invocation without changing its shared stream.
    /// Nested leaf indices are not accepted as page snapshots; not secure erasure.
    pub fn remove_page_form_draw(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            let target = expected
                .operations()
                .get(operation)
                .ok_or_else(|| PdfError::ObjectError("Form operation index out of range".into()))?;
            if target.operator.name() != "Do" || target.operator.num_operands() != 1 {
                return Err(PdfError::ObjectError(
                    "Form removal requires a named Do".into(),
                ));
            }
            let name = target.operator.get_as_name(0)?;
            let object = self.get_page(page)?.get_xobject(name.name())?;
            crate::page::pdf_form::PdfForm::try_new(&object, &self.ctx)?;
            expected.without_operation(operation, self.ctx.options())
        })
    }

    /// Apply a relative current-user-space transform to one top-level Form Do.
    /// The Form stream and other invocations are unchanged; not display space.
    pub fn transform_page_form_draw(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        matrix: [f32; 6],
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            let target = expected
                .operations()
                .get(operation)
                .ok_or_else(|| PdfError::ObjectError("Form operation index out of range".into()))?;
            if target.operator.name() != "Do" || target.operator.num_operands() != 1 {
                return Err(PdfError::ObjectError(
                    "Form transform requires a named Do".into(),
                ));
            }
            let object = self
                .get_page(page)?
                .get_xobject(target.operator.get_as_name(0)?.name())?;
            crate::page::pdf_form::PdfForm::try_new(&object, &self.ctx)?;
            expected.with_transformed_xobject_operation(operation, matrix, self.ctx.options())
        })
    }

    /// Replace one top-level Form Do with a same-document Form handle.
    /// A page-local binding isolates this call; neither shared Form is mutated.
    pub fn replace_page_form_draw(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        form: &super::identity::ObjectHandle,
    ) -> PdfResult<()> {
        if !expected
            .source_stamp()
            .is_some_and(|stamp| self.is_stamp_current(stamp))
            || expected.source_page_id != Some(self.page_location(page)?.page_id)
        {
            return Err(PdfError::ObjectError(
                "Form content snapshot is foreign, stale or belongs to another page".into(),
            ));
        }
        let target = expected
            .operations()
            .get(operation)
            .ok_or_else(|| PdfError::ObjectError("Form operation index out of range".into()))?;
        if target.operator.name() != "Do" || target.operator.num_operands() != 1 {
            return Err(PdfError::ObjectError(
                "Form replacement requires a named Do".into(),
            ));
        }
        let original = self
            .get_page(page)?
            .get_xobject(target.operator.get_as_name(0)?.name())?;
        crate::page::pdf_form::PdfForm::try_new(&original, &self.ctx)?;
        let id = self.validate_handle(form)?;
        crate::page::pdf_form::PdfForm::try_new(&self.resolve_object(id)?, &self.ctx)?;
        self.edit_transaction(|doc| {
            doc.commit_page_content_edit(page, expected, || {
                let name = doc.resources().register_handle(
                    page,
                    super::resource_manager::ResourceKind::Form,
                    form,
                )?;
                let mut replacement = Vec::new();
                crate::writer::write_name(
                    &mut replacement,
                    &crate::objects::pdf_name::PdfName::new(name),
                )?;
                replacement.extend_from_slice(b" Do");
                expected.with_replaced_operation(operation, &replacement, doc.ctx.options())
            })
        })
    }

    /// Replace decoded character-code bytes at complete character boundaries.
    /// TJ requires Some(array entry); single-string operators require None.
    /// The range is not Unicode indices or escaped PDF source offsets.
    pub fn replace_page_text_range(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        array_item: Option<usize>,
        range: std::ops::Range<usize>,
        text: &str,
    ) -> PdfResult<()> {
        let budget = self.ctx.limits().operation_budget();
        // Keep the actual buffer and its lease together through content commit.
        let mut retained_replacement = None;
        let mut retained_font = None;
        self.commit_page_content_edit_with_budget(page, expected, &budget, || {
            let target = &expected
                .operations()
                .get(operation)
                .ok_or_else(|| PdfError::ObjectError("text operation index out of range".into()))?
                .operator;
            let string = match (target.name(), array_item) {
                ("TJ", Some(item)) => target
                    .get_as_array(0)?
                    .get(item)
                    .and_then(PdfObject::as_string)
                    .ok_or_else(|| {
                        PdfError::ObjectError("TJ range requires a string array item".into())
                    })?,
                ("Tj" | "'", None) => target.get_as_string(0)?,
                ("\"", None) => target.get_as_string(2)?,
                _ => {
                    return Err(PdfError::ObjectError(
                        "text range target/item mismatch".into(),
                    ));
                }
            };
            let old = string.bytes();
            if range.start > range.end || range.end > old.len() {
                return Err(PdfError::ObjectError(
                    "text code range out of bounds".into(),
                ));
            }
            let current_page = self.get_page_with_budget(page, &budget)?;
            let font =
                current_page.font_for_text_operation_with_budget(expected, operation, &budget)?;
            let font = retained_font.insert(font);
            let codes = font.encode_text_with_budget(text, self.ctx.limits(), &budget)?;
            let size = old
                .len()
                .checked_sub(range.len())
                .and_then(|size| size.checked_add(codes.as_slice().len()))
                .ok_or_else(|| PdfError::ResourceLimit("text range replacement bytes".into()))?;
            let replacement_lease = budget.reserve(size)?;
            let replacement =
                text_range_replacement(&font, old, range, codes.as_slice(), self.ctx.limits())?;
            let replacement = retained_replacement.insert(
                crate::budget::BudgetedBytes::from_reserved_vec(replacement, replacement_lease)?,
            );
            if let Some(item) = array_item {
                expected.with_replaced_text_array_items_codes_with_budget(
                    operation,
                    &[(item, replacement.as_slice())],
                    self.ctx.options(),
                    &budget,
                )
            } else {
                expected.with_replaced_text_codes_with_budget(
                    operation,
                    replacement.as_slice(),
                    self.ctx.options(),
                    &budget,
                )
            }
        })
    }

    /// Move one top-level image by a display-space delta (top-left, y downward).
    /// Converts through its interpreted CTM/page transform; does not change size.
    pub fn translate_page_image_draw_display(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        dx: f32,
        dy: f32,
    ) -> PdfResult<()> {
        self.transform_page_image_draw_display(page, expected, operation, [1., 0., 0., 1., dx, dy])
    }

    /// Apply a display-space affine transform after the existing image matrix.
    /// Translation/pivots are display coordinates, not current PDF user space.
    pub fn transform_page_image_draw_display(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        display_matrix: [f32; 6],
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            if !display_matrix.iter().all(|value| value.is_finite()) {
                return Err(PdfError::ObjectError(
                    "image display transform must be finite".into(),
                ));
            }
            let list = self.get_page(page)?.display_list()?;
            if !list.matches_program(expected) {
                return Err(PdfError::ObjectError(
                    "image draw provenance mismatch".into(),
                ));
            }
            let image = list
                .objects()
                .iter()
                .find_map(|object| {
                    let crate::page::page_object::PageObject::Image(image) = object else {
                        return None;
                    };
                    image
                        .source()
                        .filter(|source| source.operation_index == operation)
                        .map(|_| image)
                })
                .ok_or_else(|| {
                    PdfError::ObjectError("operation is not an interpreted image draw".into())
                })?;
            // A = F * C, with image normalization F = [1 0 0 -1 0 1]
            // and C = CTM * page. F is self-inverse, including its translation.
            // Want F * T * C = A * D, hence T = C * D * inverse(C).
            let matrix = &image.matrix;
            let [a, b, c, d] = [matrix.a(), matrix.b(), -matrix.c(), -matrix.d()].map(f64::from);
            let e = f64::from(matrix.e()) + f64::from(matrix.c());
            let f = f64::from(matrix.f()) + f64::from(matrix.d());
            let determinant = a * d - b * c;
            if !matrix.is_finite() || determinant == 0.0 {
                return Err(PdfError::ObjectError(
                    "image display transform is singular or non-finite".into(),
                ));
            }
            let inverse = [
                d / determinant,
                -b / determinant,
                -c / determinant,
                a / determinant,
                (c * f - d * e) / determinant,
                (b * e - a * f) / determinant,
            ];
            let local = affine_mul_f64(
                affine_mul_f64([a, b, c, d, e, f], display_matrix.map(f64::from)),
                inverse,
            )
            .map(|value| value as f32);
            if !local.iter().all(|value| value.is_finite()) {
                return Err(PdfError::ObjectError(
                    "image user-space transform overflows".into(),
                ));
            }
            expected.with_transformed_image_operation(operation, local, self.ctx.options())
        })
    }

    /// Transform one image draw relative to its current user space, isolated
    /// from subsequent draws. Translation uses [1, 0, 0, 1, dx, dy].
    pub fn transform_page_image_draw(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        matrix: [f32; 6],
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            let target = expected.operations().get(operation).ok_or_else(|| {
                PdfError::ObjectError("image operation index out of range".into())
            })?;
            if target.operator.inline_image().is_some() {
                return expected.with_transformed_image_operation(operation, matrix, self.ctx.options());
            }
            if target.operator.name() != "Do" || target.operator.num_operands() != 1 {
                return Err(PdfError::ObjectError(
                    "image transform requires one named Do".into(),
                ));
            }
            let resource = target
                .named_resource_use()?
                .ok_or_else(|| PdfError::ObjectError("image resource missing".into()))?;
            self.get_page(page)?.extract_image_source(resource.name)?;
            expected.with_transformed_image_operation(operation, matrix, self.ctx.options())
        })
    }

    /// Replace numeric parameters of m/l/c/v/y/re without changing the operator.
    /// Values are in that operation's current user space, not display coordinates.
    pub fn replace_page_path_numbers(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        values: &[f32],
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            let target = expected
                .operations()
                .get(operation)
                .ok_or_else(|| PdfError::ObjectError("path operation index out of range".into()))?;
            let replacement =
                path_parameter_replacement(&target.operator, values, self.ctx.limits())?;
            expected.with_replaced_operation(operation, &replacement, self.ctx.options())
        })
    }

    /// Replace one top-level inline payload, preserving its graphics transform.
    pub fn replace_page_inline_image_draw(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        image: &crate::page::inline_image::InlineImage,
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            if expected.operations().get(operation)
                .and_then(|o| o.operator.inline_image()).is_none()
            {
                return Err(PdfError::ObjectError("replacement target is not an inline image".into()));
            }
            expected.with_replaced_operation(
                operation, &image.to_content_bytes(self.ctx.limits())?, self.ctx.options(),
            )
        })
    }

    /// Replace one top-level image draw, preserving its existing graphics transform.
    /// Registers a page-local image name and copies only changed content streams.
    pub fn replace_page_image_draw(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        image: &super::identity::ObjectHandle,
    ) -> PdfResult<()> {
        if !expected
            .source_stamp()
            .is_some_and(|stamp| self.is_stamp_current(stamp))
            || expected.source_page_id != Some(self.page_location(page)?.page_id)
        {
            return Err(PdfError::ObjectError(
                "image content snapshot is foreign, stale or belongs to another page".into(),
            ));
        }
        let target = expected
            .operations()
            .get(operation)
            .ok_or_else(|| PdfError::ObjectError("image operation index out of range".into()))?;
        if target.operator.inline_image().is_none() && (target.operator.name() != "Do" || target.operator.num_operands() != 1) {
            return Err(PdfError::ObjectError(
                "image replacement requires one named Do".into(),
            ));
        }
        if target.operator.inline_image().is_none() {
            let resource = target.named_resource_use()?.ok_or_else(|| PdfError::ObjectError("image resource name missing".into()))?;
            self.get_page(page)?.extract_image_source(resource.name)?;
        }
        let id = self.validate_handle(image)?;
        let object = self.resolve_object(id)?;
        let stream = object
            .as_stream()
            .ok_or_else(|| PdfError::ObjectError("replacement image must be a stream".into()))?;
        let subtype = stream
            .dict()
            .get("Subtype")
            .ok_or_else(|| PdfError::ObjectError("replacement image subtype missing".into()))?;
        if self
            .ctx
            .resolve_owned(subtype)?
            .as_name()
            .map(|name| name.name())
            != Some("Image")
        {
            return Err(PdfError::ObjectError(
                "replacement requires Subtype Image".into(),
            ));
        }
        self.edit_transaction(|doc| {
            doc.commit_page_content_edit(page, expected, || {
                let name = doc.resources().register_handle(
                    page,
                    super::resource_manager::ResourceKind::Image,
                    image,
                )?;
                let mut replacement = Vec::new();
                crate::writer::write_name(
                    &mut replacement,
                    &crate::objects::pdf_name::PdfName::new(name),
                )?;
                replacement.extend_from_slice(b" Do");
                expected.with_replaced_operation(operation, &replacement, doc.ctx.options())
            })
        })
    }

    /// Replace a Tj/'/" string using Unicode encoded by its effective existing font.
    /// Does not shape, add font coverage or compensate changed text advances.
    pub fn replace_page_text(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        text: &str,
    ) -> PdfResult<()> {
        let budget = self.ctx.limits().operation_budget();
        let mut retained_codes = None;
        self.commit_page_content_edit_with_budget(page, expected, &budget, || {
            let codes = self
                .get_page_with_budget(page, &budget)?
                .encode_text_for_operation(expected, operation, text, &budget)?;
            let codes = retained_codes.insert(codes);
            expected.with_replaced_text_codes_with_budget(
                operation,
                codes.as_slice(),
                self.ctx.options(),
                &budget,
            )
        })
    }

    /// Replace Tj/'/" Unicode text while retaining the old writing-axis end
    /// position. Uses actual font metrics and spacing, not Unicode character
    /// count. Quotes keep their line/spacing state; zero font size is rejected.
    /// This does not add glyph coverage, shape text or fit replacement glyphs
    /// inside the original painted bounds. TJ and nested Form edits are separate.
    pub fn replace_page_text_preserving_advance(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        text: &str,
    ) -> PdfResult<()> {
        let budget = self.ctx.limits().operation_budget();
        let mut retained_state = None;
        let mut retained_codes = None;
        self.commit_page_content_edit_with_budget(page, expected, &budget, || {
            let target = &expected.operations().get(operation).ok_or_else(||
                PdfError::ObjectError("text operation index out of range".into()))?.operator;
            let old = match (target.name(), target.num_operands()) {
                ("Tj" | "'", 1) => target.get_as_string(0)?.bytes(),
                ("\"", 3) => target.get_as_string(2)?.bytes(),
                _ => return Err(PdfError::UnsupportedFeature("Advance compensation requires single-string text".into())),
            };
            let state = self.get_page_with_budget(page, &budget)?
                .text_metrics_for_operation_with_budget(expected, operation, &budget)?;
            let state = retained_state.insert(state);
            let codes = state.font.encode_text_with_budget(text, self.ctx.limits(), &budget)?;
            let codes = retained_codes.insert(codes);
            let adjustment = state.replacement_adjustment(old, codes.as_slice(), self.ctx.limits())?;
            expected.with_adjusted_text_codes_with_budget(
                operation, codes.as_slice(), adjustment, self.ctx.options(), &budget,
            )
        })
    }

    /// Replace one TJ string while preserving subsequent text positions. Inserts
    /// a numeric adjustment immediately after the string; original numbers and
    /// other strings stay unchanged. Subsequent array indices shift by one.
    /// Does not shape text, add glyph coverage or prevent glyph overlap.
    pub fn replace_page_text_array_item_preserving_advance(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        item: usize,
        text: &str,
    ) -> PdfResult<()> {
        let budget = self.ctx.limits().operation_budget();
        let mut retained_state = None;
        let mut retained_codes = None;
        self.commit_page_content_edit_with_budget(page, expected, &budget, || {
            let target = &expected.operations().get(operation).ok_or_else(||
                PdfError::ObjectError("text operation index out of range".into()))?.operator;
            if target.name() != "TJ" || target.num_operands() != 1 {
                return Err(PdfError::ObjectError("text array compensation requires TJ".into()));
            }
            let old = target.get_as_array(0)?.get(item)
                .and_then(PdfObject::as_string)
                .ok_or_else(|| PdfError::ObjectError("TJ compensation requires a string item".into()))?
                .bytes();
            let state = self.get_page_with_budget(page, &budget)?
                .text_metrics_for_operation_with_budget(expected, operation, &budget)?;
            let state = retained_state.insert(state);
            let codes = state.font.encode_text_with_budget(text, self.ctx.limits(), &budget)?;
            let codes = retained_codes.insert(codes);
            let adjustment = state.replacement_adjustment(old, codes.as_slice(), self.ctx.limits())?;
            expected.with_adjusted_text_array_item_codes_with_budget(
                operation, item, codes.as_slice(), adjustment, self.ctx.options(), &budget,
            )
        })
    }

    /// Replace one TJ string item using its effective font and unchanged spacing.
    pub fn replace_page_text_array_item(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        item: usize,
        text: &str,
    ) -> PdfResult<()> {
        let budget = self.ctx.limits().operation_budget();
        let mut retained_codes = None;
        self.commit_page_content_edit_with_budget(page, expected, &budget, || {
            let target = &expected
                .operations()
                .get(operation)
                .ok_or_else(|| PdfError::ObjectError("text operation index out of range".into()))?
                .operator;
            if target.name() != "TJ" || target.num_operands() != 1 {
                return Err(PdfError::ObjectError(
                    "text array item replacement requires TJ".into(),
                ));
            }
            if target
                .get_as_array(0)?
                .get(item)
                .and_then(PdfObject::as_string)
                .is_none()
            {
                return Err(PdfError::ObjectError(
                    "TJ replacement requires a string array item".into(),
                ));
            }
            let codes = self
                .get_page_with_budget(page, &budget)?
                .encode_text_for_operation(expected, operation, text, &budget)?;
            let codes = retained_codes.insert(codes);
            expected.with_replaced_text_array_items_codes_with_budget(
                operation,
                &[(item, codes.as_slice())],
                self.ctx.options(),
                &budget,
            )
        })
    }

    /// Atomically replace one Tj/'/" string using codes for its existing font.
    /// Rejects foreign, offline, wrong-page and stale snapshots. This does not
    /// encode Unicode, validate glyph coverage or preserve text layout.
    pub fn replace_page_text_codes(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        codes: &[u8],
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            expected.with_replaced_text_codes(operation, codes, self.ctx.options())
        })
    }

    /// Atomically replace one string item of TJ. item_index includes numbers.
    /// Preserves explicit spacing values; does not compensate changed advances.
    pub fn replace_page_text_array_item_codes(
        &self,
        page: u32,
        expected: &ContentProgram,
        operation: usize,
        item_index: usize,
        codes: &[u8],
    ) -> PdfResult<()> {
        self.commit_page_content_edit(page, expected, || {
            expected.with_replaced_text_array_item_codes(
                operation,
                item_index,
                codes,
                self.ctx.options(),
            )
        })
    }

    /// Shared page-content snapshot validation and changed-stream COW commit.
    /// Callers that register resources during preparation must enclose the
    /// preparation and this commit in the same outer edit transaction.
    pub(crate) fn commit_page_content_edit(
        &self,
        page: u32,
        expected: &ContentProgram,
        edit: impl FnOnce() -> PdfResult<ContentProgram>,
    ) -> PdfResult<()> {
        self.commit_page_content_edit_inner(page, expected, None, edit)
    }

    pub(super) fn commit_page_content_edit_with_budget<E: AsRef<ContentProgram>>(
        &self,
        page: u32,
        expected: &ContentProgram,
        budget: &crate::budget::OperationBudget,
        edit: impl FnOnce() -> PdfResult<E>,
    ) -> PdfResult<()> {
        self.commit_page_content_edit_inner(page, expected, Some(budget), edit)
    }

    pub(super) fn commit_page_content_edit_inner<E: AsRef<ContentProgram>>(
        &self,
        page: u32,
        expected: &ContentProgram,
        budget: Option<&crate::budget::OperationBudget>,
        edit: impl FnOnce() -> PdfResult<E>,
    ) -> PdfResult<()> {
        self.ctx.require_content_edit_permission()?;
        if !expected
            .source_stamp()
            .is_some_and(|stamp| self.is_stamp_current(stamp))
        {
            return Err(PdfError::ObjectError(
                "content snapshot is foreign, offline or stale".into(),
            ));
        }
        let (_location_lease, location) = if let Some(budget) = budget {
            let (location, lease) = self.page_location_with_budget(page, budget)?.into_parts();
            (Some(lease), location)
        } else {
            (None, self.page_location(page)?)
        };
        if expected.source_page_id != Some(location.page_id) {
            return Err(PdfError::ObjectError(
                "content snapshot belongs to another page".into(),
            ));
        }
        drop(location);
        drop(_location_lease);
        let retained_current = budget
            .map(|budget| {
                self.get_page_with_budget(page, budget)?
                    .content_program_with_budget(budget)
            })
            .transpose()?;
        let legacy_current = if budget.is_none() {
            Some(self.get_page(page)?.content_program()?)
        } else {
            None
        };
        let current = retained_current
            .as_ref()
            .map(|owner| owner.program())
            .or(legacy_current.as_ref())
            .unwrap();
        if current.source_bytes() != expected.source_bytes()
            || current.stream_ranges() != expected.stream_ranges()
            || (0..current.stream_ranges().len())
                .any(|index| current.stream_object_id(index) != expected.stream_object_id(index))
        {
            return Err(PdfError::ObjectError(
                "content snapshot no longer matches the page".into(),
            ));
        }
        let edited_owner = edit()?;
        let edited = edited_owner.as_ref();
        // Preparation may have registered a new resource in an enclosing
        // transaction. Preserve that current page dictionary when updating Contents.
        let (_page_dict_lease, mut location) = if let Some(budget) = budget {
            let (location, lease) = self.page_location_with_budget(page, budget)?.into_parts();
            (Some(lease), location)
        } else {
            (None, self.page_location(page)?)
        };
        // Reserve all changed payload copies before entering the transaction.
        // This quota remains held until commit or rollback has completed.
        let mut stream_payload_lease = if let Some(budget) = budget {
            let mut total = 0usize;
            for index in 0..current.stream_ranges().len() {
                let before = current
                    .stream_bytes(index)
                    .ok_or(PdfError::PageContentIsNotStream)?;
                let after = edited
                    .stream_bytes(index)
                    .ok_or(PdfError::PageContentIsNotStream)?;
                if before != after {
                    total = total.checked_add(after.len()).ok_or_else(|| {
                        PdfError::ResourceLimit("edited stream payload total".into())
                    })?;
                }
            }
            Some(budget.reserve(total)?)
        } else {
            None
        };
        let mut contents_table_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut stream_dict_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut source_object_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let commit = |doc: &Self| {
            let mut dict = location.page_dict;
            let contents_source = dict
                .get("Contents")
                .ok_or(PdfError::PageContentIsNotStream)?;
            let contents = if let Some(budget) = budget {
                let (object, mut lease) =
                    doc.ctx.resolve_owned_with_budget(contents_source, budget)?;
                source_object_lease.as_mut().unwrap().absorb(&mut lease)?;
                object
            } else {
                doc.ctx.resolve_owned(contents_source)?
            };
            let mut index = 0;
            let mut replace = |original: &PdfObject| -> PdfResult<PdfObject> {
                let before = current
                    .stream_bytes(index)
                    .ok_or(PdfError::PageContentIsNotStream)?;
                let after = edited
                    .stream_bytes(index)
                    .ok_or(PdfError::PageContentIsNotStream)?;
                index += 1;
                if before == after {
                    return Ok(original.clone());
                }
                doc.validate_content_bytes(after.len())?;
                let source = if let Some(budget) = budget {
                    let (object, mut lease) =
                        doc.ctx.resolve_owned_with_budget(original, budget)?;
                    source_object_lease.as_mut().unwrap().absorb(&mut lease)?;
                    object
                } else {
                    doc.ctx.resolve_owned(original)?
                };
                let stream = source
                    .into_stream()
                    .ok_or(PdfError::PageContentIsNotStream)?;
                let mut bytes = Vec::new();
                bytes
                    .try_reserve_exact(after.len())
                    .map_err(|_| PdfError::ResourceLimit("edited stream allocation".into()))?;
                if let Some(lease) = stream_payload_lease.as_mut() {
                    lease.resize(
                        lease
                            .bytes()
                            .checked_add(bytes.capacity() - after.len())
                            .ok_or_else(|| {
                                PdfError::ResourceLimit("edited stream payload capacity".into())
                            })?,
                    )?;
                }
                bytes.extend_from_slice(after);
                let output = if let Some(budget) = budget {
                    let (output, mut lease) = PdfStream::from_decoded_data_with_budget(
                        stream.dict(),
                        bytes,
                        budget,
                        doc.ctx.limits(),
                    )?;
                    stream_dict_lease.as_mut().unwrap().absorb(&mut lease)?;
                    output
                } else {
                    PdfStream::from_decoded_data(stream.dict().clone(), bytes)?
                };
                let id = doc.try_insert_object(PdfObject::PdfStream(output))?;
                Ok(PdfObject::PdfReference(
                    crate::objects::pdf_reference::PdfReference::from_id(id),
                ))
            };
            let result = match contents {
                PdfObject::PdfArray(array) => {
                    let size = std::mem::size_of::<PdfObject>();
                    if let Some(lease) = contents_table_lease.as_mut() {
                        lease.resize(array.len().checked_mul(size).ok_or_else(|| {
                            PdfError::ResourceLimit("Contents table size".into())
                        })?)?;
                    }
                    let mut output = Vec::new();
                    output
                        .try_reserve_exact(array.len())
                        .map_err(|_| PdfError::ResourceLimit("Contents table allocation".into()))?;
                    if let Some(lease) = contents_table_lease.as_mut() {
                        lease.resize(output.capacity().checked_mul(size).ok_or_else(|| {
                            PdfError::ResourceLimit("Contents table capacity".into())
                        })?)?;
                    }
                    for original in &array {
                        output.push(replace(original)?);
                    }
                    PdfObject::PdfArray(PdfArray::from_items(output))
                }
                PdfObject::PdfStream(_) => replace(dict.get("Contents").unwrap())?,
                _ => return Err(PdfError::PageContentIsNotStream),
            };
            dict.replace_existing("Contents", result)?;
            doc.update_object(location.page_id, PdfObject::PdfDict(dict))?;
            Ok(())
        };
        if let Some(budget) = budget {
            self.edit_transaction_with_budget(budget, commit)
        } else {
            self.edit_transaction(commit)
        }
    }
}

fn affine_mul_f64(left: [f64; 6], right: [f64; 6]) -> [f64; 6] {
    let [a, b, c, d, e, f] = left;
    let [g, h, i, j, k, l] = right;
    [
        a * g + b * i,
        a * h + b * j,
        c * g + d * i,
        c * h + d * j,
        e * g + f * i + k,
        e * h + f * j + l,
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        objects::{pdf_dict::PdfDict, pdf_name::PdfName, pdf_reference::PdfReference},
        parser::syntax::ParseMode,
    };

    #[test]
    fn contents_table_budget_failure_rolls_back_multistream_edit() {
        use crate::{budget::OperationBudget, document::owned::Document};
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let mut contents = PdfArray::default();
        for bytes in [b"q ".as_slice(), b"Q".as_slice()]
            .into_iter()
            .chain(std::iter::repeat(b"".as_slice()).take(14))
        {
            let id = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(PdfDict::default(), bytes.to_vec()).unwrap(),
                ))
                .unwrap();
            contents.add_obj(PdfObject::PdfReference(PdfReference::from_id(id)));
        }
        let location = doc.page_location(0).unwrap();
        let mut dict = location.page_dict;
        dict.insert("Contents".into(), PdfObject::PdfArray(contents));
        doc.update_object(location.page_id, PdfObject::PdfDict(dict))
            .unwrap();
        let expected = doc.get_page(0).unwrap().content_program().unwrap();
        let revision = doc.revision();
        let budget = OperationBudget::new(16384);
        let probe = OperationBudget::new(16384);
        let (_, page_lease) = doc
            .page_location_with_budget(0, &probe)
            .unwrap()
            .into_parts();
        let page_bytes = page_lease.bytes();
        let page_peak = probe.peak_bytes();
        let snapshot_probe = OperationBudget::new(16384);
        doc.edit_transaction_with_budget(&snapshot_probe, |_| Ok(()))
            .unwrap();
        let snapshot_peak = snapshot_probe.peak_bytes();
        let mut held = None;
        let result = doc.commit_page_content_edit_with_budget(0, &expected, &budget, || {
            let edited = expected.with_replaced_operation(0, b"Q", doc.ctx.options())?;
            let changed = edited.stream_bytes(0).unwrap().len();
            let headroom = page_peak.max(page_bytes + changed + snapshot_peak);
            held = Some(budget.reserve(budget.maximum_bytes() - budget.live_bytes() - headroom)?);
            Ok(edited)
        });
        assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        // Entered transactions invalidate old snapshots even when rolled back.
        assert!(doc.revision() > revision);
        let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(unchanged.source_bytes(), expected.source_bytes());
        for index in 0..16 {
            assert_eq!(
                unchanged.stream_object_id(index),
                expected.stream_object_id(index)
            );
        }
        assert_eq!(budget.live_bytes(), held.as_ref().unwrap().bytes());
        drop(held);
        doc.commit_page_content_edit_with_budget(0, &unchanged, &budget, || {
            unchanged.with_replaced_operation_with_budget(0, b"Q", doc.ctx.options(), &budget)
        })
        .unwrap();
        assert_eq!(budget.live_bytes(), 0);
        let edited = doc.get_page(0).unwrap().content_program().unwrap();
        assert_ne!(edited.stream_object_id(0), expected.stream_object_id(0));
        assert_eq!(edited.stream_object_id(1), expected.stream_object_id(1));
        assert_eq!(edited.stream_ranges().len(), 16);
        assert_eq!(edited.operations()[0].operator.name(), "Q");
    }

    #[test]
    fn content_commit_snapshot_budget_failure_does_not_enter_transaction() {
        use crate::{budget::OperationBudget, document::owned::Document};
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        let expected = doc.get_page(0).unwrap().content_program().unwrap();
        let probe = OperationBudget::new(65536);
        let (_, page_lease) = doc
            .page_location_with_budget(0, &probe)
            .unwrap()
            .into_parts();
        let page_peak = probe.peak_bytes();
        let snapshot_probe = OperationBudget::new(65536);
        doc.edit_transaction_with_budget(&snapshot_probe, |_| {
            assert!(snapshot_probe.live_bytes() > page_peak - page_lease.bytes());
            Ok(())
        })
        .unwrap();
        let revision = doc.revision();
        let pool = OperationBudget::new(65536);
        let mut held = None;
        let result = doc.commit_page_content_edit_with_budget(0, &expected, &pool, || {
            held = Some(pool.reserve(pool.maximum_bytes() - pool.live_bytes() - page_peak)?);
            Ok(expected.clone())
        });
        assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        assert_eq!(doc.revision(), revision);
        assert_eq!(pool.live_bytes(), held.as_ref().unwrap().bytes());
        let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(unchanged.source_bytes(), expected.source_bytes());
        assert_eq!(unchanged.stream_object_id(0), expected.stream_object_id(0));
        drop(held);
        doc.commit_page_content_edit_with_budget(0, &expected, &pool, || Ok(expected.clone()))
            .unwrap();
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn working_page_preflight_rejects_without_mutation_and_can_retry() {
        use crate::{budget::OperationBudget, document::owned::Document};
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        let expected = doc.get_page(0).unwrap().content_program().unwrap();
        let revision = doc.revision();
        let budget = OperationBudget::new(16384);
        let mut held = None;
        let result = doc.commit_page_content_edit_with_budget(0, &expected, &budget, || {
            let edited = expected.with_replaced_operation(0, b"Q", doc.ctx.options())?;
            held = Some(budget.reserve(budget.maximum_bytes() - budget.live_bytes())?);
            Ok(edited)
        });
        assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        assert_eq!(doc.revision(), revision);
        let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(unchanged.source_bytes(), expected.source_bytes());
        assert_eq!(unchanged.stream_object_id(0), expected.stream_object_id(0));
        assert_eq!(budget.live_bytes(), held.as_ref().unwrap().bytes());
        drop(held);
        assert_eq!(budget.live_bytes(), 0);
        doc.commit_page_content_edit_with_budget(0, &expected, &budget, || {
            expected.with_replaced_operation_with_budget(0, b"Q", doc.ctx.options(), &budget)
        })
        .unwrap();
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .operations()[0]
                .operator
                .name(),
            "Q"
        );
    }

    #[test]
    fn commit_preflight_budget_rejects_before_edit_and_releases_current_program() {
        use crate::{budget::OperationBudget, document::owned::Document};
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        let expected = doc.get_page(0).unwrap().content_program().unwrap();
        let revision = doc.revision();
        let called = std::cell::Cell::new(false);
        let tiny = OperationBudget::new(1);
        let neighbor = tiny.reserve(1).unwrap();
        assert!(matches!(
            doc.commit_page_content_edit_with_budget(0, &expected, &tiny, || {
                called.set(true);
                Ok(expected.clone())
            }),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(!called.get());
        assert_eq!(tiny.live_bytes(), 1);
        assert_eq!(doc.revision(), revision);
        drop(neighbor);
        let budget = OperationBudget::new(8192);
        let neighbor = budget.reserve(3).unwrap();
        let result = doc.commit_page_content_edit_with_budget(0, &expected, &budget, || {
            assert!(budget.live_bytes() > 3);
            Err::<ContentProgram, _>(PdfError::ObjectError("test edit failure".into()))
        });
        assert!(result.is_err());
        assert_eq!(budget.live_bytes(), 3);
        assert_eq!(doc.revision(), revision);
        doc.commit_page_content_edit_with_budget(0, &expected, &budget, || Ok(expected.clone()))
            .unwrap();
        assert_eq!(budget.live_bytes(), 3);
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            b"q Q"
        );
        let fresh = doc.get_page(0).unwrap().content_program().unwrap();
        doc.commit_page_content_edit_with_budget(0, &fresh, &budget, || {
            fresh.with_replaced_operation_with_budget(0, b"(abc) Tj", doc.ctx.options(), &budget)
        })
        .unwrap();
        assert_eq!(budget.live_bytes(), 3);
        assert!(doc
            .get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes()
            .starts_with(b"(abc) Tj"));
        drop(neighbor);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn borrowed_text_table_retains_lease_after_constructor_return() {
        let entries = vec![(0, b"A".to_vec()), (2, b"B".to_vec())];
        let bytes = 2 * std::mem::size_of::<(usize, &[u8])>();
        let budget = crate::budget::OperationBudget::new(bytes);
        let table = BudgetedTextItems::new(&budget, &entries).unwrap();
        assert_eq!(budget.live_bytes(), bytes);
        assert_eq!(
            table.as_slice(),
            &[(0, b"A".as_slice()), (2, b"B".as_slice())]
        );
        assert!(budget.reserve(1).is_err());
        drop(table);
        assert_eq!(budget.live_bytes(), 0);
        let too_small = crate::budget::OperationBudget::new(bytes - 1);
        assert!(BudgetedTextItems::new(&too_small, &entries).is_err());
        assert_eq!(too_small.live_bytes(), 0);
    }

    #[test]
    fn tj_range_preflight_rejects_invalid_items_outside_endpoints() {
        let limits = crate::parser::options::Limits::default();
        for invalid in ["true", "null", "/Name", "[]", "<<>>"] {
            let source = format!("[(A) {invalid} (B)] TJ");
            let program =
                ContentProgram::from_decoded_bytes(source.into_bytes(), Default::default())
                    .unwrap();
            let array = program.operations()[0].operator.get_as_array(0).unwrap();
            // Both endpoints are valid strings; the malformed middle item
            // must still be rejected, even if it lies outside the edit range.
            for range in [(0, 0)..(0, 1), (0, 0)..(2, 1)] {
                assert!(matches!(
                    validate_text_array_range(array, &range, limits),
                    Err(PdfError::ObjectError(_))
                ));
            }
        }
        let program = ContentProgram::from_decoded_bytes(
            b"[(A) 25 () -10 (B)] TJ".to_vec(),
            Default::default(),
        )
        .unwrap();
        let array = program.operations()[0].operator.get_as_array(0).unwrap();
        validate_text_array_range(array, &((0, 0)..(4, 1)), limits).unwrap();
    }

    #[test]
    fn font_backed_content_commit_shared_pool_rolls_back_error_and_unwind() {
        use crate::{budget::OperationBudget, document::resource_manager::StandardFont};
        for array in [false, true] {
            for unwind in [false, true] {
                let doc = PdfDocument::new().unwrap();
                doc.add_page(0).unwrap();
                let name = doc
                    .resources()
                    .register_standard_font(0, StandardFont::Helvetica)
                    .unwrap();
                let show = if array {
                    "[(AB) 25 (CD)] TJ"
                } else {
                    "(AB) Tj"
                };
                doc.replace_page_contents(0, format!("BT /{name} 12 Tf {show} ET").into_bytes())
                    .unwrap();
                let before = doc.get_page(0).unwrap().content_program().unwrap();
                let revision = doc.revision();
                let pool = OperationBudget::new(2 * 1024 * 1024);
                let neighbor = pool.reserve(7).unwrap();
                let replace = |expected: &ContentProgram| -> PdfResult<()> {
                    let mut retained_codes = None;
                    doc.commit_page_content_edit_with_budget(0, expected, &pool, || {
                        let codes = doc
                            .get_page(0)?
                            .encode_text_for_operation(expected, 2, "XYZ", &pool)?;
                        let codes = retained_codes.insert(codes);
                        assert!(pool.live_bytes() > 7 + codes.as_slice().len());
                        if array {
                            expected.with_replaced_text_array_items_codes_with_budget(
                                2,
                                &[(0, codes.as_slice())],
                                doc.ctx.options(),
                                &pool,
                            )
                        } else {
                            expected.with_replaced_text_codes_with_budget(
                                2,
                                codes.as_slice(),
                                doc.ctx.options(),
                                &pool,
                            )
                        }
                    })?;
                    // The encoded owner remains charged through commit, not just parsing.
                    assert!(
                        pool.live_bytes() > 7 + retained_codes.as_ref().unwrap().as_slice().len()
                    );
                    Ok(())
                };
                let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                    doc.edit_transaction_with_budget(&pool, |_| -> PdfResult<()> {
                        replace(&before)?;
                        let edited = doc.get_page(0)?.content_program()?;
                        assert_ne!(edited.source_bytes(), before.source_bytes());
                        assert!(pool.live_bytes() > 7);
                        if unwind {
                            panic!("font-backed commit outer unwind");
                        }
                        Err(PdfError::DocumentError(
                            "font-backed commit outer rollback".into(),
                        ))
                    })
                }));
                if unwind {
                    assert!(caught.is_err());
                } else {
                    assert!(matches!(caught, Ok(Err(PdfError::DocumentError(_)))));
                }
                assert_eq!(pool.live_bytes(), 7);
                assert!(doc.revision() > revision);
                let restored = doc.get_page(0).unwrap().content_program().unwrap();
                assert_eq!(restored.source_bytes(), before.source_bytes());
                assert_eq!(restored.stream_object_id(0), before.stream_object_id(0));
                assert!(
                    replace(&before).is_err(),
                    "rollback must invalidate the old snapshot"
                );
                assert_eq!(pool.live_bytes(), 7);
                replace(&restored).unwrap();
                assert_eq!(pool.live_bytes(), 7);
                assert_ne!(
                    doc.get_page(0)
                        .unwrap()
                        .content_program()
                        .unwrap()
                        .source_bytes(),
                    before.source_bytes()
                );
                drop(neighbor);
                assert_eq!(pool.live_bytes(), 0);
            }
        }
    }

    #[test]
    fn whole_text_operation_budget_rejects_codes_without_mutation() {
        use crate::{
            document::resource_manager::StandardFont,
            parser::options::{Limits, ParseOptions},
        };
        for array in [false, true] {
            let doc = PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_operation_bytes: 131072,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
            doc.add_page(0).unwrap();
            let name = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            let show = if array {
                "[(AB) 25 (CD)] TJ"
            } else {
                "(AB) Tj"
            };
            doc.replace_page_contents(0, format!("BT /{name} 12 Tf {show} ET").into_bytes())
                .unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let revision = doc.revision();
            let replace = |text: &str| {
                if array {
                    doc.replace_page_text_array_item(0, &root, 2, 0, text)
                } else {
                    doc.replace_page_text(0, &root, 2, text)
                }
            };
            if array {
                for range in [(1, 0)..(2, 0), (0, usize::MAX)..(2, 0), (2, 0)..(0, 0)] {
                    assert!(matches!(
                        doc.replace_page_text_array_range(0, &root, 2, range, "中文"),
                        Err(PdfError::ObjectError(_))
                    ));
                    assert_eq!(doc.revision(), revision);
                }
                for item in [1, usize::MAX] {
                    assert!(matches!(
                        doc.replace_page_text_array_item(0, &root, 2, item, "中文"),
                        Err(PdfError::ObjectError(_))
                    ));
                    assert_eq!(doc.revision(), revision);
                }
            }
            assert!(matches!(
                replace(&"X".repeat(131072)),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(doc.revision(), revision);
            let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
            assert_eq!(unchanged.source_bytes(), root.source_bytes());
            assert_eq!(unchanged.stream_object_id(0), root.stream_object_id(0));
            replace("XY").unwrap();
        }
    }

    #[test]
    fn top_text_range_operation_budget_charges_code_and_replacement_overlap() {
        use crate::{
            document::resource_manager::StandardFont,
            parser::options::{Limits, ParseOptions},
        };
        // Keep the input-dependent encoding peak above fixed font construction.
        let text = "XYZ".repeat(4096);
        let probe = PdfDocument::new().unwrap();
        probe.add_page(0).unwrap();
        let name = probe
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        probe
            .replace_page_contents(0, format!("BT /{name} 12 Tf (AB) Tj ET").into_bytes())
            .unwrap();
        let pool = crate::budget::OperationBudget::new(1048576);
        let expected = probe.get_page(0).unwrap().content_program().unwrap();
        let mut retained_font = None;
        let mut retained_replacement = None;
        probe
            .commit_page_content_edit_with_budget(0, &expected, &pool, || {
                let page = probe.get_page(0)?;
                let font = retained_font
                    .insert(page.font_for_text_operation_with_budget(&expected, 2, &pool)?);
                let codes = font.encode_text_with_budget(&text, probe.ctx.limits(), &pool)?;
                let lease = pool.reserve(2 + codes.as_slice().len())?;
                let bytes = text_range_replacement(
                    font,
                    b"AB",
                    1..1,
                    codes.as_slice(),
                    probe.ctx.limits(),
                )?;
                let replacement = retained_replacement.insert(
                    crate::budget::BudgetedBytes::from_reserved_vec(bytes, lease)?,
                );
                expected.with_replaced_text_codes_with_budget(
                    2,
                    replacement.as_slice(),
                    probe.ctx.options(),
                    &pool,
                )
            })
            .unwrap();
        let peak = pool.peak_bytes();
        drop(retained_replacement);
        drop(retained_font);
        assert_eq!(pool.live_bytes(), 0);
        for (maximum, succeeds) in [(peak - 1, false), (peak, true)] {
            let doc = PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_operation_bytes: maximum,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
            doc.add_page(0).unwrap();
            let name = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            doc.replace_page_contents(0, format!("BT /{name} 12 Tf (AB) Tj ET").into_bytes())
                .unwrap();
            let root = doc.get_page(0).unwrap().content_program().unwrap();
            let revision = doc.revision();
            let result = doc.replace_page_text_range(0, &root, 2, None, 1..1, &text);
            assert_eq!(result.is_ok(), succeeds);
            if !succeeds {
                assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
                assert!(doc.revision() >= revision);
                let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
                assert_eq!(unchanged.source_bytes(), root.source_bytes());
                assert_eq!(unchanged.stream_object_id(0), root.stream_object_id(0));
                // A failed operation does not consume the next operation's pool.
                doc.replace_page_text_range(0, &unchanged, 2, None, 0..2, "X")
                    .unwrap();
            }
        }
    }

    #[test]
    fn text_array_range_checks_total_limits_and_empty_boundaries() {
        use crate::{document::resource_manager::StandardFont, parser::options::Limits};
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{name} 12 Tf [(AB) 25 () -10 (CD)] TJ ET").into_bytes(),
        )
        .unwrap();
        let page = doc.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let font = page.font_for_text_operation(&program, 2).unwrap();
        let array = program.operations()[2].operator.get_as_array(0).unwrap();
        let limits = Limits {
            max_decoded_bytes: 5,
            max_text_chars: 5,
            ..Default::default()
        };
        let budget = limits.operation_budget();
        let owned = budgeted_text_array_range_replacements(
            &budget,
            &font,
            array,
            (0, 1)..(4, 1),
            b"X",
            limits,
        )
        .unwrap();
        let charged = owned.items.capacity() * std::mem::size_of::<(usize, Vec<u8>)>()
            + owned
                .items
                .iter()
                .map(|(_, bytes)| bytes.capacity())
                .sum::<usize>();
        assert_eq!(budget.live_bytes(), charged);
        assert_eq!(
            owned.as_slice(),
            &[(0, b"AX".to_vec()), (2, vec![]), (4, b"D".to_vec())]
        );
        drop(owned);
        assert_eq!(budget.live_bytes(), 0);
        assert!(budgeted_text_array_range_replacements(
            &budget,
            &font,
            array,
            (0, 1)..(4, 1),
            b"XYZW",
            limits,
        )
        .is_err());
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(
            text_array_range_replacements(&font, array, (2, 0)..(2, 0), b"X", limits).unwrap(),
            vec![(2, b"X".to_vec())]
        );
        let retained = crate::budget::BudgetedBytes::copy_from_slice(&budget, b"neighbor").unwrap();
        let retained_capacity = retained.capacity();
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let replacements = budgeted_text_array_range_replacements(
                &budget,
                &font,
                array,
                (0, 1)..(4, 1),
                b"X",
                limits,
            )
            .unwrap();
            let _borrowed = BudgetedTextItems::new(&budget, replacements.as_slice()).unwrap();
            assert!(budget.live_bytes() > retained_capacity);
            panic!("injected TJ owner unwind");
        }));
        assert!(unwind.is_err());
        assert_eq!(budget.live_bytes(), retained_capacity);
        assert_eq!(retained.as_slice(), b"neighbor");
        drop(retained);
        assert_eq!(budget.live_bytes(), 0);
        assert_eq!(
            text_array_range_replacements(&font, array, (0, 1)..(4, 1), b"X", limits).unwrap(),
            vec![(0, b"AX".to_vec()), (2, vec![]), (4, b"D".to_vec())]
        );
        for limited in [
            Limits {
                max_text_chars: 4,
                ..limits
            },
            Limits {
                max_decoded_bytes: 4,
                ..limits
            },
        ] {
            assert!(matches!(
                text_array_range_replacements(&font, array, (2, 0)..(2, 0), b"X", limited),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert_eq!(
            text_array_range_replacements(&font, array, (0, 0)..(4, 2), b"", limits).unwrap(),
            vec![(0, vec![]), (2, vec![]), (4, vec![])]
        );
        // Even deletion to an empty result must validate the complete input.
        assert!(matches!(
            text_array_range_replacements(
                &font,
                array,
                (0, 0)..(4, 2),
                b"",
                Limits {
                    max_text_chars: 3,
                    ..limits
                },
            ),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn text_range_output_limits_validate_combined_bytes_and_characters() {
        use crate::{document::resource_manager::StandardFont, parser::options::Limits};
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        doc.replace_page_contents(0, format!("BT /{name} 12 Tf (AB) Tj ET").into_bytes())
            .unwrap();
        let page = doc.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let font = page.font_for_text_operation(&program, 2).unwrap();
        let limits = Limits {
            max_decoded_bytes: 5,
            max_text_chars: 5,
            ..Limits::default()
        };
        assert_eq!(
            text_range_replacement(&font, b"AB", 1..1, b"XYZ", limits).unwrap(),
            b"AXYZB"
        );
        for limited in [
            Limits {
                max_decoded_bytes: 4,
                ..limits
            },
            Limits {
                max_text_chars: 4,
                ..limits
            },
        ] {
            assert!(matches!(
                text_range_replacement(&font, b"AB", 1..1, b"XYZ", limited),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        // Empty output is legal even with zero byte and character allowance.
        assert_eq!(
            text_range_replacement(
                &font,
                b"",
                0..0,
                b"",
                Limits {
                    max_decoded_bytes: 0,
                    max_text_chars: 0,
                    ..limits
                }
            )
            .unwrap(),
            b""
        );
    }

    #[test]
    fn path_parameter_replacement_rejects_invalid_old_and_new_values() {
        use crate::{
            objects::pdf_number::PdfNumber, page::operator::Operator, parser::options::Limits,
        };
        let limits = Limits::default();
        for value in [f32::NAN, f32::INFINITY, f32::NEG_INFINITY] {
            let old = Operator::new(
                "m".into(),
                vec![
                    PdfObject::PdfNumber(PdfNumber::new(value)),
                    PdfObject::PdfNumber(PdfNumber::new(0.)),
                ],
            );
            assert!(matches!(
                path_parameter_replacement(&old, &[1., 2.], limits),
                Err(PdfError::PageOperatorError(_))
            ));
            let valid =
                ContentProgram::from_decoded_bytes(b"0 0 m".to_vec(), Default::default()).unwrap();
            assert!(matches!(
                path_parameter_replacement(&valid.operations()[0].operator, &[value, 2.], limits),
                Err(PdfError::ObjectError(_))
            ));
        }
        for body in [
            b"(bad) 0 m".as_slice(),
            b"0 m".as_slice(),
            b"0 0 0 m".as_slice(),
            b"S".as_slice(),
        ] {
            let program =
                ContentProgram::from_decoded_bytes(body.to_vec(), Default::default()).unwrap();
            assert!(path_parameter_replacement(
                &program.operations()[0].operator,
                &[1., 2.],
                limits
            )
            .is_err());
        }
    }

    #[test]
    fn path_parameter_output_budget_includes_operator_name() {
        use crate::parser::options::Limits;
        for (body, values, expected) in [
            (b"0 0 m".as_slice(), vec![1., -2.], b"1 -2 m".as_slice()),
            (
                b"0 0 0 0 re".as_slice(),
                vec![1., 2., -3., 4.],
                b"1 2 -3 4 re".as_slice(),
            ),
            (
                b"0 0 0 0 0 0 c".as_slice(),
                vec![1., 2., 3., 4., 5., 6.],
                b"1 2 3 4 5 6 c".as_slice(),
            ),
        ] {
            let program =
                ContentProgram::from_decoded_bytes(body.to_vec(), Default::default()).unwrap();
            let target = &program.operations()[0].operator;
            let limits = Limits {
                max_decoded_bytes: expected.len(),
                ..Limits::default()
            };
            assert_eq!(
                path_parameter_replacement(target, &values, limits).unwrap(),
                expected
            );
            for limit in [0, expected.len() - 1] {
                assert!(matches!(
                    path_parameter_replacement(
                        target,
                        &values,
                        Limits {
                            max_decoded_bytes: limit,
                            ..limits
                        }
                    ),
                    Err(PdfError::ResourceLimit(_))
                ));
            }
        }
    }

    #[test]
    fn text_commit_copies_shared_encoded_stream_and_preserves_untouched_encoding() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let mut stream_dict = PdfDict::default();
        stream_dict.insert(
            "Filter".into(),
            PdfObject::PdfName(PdfName::new("ASCIIHexDecode".into())),
        );
        stream_dict.insert(
            "Private".into(),
            PdfObject::PdfName(PdfName::new("Keep".into())),
        );
        let encoded = b"42542028412920546A2045540A>".to_vec(); // BT (A) Tj ET\n
        let shared = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                stream_dict.clone(),
                encoded.clone(),
            )))
            .unwrap();
        let tail_encoded = b"7120510A>".to_vec(); // q Q\n
        let tail = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                stream_dict,
                tail_encoded.clone(),
            )))
            .unwrap();
        let mut contents = PdfArray::default();
        for id in [shared, tail] {
            contents
                .try_add_obj(PdfObject::PdfReference(PdfReference::from_id(id)))
                .unwrap();
        }
        for page in [0, 1] {
            let location = doc.page_location(page).unwrap();
            let mut dict = location.page_dict;
            dict.insert("Contents".into(), PdfObject::PdfArray(contents.clone()));
            doc.update_object(location.page_id, PdfObject::PdfDict(dict))
                .unwrap();
        }
        let original = doc.get_page(0).unwrap().content_program().unwrap();
        doc.replace_page_text_codes(0, &original, 1, b"BC").unwrap();
        let edited = doc.get_page(0).unwrap().content_program().unwrap();
        let other = doc.get_page(1).unwrap().content_program().unwrap();
        assert_eq!(other.source_bytes(), original.source_bytes());
        assert_eq!(other.stream_object_id(0), Some(shared));
        assert_eq!(edited.stream_object_id(1), Some(tail));
        assert_ne!(edited.stream_object_id(0), Some(shared));
        let changed = doc
            .resolve_object(edited.stream_object_id(0).unwrap())
            .unwrap()
            .into_stream()
            .unwrap();
        assert!(changed.dict().get("Filter").is_none());
        let private = PdfObject::PdfName(PdfName::new("Keep".into()));
        assert_eq!(changed.dict().get("Private"), Some(&private));
        assert_eq!(changed.raw_data(), edited.stream_bytes(0).unwrap());
        for (id, bytes) in [(shared, &encoded), (tail, &tail_encoded)] {
            let stream = doc.resolve_object(id).unwrap().into_stream().unwrap();
            assert_eq!(stream.raw_data(), bytes);
            assert!(stream.dict().get("Filter").is_some());
        }
        let reopened = crate::document::owned::Document::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(
            reopened
                .get_page(0)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            edited.source_bytes()
        );
        assert_eq!(
            reopened
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            original.source_bytes()
        );
    }
}
