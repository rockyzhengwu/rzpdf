//! Immutable signed-form/annotation update workflow. No arbitrary object edits.
use super::*;
use crate::{
    budget::MemoryLease,
    document::{
        acroform::{FormTextStyle, FormValue},
        identity::ObjectHandle,
    },
};

/// Source-specific, immutable, single-use controlled update. Not a general edit
/// transaction. Private bytes are revalidated before being returned to storage.
pub struct PreparedSignedUpdate {
    bytes: BudgetedBytes,
    stamp: DocumentStamp,
    completed: Cell<bool>,
    previous: Vec<SignatureReport>,
    _lease: MemoryLease,
}
impl std::fmt::Debug for PreparedSignedUpdate {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PreparedSignedUpdate")
            .field("completed", &self.completed.get())
            .finish_non_exhaustive()
    }
}

fn validate(
    doc: &Document,
    bytes: &[u8],
    previous: &[SignatureReport],
    budget: &OperationBudget,
) -> PdfResult<()> {
    if !bytes.starts_with(
        doc.signature_source()?
            .ok_or_else(|| invalid("immutable source"))?,
    ) {
        return Err(malformed("signed update source prefix"));
    }
    let (copy, _lease) = BudgetedBytes::copy_from_slice(budget, bytes)?.into_reserved_parts();
    let check = Document::from_bytes_with_options(copy, doc.ctx().options())?;
    let reports = check.verify_signatures_with_budget(budget)?;
    fields::preserve_reports(previous, &reports)?;
    if reports.len() != previous.len() {
        return Err(malformed("controlled update changed signature count"));
    }
    changes::candidate(doc, &check, budget)?;
    if check
        .signature_modifications_with_budget(budget)?
        .iter()
        .any(|r| r.authorization != changes::ModificationAuthorization::Allowed)
    {
        return Err(invalid("controlled update failed history audit"));
    }
    Ok(())
}

fn prepared(
    doc: &Document,
    entries: Vec<(ObjectId, PdfObject)>,
    previous: Vec<SignatureReport>,
    budget: &OperationBudget,
) -> PdfResult<PreparedSignedUpdate> {
    let plan = append::SignedRevisionPlan::for_validated_entries(doc, entries)?;
    let bytes = doc.append_validated_revision_with_budget(&plan, budget)?;
    validate(doc, bytes.as_slice(), &previous, budget)?;
    Ok(PreparedSignedUpdate {
        bytes,
        stamp: doc.stamp(),
        completed: Cell::new(false),
        previous,
        _lease: budget.reserve(16 * 8192)?,
    })
}

impl Document {
    pub fn prepare_signed_form_fill(
        &self,
        field: &ObjectHandle,
        value: FormValue,
        style: Option<FormTextStyle>,
    ) -> PdfResult<PreparedSignedUpdate> {
        self.prepare_signed_form_fill_with_budget(
            field,
            value,
            style,
            &self.ctx().limits().operation_budget(),
        )
    }
    /// Existing Tx/checkbox fields only. Style is required for Tx; old resources
    /// remain unchanged and supplied fonts must already belong to the source.
    pub fn prepare_signed_form_fill_with_budget(
        &self,
        field: &ObjectHandle,
        value: FormValue,
        style: Option<FormTextStyle>,
        budget: &OperationBudget,
    ) -> PdfResult<PreparedSignedUpdate> {
        let source = self
            .signature_source()?
            .ok_or_else(|| invalid("immutable source"))?;
        let _construction = budget.reserve(
            source
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(4 * 1024 * 1024))
                .ok_or_else(|| PdfError::ResourceLimit("signed form construction".into()))?,
        )?;
        let previous = fields::ready_source(self, budget)?;
        if previous.is_empty() {
            return Err(invalid("signed source required"));
        }
        let id = self.validate_handle(field)?;
        let tree = self.form_field_tree()?;
        let node = tree
            .iter()
            .find(|n| n.handle.object_id() == id)
            .ok_or_else(|| invalid("field not in source tree"))?;
        if !node.children.is_empty() || node.widgets.is_empty() || node.flags & 1 != 0 {
            return Err(invalid("writable terminal field required"));
        }
        if !policy::permits(
            &self.signature_constraints_with_budget(budget)?,
            changes::ModificationKind::FormFill,
            Some(id),
        ) {
            return Err(PdfError::PermissionDenied("signed field is locked".into()));
        }
        let (mut dict, _dict_lease) = resolve_dict(self, &reference(id), budget)?;
        let mut appearance_leases = Vec::new();
        let mut appearance_work = crate::document::acroform::AppearanceWork::default();
        let mut entries = Vec::new();
        let mut next = fields::fresh_id(self)?;
        match (node.field_type.as_deref(), value) {
            (Some("Tx"), FormValue::Text(text)) => {
                if text.chars().count() > self.ctx().limits().max_text_chars
                    || (node.flags & (1 << 12) == 0 && text.chars().any(char::is_control))
                {
                    return Err(invalid("text length/control characters"));
                }
                // Inherited MaxLen and sensitive text modes must not be lost.
                let mut ancestor = Some(node);
                while let Some(n) = ancestor {
                    let d = self
                        .resolve_object(n.handle.object_id())?
                        .into_dict()
                        .ok_or_else(|| malformed("field ancestor"))?;
                    if let Some(max) = d.get("MaxLen") {
                        let max = max.as_u32().ok_or_else(|| malformed("MaxLen integer"))?;
                        if text.chars().count() > max as usize {
                            return Err(invalid("text exceeds MaxLen"));
                        }
                        break;
                    }
                    ancestor = n
                        .parent
                        .as_ref()
                        .and_then(|p| tree.iter().find(|n| n.handle.object_id() == p.object_id()));
                }
                if node.flags & ((1 << 13) | (1 << 20) | (1 << 24) | (1 << 25)) != 0 {
                    return Err(invalid("password/file/comb/rich text mode unsupported"));
                }
                let style =
                    style.ok_or_else(|| invalid("explicit text appearance style required"))?;
                dict.try_insert(
                    "V".into(),
                    PdfObject::PdfString(PdfString::from_text_with_limits(
                        &text,
                        &self.ctx().limits(),
                    )?),
                )?;
                for widget in &node.widgets {
                    let wid = self.validate_handle(&widget.handle)?;
                    let (mut w, _lease) = if wid == id {
                        crate::objects::clone_dict_with_budget(
                            &dict,
                            budget,
                            self.ctx().limits(),
                            0,
                        )?
                    } else {
                        resolve_dict(self, &reference(wid), budget)?
                    };
                    let r = widget.rect;
                    let (stream, payload_lease) = self.signature_text_appearance(
                        &text,
                        &style,
                        r[2] - r[0],
                        r[3] - r[1],
                        budget,
                        &mut appearance_work,
                    )?;
                    appearance_leases.push(payload_lease);
                    let aid = ObjectId::new(next, 0);
                    next = next
                        .checked_add(1)
                        .ok_or_else(|| PdfError::ResourceLimit("form AP object number".into()))?;
                    entries.push((aid, PdfObject::PdfStream(stream)));
                    let mut ap = PdfDict::default();
                    ap.try_insert("N".into(), reference(aid))?;
                    w.try_insert("AP".into(), PdfObject::PdfDict(ap))?;
                    if wid == id {
                        dict = w;
                    } else {
                        entries.push((wid, PdfObject::PdfDict(w)));
                    }
                }
            }
            (Some("Btn"), FormValue::Checkbox(checked))
                if node.flags & ((1 << 15) | (1 << 16)) == 0 && style.is_none() =>
            {
                let mut selected: Option<String> = None;
                for widget in &node.widgets {
                    let wid = self.validate_handle(&widget.handle)?;
                    let (mut w, _lease) = resolve_dict(self, &reference(wid), budget)?;
                    let ap = w
                        .get("AP")
                        .and_then(PdfObject::as_dict)
                        .and_then(|d| d.get("N"))
                        .and_then(PdfObject::as_dict)
                        .ok_or_else(|| invalid("direct checkbox state AP required"))?;
                    let states: Vec<_> = ap
                        .into_iter()
                        .filter(|(k, _)| k.as_str() != "Off")
                        .map(|(k, _)| k.clone())
                        .collect();
                    if !ap.contains_key("Off") || states.len() != 1 {
                        return Err(invalid("unambiguous checkbox on/off states required"));
                    }
                    let state = if checked {
                        states[0].clone()
                    } else {
                        "Off".into()
                    };
                    if selected.as_ref().is_some_and(|s| *s != state) {
                        return Err(invalid("checkbox presentations disagree"));
                    }
                    selected = Some(state.clone());
                    w.try_insert("AS".into(), name(&state))?;
                    if wid == id {
                        dict = w;
                    } else {
                        entries.push((wid, PdfObject::PdfDict(w)));
                    }
                }
                dict.try_insert(
                    "V".into(),
                    name(&selected.ok_or_else(|| invalid("checkbox Widget required"))?),
                )?;
            }
            _ => return Err(invalid("signed fill supports text and checkbox only")),
        }
        entries.push((id, PdfObject::PdfDict(dict)));
        prepared(self, entries, previous, budget)
    }

    pub fn prepare_signed_square(
        &self,
        page_index: u32,
        rect: [f32; 4],
        contents: &str,
    ) -> PdfResult<PreparedSignedUpdate> {
        self.prepare_signed_square_with_budget(
            page_index,
            rect,
            contents,
            &self.ctx().limits().operation_budget(),
        )
    }
    pub fn prepare_signed_square_with_budget(
        &self,
        page_index: u32,
        rect: [f32; 4],
        contents: &str,
        budget: &OperationBudget,
    ) -> PdfResult<PreparedSignedUpdate> {
        let source = self
            .signature_source()?
            .ok_or_else(|| invalid("immutable source"))?;
        let _construction = budget.reserve(
            source
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(4 * 1024 * 1024))
                .ok_or_else(|| PdfError::ResourceLimit("signed annotation construction".into()))?,
        )?;
        let previous = fields::ready_source(self, budget)?;
        if previous.is_empty() {
            return Err(invalid("signed source required"));
        }
        if !policy::permits(
            &self.signature_constraints_with_budget(budget)?,
            changes::ModificationKind::Annotation,
            None,
        ) {
            return Err(PdfError::PermissionDenied(
                "DocMDP denies annotations".into(),
            ));
        }
        let width = rect[2] - rect[0];
        let height = rect[3] - rect[1];
        if !rect.iter().all(|v| v.is_finite())
            || !width.is_finite()
            || !height.is_finite()
            || width <= 2.
            || height <= 2.
        {
            return Err(invalid("annotation rectangle"));
        }
        let page = PageTree::page_at_with_budget(self.ctx(), page_index, budget)?;
        let (page, _lease) = page.into_parts();
        let id = ObjectId::new(fields::fresh_id(self)?, 0);
        let apid = ObjectId::new(
            id.obj_num()
                .checked_add(1)
                .ok_or_else(|| PdfError::ResourceLimit("annotation ID".into()))?,
            0,
        );
        let mut appearance = PdfDict::default();
        appearance.try_insert("Type".into(), name("XObject"))?;
        appearance.try_insert("Subtype".into(), name("Form"))?;
        appearance.try_insert(
            "BBox".into(),
            crate::document::appearance::nums(&[0., 0., width, height])?,
        )?;
        appearance.try_insert(
            "Matrix".into(),
            crate::document::appearance::nums(&[1., 0., 0., 1., 0., 0.])?,
        )?;
        appearance.try_insert("Resources".into(), PdfObject::PdfDict(PdfDict::default()))?;
        let stream = crate::objects::pdf_stream::PdfStream::from_decoded_data(
            appearance,
            format!("q 1 0 0 RG 1 w .5 .5 {} {} re S Q", width - 1., height - 1.).into_bytes(),
        )?;
        let mut d = PdfDict::default();
        for (key, value) in [
            ("Type", name("Annot")),
            ("Subtype", name("Square")),
            ("P", reference(page.page_id)),
            ("F", number(4)?),
            ("Rect", crate::document::appearance::nums(&rect)?),
            (
                "Contents",
                PdfObject::PdfString(PdfString::from_text_with_limits(
                    contents,
                    &self.ctx().limits(),
                )?),
            ),
            ("C", crate::document::appearance::nums(&[1., 0., 0.])?),
        ] {
            d.try_insert(key.into(), value)?;
        }
        let mut ap = PdfDict::default();
        ap.try_insert("N".into(), reference(apid))?;
        d.try_insert("AP".into(), PdfObject::PdfDict(ap))?;
        let mut page_dict = page.page_dict;
        let (mut annots, _lease) = match page_dict.get("Annots") {
            Some(v) => resolve_array(self, v, budget)?,
            None => (PdfArray::default(), budget.reserve(0)?),
        };
        annots.try_add_obj(reference(id))?;
        page_dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
        prepared(
            self,
            vec![
                (id, PdfObject::PdfDict(d)),
                (apid, PdfObject::PdfStream(stream)),
                (page.page_id, PdfObject::PdfDict(page_dict)),
            ],
            previous,
            budget,
        )
    }

    pub fn complete_signed_update(&self, prepared: &PreparedSignedUpdate) -> PdfResult<Vec<u8>> {
        Ok(self
            .complete_signed_update_with_budget(prepared, &self.ctx().limits().operation_budget())?
            .into_reserved_parts()
            .0)
    }
    pub fn complete_signed_update_with_budget(
        &self,
        prepared: &PreparedSignedUpdate,
        budget: &OperationBudget,
    ) -> PdfResult<BudgetedBytes> {
        if !self.is_stamp_current(&prepared.stamp) || prepared.completed.get() {
            return Err(invalid("stale/foreign/consumed signed update"));
        }
        let output = BudgetedBytes::copy_from_slice(budget, prepared.bytes.as_slice())?;
        validate(self, output.as_slice(), &prepared.previous, budget)?;
        prepared.completed.set(true);
        Ok(output)
    }
}
