//! Immutable signed-revision capabilities and the flat approval workflow.
//! Only sibling bounded workflow builders can construct entry plans. No source
//! editing scope, public arbitrary-object plan or global guard bypass exists.
use super::*;
use crate::{
    budget::MemoryLease,
    document::{PdfDocument, store::StoreEntry},
};
use std::io::{Read, Seek};

pub(crate) struct SignedRevisionPlan {
    stamp: DocumentStamp,
    source_hash: [u8; 32],
    entries: Vec<(ObjectId, PdfObject)>,
}

impl SignedRevisionPlan {
    pub(super) fn for_validated_entries(
        doc: &Document,
        entries: Vec<(ObjectId, PdfObject)>,
    ) -> PdfResult<Self> {
        let source = doc
            .signature_source()?
            .ok_or_else(|| invalid("immutable signature source required"))?;
        Ok(Self {
            stamp: doc.stamp(),
            source_hash: Sha256::digest(source).into(),
            entries,
        })
    }
    pub(crate) fn changes_with_budget<R: Read + Seek>(
        &self,
        doc: &PdfDocument<R>,
        source: &[u8],
        original: &PdfDict,
        budget: &OperationBudget,
    ) -> PdfResult<(Vec<(ObjectId, StoreEntry)>, MemoryLease)> {
        if !doc.is_stamp_current(&self.stamp)
            || <[u8; 32]>::from(Sha256::digest(source)) != self.source_hash
            || &*doc.ctx().trailer() != original
        {
            return Err(invalid("approval plan source/stamp mismatch"));
        }
        let (pending, _pending_lease) = doc.ctx().save_changes_with_budget(budget)?;
        if !pending.into_entries().is_empty() {
            return Err(invalid("approval requires an unchanged source overlay"));
        }
        let mut lease =
            budget.reserve(self.entries.len() * std::mem::size_of::<(ObjectId, StoreEntry)>())?;
        let mut changes = Vec::new();
        changes
            .try_reserve_exact(self.entries.len())
            .map_err(|_| PdfError::ResourceLimit("approval change allocation".into()))?;
        lease.resize(changes.capacity() * std::mem::size_of::<(ObjectId, StoreEntry)>())?;
        for (id, object) in &self.entries {
            let (object, mut payload) =
                crate::objects::clone_object_with_budget(object, budget, doc.ctx().limits(), 0)?;
            lease.absorb(&mut payload)?;
            changes.push((*id, StoreEntry::Upsert(object)));
        }
        Ok((changes, lease))
    }
}

pub(super) struct AppendProof {
    previous: Vec<SignatureReport>,
    signature: ObjectId,
    field: ObjectId,
    field_name: String,
    _lease: MemoryLease,
}

impl AppendProof {
    pub(super) fn validate_output(
        &self,
        source: &Document,
        bytes: &[u8],
        completed: bool,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        let original = source
            .signature_source()?
            .ok_or_else(|| invalid("approval source missing"))?;
        if !bytes.starts_with(original) {
            return Err(malformed("approval lost original source prefix"));
        }
        // The immutable verification copy remains charged until its parser drops.
        let (copy, _copy_lease) =
            BudgetedBytes::copy_from_slice(budget, bytes)?.into_reserved_parts();
        let check = Document::from_bytes_with_options(copy, source.ctx().options())?;
        let reports = check.verify_signatures_with_budget(budget)?;
        if reports.len() != self.previous.len() + 1 {
            return Err(malformed("approval signature count changed"));
        }
        for (before, after) in self.previous.iter().zip(&reports) {
            if after.verification != SignatureVerification::Valid
                || before.object_id != after.object_id
                || before.byte_range != after.byte_range
                || before.signed_revision_bytes != after.signed_revision_bytes
                || before.signed_revision != after.signed_revision
                || before.signed_revision_xref != after.signed_revision_xref
                || before.field != after.field
                || after.source_object_state != SignatureObjectState::Unchanged
            {
                return Err(malformed(
                    "approval altered an existing signature or its historical binding",
                ));
            }
        }
        let last = reports.last().unwrap();
        if last.object_id != self.signature
            || !last.covers_current_document
            || last
                .field
                .as_ref()
                .is_none_or(|field| field.object_id != self.field || field.name != self.field_name)
            || last.source_object_state != SignatureObjectState::Unchanged
            || last.verification
                != if completed {
                    SignatureVerification::Valid
                } else {
                    SignatureVerification::Malformed
                }
        {
            return Err(malformed(
                "new approval coverage/field/CMS verification failed",
            ));
        }
        Ok(())
    }
}

// Reject every historical declaration as well as the current one. Removing a
// Perms/Lock/Reference dictionary in a later revision cannot unlock this API.
fn constraints(
    object: &PdfObject,
    limits: Limits,
    depth: usize,
    work: &mut usize,
) -> PdfResult<()> {
    if depth > limits.max_depth || *work >= limits.max_objects {
        return Err(PdfError::ResourceLimit(
            "approval constraint inspection".into(),
        ));
    }
    *work += 1;
    if let Some(dict) = object
        .as_dict()
        .or_else(|| object.as_stream().map(|s| s.dict()))
    {
        if [
            "Perms",
            "Reference",
            "DocMDP",
            "FieldMDP",
            "Lock",
            "SV",
            "AA",
            "XFA",
            "Linearized",
        ]
        .iter()
        .any(|key| dict.contains_key(key))
        {
            return Err(invalid(
                "approval constraints/actions/linearization require a separately supported policy",
            ));
        }
        for (_, child) in dict {
            constraints(child, limits, depth + 1, work)?;
        }
    } else if let Some(array) = object.as_array() {
        for child in array {
            constraints(child, limits, depth + 1, work)?;
        }
    }
    Ok(())
}

pub(super) fn history_constraints(doc: &Document, budget: &OperationBudget) -> PdfResult<()> {
    let source = doc
        .signature_source()?
        .ok_or_else(|| invalid("immutable approval source required"))?;
    let mut work = 0;
    for end in history::revision_ends(source, doc.ctx().options())? {
        let view = PdfDocument::from_reader_with_options(
            Cursor::new(&source[..end]),
            doc.ctx().options(),
        )?;
        for info in view.ctx().source_xref().objects().values() {
            if matches!(info.state(), ObjectState::Free) {
                continue;
            }
            let (object, _lease) = view
                .ctx()
                .resolve_source_owned_with_budget(info.object_id(), budget)?;
            constraints(&object, doc.ctx().limits(), 0, &mut work)?;
        }
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "PDF revision/signature invariants stay explicit; retain existing call contracts"
)]
fn form_reference_ownership(
    object: &PdfObject,
    owner: ObjectId,
    root: ObjectId,
    form: ObjectId,
    allowed: bool,
    depth: usize,
    limits: Limits,
    work: &mut usize,
) -> PdfResult<()> {
    if depth > limits.max_depth || *work >= limits.max_objects {
        return Err(PdfError::ResourceLimit(
            "approval ownership inspection".into(),
        ));
    }
    *work += 1;
    if object.as_object_id() == Some(form) && !allowed {
        return Err(invalid("AcroForm has a shared or unknown reference owner"));
    }
    if let Some(dict) = object
        .as_dict()
        .or_else(|| object.as_stream().map(|s| s.dict()))
    {
        for (key, value) in dict {
            form_reference_ownership(
                value,
                owner,
                root,
                form,
                owner == root && depth == 0 && key == "AcroForm",
                depth + 1,
                limits,
                work,
            )?;
        }
    } else if let Some(array) = object.as_array() {
        for value in array {
            form_reference_ownership(value, owner, root, form, false, depth + 1, limits, work)?;
        }
    }
    Ok(())
}

fn widget_ownership(
    doc: &Document,
    fields: &[ObjectId],
    budget: &OperationBudget,
) -> PdfResult<()> {
    // Reuse the existing whole page-tree validator. Its legacy builders are
    // covered by the explicit preparation envelope, not an RSS guarantee.
    let pages = PageTree::entries(doc.ctx())?;
    let mut owners = vec![false; fields.len()];
    let mut work = 0usize;
    for page in pages {
        let Some(values) = page.page_dict.get("Annots") else {
            continue;
        };
        let (values, _lease) = resolve_array(doc, values, budget)?;
        work = work
            .checked_add(values.len())
            .filter(|n| *n <= doc.ctx().limits().max_objects)
            .ok_or_else(|| PdfError::ResourceLimit("approval page annotation visits".into()))?;
        for value in &values {
            let (dict, _dict_lease) = resolve_dict(doc, value, budget)?;
            if dict
                .get("Subtype")
                .and_then(PdfObject::as_name)
                .is_none_or(|n| n.name() != "Widget")
            {
                continue;
            }
            let id = value
                .as_object_id()
                .ok_or_else(|| invalid("approval indirect Widget required"))?;
            let index = fields
                .iter()
                .position(|field| *field == id)
                .ok_or_else(|| invalid("Widget not in supported flat Fields"))?;
            if std::mem::replace(&mut owners[index], true) {
                return Err(malformed("Widget has multiple page owners"));
            }
            if dict
                .get("P")
                .is_some_and(|p| p.as_object_id() != Some(page.page_id))
            {
                return Err(malformed("Widget P does not match its page"));
            }
            let (rect, _rect_lease) = resolve_array(
                doc,
                dict.get("Rect")
                    .ok_or_else(|| malformed("Widget Rect missing"))?,
                budget,
            )?;
            if rect.len() != 4 {
                return Err(malformed("Widget Rect must have four numbers"));
            }
            let mut r = [0.; 4];
            for (i, value) in rect.into_iter().enumerate() {
                r[i] = value
                    .as_number()
                    .ok_or_else(|| invalid("direct Widget rectangle numbers required"))?
                    .value();
            }
            if !r.iter().all(|v| v.is_finite())
                || r[2] < r[0]
                || r[3] < r[1]
                || (dict
                    .get("FT")
                    .and_then(PdfObject::as_name)
                    .is_none_or(|n| n.name() != "Sig")
                    && (r[2] == r[0] || r[3] == r[1]))
            {
                return Err(malformed("invalid Widget rectangle"));
            }
        }
    }
    if owners.iter().any(|found| !found) {
        return Err(malformed("field Widget absent from page Annots"));
    }
    Ok(())
}

impl Document {
    /// Prepare one additional invisible approval on a strictly verified,
    /// unencrypted PDF 2.0 source with flat fields and no modification locks.
    /// This does not authorize ordinary editing of signed input.
    pub fn prepare_additional_signature(
        &self,
        options: SignatureOptions,
    ) -> PdfResult<PreparedSignature> {
        self.prepare_additional_signature_with_budget(
            options,
            &self.ctx().limits().operation_budget(),
        )
    }

    pub fn prepare_additional_signature_with_budget(
        &self,
        options: SignatureOptions,
        budget: &OperationBudget,
    ) -> PdfResult<PreparedSignature> {
        let source = self
            .signature_source()?
            .ok_or_else(|| invalid("immutable signed source required"))?;
        let limits = self.ctx().limits();
        if self.security_report().is_some() || self.ctx().source_xref().has_hybrid_references() {
            return Err(invalid("encrypted/hybrid approval input unsupported"));
        }
        if self.effective_version_with_budget(budget)?
            < crate::document::save_options::PdfVersion::V2_0
        {
            return Err(invalid("additional approval currently requires PDF 2.0"));
        }
        if options.field_name.is_empty()
            || options.field_name.len() > 1024
            || options.field_name.contains('.')
            || options.field_name.chars().any(char::is_control)
            || !(1024..=1024 * 1024).contains(&options.reserved_cms_bytes)
            || options
                .reserved_cms_bytes
                .checked_mul(2)
                .is_none_or(|n| n > limits.max_token_bytes)
        {
            return Err(invalid(
                "additional signature options outside supported bounds",
            ));
        }
        let _construction = budget.reserve(
            source
                .len()
                .checked_mul(8)
                .and_then(|n| n.checked_add(options.reserved_cms_bytes * 8 + 1024 * 1024))
                .ok_or_else(|| PdfError::ResourceLimit("approval construction envelope".into()))?,
        )?;
        let proof_lease = budget.reserve(16 * 8192)?;
        let (pending, _pending_lease) = self.ctx().save_changes_with_budget(budget)?;
        if !pending.into_entries().is_empty() {
            return Err(invalid("approval requires no pending changes"));
        }
        let previous = self.verify_signatures_with_budget(budget)?;
        if previous.is_empty()
            || previous.len() >= 16
            || previous.iter().any(|r| {
                r.verification != SignatureVerification::Valid
                    || r.source_object_state != SignatureObjectState::Unchanged
                    || r.field.is_none()
            })
            || !previous.last().unwrap().covers_current_document
        {
            return Err(invalid(
                "approval requires valid bound signatures and a fully signed current source",
            ));
        }
        history_constraints(self, budget)?;
        let root_id = self
            .ctx()
            .trailer()
            .get("Root")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| invalid("approval Root reference"))?;
        let (catalog, _catalog_lease) = resolve_dict(self, &reference(root_id), budget)?;
        let form_id = catalog
            .get("AcroForm")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| invalid("indirect AcroForm required"))?;
        let (mut form, _form_lease) = resolve_dict(self, &reference(form_id), budget)?;
        if form
            .get("SigFlags")
            .is_some_and(|value| value.as_u32().is_none_or(|flags| flags & !3 != 0))
        {
            return Err(invalid("unsupported AcroForm SigFlags"));
        }
        if form
            .get("NeedAppearances")
            .is_some_and(|value| value.as_bool().is_none_or(|v| v.value()))
        {
            return Err(invalid("approval NeedAppearances unsupported"));
        }
        let (mut fields, _fields_lease) = resolve_array(
            self,
            form.get("Fields")
                .ok_or_else(|| invalid("approval Fields missing"))?,
            budget,
        )?;
        if fields.len() > 256 {
            return Err(invalid("approval at most 256 flat fields"));
        }
        let mut signed_fields = 0;
        let mut field_ids = Vec::new();
        for value in &fields {
            let id = value
                .as_object_id()
                .ok_or_else(|| invalid("approval indirect field required"))?;
            if id == form_id || id == root_id {
                return Err(malformed("approval field/owner roles alias"));
            }
            field_ids.push(id);
            let (field, _field_lease) = resolve_dict(self, value, budget)?;
            let ft = field
                .get("FT")
                .and_then(PdfObject::as_name)
                .map(|n| n.name());
            if ["Kids", "Parent"].iter().any(|key| field.contains_key(key))
                || !matches!(ft, Some("Sig" | "Tx" | "Btn"))
                || field
                    .get("Subtype")
                    .and_then(PdfObject::as_name)
                    .is_none_or(|n| n.name() != "Widget")
            {
                return Err(invalid(
                    "approval supports flat text/checkbox/signed Widget fields only",
                ));
            }
            if ft == Some("Btn")
                && field
                    .get("Ff")
                    .map_or(0, |v| v.as_u32().unwrap_or(u32::MAX))
                    & ((1 << 15) | (1 << 16))
                    != 0
            {
                return Err(invalid("approval radio/pushbutton fields unsupported"));
            }
            let title = field
                .get("T")
                .and_then(PdfObject::as_string)
                .ok_or_else(|| invalid("approval field name"))?;
            if title.bytes().len() > 2050 {
                return Err(invalid("approval field name limit"));
            }
            let _text = budget.reserve(title.bytes().len() * 4)?;
            let title = title.text_content_with_limits(&limits)?;
            if title.len() > 1024 || title.contains('.') || title == options.field_name {
                return Err(invalid("approval nested/duplicate field name"));
            }
            if ft == Some("Sig") {
                signed_fields += 1;
                let signature = field
                    .get("V")
                    .and_then(PdfObject::as_object_id)
                    .ok_or_else(|| {
                        invalid("empty signature field requires a later field-target API")
                    })?;
                if !previous.iter().any(|r| {
                    r.object_id == signature && r.field.as_ref().is_some_and(|f| f.object_id == id)
                }) {
                    return Err(invalid("approval signature field/history mismatch"));
                }
            }
        }
        if signed_fields != previous.len() {
            return Err(invalid("approval orphan or shared signature fields"));
        }
        widget_ownership(self, &field_ids, budget)?;
        let mut reference_work = 0;
        for info in self.ctx().source_xref().objects().values() {
            if matches!(info.state(), ObjectState::Free) {
                continue;
            }
            let (object, _lease) = self
                .ctx()
                .resolve_source_owned_with_budget(info.object_id(), budget)?;
            form_reference_ownership(
                &object,
                info.object_id(),
                root_id,
                form_id,
                false,
                0,
                limits,
                &mut reference_work,
            )?;
            let known = info.object_id() == form_id
                || field_ids.contains(&info.object_id())
                || previous.iter().any(|r| r.object_id == info.object_id());
            if super::has_signature(&object, limits, 0)? && !known {
                return Err(invalid("unknown or direct signature-bearing owner"));
            }
            if known && let Some(dict) = object.as_dict() {
                for (_, child) in dict {
                    if super::has_signature(child, limits, 0)? {
                        return Err(invalid("nested signature-bearing containers unsupported"));
                    }
                }
            }
        }
        let first = PageTree::page_at_with_budget(self.ctx(), 0, budget)?;
        let (first, _page_location_lease) = first.into_parts();
        if [root_id, form_id].contains(&first.page_id)
            || root_id == form_id
            || field_ids.contains(&first.page_id)
            || previous.iter().any(|r| {
                [root_id, form_id, first.page_id].contains(&r.object_id)
                    || field_ids.contains(&r.object_id)
            })
        {
            return Err(malformed("approval ownership roles alias"));
        }
        // Only the AcroForm Fields/SigFlags and one page Annots may change.
        // Fresh objects are monotonically allocated; no source object is reused.
        let next = self
            .ctx()
            .source_xref()
            .objects()
            .keys()
            .copied()
            .max()
            .unwrap_or(0)
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("approval object number overflow".into()))?
            .max(
                self.ctx()
                    .trailer()
                    .get("Size")
                    .and_then(PdfObject::as_u32)
                    .ok_or_else(|| invalid("approval Size"))?,
            );
        let signature_id = ObjectId::new(next, 0);
        let appearance_id = ObjectId::new(
            next.checked_add(1)
                .ok_or_else(|| PdfError::ResourceLimit("approval ID overflow".into()))?,
            0,
        );
        let field_id = ObjectId::new(
            next.checked_add(2)
                .ok_or_else(|| PdfError::ResourceLimit("approval ID overflow".into()))?,
            0,
        );
        next.checked_add(3)
            .ok_or_else(|| PdfError::ResourceLimit("approval Size overflow".into()))?;
        if self.ctx().source_xref().objects().len().saturating_add(3) > limits.max_objects {
            return Err(PdfError::ResourceLimit("approval object count".into()));
        }
        let mut signature = PdfDict::default();
        for (key, value) in [
            ("Type", name("Sig")),
            ("Filter", name("Adobe.PPKLite")),
            ("SubFilter", name("adbe.pkcs7.detached")),
        ] {
            signature.try_insert(key.into(), value)?;
        }
        let mut range = PdfArray::default();
        for _ in 0..4 {
            range.try_add_obj(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                b"00000000000000000000",
            )?))?;
        }
        signature.try_insert("ByteRange".into(), PdfObject::PdfArray(range))?;
        signature.try_insert(
            "Contents".into(),
            PdfObject::PdfString(PdfString::new(
                vec![b'0'; options.reserved_cms_bytes * 2],
                false,
            )),
        )?;
        let mut appearance = PdfDict::default();
        appearance.try_insert("Type".into(), name("XObject"))?;
        appearance.try_insert("Subtype".into(), name("Form"))?;
        appearance.try_insert("Resources".into(), PdfObject::PdfDict(PdfDict::default()))?;
        let mut bbox = PdfArray::default();
        for value in [0, 0, 1, 1] {
            bbox.try_add_obj(number(value)?)?;
        }
        appearance.try_insert("BBox".into(), PdfObject::PdfArray(bbox))?;
        let mut field = PdfDict::default();
        for (key, value) in [
            ("Type", name("Annot")),
            ("Subtype", name("Widget")),
            ("FT", name("Sig")),
            (
                "T",
                PdfObject::PdfString(PdfString::from_text_with_limits(
                    &options.field_name,
                    &limits,
                )?),
            ),
            ("V", reference(signature_id)),
            ("P", reference(first.page_id)),
            ("F", number(132)?),
        ] {
            field.try_insert(key.into(), value)?;
        }
        let mut rect = PdfArray::default();
        for _ in 0..4 {
            rect.try_add_obj(number(0)?)?;
        }
        field.try_insert("Rect".into(), PdfObject::PdfArray(rect))?;
        let mut ap = PdfDict::default();
        ap.try_insert("N".into(), reference(appearance_id))?;
        field.try_insert("AP".into(), PdfObject::PdfDict(ap))?;
        fields.try_add_obj(reference(field_id))?;
        form.try_insert("Fields".into(), PdfObject::PdfArray(fields))?;
        form.try_insert("SigFlags".into(), number(3)?)?;
        let (mut page, _page_lease) =
            crate::objects::clone_dict_with_budget(&first.page_dict, budget, limits, 0)?;
        let (mut annots, _annots_lease) = match page.get("Annots") {
            Some(value) => resolve_array(self, value, budget)?,
            None => (PdfArray::default(), budget.reserve(0)?),
        };
        annots.try_add_obj(reference(field_id))?;
        page.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
        let plan = SignedRevisionPlan {
            stamp: self.stamp(),
            source_hash: Sha256::digest(source).into(),
            entries: vec![
                (signature_id, PdfObject::PdfDict(signature)),
                (
                    appearance_id,
                    PdfObject::PdfStream(crate::objects::pdf_stream::PdfStream::new(
                        appearance,
                        Vec::new(),
                    )),
                ),
                (field_id, PdfObject::PdfDict(field)),
                (form_id, PdfObject::PdfDict(form)),
                (first.page_id, PdfObject::PdfDict(page)),
            ],
        };
        let output = self.append_validated_revision_with_budget(&plan, budget)?;
        let (mut bytes, lease) = output.into_reserved_parts();
        let locations = signature_locations_with_budget(&bytes, signature_id, limits, budget)?;
        if bytes[locations.contents.start] != b'(' || bytes[locations.contents.end - 1] != b')' {
            return Err(malformed("approval placeholder token"));
        }
        bytes[locations.contents.start] = b'<';
        bytes[locations.contents.end - 1] = b'>';
        let range = [
            0,
            locations.contents.start as u64,
            locations.contents.end as u64,
            (bytes.len() - locations.contents.end) as u64,
        ];
        let replacement = format!(
            "[{:020} {:020} {:020} {:020}]",
            range[0], range[1], range[2], range[3]
        );
        if replacement.len() != locations.byte_range.len() {
            return Err(malformed("approval placeholder width"));
        }
        bytes[locations.byte_range].copy_from_slice(replacement.as_bytes());
        let mut digest = Sha256::new();
        digest.update(&bytes[..locations.contents.start]);
        digest.update(&bytes[locations.contents.end..]);
        let bytes = BudgetedBytes::from_reserved_vec(bytes, lease)?;
        let proof = AppendProof {
            previous,
            signature: signature_id,
            field: field_id,
            field_name: options.field_name,
            _lease: proof_lease,
        };
        proof.validate_output(self, bytes.as_slice(), false, budget)?;
        Ok(PreparedSignature {
            bytes,
            contents: locations.contents,
            byte_range: range,
            digest: digest.finalize().into(),
            stamp: self.stamp(),
            completed: Cell::new(false),
            append_proof: Some(proof),
            field_proof: None,
        })
    }
}
