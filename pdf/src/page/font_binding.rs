use super::{content_program::ContentProgram, PdfPage};
use crate::{
    error::{PdfError, PdfResult},
    font::pdf_font::Font,
    objects::PdfObject,
};
use std::{
    io::{Read, Seek},
};
#[cfg(test)]
use std::rc::Rc;

pub(crate) struct BudgetedFont {
    font: Font,
    _lease: crate::budget::MemoryLease,
}

/// The object is destroyed before its reservation. No raw owned object escapes
/// resource lookup while its lease is released separately.
struct ResolvedResource {
    object: PdfObject,
    _lease: Option<crate::budget::MemoryLease>,
}

impl std::ops::Deref for ResolvedResource {
    type Target = PdfObject;
    fn deref(&self) -> &PdfObject { &self.object }
}

impl std::borrow::Borrow<PdfObject> for ResolvedResource {
    fn borrow(&self) -> &PdfObject { &self.object }
}

// q/Q aliases retain the same derived font data and reservation.
struct SelectedFont {
    font: Font,
    lease: Option<crate::budget::MemoryLease>,
}

struct FontSlot {
    font: SelectedFont,
    aliases: usize,
}

/// Stable indices share fonts without an opaque Rc control allocation. Vacant
/// slots are reused; the allocator charges the actual old/new vector layouts.
struct FontSelections {
    slots: allocator_api2::vec::Vec<Option<FontSlot>, crate::budget::table_allocator::TableAllocator>,
}

impl FontSelections {
    fn new(budget: Option<&crate::budget::OperationBudget>) -> Self {
        let allocator = budget.map(crate::budget::table_allocator::TableAllocator::new)
            .unwrap_or_else(crate::budget::table_allocator::TableAllocator::persistent);
        Self { slots: allocator_api2::vec::Vec::new_in(allocator) }
    }

    fn insert(&mut self, font: SelectedFont) -> PdfResult<usize> {
        let slot = FontSlot { font, aliases: 1 };
        if let Some(index) = self.slots.iter().position(Option::is_none) {
            self.slots[index] = Some(slot);
            return Ok(index);
        }
        self.slots.try_reserve(1).map_err(|_| PdfError::ResourceLimit("font selection slots".into()))?;
        let index = self.slots.len();
        self.slots.push(Some(slot));
        Ok(index)
    }

    fn share(&mut self, index: usize) -> PdfResult<()> {
        let slot = self.slots[index].as_mut().expect("live font selection");
        slot.aliases = slot.aliases.checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("font selection aliases".into()))?;
        Ok(())
    }

    fn release(&mut self, index: usize) {
        let slot = self.slots[index].as_mut().expect("live font selection");
        slot.aliases -= 1;
        if slot.aliases == 0 { self.slots[index] = None; }
    }

    fn take(&mut self, index: usize) -> PdfResult<SelectedFont> {
        if self.slots[index].as_ref().expect("live font selection").aliases != 1 {
            return Err(PdfError::FontError("font selection still shared".into()));
        }
        Ok(self.slots[index].take().unwrap().font)
    }
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct TextMetrics {
    pub(crate) font_size: f32,
    pub(crate) char_space: f32,
    pub(crate) word_space: f32,
    pub(crate) horizontal_scale: f32,
}

impl Default for TextMetrics {
    fn default() -> Self {
        Self { font_size: 1.0, char_space: 0.0, word_space: 0.0, horizontal_scale: 1.0 }
    }
}

pub(crate) struct BudgetedTextMetrics {
    pub(crate) font: BudgetedFont,
    pub(crate) metrics: TextMetrics,
}

/// Form callers can carry spacing even before any font has been selected.
/// Moving this state also moves its optional font reservation, never a clone.
#[derive(Default)]
pub(crate) struct BudgetedTextState {
    pub(crate) font: Option<BudgetedFont>,
    pub(crate) metrics: TextMetrics,
}

impl BudgetedTextState {
    pub(crate) fn into_metrics(self) -> PdfResult<BudgetedTextMetrics> {
        Ok(BudgetedTextMetrics {
            font: self.font.ok_or_else(|| PdfError::FontError("Nested text has no effective font".into()))?,
            metrics: self.metrics,
        })
    }
}

impl BudgetedTextMetrics {
    /// Bind a newly registered font to the same effective text state.
    pub(crate) fn with_replacement_font(
        &self,
        font: Font,
        lease: crate::budget::MemoryLease,
    ) -> Self {
        Self { font: BudgetedFont { font, _lease: lease }, metrics: self.metrics }
    }

    /// Advance in TJ units for this effective font and spacing. Preserve f64
    /// until old/new measurements are combined, including across font bindings.
    /// Tw applies to a one-byte 0x20 code, not a two-byte CID for Unicode space.
    pub(crate) fn advance_1000(
        &self,
        codes: &[u8],
        limits: crate::parser::options::Limits,
    ) -> PdfResult<f64> {
        if self.metrics.font_size == 0.0 {
            return Err(PdfError::UnsupportedFeature("Advance compensation at zero font size".into()));
        }
        let mut total = 0.0f64;
        self.font.visit_chars_limited(codes, limits.max_text_chars, |character| {
            let mut advance = f64::from(character.width())
                + 1000.0 * f64::from(self.metrics.char_space) / f64::from(self.metrics.font_size);
            if character.code() == 32 && character.length() == 1 {
                advance += 1000.0 * f64::from(self.metrics.word_space) / f64::from(self.metrics.font_size);
            }
            total += advance;
            if !total.is_finite() {
                return Err(PdfError::FontError("Text advance overflows".into()));
            }
            Ok(())
        })?;
        Ok(total)
    }

    /// The writing-axis displacement is measured before Tz/CTM, using signed
    /// W2 advances for vertical fonts. Tw applies only to a one-byte 0x20 code.
    pub(crate) fn replacement_adjustment(
        &self,
        old: &[u8],
        new: &[u8],
        limits: crate::parser::options::Limits,
    ) -> PdfResult<f32> {
        self.replacement_adjustment_to(old, self, new, limits)
    }

    /// Cross-font compensation requires the same effective text state and axis.
    /// Each code string is measured by its own binding (including Tw semantics).
    pub(crate) fn replacement_adjustment_to(
        &self,
        old: &[u8],
        replacement: &Self,
        new: &[u8],
        limits: crate::parser::options::Limits,
    ) -> PdfResult<f32> {
        if self.metrics != replacement.metrics || self.font.writting_mode() != replacement.font.writting_mode() {
            return Err(PdfError::UnsupportedFeature("Cross-font compensation requires matching text state and writing axis".into()));
        }
        let adjustment = (replacement.advance_1000(new, limits)? - self.advance_1000(old, limits)?) as f32;
        if !adjustment.is_finite() {
            return Err(PdfError::FontError("TJ compensation overflows".into()));
        }
        Ok(adjustment)
    }
}

impl std::ops::Deref for BudgetedFont {
    type Target = Font;
    fn deref(&self) -> &Font {
        &self.font
    }
}

pub(crate) struct BudgetedEncodedText {
    codes: crate::budget::BudgetedBytes,
    _font: BudgetedFont,
}

impl BudgetedEncodedText {
    pub(crate) fn as_slice(&self) -> &[u8] {
        self.codes.as_slice()
    }
}

impl<R: Read + Seek> PdfPage<'_, R> {
    /// Track only font selection in the top-level content program, not layout.
    /// Unknown prefix operators are rejected rather than assuming no state effect.
    pub(crate) fn encode_text_for_operation(
        &self,
        program: &ContentProgram,
        target: usize,
        text: &str,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedEncodedText> {
        let font = self.font_for_text_operation_with_budget(program, target, budget)?;
        let codes = font.encode_text_with_budget(text, self.ctx.limits(), budget)?;
        Ok(BudgetedEncodedText { codes, _font: font })
    }

    pub(crate) fn font_for_text_operation_with_budget(
        &self,
        program: &ContentProgram,
        target: usize,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedFont> {
        Ok(self.text_metrics_for_operation_with_budget(program, target, budget)?.font)
    }

    pub(crate) fn text_metrics_for_operation_with_budget(
        &self,
        program: &ContentProgram,
        target: usize,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedTextMetrics> {
        if !program
            .operations()
            .get(target)
            .is_some_and(|operation| matches!(operation.operator.name(), "Tj" | "TJ" | "'" | "\""))
        {
            return Err(PdfError::UnsupportedFeature(
                "Unicode replacement requires a text-show operation".into(),
            ));
        }
        let (scope, mut scope_lease) = self.resource_with_budget(budget)?;
        let mut retained = None;
        let mut metrics = TextMetrics::default();
        let font = self
            .font_before_operation_in_scope_tracked(
                program,
                target,
                &scope,
                None,
                Some(budget),
                &mut retained,
                Some(&mut metrics),
                TextMetrics::default(),
            )?
            .ok_or_else(|| PdfError::FontError("text replacement has no effective font".into()))?;
        if let Some(mut lease) = retained {
            scope_lease.absorb(&mut lease)?;
        }
        Ok(BudgetedTextMetrics {
            font: BudgetedFont { font, _lease: scope_lease },
            metrics,
        })
    }

    pub(crate) fn font_for_text_operation(
        &self,
        program: &ContentProgram,
        target: usize,
    ) -> PdfResult<Font> {
        self.font_for_text_operation_in_scope(program, target, &self.resource()?)
    }

    /// Prefix font selection within one explicit effective resource scope.
    /// Does not infer an inherited caller font when this stream has no binding.
    pub(crate) fn font_for_text_operation_in_scope(
        &self,
        program: &ContentProgram,
        target: usize,
        scope: &crate::objects::pdf_dict::PdfDict,
    ) -> PdfResult<Font> {
        if !program
            .operations()
            .get(target)
            .is_some_and(|operation| matches!(operation.operator.name(), "Tj" | "TJ" | "'" | "\""))
        {
            return Err(PdfError::UnsupportedFeature(
                "Unicode replacement requires a text-show operation".into(),
            ));
        }
        self.font_before_operation_in_scope(program, target, scope, None)?
            .ok_or_else(|| PdfError::FontError("text replacement has no effective font".into()))
    }

    /// Caller font is inherited into a Form; local Tf/gs can replace it and q/Q
    /// restores the actual font, rather than an index from a different stream.
    pub(crate) fn font_before_operation_in_scope(
        &self,
        program: &ContentProgram,
        target: usize,
        scope: &crate::objects::pdf_dict::PdfDict,
        inherited: Option<Font>,
    ) -> PdfResult<Option<Font>> {
        self.font_before_operation_in_scope_inner(program, target, scope, inherited, None)
    }

    /// Move the inherited owner across stream boundaries without cloning or
    /// releasing its reservation while the next prefix is being interpreted.
    pub(crate) fn font_before_operation_in_scope_with_budget(
        &self,
        program: &ContentProgram,
        target: usize,
        scope: &crate::objects::pdf_dict::PdfDict,
        inherited: Option<BudgetedFont>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<Option<BudgetedFont>> {
        let mut lease = budget.reserve(0)?;
        let inherited = if let Some(mut inherited) = inherited {
            lease.absorb(&mut inherited._lease)?;
            Some(SelectedFont {
                font: inherited.font,
                lease: Some(std::mem::replace(&mut lease, budget.reserve(0)?)),
            })
        } else {
            None
        };
        let mut retained = None;
        let font = self.font_before_operation_in_scope_tracked(
            program,
            target,
            scope,
            inherited,
            Some(budget),
            &mut retained,
            None,
            TextMetrics::default(),
        )?;
        if let Some(mut child) = retained {
            lease.absorb(&mut child)?;
        }
        Ok(font.map(|font| BudgetedFont {
            font,
            _lease: lease,
        }))
    }

    /// Preserve inherited font and metrics through one stream prefix. Returns
    /// scalar state even when neither the caller nor this prefix selected a font.
    pub(crate) fn text_state_before_operation_in_scope_with_budget(
        &self,
        program: &ContentProgram,
        target: usize,
        scope: &crate::objects::pdf_dict::PdfDict,
        inherited: BudgetedTextState,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<BudgetedTextState> {
        let mut lease = budget.reserve(0)?;
        let initial_metrics = inherited.metrics;
        let inherited_font = if let Some(mut font) = inherited.font {
            lease.absorb(&mut font._lease)?;
            Some(SelectedFont {
                font: font.font,
                lease: Some(std::mem::replace(&mut lease, budget.reserve(0)?)),
            })
        } else { None };
        let mut retained = None;
        let mut metrics = initial_metrics;
        let font = self.font_before_operation_in_scope_tracked(
            program, target, scope, inherited_font, Some(budget), &mut retained,
            Some(&mut metrics), initial_metrics,
        )?;
        if let Some(mut retained) = retained { lease.absorb(&mut retained)?; }
        Ok(BudgetedTextState {
            font: font.map(|font| BudgetedFont { font, _lease: lease }),
            metrics,
        })
    }

    fn font_before_operation_in_scope_inner(
        &self,
        program: &ContentProgram,
        target: usize,
        scope: &crate::objects::pdf_dict::PdfDict,
        inherited: Option<Font>,
        budget: Option<&crate::budget::OperationBudget>,
    ) -> PdfResult<Option<Font>> {
        self.font_before_operation_in_scope_tracked(
            program, target, scope,
            inherited.map(|font| SelectedFont { font, lease: None }),
            budget, &mut None, None, TextMetrics::default(),
        )
    }

    fn font_before_operation_in_scope_tracked(
        &self,
        program: &ContentProgram,
        target: usize,
        scope: &crate::objects::pdf_dict::PdfDict,
        inherited: Option<SelectedFont>,
        budget: Option<&crate::budget::OperationBudget>,
        retained: &mut Option<crate::budget::MemoryLease>,
        metrics_out: Option<&mut TextMetrics>,
        initial_metrics: TextMetrics,
    ) -> PdfResult<Option<Font>> {
        let construct = |dict: &crate::objects::pdf_dict::PdfDict| -> PdfResult<SelectedFont> {
            if let Some(pool) = budget {
                let (font, lease) = Font::try_new_with_budget(dict, self.ctx, pool)?;
                Ok(SelectedFont { font, lease: Some(lease) })
            } else {
                Ok(SelectedFont { font: Font::try_new(dict, self.ctx)?, lease: None })
            }
        };
        let target_op = program.operations().get(target).ok_or_else(|| {
            PdfError::ContentParseError("text operation index out of range".into())
        })?;
        let text_target = matches!(target_op.operator.name(), "Tj" | "TJ" | "'" | "\"");
        if !text_target && target_op.operator.name() != "Do" {
            return Err(PdfError::UnsupportedFeature(
                "Unicode replacement requires a text-show operation".into(),
            ));
        }
        let limits = self.ctx.limits();
        // Savepoints share immutable fonts rather than deep-cloning font tables.
        let mut selections = FontSelections::new(budget);
        let mut selection = inherited.map(|font| selections.insert(font)).transpose()?;
        let mut metrics = initial_metrics;
        let mut _stack_lease = budget.map(|pool| pool.reserve(0)).transpose()?;
        let mut stack = Vec::new();
        let mut in_text = false;
        for source in &program.operations()[..target] {
            let op = &source.operator;
            if !super::opcode::is_op(op.name().as_bytes()) {
                return Err(PdfError::UnsupportedFeature(
                    "font binding across unknown content operator".into(),
                ));
            }
            if let Some(count) = super::opcode::operand_count(op.name()) {
                if op.num_operands() != count {
                    return Err(PdfError::ContentParseError(
                        "font binding prefix operand count".into(),
                    ));
                }
            }
            let mut resolve = |object: &PdfObject| -> PdfResult<ResolvedResource> {
                if let Some(pool) = budget {
                    let (object, lease) = self.ctx.resolve_owned_with_budget(object, pool)?;
                    Ok(ResolvedResource { object, _lease: Some(lease) })
                } else {
                    Ok(ResolvedResource { object: self.ctx.resolve_owned(object)?, _lease: None })
                }
            };
            match op.name() {
                "q" => {
                    let bytes = stack
                        .len()
                        .checked_add(1)
                        .and_then(|count| {
                            count.checked_mul(std::mem::size_of::<(Option<usize>, TextMetrics)>())
                        })
                        .filter(|bytes| *bytes <= limits.max_decoded_bytes);
                    if stack.len() >= limits.max_depth || bytes.is_none() {
                        return Err(PdfError::ResourceLimit("font binding state stack".into()));
                    }
                    if let Some(pool) = budget {
                        if stack.len() == stack.capacity() {
                            let size = std::mem::size_of::<(Option<usize>, TextMetrics)>();
                            let maximum = limits.max_depth.min(limits.max_decoded_bytes / size);
                            let capacity = stack
                                .capacity()
                                .checked_mul(2)
                                .unwrap_or(maximum)
                                .max(2)
                                .min(maximum);
                            let bytes = capacity.checked_mul(size).ok_or_else(|| {
                                PdfError::ResourceLimit("font stack capacity".into())
                            })?;
                            let mut lease = pool.reserve(bytes)?;
                            let mut replacement = Vec::new();
                            replacement.try_reserve_exact(capacity).map_err(|_| {
                                PdfError::ResourceLimit("font binding stack allocation".into())
                            })?;
                            lease.resize(replacement.capacity().checked_mul(size).ok_or_else(
                                || PdfError::ResourceLimit("font stack capacity".into()),
                            )?)?;
                            replacement.append(&mut stack);
                            stack = replacement;
                            _stack_lease = Some(lease);
                        }
                    } else {
                        stack.try_reserve(1).map_err(|_| {
                            PdfError::ResourceLimit("font binding stack allocation".into())
                        })?;
                    }
                    if let Some(index) = selection { selections.share(index)?; }
                    stack.push((selection, metrics));
                }
                "Q" => {
                    if let Some(index) = selection { selections.release(index); }
                    (selection, metrics) = stack.pop().ok_or_else(|| {
                        PdfError::ContentParseError("font binding Q without q".into())
                    })?
                }
                "BT" => {
                    if in_text {
                        return Err(PdfError::ContentParseError(
                            "nested BT before text replacement".into(),
                        ));
                    }
                    in_text = true;
                }
                "ET" => {
                    if !in_text {
                        return Err(PdfError::ContentParseError(
                            "ET without BT before text replacement".into(),
                        ));
                    }
                    in_text = false;
                }
                "Tf" => {
                    metrics.font_size = finite_text_metric(op.get_as_f32(1)?)?;
                    let object = self.font_scope_resource(
                        scope,
                        "Font",
                        op.get_as_name(0)?.name(),
                        &mut resolve,
                    )?;
                    let dict = object.as_dict().ok_or_else(|| {
                        PdfError::FontError("text font resource is not a dictionary".into())
                    })?;
                    let next = selections.insert(construct(dict)?)?;
                    if let Some(index) = selection { selections.release(index); }
                    selection = Some(next);
                }
                "gs" => {
                    let object = self.font_scope_resource(
                        scope,
                        "ExtGState",
                        op.get_as_name(0)?.name(),
                        &mut resolve,
                    )?;
                    let dict = object.as_dict().ok_or_else(|| {
                        PdfError::PageResourceError("ExtGState must be a dictionary".into())
                    })?;
                    if !super::all_state::supported_ext_gs_keys(dict) {
                        return Err(PdfError::UnsupportedFeature(
                            "font binding across unsupported ExtGState".into(),
                        ));
                    }
                    if let Some(value) = dict.get("BM") {
                        if super::general_state::resolve_blend_mode_using(
                            value,
                            limits,
                            &mut resolve,
                        )?
                        .is_none()
                        {
                            return Err(PdfError::UnsupportedFeature(
                                "font binding across unknown blend mode".into(),
                            ));
                        }
                    }
                    if let Some(value) = dict.get("Font") {
                        let (font, size) = self.ext_state_font(value, &mut resolve)?;
                        metrics.font_size = size;
                        let next = selections.insert(construct(font.as_dict().ok_or_else(|| {
                            PdfError::FontError(
                                "ExtGState Font must reference a font dictionary".into(),
                            )
                        })?)?)?;
                        if let Some(index) = selection { selections.release(index); }
                        selection = Some(next);
                    }
                }
                "Tc" => metrics.char_space = finite_text_metric(op.get_as_f32(0)?)?,
                "Tw" => metrics.word_space = finite_text_metric(op.get_as_f32(0)?)?,
                "Tz" => metrics.horizontal_scale = finite_text_metric(op.get_as_f32(0)?)? / 100.0,
                "\"" => {
                    metrics.word_space = finite_text_metric(op.get_as_f32(0)?)?;
                    metrics.char_space = finite_text_metric(op.get_as_f32(1)?)?;
                }
                _ => {}
            }
        }
        if text_target && !in_text {
            return Err(PdfError::ContentParseError(
                "text replacement outside BT/ET".into(),
            ));
        }
        if !text_target && in_text {
            return Err(PdfError::ContentParseError(
                "Form font binding invocation inside BT/ET".into(),
            ));
        }
        // Drop all saved aliases before transferring the selected Font out.
        for (saved, _) in stack.drain(..) {
            if let Some(index) = saved { selections.release(index); }
        }
        drop(stack);
        let selected = selection
            .map(|index| selections.take(index))
            .transpose()?;
        let result = if let Some(mut selected) = selected {
            *retained = selected.lease.take();
            Some(selected.font)
        } else { None };
        // The target double quote sets spacing before painting its string.
        if target_op.operator.name() == "\"" {
            metrics.word_space = finite_text_metric(target_op.operator.get_as_f32(0)?)?;
            metrics.char_space = finite_text_metric(target_op.operator.get_as_f32(1)?)?;
        }
        if let Some(output) = metrics_out { *output = metrics; }
        Ok(result)
    }

    fn font_scope_resource(
        &self,
        scope: &crate::objects::pdf_dict::PdfDict,
        category: &str,
        name: &str,
        resolve: &mut impl FnMut(&PdfObject) -> PdfResult<ResolvedResource>,
    ) -> PdfResult<ResolvedResource> {
        let entries = scope.get(category).ok_or_else(|| {
            PdfError::PageResourceError(format!("font binding {category} scope missing"))
        })?;
        let owned = resolve(entries)?;
        let entries = owned.as_dict().ok_or_else(|| {
            PdfError::PageResourceError("font binding scope must be dictionary".into())
        })?;
        let object = entries
            .get(name)
            .ok_or_else(|| PdfError::PageResourceError("font binding resource missing".into()))?;
        resolve(object)
    }

    fn ext_state_font(
        &self,
        value: &PdfObject,
        resolve: &mut impl FnMut(&PdfObject) -> PdfResult<ResolvedResource>,
    ) -> PdfResult<(ResolvedResource, f32)> {
        let value = resolve(value)?;
        let array = value
            .as_array()
            .filter(|array| array.len() == 2)
            .ok_or_else(|| PdfError::FontError("ExtGState Font requires [font size]".into()))?;
        let size = {
            let size = resolve(array.get(1).unwrap())?;
            size.as_f32().filter(|size| size.is_finite())
                .ok_or_else(|| PdfError::FontError("ExtGState font size must be finite".into()))?
        };
        let font = resolve(array.get(0).unwrap())?;
        if font.as_dict().is_none() {
            return Err(PdfError::FontError(
                "ExtGState font must be a dictionary".into(),
            ));
        }
        Ok((font, size))
    }
}

fn finite_text_metric(value: f32) -> PdfResult<f32> {
    if value.is_finite() { Ok(value) }
    else { Err(PdfError::FontError("Non-finite text metric".into())) }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn resolved_resource_blend_elements_release_individually_and_retry() {
        use crate::{budget::OperationBudget, objects::{pdf_array::PdfArray, pdf_name::PdfName}, parser::options::Limits};
        let limits = Limits::default();
        let name = || PdfObject::PdfName(PdfName::new("Multiply".into()));
        let value = PdfObject::PdfArray(PdfArray::from_items((0..32).map(|_| name()).collect()));
        let measure = OperationBudget::new(1024 * 1024);
        let (object, lease) = crate::objects::clone_object_with_budget(&value, &measure, limits, 0).unwrap();
        let array_bytes = measure.live_bytes();
        drop(ResolvedResource { object, _lease: Some(lease) });
        assert_eq!(measure.live_bytes(), 0);
        let child_pool = OperationBudget::new(65536);
        let (object, lease) = crate::objects::clone_object_with_budget(&name(), &child_pool, limits, 0).unwrap();
        let child_peak = child_pool.peak_bytes();
        drop(ResolvedResource { object, _lease: Some(lease) });
        assert_eq!(child_pool.live_bytes(), 0);
        let parse = |input: &PdfObject, pool: &OperationBudget, panic_at: Option<usize>| {
            let mut calls = 0;
            super::super::general_state::resolve_blend_mode_using(input, limits, &mut |source| {
                if std::ptr::eq(input, &value) && calls > 0 {
                    // Only the array survives between child resolutions.
                    assert_eq!(pool.live_bytes(), array_bytes);
                }
                calls += 1;
                if panic_at == Some(calls) { panic!("resource resolver unwind"); }
                let (object, lease) = crate::objects::clone_object_with_budget(source, pool, limits, 0)?;
                Ok(ResolvedResource { object, _lease: Some(lease) })
            })
        };
        let probe = OperationBudget::new(1024 * 1024);
        assert_eq!(parse(&value, &probe, None).unwrap(), Some(super::super::general_state::BlendMode::Multiply));
        assert_eq!(probe.peak_bytes(), array_bytes + child_peak);
        assert_eq!(probe.live_bytes(), 0);
        let tight = OperationBudget::new(probe.peak_bytes());
        let neighbor = tight.reserve(1).unwrap();
        // Use an equivalent independent input to avoid the no-neighbor assertion.
        let copy = value.clone();
        assert!(matches!(parse(&copy, &tight, None), Err(PdfError::ResourceLimit(_))));
        assert_eq!(tight.live_bytes(), 1);
        drop(neighbor);
        assert!(parse(&value, &tight, None).is_ok());
        assert_eq!(tight.live_bytes(), 0);
        let invalid = PdfObject::PdfArray(PdfArray::from_items(vec![name(), PdfObject::PdfNull]));
        assert!(matches!(parse(&invalid, &probe, None), Err(PdfError::PageResourceError(_))));
        assert_eq!(probe.live_bytes(), 0);
        assert!(std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _ = parse(&value, &probe, Some(5));
        })).is_err());
        assert_eq!(probe.live_bytes(), 0);
    }

    #[test]
    fn advance_measurement_matches_compensation_and_enforces_limits() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc.resources().register_standard_font(0, StandardFont::Courier).unwrap();
        doc.replace_page_contents(0, format!("BT /{name} 10 Tf 2 Tc 7 Tw -75 Tz (A A) Tj ET").into_bytes()).unwrap();
        let page = doc.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let target = program.operations().iter().position(|op| op.operator.name() == "Tj").unwrap();
        let pool = OperationBudget::new(4 * 1024 * 1024);
        let mut state = page.text_metrics_for_operation_with_budget(&program, target, &pool).unwrap();
        let limits = crate::parser::options::Limits::default();
        assert_eq!(state.advance_1000(b"A A", limits).unwrap(), 3100.0);
        assert_eq!(state.advance_1000(b"", limits).unwrap(), 0.0);
        assert_eq!(state.replacement_adjustment(b"A A", b"A", limits).unwrap(), -2300.0);
        assert!(state.advance_1000(b"A A", crate::parser::options::Limits { max_text_chars: 2, ..limits }).is_err());
        assert_eq!(state.advance_1000(b"A A", limits).unwrap(), 3100.0);
        state.metrics.font_size = 0.0;
        assert!(matches!(state.advance_1000(b"A", limits), Err(PdfError::UnsupportedFeature(_))));
        drop(state);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn cross_font_adjustment_measures_each_binding_and_rejects_state_mismatch() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let courier = doc.resources().register_standard_font(0, StandardFont::Courier).unwrap();
        let helvetica = doc.resources().register_standard_font(0, StandardFont::Helvetica).unwrap();
        doc.replace_page_contents(0, format!("BT /{courier} 10 Tf 2 Tc 7 Tw -75 Tz (A A) Tj /{helvetica} 10 Tf (A A) Tj ET").into_bytes()).unwrap();
        let page = doc.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let targets: Vec<_> = program.operations().iter().enumerate().filter_map(|(index, op)| (op.operator.name() == "Tj").then_some(index)).collect();
        let pool = OperationBudget::new(4 * 1024 * 1024);
        let old = page.text_metrics_for_operation_with_budget(&program, targets[0], &pool).unwrap();
        let mut new = page.text_metrics_for_operation_with_budget(&program, targets[1], &pool).unwrap();
        let limits = crate::parser::options::Limits::default();
        let adjustment = old.replacement_adjustment_to(b"A A", &new, b"A A", limits).unwrap();
        assert_ne!(adjustment, 0.0);
        assert_eq!(adjustment, (new.advance_1000(b"A A", limits).unwrap() - old.advance_1000(b"A A", limits).unwrap()) as f32);
        assert_eq!(old.replacement_adjustment(b"A A", b"A A", limits).unwrap(), 0.0);
        new.metrics.word_space += 1.0;
        assert!(matches!(old.replacement_adjustment_to(b"A A", &new, b"A A", limits), Err(PdfError::UnsupportedFeature(_))));
        new.metrics = old.metrics;
        assert_eq!(old.replacement_adjustment_to(b"A A", &new, b"A A", limits).unwrap(), adjustment);
        drop(old);
        drop(new);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn inherited_text_state_keeps_unbound_spacing_and_restores_local_font() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let courier = doc.resources().register_standard_font(0, StandardFont::Courier).unwrap();
        let helvetica = doc.resources().register_standard_font(0, StandardFont::Helvetica).unwrap();
        let page = doc.get_page(0).unwrap();
        let scope = page.resource().unwrap();
        let parse = |bytes: Vec<u8>| ContentProgram::from_decoded_bytes(bytes, Default::default()).unwrap();
        let unbound = parse(b"2 Tc 7 Tw -75 Tz /Outer Do".to_vec());
        let parent = parse(format!("BT /{courier} 10 Tf 2 Tc 7 Tw -75 Tz ET /Outer Do").into_bytes());
        let middle = parse(b"q 8 Tc 9 Tw 50 Tz /Inner Do Q /Inner Do".to_vec());
        let leaf = parse(format!("BT q /{helvetica} 20 Tf 4 Tc 5 Tw 25 Tz Q (A) Tj ET").into_bytes());
        let bound_leaf = parse(format!("BT /{courier} 10 Tf (A) Tj ET").into_bytes());
        let pool = OperationBudget::new(4 * 1024 * 1024);
        let neighbor = pool.reserve(7).unwrap();
        let state = page.text_state_before_operation_in_scope_with_budget(&unbound, 3, &scope, Default::default(), &pool).unwrap();
        assert!(state.font.is_none());
        assert_eq!(state.metrics.char_space, 2.0);
        assert_eq!(state.metrics.word_space, 7.0);
        assert_eq!(state.metrics.horizontal_scale, -0.75);
        assert_eq!(pool.live_bytes(), 7);
        let state = page.text_state_before_operation_in_scope_with_budget(&bound_leaf, 2, &scope, state, &pool).unwrap();
        assert_eq!(state.metrics, TextMetrics { font_size: 10.0, char_space: 2.0, word_space: 7.0, horizontal_scale: -0.75 });
        assert_eq!(state.font.as_ref().unwrap().name(), "Courier");
        drop(state);
        assert_eq!(pool.live_bytes(), 7);
        for (target, char_space, word_space, scale) in [(4, 8.0, 9.0, 0.5), (6, 2.0, 7.0, -0.75)] {
            let state = page.text_state_before_operation_in_scope_with_budget(&parent, 6, &scope, Default::default(), &pool).unwrap();
            let state = page.text_state_before_operation_in_scope_with_budget(&middle, target, &scope, state, &pool).unwrap();
            let state = page.text_state_before_operation_in_scope_with_budget(&leaf, 7, &scope, state, &pool).unwrap();
            assert_eq!(state.metrics, TextMetrics { font_size: 10.0, char_space, word_space, horizontal_scale: scale });
            assert_eq!(state.font.as_ref().unwrap().name(), "Courier");
            assert_eq!(pool.live_bytes(), state.font.as_ref().unwrap()._lease.bytes() + 7);
            drop(state);
            assert_eq!(pool.live_bytes(), 7);
        }
        let invalid = parse(b"BT bogus (A) Tj ET".to_vec());
        let state = page.text_state_before_operation_in_scope_with_budget(&parent, 6, &scope, Default::default(), &pool).unwrap();
        assert!(page.text_state_before_operation_in_scope_with_budget(&invalid, 2, &scope, state, &pool).is_err());
        assert_eq!(pool.live_bytes(), 7);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let state = page.text_state_before_operation_in_scope_with_budget(&parent, 6, &scope, Default::default(), &pool).unwrap();
            let _state = page.text_state_before_operation_in_scope_with_budget(&leaf, 7, &scope, state, &pool).unwrap();
            panic!("inherited text state owner unwind");
        }));
        assert_eq!(caught.unwrap_err().downcast_ref::<&str>().copied(), Some("inherited text state owner unwind"));
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn budgeted_text_metrics_restore_font_spacing_and_target_quote_state() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let courier = doc.resources().register_standard_font(0, StandardFont::Courier).unwrap();
        let helvetica = doc.resources().register_standard_font(0, StandardFont::Helvetica).unwrap();
        doc.replace_page_contents(0, format!(
            "BT /{courier} 12 Tf 2 Tc 3 Tw 50 Tz q /{helvetica} 24 Tf 7 Tc 8 Tw -75 Tz (X) Tj Q 9 10 (Y) \" (Z) Tj ET"
        ).into_bytes()).unwrap();
        let page = doc.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let targets: Vec<_> = program.operations().iter().enumerate()
            .filter(|(_, op)| matches!(op.operator.name(), "Tj" | "\""))
            .map(|(index, _)| index).collect();
        let expected = [
            ("Helvetica", TextMetrics { font_size: 24.0, char_space: 7.0, word_space: 8.0, horizontal_scale: -0.75 }),
            ("Courier", TextMetrics { font_size: 12.0, char_space: 10.0, word_space: 9.0, horizontal_scale: 0.5 }),
            ("Courier", TextMetrics { font_size: 12.0, char_space: 10.0, word_space: 9.0, horizontal_scale: 0.5 }),
        ];
        let pool = OperationBudget::new(2 * 1024 * 1024);
        let neighbor = pool.reserve(7).unwrap();
        for (&target, (name, metrics)) in targets.iter().zip(expected) {
            let owned = page.text_metrics_for_operation_with_budget(&program, target, &pool).unwrap();
            assert_eq!(owned.font.name(), name);
            assert_eq!(owned.metrics, metrics);
            assert_eq!(pool.live_bytes(), owned.font._lease.bytes() + 7);
            drop(owned);
            assert_eq!(pool.live_bytes(), 7);
        }
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _owned = page.text_metrics_for_operation_with_budget(&program, targets[0], &pool).unwrap();
            panic!("text metric owner unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn ext_state_font_size_is_tracked_and_restored_with_budget() {
        use crate::{budget::OperationBudget, objects::{pdf_array::PdfArray, pdf_dict::PdfDict, pdf_number::PdfNumber}};
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let page_id = doc.get_page(0).unwrap().page_node.as_object_id().unwrap();
        let courier = doc.resources().register_standard_font(0, StandardFont::Courier).unwrap();
        let helvetica = doc.resources().register_standard_font(0, StandardFont::Helvetica).unwrap();
        let mut scope = doc.get_page(0).unwrap().resource().unwrap();
        let font = scope.get("Font").unwrap().as_dict().unwrap().get(&helvetica).unwrap().clone();
        let mut gs = PdfDict::default();
        gs.insert("Font".into(), PdfObject::PdfArray(PdfArray::from_items(vec![font, PdfObject::PdfNumber(PdfNumber::new(17.0))])));
        let mut states = PdfDict::default();
        states.insert("GS".into(), PdfObject::PdfDict(gs));
        scope.insert("ExtGState".into(), PdfObject::PdfDict(states));
        let mut dict = doc.resolve_object(page_id).unwrap().into_dict().unwrap();
        dict.insert("Resources".into(), PdfObject::PdfDict(scope));
        doc.update_object(page_id, PdfObject::PdfDict(dict)).unwrap();
        doc.replace_page_contents(0, format!("BT /{courier} 12 Tf 2 Tc 3 Tw 50 Tz q /GS gs (X) Tj Q (Y) Tj ET").into_bytes()).unwrap();
        let page = doc.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let targets: Vec<_> = program.operations().iter().enumerate()
            .filter(|(_, op)| op.operator.name() == "Tj").map(|(index, _)| index).collect();
        let probe = OperationBudget::new(2 * 1024 * 1024);
        for (target, name, size) in [(targets[0], "Helvetica", 17.0), (targets[1], "Courier", 12.0)] {
            let owned = page.text_metrics_for_operation_with_budget(&program, target, &probe).unwrap();
            assert_eq!(owned.font.name(), name);
            assert_eq!(owned.metrics, TextMetrics { font_size: size, char_space: 2.0, word_space: 3.0, horizontal_scale: 0.5 });
            drop(owned);
            assert_eq!(probe.live_bytes(), 0);
        }
        let pool = OperationBudget::new(probe.peak_bytes() + 6);
        let neighbor = pool.reserve(7).unwrap();
        assert!(page.text_metrics_for_operation_with_budget(&program, targets[0], &pool).is_err());
        assert_eq!(pool.live_bytes(), 7);
        drop(neighbor);
        let owned = page.text_metrics_for_operation_with_budget(&program, targets[0], &pool).unwrap();
        assert_eq!(owned.metrics.font_size, 17.0);
        drop(owned);
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn budgeted_inherited_font_moves_across_streams_and_releases_on_error() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let first = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        let second = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let page = doc.get_page(0).unwrap();
        let scope = page.resource().unwrap();
        let caller = ContentProgram::from_decoded_bytes(
            format!("BT /{first} 12 Tf ET /X Do").into_bytes(),
            Default::default(),
        )
        .unwrap();
        let leaf = ContentProgram::from_decoded_bytes(
            format!("BT q /{second} 12 Tf Q (A) Tj ET").into_bytes(),
            Default::default(),
        )
        .unwrap();
        let pool = OperationBudget::new(2 * 1024 * 1024);
        let construct = || {
            page.font_before_operation_in_scope_with_budget(&caller, 3, &scope, None, &pool)
                .unwrap()
                .unwrap()
        };
        let inherited = construct();
        let retained = pool.live_bytes();
        assert!(retained > 0);
        let selected = page
            .font_before_operation_in_scope_with_budget(&leaf, 4, &scope, Some(inherited), &pool)
            .unwrap()
            .unwrap();
        assert_eq!(selected.name(), "Courier");
        assert_eq!(pool.live_bytes(), retained);
        assert_eq!(pool.live_bytes(), selected._lease.bytes());
        drop(selected);
        assert_eq!(pool.live_bytes(), 0);
        // A local replacement without a saved alias must release the inherited
        // owner, including its retained resource copies, before returning.
        let replacement = ContentProgram::from_decoded_bytes(
            format!("BT /{second} 12 Tf (A) Tj ET").into_bytes(),
            Default::default(),
        ).unwrap();
        let local = page.font_before_operation_in_scope_with_budget(
            &replacement, 2, &scope, None, &pool,
        ).unwrap().unwrap();
        let local_retained = pool.live_bytes();
        assert_eq!(local.name(), "Helvetica");
        drop(local);
        let replaced = page.font_before_operation_in_scope_with_budget(
            &replacement, 2, &scope, Some(construct()), &pool,
        ).unwrap().unwrap();
        assert_eq!(replaced.name(), "Helvetica");
        assert_eq!(pool.live_bytes(), local_retained);
        drop(replaced);
        assert_eq!(pool.live_bytes(), 0);
        let replaced_state = page.text_state_before_operation_in_scope_with_budget(
            &replacement, 2, &scope,
            BudgetedTextState { font: Some(construct()), metrics: TextMetrics::default() },
            &pool,
        ).unwrap();
        assert_eq!(replaced_state.font.as_ref().unwrap().name(), "Helvetica");
        assert_eq!(pool.live_bytes(), local_retained);
        drop(replaced_state);
        assert_eq!(pool.live_bytes(), 0);
        // The second selection grows the slot table after constructing its
        // font; the third constructs with the grown table already live. After
        // that warm-up, earlier resource-copy reservations must not accumulate.
        let mut repeated_baseline = None;
        let mut initial_measurement = None;
        for count in [2, 3, 32] {
            let mut source = String::from("BT ");
            for _ in 0..count { source.push_str(&format!("/{second} 12 Tf ")); }
            source.push_str("(A) Tj ET");
            let repeated = ContentProgram::from_decoded_bytes(
                source.into_bytes(), Default::default(),
            ).unwrap();
            let repeated_pool = OperationBudget::new(2 * 1024 * 1024);
            let owned = page.font_before_operation_in_scope_with_budget(
                &repeated, count + 1, &scope, None, &repeated_pool,
            ).unwrap().unwrap();
            assert_eq!(owned.name(), "Helvetica");
            let measurement = (repeated_pool.peak_bytes(), repeated_pool.live_bytes());
            assert!(measurement.1 > 0);
            if count == 2 {
                initial_measurement = Some(measurement);
            } else if let Some(baseline) = repeated_baseline {
                assert_eq!(measurement, baseline);
            } else {
                let initial = initial_measurement.unwrap();
                assert!(measurement.0 >= initial.0);
                assert_eq!(measurement.1, initial.1);
                repeated_baseline = Some(measurement);
            }
            drop(owned);
            assert_eq!(repeated_pool.live_bytes(), 0);
        }
        let invalid =
            ContentProgram::from_decoded_bytes(b"BT bogus (A) Tj ET".to_vec(), Default::default())
                .unwrap();
        assert!(page
            .font_before_operation_in_scope_with_budget(
                &invalid,
                2,
                &scope,
                Some(construct()),
                &pool
            )
            .is_err());
        assert_eq!(pool.live_bytes(), 0);
        let foreign = OperationBudget::new(2 * 1024 * 1024);
        assert!(page
            .font_before_operation_in_scope_with_budget(
                &leaf,
                4,
                &scope,
                Some(construct()),
                &foreign
            )
            .is_err());
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(foreign.live_bytes(), 0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _selected = page
                .font_before_operation_in_scope_with_budget(
                    &leaf,
                    4,
                    &scope,
                    Some(construct()),
                    &pool,
                )
                .unwrap()
                .unwrap();
            panic!("inherited font unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        drop(construct());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn shared_font_savepoints_restore_binding_and_transfer_with_open_outer_q() {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let first = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let second = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{first} 12 Tf q /{second} 12 Tf q /{first} 12 Tf Q (A) Tj ET Q")
                .into_bytes(),
        )
        .unwrap();
        let page = doc.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        assert_eq!(
            page.font_for_text_operation(&program, 7).unwrap().name(),
            "Courier"
        );
        let pool = crate::budget::OperationBudget::new(262144);
        assert_eq!(
            page.font_for_text_operation_with_budget(&program, 7, &pool)
                .unwrap()
                .name(),
            "Courier"
        );
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn selected_font_reservations_follow_last_alias_and_retry_after_rejection() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        let mut dict = PdfDict::default();
        dict.insert("Subtype".into(), PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("Type1".into())));
        dict.insert("BaseFont".into(), PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("Courier".into())));
        let construct = |pool: &OperationBudget| -> PdfResult<Rc<SelectedFont>> {
            let (font, lease) = Font::try_new_with_budget(&dict, doc.ctx(), pool)?;
            Ok(Rc::new(SelectedFont { font, lease: Some(lease) }))
        };
        let probe = OperationBudget::new(1024 * 1024);
        let mut selection = construct(&probe).unwrap();
        let retained = probe.live_bytes();
        assert!(retained > 0);
        selection = construct(&probe).unwrap();
        let switching_peak = probe.peak_bytes();
        assert_eq!(probe.live_bytes(), retained);
        // Replacing a font constructs the new one while the old one is live,
        // but previous selections must not accumulate reservations.
        for _ in 0..32 {
            selection = construct(&probe).unwrap();
            assert_eq!(probe.live_bytes(), retained);
            assert_eq!(probe.peak_bytes(), switching_peak);
        }
        let saved = selection.clone();
        drop(selection);
        assert_eq!(probe.live_bytes(), retained);
        assert_eq!(saved.font.name(), "Courier");
        drop(saved);
        assert_eq!(probe.live_bytes(), 0);

        let pool = OperationBudget::new(switching_peak);
        let neighbor = pool.reserve(1).unwrap();
        let first = construct(&pool).unwrap();
        assert!(matches!(construct(&pool), Err(PdfError::ResourceLimit(_))));
        assert_eq!(pool.live_bytes(), retained + 1);
        drop(neighbor);
        let second = construct(&pool).unwrap();
        assert_eq!(pool.live_bytes(), 2 * retained);
        drop((first, second));
        assert_eq!(pool.live_bytes(), 0);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let first = construct(&pool).unwrap();
            let _saved = first.clone();
            let _second = construct(&pool).unwrap();
            panic!("font alias cleanup");
        }));
        assert!(unwind.is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn selection_slots_charge_layout_reuse_and_reject_growth_atomically() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        let mut dict = PdfDict::default();
        dict.insert("Subtype".into(), PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("Type1".into())));
        dict.insert("BaseFont".into(), PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("Courier".into())));
        // This test isolates control storage. Derived font allocation is tested
        // separately with budget constructors and real prefix interpretation.
        let font = || SelectedFont { font: Font::try_new(&dict, doc.ctx()).unwrap(), lease: None };
        let probe = OperationBudget::new(1024 * 1024);
        let mut arena = FontSelections::new(Some(&probe));
        let first = arena.insert(font()).unwrap();
        let capacity = arena.slots.capacity();
        let initial = probe.live_bytes();
        assert_eq!(initial, capacity * std::mem::size_of::<Option<FontSlot>>());
        arena.share(first).unwrap();
        assert!(arena.take(first).is_err());
        arena.release(first);
        assert_eq!(arena.slots[first].as_ref().unwrap().aliases, 1);
        arena.release(first);
        for _ in 0..32 {
            let index = arena.insert(font()).unwrap();
            assert_eq!(index, first);
            arena.release(index);
            assert_eq!(probe.live_bytes(), initial);
        }
        for _ in 0..capacity { arena.insert(font()).unwrap(); }
        arena.insert(font()).unwrap();
        let grown = probe.live_bytes();
        assert_eq!(grown, arena.slots.capacity() * std::mem::size_of::<Option<FontSlot>>());
        assert_eq!(probe.peak_bytes(), initial + grown);
        drop(arena);
        assert_eq!(probe.live_bytes(), 0);

        let pool = OperationBudget::new(initial + grown);
        let neighbor = pool.reserve(1).unwrap();
        let mut arena = FontSelections::new(Some(&pool));
        for _ in 0..capacity { arena.insert(font()).unwrap(); }
        assert!(matches!(arena.insert(font()), Err(PdfError::ResourceLimit(_))));
        assert_eq!(arena.slots.len(), capacity);
        assert_eq!(arena.slots.capacity(), capacity);
        assert_eq!(pool.live_bytes(), initial + 1);
        assert_eq!(arena.slots[0].as_ref().unwrap().font.font.name(), "Courier");
        drop(neighbor);
        arena.insert(font()).unwrap();
        assert_eq!(pool.live_bytes(), grown);
        drop(arena);
        assert_eq!(pool.live_bytes(), 0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let mut arena = FontSelections::new(Some(&pool));
            let index = arena.insert(font()).unwrap();
            arena.share(index).unwrap();
            panic!("selection arena cleanup");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
    }
    #[test]
    fn font_stack_growth_charges_old_and_new_tables_and_releases_on_error() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let page = doc.get_page(0).unwrap();
        let program = ContentProgram::from_decoded_bytes(
            b"q q q Q Q Q BT (A) Tj ET".to_vec(),
            doc.ctx().options(),
        )
        .unwrap();
        let size = std::mem::size_of::<(Option<usize>, TextMetrics)>();
        let pool = OperationBudget::new(6 * size);
        let neighbor = pool.reserve(1).unwrap();
        assert!(page
            .font_before_operation_in_scope_inner(
                &program,
                7,
                &PdfDict::default(),
                None,
                Some(&pool)
            )
            .is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        assert!(page
            .font_before_operation_in_scope_inner(
                &program,
                7,
                &PdfDict::default(),
                None,
                Some(&pool)
            )
            .unwrap()
            .is_none());
        assert_eq!(pool.peak_bytes(), 6 * size);
        assert_eq!(pool.live_bytes(), 0);
        let invalid =
            ContentProgram::from_decoded_bytes(b"q Q Q BT (A) Tj ET".to_vec(), doc.ctx().options())
                .unwrap();
        assert!(page
            .font_before_operation_in_scope_inner(
                &invalid,
                4,
                &PdfDict::default(),
                None,
                Some(&pool)
            )
            .is_err());
        assert_eq!(pool.live_bytes(), 0);
    }
    use crate::{
        document::{owned::Document, resource_manager::StandardFont},
        objects::{
            pdf_array::PdfArray, pdf_dict::PdfDict, pdf_number::PdfNumber,
            pdf_reference::PdfReference,
        },
    };

    #[test]
    fn two_page_font_results_account_resource_coexistence_and_unwind() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        for (index, kind) in [StandardFont::Courier, StandardFont::Helvetica]
            .into_iter()
            .enumerate()
        {
            let index = index as u32;
            doc.add_page(index).unwrap();
            let name = doc.resources().register_standard_font(index, kind).unwrap();
            doc.replace_page_contents(index, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
                .unwrap();
        }
        let pages = [doc.get_page(0).unwrap(), doc.get_page(1).unwrap()];
        let programs = [
            pages[0].content_program().unwrap(),
            pages[1].content_program().unwrap(),
        ];
        let mut individual_peak = 0;
        for index in 0..2 {
            let pool = OperationBudget::new(2 * 1024 * 1024);
            drop(
                pages[index]
                    .font_for_text_operation_with_budget(&programs[index], 2, &pool)
                    .unwrap(),
            );
            individual_peak = individual_peak.max(pool.peak_bytes());
            assert_eq!(pool.live_bytes(), 0);
        }
        let probe = OperationBudget::new(2 * 1024 * 1024);
        let first = pages[0]
            .font_for_text_operation_with_budget(&programs[0], 2, &probe)
            .unwrap();
        let retained = first._lease.bytes();
        let second = pages[1]
            .font_for_text_operation_with_budget(&programs[1], 2, &probe)
            .unwrap();
        assert_eq!(probe.live_bytes(), retained + second._lease.bytes());
        let peak = probe.peak_bytes();
        assert!(peak > individual_peak);
        drop((first, second));
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak - 1);
        assert!(pool.maximum_bytes() >= individual_peak);
        for index in 0..2 {
            drop(
                pages[index]
                    .font_for_text_operation_with_budget(&programs[index], 2, &pool)
                    .unwrap(),
            );
            assert_eq!(pool.live_bytes(), 0);
        }
        let first = pages[0]
            .font_for_text_operation_with_budget(&programs[0], 2, &pool)
            .unwrap();
        assert!(pages[1]
            .font_for_text_operation_with_budget(&programs[1], 2, &pool)
            .is_err());
        assert_eq!(pool.live_bytes(), retained);
        assert_eq!(first.name(), "Courier");
        drop(first);
        assert_eq!(pool.live_bytes(), 0);
        drop(
            pages[1]
                .font_for_text_operation_with_budget(&programs[1], 2, &pool)
                .unwrap(),
        );
        assert_eq!(pool.live_bytes(), 0);
        let unwind = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _first = pages[0]
                .font_for_text_operation_with_budget(&programs[0], 2, &probe)
                .unwrap();
            let _second = pages[1]
                .font_for_text_operation_with_budget(&programs[1], 2, &probe)
                .unwrap();
            panic!("resource font unwind");
        }));
        assert!(unwind.is_err());
        assert_eq!(probe.live_bytes(), 0);
        for index in 0..2 {
            assert_eq!(
                pages[index].content_program().unwrap().source_bytes(),
                programs[index].source_bytes()
            );
        }
    }

    #[test]
    fn budgeted_font_binding_matches_legacy_and_releases_resource_copies() {
        use crate::budget::OperationBudget;
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        doc.replace_page_contents(0, format!("BT /{name} 12 Tf (A) Tj ET").into_bytes())
            .unwrap();
        let page = doc.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let probe = OperationBudget::new(
            65536
                + crate::font::builtin_font::load_builtin_font_data("Helvetica")
                    .unwrap()
                    .len(),
        );
        let font = page
            .font_for_text_operation_with_budget(&program, 2, &probe)
            .unwrap();
        assert_eq!(
            font.name(),
            page.font_for_text_operation(&program, 2).unwrap().name()
        );
        let peak = probe.peak_bytes();
        assert!(peak > 0);
        assert!(probe.live_bytes() > 0);
        assert_eq!(probe.live_bytes(), font._lease.bytes());
        drop(font);
        assert_eq!(probe.live_bytes(), 0);
        let encoded = page
            .encode_text_for_operation(&program, 2, "ABC", &probe)
            .unwrap();
        assert_eq!(encoded.as_slice(), b"ABC");
        assert!(probe.live_bytes() > encoded.codes.capacity());
        drop(encoded);
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(page
            .font_for_text_operation_with_budget(&program, 2, &pool)
            .is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        assert_eq!(
            page.font_for_text_operation_with_budget(&program, 2, &pool)
                .unwrap()
                .name(),
            "Helvetica"
        );
        assert_eq!(pool.live_bytes(), 0);
        assert!(page
            .font_for_text_operation_with_budget(&program, 0, &pool)
            .is_err());
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn explicit_font_scope_selects_its_own_binding_and_never_falls_back() {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let page_name = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let scoped_name = doc
            .resources()
            .register_standard_font(1, StandardFont::Courier)
            .unwrap();
        assert_eq!(page_name, scoped_name);
        doc.replace_page_contents(0, format!("BT /{page_name} 12 Tf (A) Tj ET").into_bytes())
            .unwrap();
        let page = doc.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        assert_eq!(
            page.font_for_text_operation(&program, 2).unwrap().name(),
            "Helvetica"
        );
        let scope = doc.get_page(1).unwrap().resource().unwrap();
        assert_eq!(
            page.font_for_text_operation_in_scope(&program, 2, &scope)
                .unwrap()
                .name(),
            "Courier"
        );
        assert!(page
            .font_for_text_operation_in_scope(&program, 2, &PdfDict::default())
            .is_err());
        let unbound =
            ContentProgram::from_decoded_bytes(b"BT (A) Tj ET".to_vec(), doc.ctx().options())
                .unwrap();
        assert!(page
            .font_for_text_operation_in_scope(&unbound, 1, &scope)
            .is_err());
        let inherited = page.font_for_text_operation(&program, 2).unwrap();
        for bytes in [b"BT () Tj ET".as_slice(), b"BT <> Tj ET".as_slice()] {
            let empty =
                ContentProgram::from_decoded_bytes(bytes.to_vec(), doc.ctx().options()).unwrap();
            let font = page
                .font_before_operation_in_scope(
                    &empty,
                    1,
                    &PdfDict::default(),
                    Some(inherited.clone()),
                )
                .unwrap()
                .unwrap();
            assert_eq!(font.name(), "Helvetica");
            let encoded = match font {
                Font::Simple(font) => font.encode_text_with_limits("é€", doc.ctx().limits()),
                Font::Type0(font) => font.encode_text_with_limits("é€", doc.ctx().limits()),
            }
            .unwrap();
            assert_eq!(encoded, [0xe9, 0x80]);
            assert!(page
                .font_before_operation_in_scope(&empty, 1, &PdfDict::default(), None)
                .unwrap()
                .is_none());
        }
        assert_eq!(
            page.font_before_operation_in_scope(&unbound, 1, &scope, Some(inherited.clone()))
                .unwrap()
                .unwrap()
                .name(),
            "Helvetica"
        );
        let restored = ContentProgram::from_decoded_bytes(
            format!("q BT /{scoped_name} 12 Tf ET Q BT (A) Tj ET").into_bytes(),
            doc.ctx().options(),
        )
        .unwrap();
        assert_eq!(
            page.font_before_operation_in_scope(&restored, 6, &scope, Some(inherited))
                .unwrap()
                .unwrap()
                .name(),
            "Helvetica"
        );
        let caller = ContentProgram::from_decoded_bytes(
            format!("BT /{page_name} 12 Tf ET /Form Do").into_bytes(),
            doc.ctx().options(),
        )
        .unwrap();
        assert_eq!(
            page.font_before_operation_in_scope(&caller, 3, &page.resource().unwrap(), None)
                .unwrap()
                .unwrap()
                .name(),
            "Helvetica"
        );
    }

    #[test]
    fn unicode_binding_uses_extgstate_font_and_preserves_fontless_state() {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let page = doc.get_page(0).unwrap();
        let font = doc
            .try_insert_object(page.get_resource_font(&name).unwrap().unwrap())
            .unwrap();
        let mut array = PdfArray::default();
        array
            .try_add_obj(PdfObject::PdfReference(PdfReference::from_id(font)))
            .unwrap();
        array
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(12.0)))
            .unwrap();
        let mut state = PdfDict::default();
        state.insert("Font".into(), PdfObject::PdfArray(array));
        let mode = doc
            .try_insert_object(PdfObject::PdfName(crate::objects::pdf_name::PdfName::new(
                "Normal".into(),
            )))
            .unwrap();
        let preferences = doc
            .try_insert_object(PdfObject::PdfArray(PdfArray::from_items(vec![
                PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("UnknownMode".into())),
                PdfObject::PdfReference(PdfReference::from_id(mode)),
            ])))
            .unwrap();
        state.insert(
            "BM".into(),
            PdfObject::PdfReference(PdfReference::from_id(preferences)),
        );
        let mut states = PdfDict::default();
        states.insert("GS".into(), PdfObject::PdfDict(state));
        states.insert("NoFont".into(), PdfObject::PdfDict(PdfDict::default()));
        let mut dict = page.pagedict().unwrap();
        let mut resources = page.resource().unwrap();
        resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
        dict.insert("Resources".into(), PdfObject::PdfDict(resources));
        doc.update_object(
            page.page_node.as_object_id().unwrap(),
            PdfObject::PdfDict(dict),
        )
        .unwrap();
        for (source, index) in [
            ("BT /GS gs (A) Tj ET".to_string(), 2),
            (format!("BT /{name} 12 Tf /NoFont gs (A) Tj ET"), 3),
        ] {
            doc.replace_page_contents(0, source.into_bytes()).unwrap();
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            doc.replace_page_text(0, &snapshot, index, "BC").unwrap();
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .operations()[index]
                    .operator
                    .get_as_string(0)
                    .unwrap()
                    .bytes(),
                b"BC"
            );
            let reopened = Document::from_bytes_with_mode(
                doc.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            let report = reopened
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            assert!(report.diagnostics.is_empty());
            assert_eq!(
                report
                    .characters
                    .iter()
                    .map(|character| character.unicode.as_deref().unwrap())
                    .collect::<String>(),
                "BC"
            );
            assert!(report
                .characters
                .iter()
                .all(|character| character.font_size == 12.0));
        }
    }
}
