//! Existing signature fields: immutable V/AP plans, never a mutable guard scope.
use super::*;
use crate::budget::MemoryLease;
use crate::document::{
    acroform::{FormTextStyle, FormWidgetSpec},
    identity::ObjectHandle,
};

#[derive(Debug, Clone)]
pub struct SignatureAppearance {
    /// Explicit lines, no automatic layout, identity BBox/Matrix. Not an identity assertion.
    pub text: String,
    pub style: FormTextStyle,
}

#[derive(Debug, Clone)]
pub struct SignatureFieldOptions {
    pub field_name: String,
    pub reserved_cms_bytes: usize,
    /// None preserves each Widget's original AP, including its Matrix/resources.
    pub appearance: Option<SignatureAppearance>,
}

pub(super) struct FieldProof {
    previous: Vec<SignatureReport>,
    field: ObjectId,
    name: String,
    signature: ObjectId,
    _lease: MemoryLease,
}

pub(super) fn preserve_reports(
    before: &[SignatureReport],
    after: &[SignatureReport],
) -> PdfResult<()> {
    if after.len() < before.len() {
        return Err(malformed("lost historical signature"));
    }
    for (old, new) in before.iter().zip(after) {
        if new.verification != SignatureVerification::Valid
            || old.object_id != new.object_id
            || old.byte_range != new.byte_range
            || old.signed_revision != new.signed_revision
            || old.signed_revision_xref != new.signed_revision_xref
            || old.field != new.field
            || old.signed_revision_bytes != new.signed_revision_bytes
            || new.source_object_state != SignatureObjectState::Unchanged
        {
            return Err(malformed("historical signature changed"));
        }
    }
    Ok(())
}

impl FieldProof {
    pub(super) fn validate_output(
        &self,
        source: &Document,
        bytes: &[u8],
        completed: bool,
        budget: &OperationBudget,
    ) -> PdfResult<()> {
        if !bytes.starts_with(
            source
                .signature_source()?
                .ok_or_else(|| invalid("immutable source required"))?,
        ) {
            return Err(malformed("lost signed source prefix"));
        }
        let (copy, _copy) = BudgetedBytes::copy_from_slice(budget, bytes)?.into_reserved_parts();
        let check = Document::from_bytes_with_options(copy, source.ctx().options())?;
        let reports = check.verify_signatures_with_budget(budget)?;
        preserve_reports(&self.previous, &reports)?;
        if reports.len() != self.previous.len() + 1 {
            return Err(malformed("signature count"));
        }
        let last = reports.last().unwrap();
        if last.object_id != self.signature
            || !last.covers_current_document
            || last
                .field
                .as_ref()
                .is_none_or(|f| f.object_id != self.field || f.name != self.name)
            || last.verification
                != if completed {
                    SignatureVerification::Valid
                } else {
                    SignatureVerification::Malformed
                }
        {
            return Err(malformed("new signature field/coverage/CMS"));
        }
        changes::candidate(source, &check, budget)?;
        Ok(())
    }
}

pub(super) fn ready_source(
    doc: &Document,
    budget: &OperationBudget,
) -> PdfResult<Vec<SignatureReport>> {
    doc.signature_source()?
        .ok_or_else(|| invalid("strict immutable owned bytes required"))?;
    if doc.security_report().is_some()
        || doc.ctx().source_xref().has_hybrid_references()
        || doc.effective_version_with_budget(budget)?
            < crate::document::save_options::PdfVersion::V2_0
    {
        return Err(invalid(
            "field signing requires unencrypted nonhybrid PDF 2.0",
        ));
    }
    let (pending, _lease) = doc.ctx().save_changes_with_budget(budget)?;
    if !pending.into_entries().is_empty() {
        return Err(invalid("pending source edits"));
    }
    let reports = doc.verify_signatures_with_budget(budget)?;
    if reports.iter().any(|r| {
        r.verification != SignatureVerification::Valid
            || r.source_object_state != SignatureObjectState::Unchanged
            || r.field.is_none()
    }) {
        return Err(invalid("valid unchanged signatures required"));
    }
    if doc
        .signature_modifications_with_budget(budget)?
        .iter()
        .any(|r| r.authorization != changes::ModificationAuthorization::Allowed)
    {
        return Err(invalid("prior modifications are forbidden or unclassified"));
    }
    Ok(reports)
}

pub(super) fn fresh_id(doc: &Document) -> PdfResult<u32> {
    doc.ctx()
        .source_xref()
        .objects()
        .keys()
        .copied()
        .max()
        .unwrap_or(0)
        .checked_add(1)
        .map(|n| {
            n.max(
                doc.ctx()
                    .trailer()
                    .get("Size")
                    .and_then(PdfObject::as_u32)
                    .unwrap_or(0),
            )
        })
        .ok_or_else(|| PdfError::ResourceLimit("signature object number".into()))
}

impl Document {
    /// Create an empty signature field before signing. Reuses AcroForm paths,
    /// ownership, font encoding and AP assembly. General signed editing stays denied.
    pub fn add_signature_field(
        &self,
        full_name: &str,
        widgets: &[FormWidgetSpec],
    ) -> PdfResult<ObjectHandle> {
        self.ctx().require_unsigned_source()?;
        self.create_signature_field(full_name, widgets)
    }

    pub fn prepare_field_signature(
        &self,
        options: SignatureFieldOptions,
    ) -> PdfResult<PreparedSignature> {
        self.prepare_field_signature_with_budget(options, &self.ctx().limits().operation_budget())
    }

    pub fn prepare_field_signature_with_budget(
        &self,
        options: SignatureFieldOptions,
        budget: &OperationBudget,
    ) -> PdfResult<PreparedSignature> {
        let source = self
            .signature_source()?
            .ok_or_else(|| invalid("immutable source required"))?;
        let limits = self.ctx().limits();
        if options.field_name.is_empty()
            || options.field_name.len() > 4096
            || !(1024..=1024 * 1024).contains(&options.reserved_cms_bytes)
            || options.reserved_cms_bytes * 2 > limits.max_token_bytes
        {
            return Err(PdfError::ResourceLimit("signature field options".into()));
        }
        let _construction = budget.reserve(
            source
                .len()
                .checked_mul(8)
                .and_then(|n| n.checked_add(options.reserved_cms_bytes * 8 + 1024 * 1024))
                .ok_or_else(|| PdfError::ResourceLimit("field signature construction".into()))?,
        )?;
        let previous = ready_source(self, budget)?;
        if previous.len() >= 16 {
            return Err(PdfError::ResourceLimit("signature count".into()));
        }
        let tree = self.form_field_tree()?;
        let targets: Vec<_> = tree
            .iter()
            .filter(|n| n.name == options.field_name)
            .collect();
        let node = match targets.as_slice() {
            [node] => *node,
            _ => return Err(invalid("unique existing signature field required")),
        };
        if node.field_type.as_deref() != Some("Sig")
            || !node.children.is_empty()
            || node.widgets.is_empty()
            || node.flags & 1 != 0
        {
            return Err(invalid("empty writable terminal signature field required"));
        }
        let field_id = self.validate_handle(&node.handle)?;
        if !policy::permits(
            &self.signature_constraints_with_budget(budget)?,
            changes::ModificationKind::Signature,
            Some(field_id),
        ) {
            return Err(PdfError::PermissionDenied(
                "signature field is locked by a signed constraint".into(),
            ));
        }
        let mut ancestor = node.parent.as_ref();
        while let Some(parent) = ancestor {
            let parent_node = tree
                .iter()
                .find(|n| n.handle.object_id() == parent.object_id())
                .ok_or_else(|| malformed("field ancestor"))?;
            let (dict, _lease) = resolve_dict(self, &reference(parent.object_id()), budget)?;
            if dict
                .get("V")
                .is_some_and(|v| !matches!(v, PdfObject::PdfNull))
            {
                return Err(invalid("inherited filled signature value"));
            }
            ancestor = parent_node.parent.as_ref();
        }
        let (mut field, _field_lease) = resolve_dict(self, &reference(field_id), budget)?;
        if field
            .get("V")
            .is_some_and(|v| !matches!(v, PdfObject::PdfNull))
            || ["Lock", "SV", "AA", "A"]
                .iter()
                .any(|k| field.contains_key(k))
        {
            return Err(invalid("filled/locked/seeded signature field"));
        }
        let signature_id = ObjectId::new(fresh_id(self)?, 0);
        let mut next = signature_id
            .obj_num()
            .checked_add(1)
            .ok_or_else(|| PdfError::ResourceLimit("signature ID".into()))?;
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
        field.try_insert("V".into(), reference(signature_id))?;
        let mut appearance_leases = Vec::new();
        let mut appearance_work = crate::document::acroform::AppearanceWork::default();
        let mut entries = vec![(signature_id, PdfObject::PdfDict(signature))];
        // Annotation Locked protects geometry, not field values. ReadOnly is
        // ignored for Widgets; effective Ff already enforces field read-only.
        // LockedContents and unknown flag bits remain unsupported.
        for widget in &node.widgets {
            let id = self.validate_handle(&widget.handle)?;
            let (mut dict, _lease) = if id == field_id {
                crate::objects::clone_dict_with_budget(&field, budget, limits, 0)?
            } else {
                resolve_dict(self, &reference(id), budget)?
            };
            if ["AA", "A", "OC", "StructParent", "Popup", "IRT"]
                .iter()
                .any(|key| dict.contains_key(key))
                || dict.get("F").is_some_and(|v| {
                    v.as_u32()
                        .is_none_or(|flags| flags & 512 != 0 || flags & !1023 != 0)
                })
            {
                return Err(invalid("unsupported signature Widget relations/flags"));
            }
            if let Some(appearance) = &options.appearance {
                let r = widget.rect;
                let (stream, payload_lease) = self.signature_text_appearance(
                    &appearance.text,
                    &appearance.style,
                    r[2] - r[0],
                    r[3] - r[1],
                    budget,
                    &mut appearance_work,
                )?;
                appearance_leases.push(payload_lease);
                let ap_id = ObjectId::new(next, 0);
                next = next
                    .checked_add(1)
                    .ok_or_else(|| PdfError::ResourceLimit("appearance ID".into()))?;
                entries.push((ap_id, PdfObject::PdfStream(stream)));
                let mut ap = PdfDict::default();
                ap.try_insert("N".into(), reference(ap_id))?;
                dict.try_insert("AP".into(), PdfObject::PdfDict(ap))?;
            }
            if id == field_id {
                field = dict;
            } else {
                entries.push((id, PdfObject::PdfDict(dict)));
            }
        }
        entries.push((field_id, PdfObject::PdfDict(field)));
        let root = self.ctx().get_root_owned()?;
        let form_id = root
            .get("AcroForm")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| invalid("indirect AcroForm required"))?;
        let (mut form, _form_lease) = resolve_dict(self, &reference(form_id), budget)?;
        if form
            .get("NeedAppearances")
            .is_some_and(|v| v.as_bool().is_none_or(|v| v.value()))
        {
            return Err(invalid("NeedAppearances"));
        }
        form.try_insert("SigFlags".into(), number(3)?)?;
        entries.push((form_id, PdfObject::PdfDict(form)));
        let plan = append::SignedRevisionPlan::for_validated_entries(self, entries)?;
        let (mut bytes, lease) = self
            .append_validated_revision_with_budget(&plan, budget)?
            .into_reserved_parts();
        let locations = signature_locations_with_budget(&bytes, signature_id, limits, budget)?;
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
            return Err(malformed("placeholder width"));
        }
        bytes[locations.byte_range].copy_from_slice(replacement.as_bytes());
        let mut digest = Sha256::new();
        digest.update(&bytes[..locations.contents.start]);
        digest.update(&bytes[locations.contents.end..]);
        let proof = FieldProof {
            previous,
            field: field_id,
            name: options.field_name,
            signature: signature_id,
            _lease: budget.reserve(16 * 8192)?,
        };
        proof.validate_output(self, &bytes, false, budget)?;
        Ok(PreparedSignature {
            bytes: BudgetedBytes::from_reserved_vec(bytes, lease)?,
            contents: locations.contents,
            byte_range: range,
            digest: digest.finalize().into(),
            stamp: self.stamp(),
            completed: Cell::new(false),
            append_proof: None,
            field_proof: Some(proof),
        })
    }
}
