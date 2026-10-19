//! Historical declarations, separate from certificate trust and change approval.
use super::*;
use crate::document::{PdfDocument, acroform::FormFieldNode};
use std::collections::HashSet;
use std::io::{Read, Seek};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DocMdpPermission {
    NoChanges,
    FillFormsAndSign,
    Annotate,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FieldLockAction {
    All,
    Include,
    Exclude,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FieldLockRule {
    pub action: FieldLockAction,
    pub fields: Vec<String>,
    /// Signed-revision identities, including the descendants of named groups.
    pub locked_fields: Vec<ObjectId>,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SignatureConstraints {
    pub signature: ObjectId,
    pub signed_revision: usize,
    pub doc_mdp: Option<DocMdpPermission>,
    pub field_locks: Vec<FieldLockRule>,
}

fn dict<R: Read + Seek>(
    doc: &PdfDocument<R>,
    value: &PdfObject,
    budget: &OperationBudget,
) -> PdfResult<(PdfDict, crate::budget::MemoryLease)> {
    let (object, lease) = doc.ctx().resolve_owned_with_budget(value, budget)?;
    Ok((
        object
            .into_dict()
            .ok_or_else(|| malformed("MDP dictionary"))?,
        lease,
    ))
}

pub(super) fn lock_rule<R: Read + Seek>(
    doc: &PdfDocument<R>,
    params: &PdfDict,
    tree: &[FormFieldNode],
    budget: &OperationBudget,
) -> PdfResult<FieldLockRule> {
    let action = match params
        .get("Action")
        .and_then(PdfObject::as_name)
        .map(|n| n.name())
    {
        Some("All") => FieldLockAction::All,
        Some("Include") => FieldLockAction::Include,
        Some("Exclude") => FieldLockAction::Exclude,
        _ => return Err(malformed("FieldMDP Action")),
    };
    let mut fields = Vec::new();
    if let Some(value) = params.get("Fields") {
        if action == FieldLockAction::All {
            return Err(malformed("All FieldMDP must omit Fields"));
        }
        let (values, _lease) = doc.ctx().resolve_owned_with_budget(value, budget)?;
        let values = values
            .as_array()
            .ok_or_else(|| malformed("FieldMDP Fields array"))?;
        if values.len() > 256 {
            return Err(PdfError::ResourceLimit("FieldMDP field count".into()));
        }
        for value in values {
            let text = value
                .as_string()
                .ok_or_else(|| malformed("FieldMDP field text string"))?;
            if text.bytes().len() > 8194 {
                return Err(PdfError::ResourceLimit("FieldMDP name".into()));
            }
            let text = text.text_content_with_limits(&doc.ctx().limits())?;
            if text.is_empty()
                || text.len() > 4096
                || fields.contains(&text)
                || !tree.iter().any(|n| n.name == text)
            {
                return Err(malformed("FieldMDP duplicate/unknown/excessive field name"));
            }
            fields.push(text);
        }
    } else if action != FieldLockAction::All {
        return Err(malformed("FieldMDP Fields required"));
    }
    fields.sort();
    let mut locked_fields = Vec::new();
    for node in tree {
        let named = fields.iter().any(|name| {
            node.name == *name
                || node
                    .name
                    .strip_prefix(name)
                    .is_some_and(|rest| rest.starts_with('.'))
        });
        if match action {
            FieldLockAction::All => true,
            FieldLockAction::Include => named,
            FieldLockAction::Exclude => !named,
        } {
            locked_fields.push(node.handle.object_id());
        }
    }
    locked_fields.sort_unstable_by_key(|id| (id.obj_num(), id.gen_num()));
    Ok(FieldLockRule {
        action,
        fields,
        locked_fields,
    })
}

pub(super) fn read<R: Read + Seek>(
    doc: &PdfDocument<R>,
    report: &SignatureReport,
    first: ObjectId,
    budget: &OperationBudget,
) -> PdfResult<SignatureConstraints> {
    let tree = doc.form_field_tree()?;
    if tree.len() > 256 {
        return Err(PdfError::ResourceLimit("MDP field count".into()));
    }
    let root_id = doc
        .ctx()
        .trailer()
        .get("Root")
        .and_then(PdfObject::as_object_id)
        .ok_or_else(|| malformed("MDP Root"))?;
    let root = doc.ctx().get_root_owned()?;
    let certified = if let Some(value) = root.get("Perms") {
        let (perms, _lease) = dict(doc, value, budget)?;
        if perms.into_iter().any(|(key, _)| key != "DocMDP") {
            return Err(invalid("unknown permissions transform"));
        }
        let (perms, _lease) = dict(doc, value, budget)?;
        Some(
            perms
                .get("DocMDP")
                .and_then(PdfObject::as_object_id)
                .ok_or_else(|| malformed("Perms DocMDP reference"))?,
        )
    } else {
        None
    };
    let (signature, _lease) = dict(doc, &reference(report.object_id), budget)?;
    let mut result = SignatureConstraints {
        signature: report.object_id,
        signed_revision: report.signed_revision,
        doc_mdp: None,
        field_locks: Vec::new(),
    };
    if let Some(value) = signature.get("Reference") {
        if report.field.is_none() {
            return Err(malformed(
                "signature constraints require a bound signature field",
            ));
        }
        let (refs, _lease) = doc.ctx().resolve_owned_with_budget(value, budget)?;
        let refs = refs
            .as_array()
            .ok_or_else(|| malformed("signature Reference array"))?;
        if refs.len() > 16 {
            return Err(PdfError::ResourceLimit("signature transform count".into()));
        }
        for value in refs {
            let (reference, _lease) = dict(doc, value, budget)?;
            if reference
                .get("Type")
                .is_some_and(|v| v.as_name().is_none_or(|n| n.name() != "SigRef"))
                || reference
                    .get("Data")
                    .is_some_and(|v| v.as_object_id() != Some(root_id))
            {
                return Err(malformed("signature transform type/Data"));
            }
            let (params, _lease) = dict(
                doc,
                reference
                    .get("TransformParams")
                    .ok_or_else(|| malformed("TransformParams missing"))?,
                budget,
            )?;
            if params
                .get("Type")
                .is_some_and(|v| v.as_name().is_none_or(|n| n.name() != "TransformParams"))
                || params
                    .get("V")
                    .is_some_and(|v| v.as_name().is_none_or(|n| n.name() != "1.2"))
            {
                return Err(invalid("unknown transform version/type"));
            }
            match reference
                .get("TransformMethod")
                .and_then(PdfObject::as_name)
                .map(|n| n.name())
            {
                Some("DocMDP") => {
                    if params
                        .into_iter()
                        .any(|(key, _)| !["Type", "V", "P"].contains(&key.as_str()))
                    {
                        return Err(invalid("unknown DocMDP transform parameter"));
                    }
                    if result.doc_mdp.is_some()
                        || certified != Some(report.object_id)
                        || report.object_id != first
                    {
                        return Err(malformed(
                            "DocMDP must be the unique first certification signature",
                        ));
                    }
                    let p = match params.get("P") {
                        Some(p) => p.as_u32().ok_or_else(|| malformed("DocMDP P integer"))?,
                        None => 2,
                    };
                    result.doc_mdp = Some(match p {
                        1 => DocMdpPermission::NoChanges,
                        2 => DocMdpPermission::FillFormsAndSign,
                        3 => DocMdpPermission::Annotate,
                        _ => return Err(malformed("DocMDP P range")),
                    });
                }
                Some("FieldMDP") => {
                    if reference.get("Data").and_then(PdfObject::as_object_id) != Some(root_id)
                        || params.into_iter().any(|(key, _)| {
                            !["Type", "V", "Action", "Fields"].contains(&key.as_str())
                        })
                    {
                        return Err(invalid(
                            "FieldMDP requires catalog Data and known parameters",
                        ));
                    }
                    if !result.field_locks.is_empty() {
                        return Err(malformed("duplicate FieldMDP"));
                    }
                    result
                        .field_locks
                        .push(lock_rule(doc, &params, &tree, budget)?);
                }
                _ => return Err(invalid("unknown signature transform")),
            }
        }
    }
    if certified == Some(report.object_id) && result.doc_mdp.is_none() {
        return Err(malformed("Perms lacks DocMDP transform"));
    }
    if let Some(field) = &report.field {
        let (field, _lease) = dict(doc, &reference(field.object_id), budget)?;
        if let Some(value) = field.get("Lock") {
            let (lock, _lease) = dict(doc, value, budget)?;
            if lock.contains_key("P") {
                return Err(invalid("signature Lock P profile unsupported"));
            }
            let rule = lock_rule(doc, &lock, &tree, budget)?;
            if !result.field_locks.contains(&rule) {
                return Err(malformed("field Lock and signed FieldMDP disagree"));
            }
        }
    }
    Ok(result)
}

pub(super) fn permits(
    policies: &[SignatureConstraints],
    kind: changes::ModificationKind,
    field: Option<ObjectId>,
) -> bool {
    if kind == changes::ModificationKind::Unknown {
        return false;
    }
    policies.iter().all(|policy| {
        let document = match policy.doc_mdp {
            Some(DocMdpPermission::NoChanges) => false,
            Some(DocMdpPermission::FillFormsAndSign) => {
                kind != changes::ModificationKind::Annotation
            }
            _ => true,
        };
        document
            && !field.is_some_and(|id| {
                policy
                    .field_locks
                    .iter()
                    .any(|rule| rule.locked_fields.contains(&id))
            })
    })
}

type Edges = HashSet<(ObjectId, ObjectId, String)>;
fn ownership_slot<R: Read + Seek>(
    doc: &PdfDocument<R>,
    owner: ObjectId,
    key: &str,
    value: &PdfObject,
    protected: &HashSet<ObjectId>,
    edges: &mut Edges,
    budget: &OperationBudget,
) -> PdfResult<()> {
    let (value_owned, _lease) = doc.ctx().resolve_owned_with_budget(value, budget)?;
    if let Some(array) = value_owned.as_array() {
        let (array_owner, path) = if let Some(id) = value.as_object_id() {
            (id, "[]".into())
        } else {
            (owner, format!("{key}/[]"))
        };
        for child in array {
            if let Some(id) = child.as_object_id().filter(|id| protected.contains(id)) {
                edges.insert((array_owner, id, path.clone()));
            }
        }
    } else if let Some(id) = value.as_object_id().filter(|id| protected.contains(id)) {
        edges.insert((owner, id, key.into()));
    }
    Ok(())
}

#[expect(
    clippy::too_many_arguments,
    reason = "PDF revision/signature invariants stay explicit; retain existing call contracts"
)]
fn scan(
    object: &PdfObject,
    owner: ObjectId,
    path: &str,
    root: ObjectId,
    permissions: Option<ObjectId>,
    form: Option<ObjectId>,
    fields: &HashSet<ObjectId>,
    signatures: &HashSet<ObjectId>,
    protected: &HashSet<ObjectId>,
    edges: &Edges,
    depth: usize,
    work: &mut usize,
    limits: Limits,
) -> PdfResult<()> {
    if depth > limits.max_depth || *work >= limits.max_objects {
        return Err(PdfError::ResourceLimit(
            "signed ownership/constraints visits".into(),
        ));
    }
    *work += 1;
    if let Some(id) = object.as_object_id().filter(|id| protected.contains(id))
        && !edges.contains(&(owner, id, path.to_owned()))
    {
        return Err(invalid("shared or unknown field/AcroForm reference owner"));
    }
    if let Some(dict) = object
        .as_dict()
        .or_else(|| object.as_stream().map(|s| s.dict()))
    {
        for (key, value) in dict {
            if key == "SigFlags"
                && value.as_u32() != Some(0)
                && !(form == Some(owner) && depth == 0)
            {
                return Err(invalid("orphan signature flags"));
            }
            if ["AA", "SV", "XFA", "Linearized"].contains(&key.as_str()) {
                return Err(invalid("unsupported signed action/seed/XFA/linearization"));
            }
            if key == "Perms" && owner == root && depth == 0 {
                continue;
            }
            if key == "DocMDP" && permissions == Some(owner) && depth == 0 {
                continue;
            }
            if key == "Reference" && signatures.contains(&owner) && depth == 0 {
                continue;
            }
            if key == "Lock" && fields.contains(&owner) && depth == 0 {
                continue;
            }
            if [
                "Perms",
                "Reference",
                "Lock",
                "DocMDP",
                "FieldMDP",
                "ByteRange",
            ]
            .contains(&key.as_str())
                && !(key == "ByteRange" && signatures.contains(&owner) && depth == 0)
            {
                return Err(invalid("orphan/nested/unknown signature constraint"));
            }
            if ["FT", "Type"].contains(&key.as_str())
                && value.as_name().is_some_and(|n| n.name() == "Sig")
                && !(depth == 0
                    && if key == "FT" {
                        fields.contains(&owner)
                    } else {
                        signatures.contains(&owner)
                    })
            {
                return Err(invalid("unknown/direct signature container"));
            }
            let child = if path.is_empty() {
                key.clone()
            } else {
                format!("{path}/{key}")
            };
            scan(
                value,
                owner,
                &child,
                root,
                permissions,
                form,
                fields,
                signatures,
                protected,
                edges,
                depth + 1,
                work,
                limits,
            )?;
        }
    } else if let Some(array) = object.as_array() {
        let child = if path.is_empty() {
            "[]".into()
        } else {
            format!("{path}/[]")
        };
        for value in array {
            scan(
                value,
                owner,
                &child,
                root,
                permissions,
                form,
                fields,
                signatures,
                protected,
                edges,
                depth + 1,
                work,
                limits,
            )?;
        }
    }
    Ok(())
}

/// Check declarations/owners in *every* historical view, including constraints
/// later deleted or hidden inside an unreferenced source object.
pub(super) fn safe_history(doc: &Document, budget: &OperationBudget) -> PdfResult<()> {
    let source = doc
        .signature_source()?
        .ok_or_else(|| invalid("immutable source"))?;
    let reports = doc.verify_signatures_with_budget(budget)?;
    let policies = doc.signature_constraints_with_budget(budget)?;
    let mut work = 0;
    for (revision, end) in history::revision_ends(source, doc.ctx().options())?
        .into_iter()
        .enumerate()
    {
        let view = PdfDocument::from_reader_with_options(
            Cursor::new(&source[..end]),
            doc.ctx().options(),
        )?;
        let root = view.ctx().get_root_owned()?;
        let permissions = root.get("Perms").and_then(PdfObject::as_object_id);
        if let Some(value) = root.get("Perms") {
            let (perms, _lease) = dict(&view, value, budget)?;
            let signature = perms
                .get("DocMDP")
                .and_then(PdfObject::as_object_id)
                .ok_or_else(|| malformed("permissions must reference a certification signature"))?;
            if perms.into_iter().any(|(key, _)| key != "DocMDP")
                || !policies.iter().any(|p| {
                    p.signature == signature && p.signed_revision <= revision && p.doc_mdp.is_some()
                })
            {
                return Err(invalid("orphan or unsupported permission declaration"));
            }
        }
        let root_id = view
            .ctx()
            .trailer()
            .get("Root")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| malformed("Root reference"))?;
        let form = root.get("AcroForm").and_then(PdfObject::as_object_id);
        if root.get("AcroForm").is_some() && form.is_none() {
            return Err(invalid("indirect AcroForm required"));
        }
        let tree = view.form_field_tree()?;
        if tree.len() > 256 {
            return Err(PdfError::ResourceLimit("signed field count".into()));
        }
        let mut fields: HashSet<_> = tree.iter().map(|n| n.handle.object_id()).collect();
        for node in &tree {
            fields.extend(node.widgets.iter().map(|w| w.handle.object_id()));
        }
        let signatures: HashSet<_> = reports
            .iter()
            .filter(|r| r.signed_revision <= revision)
            .map(|r| r.object_id)
            .collect();
        let pages = PageTree::entries(view.ctx())?;
        if form == Some(root_id)
            || fields.contains(&root_id)
            || form.is_some_and(|id| fields.contains(&id))
            || pages.iter().any(|p| {
                fields.contains(&p.page_id)
                    || signatures.contains(&p.page_id)
                    || form == Some(p.page_id)
            })
            || signatures
                .iter()
                .any(|id| fields.contains(id) || form == Some(*id) || *id == root_id)
        {
            return Err(malformed("signed object roles alias"));
        }
        let mut protected = fields.clone();
        if let Some(id) = form {
            protected.insert(id);
        }
        let mut edges = Edges::new();
        if let Some(id) = form {
            edges.insert((root_id, id, "AcroForm".into()));
            let (dict, _lease) = dict(&view, &reference(id), budget)?;
            if dict
                .get("NeedAppearances")
                .is_some_and(|v| v.as_bool().is_none_or(|v| v.value()))
            {
                return Err(invalid("NeedAppearances"));
            }
            if dict
                .get("SigFlags")
                .is_some_and(|v| v.as_u32().is_none_or(|n| n & !3 != 0))
            {
                return Err(invalid("unknown signature flags"));
            }
            if let Some(value) = dict.get("Fields") {
                ownership_slot(&view, id, "Fields", value, &protected, &mut edges, budget)?;
            }
        }
        for node in &tree {
            let id = node.handle.object_id();
            let (d, _lease) = dict(&view, &reference(id), budget)?;
            for key in ["Kids", "Parent"] {
                if let Some(value) = d.get(key) {
                    ownership_slot(&view, id, key, value, &protected, &mut edges, budget)?;
                }
            }
            if let Some(value) = d.get("Lock") {
                if node.field_type.as_deref() != Some("Sig") {
                    return Err(invalid("Lock on non-signature field"));
                }
                let (lock, _lease) = dict(&view, value, budget)?;
                if lock.contains_key("P") {
                    return Err(invalid("unsupported Lock P"));
                }
                lock_rule(&view, &lock, &tree, budget)?;
            }
            for widget in &node.widgets {
                if widget.handle.object_id() == id {
                    continue;
                }
                let (w, _lease) = dict(&view, &reference(widget.handle.object_id()), budget)?;
                if w.contains_key("Lock") {
                    return Err(invalid(
                        "Lock belongs to a signature field, not a pure Widget",
                    ));
                }
                if let Some(value) = w.get("Parent") {
                    ownership_slot(
                        &view,
                        widget.handle.object_id(),
                        "Parent",
                        value,
                        &protected,
                        &mut edges,
                        budget,
                    )?;
                }
            }
        }
        for page in pages {
            if let Some(value) = page.page_dict.get("Annots") {
                ownership_slot(
                    &view,
                    page.page_id,
                    "Annots",
                    value,
                    &protected,
                    &mut edges,
                    budget,
                )?;
            }
        }
        for info in view.ctx().source_xref().objects().values() {
            if matches!(
                info.state(),
                crate::document::cross_ref_table::ObjectState::Free
            ) {
                continue;
            }
            let (value, _lease) = view
                .ctx()
                .resolve_source_owned_with_budget(info.object_id(), budget)?;
            scan(
                &value,
                info.object_id(),
                "",
                root_id,
                permissions,
                form,
                &fields,
                &signatures,
                &protected,
                &edges,
                0,
                &mut work,
                doc.ctx().limits(),
            )?;
        }
    }
    Ok(())
}

impl Document {
    pub fn signature_constraints(&self) -> PdfResult<Vec<SignatureConstraints>> {
        self.signature_constraints_with_budget(&self.ctx().limits().operation_budget())
    }
    pub fn signature_constraints_with_budget(
        &self,
        budget: &OperationBudget,
    ) -> PdfResult<Vec<SignatureConstraints>> {
        let Some(source) = self.signature_source()? else {
            return Ok(Vec::new());
        };
        let _envelope = budget.reserve(
            source
                .len()
                .checked_mul(8)
                .and_then(|n| n.checked_add(2 * 1024 * 1024))
                .ok_or_else(|| PdfError::ResourceLimit("MDP history envelope".into()))?,
        )?;
        let reports = self.signature_reports_with_budget(budget)?;
        let Some(first) = reports.first() else {
            return Ok(Vec::new());
        };
        // A transform may retain 256 qualified names of up to 4096 bytes plus
        // identity tables. Reserve their aggregate before accumulating reports.
        let _retained_rules = budget.reserve(
            reports
                .len()
                .checked_mul(2 * 1024 * 1024)
                .ok_or_else(|| PdfError::ResourceLimit("retained MDP declarations".into()))?,
        )?;
        let mut result = Vec::new();
        for report in &reports {
            let view = PdfDocument::from_reader_with_options(
                Cursor::new(&source[..report.signed_revision_bytes as usize]),
                self.ctx().options(),
            )?;
            result.push(read(&view, report, first.object_id, budget)?);
        }
        Ok(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::document::{
        acroform::{FormFieldDefinition, FormFieldKind, FormValue, FormWidgetSpec},
        save_options::PdfVersion,
    };

    fn graph() -> Document {
        let doc = Document::new_with_version(PdfVersion::V2_0).unwrap();
        doc.add_page(0).unwrap();
        for name in ["Data.Child", "Data.Other", "Outside"] {
            doc.add_form_field(
                name,
                &FormFieldDefinition::new(
                    FormFieldKind::Text { multiline: false },
                    FormValue::Text("".into()),
                ),
                &[FormWidgetSpec::new(0, [0., 0., 200., 30.])],
            )
            .unwrap();
        }
        doc
    }
    fn params(action: &str, names: Option<&[&str]>) -> PdfDict {
        let mut d = PdfDict::default();
        d.try_insert("Action".into(), name(action)).unwrap();
        if let Some(names) = names {
            let mut a = PdfArray::default();
            for text in names {
                a.try_add_obj(PdfObject::PdfString(PdfString::from_text(text).unwrap()))
                    .unwrap();
            }
            d.try_insert("Fields".into(), PdfObject::PdfArray(a))
                .unwrap();
        }
        d
    }
    #[test]
    fn all_include_exclude_expand_groups_to_signed_object_identities() {
        let doc = graph();
        let tree = doc.form_field_tree().unwrap();
        let budget = OperationBudget::new(8 * 1024 * 1024);
        let all = lock_rule(&*doc, &params("All", None), &tree, &budget).unwrap();
        assert_eq!(all.locked_fields.len(), 4);
        let included =
            lock_rule(&*doc, &params("Include", Some(&["Data"])), &tree, &budget).unwrap();
        assert_eq!(included.locked_fields.len(), 3);
        let excluded =
            lock_rule(&*doc, &params("Exclude", Some(&["Data"])), &tree, &budget).unwrap();
        assert_eq!(
            excluded.locked_fields,
            vec![
                tree.iter()
                    .find(|n| n.name == "Outside")
                    .unwrap()
                    .handle
                    .object_id()
            ]
        );
        for p in [
            params("Unknown", None),
            params("Include", None),
            params("Include", Some(&["Missing"])),
            params("Include", Some(&["Data", "Data"])),
            params("All", Some(&["Data"])),
        ] {
            assert!(lock_rule(&*doc, &p, &tree, &budget).is_err());
        }
        assert_eq!(budget.live_bytes(), 0);
    }
    #[test]
    fn document_permissions_and_field_locks_intersect_without_owner_override() {
        let id = ObjectId::new(9, 0);
        let mut p = SignatureConstraints {
            signature: ObjectId::new(1, 0),
            signed_revision: 0,
            doc_mdp: Some(DocMdpPermission::NoChanges),
            field_locks: Vec::new(),
        };
        for kind in [
            changes::ModificationKind::FormFill,
            changes::ModificationKind::Signature,
            changes::ModificationKind::Annotation,
        ] {
            assert!(!permits(&[p.clone()], kind, Some(id)));
        }
        p.doc_mdp = Some(DocMdpPermission::FillFormsAndSign);
        assert!(permits(
            &[p.clone()],
            changes::ModificationKind::Signature,
            Some(id)
        ));
        assert!(!permits(
            &[p.clone()],
            changes::ModificationKind::Annotation,
            None
        ));
        p.doc_mdp = Some(DocMdpPermission::Annotate);
        p.field_locks.push(FieldLockRule {
            action: FieldLockAction::Include,
            fields: vec!["Locked".into()],
            locked_fields: vec![id],
        });
        assert!(!permits(
            &[p.clone()],
            changes::ModificationKind::FormFill,
            Some(id)
        ));
        assert!(permits(&[p], changes::ModificationKind::Annotation, None));
    }
    fn certification(p: Option<&str>) -> (Document, SignatureReport) {
        let doc = Document::new_with_version(PdfVersion::V2_0).unwrap();
        doc.add_page(0).unwrap();
        let handle = doc
            .add_signature_field("Approval", &[FormWidgetSpec::new(0, [0., 0., 200., 30.])])
            .unwrap();
        let mut params = PdfDict::default();
        if let Some(p) = p {
            params
                .try_insert(
                    "P".into(),
                    PdfObject::PdfNumber(PdfNumber::new_from_lexeme(p.as_bytes()).unwrap()),
                )
                .unwrap();
        }
        let mut r = PdfDict::default();
        r.try_insert("TransformMethod".into(), name("DocMDP"))
            .unwrap();
        r.try_insert("TransformParams".into(), PdfObject::PdfDict(params))
            .unwrap();
        let mut refs = PdfArray::default();
        refs.try_add_obj(PdfObject::PdfDict(r)).unwrap();
        let mut sig = PdfDict::default();
        sig.try_insert("Reference".into(), PdfObject::PdfArray(refs))
            .unwrap();
        let sid = doc.try_insert_object(PdfObject::PdfDict(sig)).unwrap();
        let mut field = doc
            .resolve_object(handle.object_id())
            .unwrap()
            .into_dict()
            .unwrap();
        field.try_insert("V".into(), reference(sid)).unwrap();
        doc.update_object(handle.object_id(), PdfObject::PdfDict(field))
            .unwrap();
        let root_id = doc
            .ctx()
            .trailer()
            .get("Root")
            .unwrap()
            .as_object_id()
            .unwrap();
        let mut root = doc.ctx().get_root_owned().unwrap();
        let mut perms = PdfDict::default();
        perms.try_insert("DocMDP".into(), reference(sid)).unwrap();
        root.try_insert("Perms".into(), PdfObject::PdfDict(perms))
            .unwrap();
        doc.update_object(root_id, PdfObject::PdfDict(root))
            .unwrap();
        (
            doc,
            SignatureReport {
                object_id: sid,
                byte_range: [0; 4],
                signed_revision_bytes: 0,
                covers_current_document: true,
                verification: SignatureVerification::NotPerformed,
                signed_revision: 0,
                signed_revision_xref: 0,
                field: Some(SignatureField {
                    object_id: handle.object_id(),
                    name: "Approval".into(),
                }),
                source_object_state: SignatureObjectState::Unchanged,
            },
        )
    }
    #[test]
    fn docmdp_default_integer_range_and_first_signature_binding() {
        for (p, expected) in [
            (None, DocMdpPermission::FillFormsAndSign),
            (Some("1"), DocMdpPermission::NoChanges),
            (Some("2"), DocMdpPermission::FillFormsAndSign),
            (Some("3"), DocMdpPermission::Annotate),
        ] {
            let (doc, report) = certification(p);
            assert_eq!(
                read(
                    &*doc,
                    &report,
                    report.object_id,
                    &OperationBudget::new(8 * 1024 * 1024)
                )
                .unwrap()
                .doc_mdp,
                Some(expected)
            );
            let mut orphan = report.clone();
            orphan.field = None;
            assert!(
                read(
                    &*doc,
                    &orphan,
                    report.object_id,
                    &OperationBudget::new(8 * 1024 * 1024)
                )
                .is_err()
            );
            assert!(
                read(
                    &*doc,
                    &report,
                    ObjectId::new(999, 0),
                    &OperationBudget::new(8 * 1024 * 1024)
                )
                .is_err()
            );
        }
        for p in ["0", "4", "2.0", "-1"] {
            let (doc, report) = certification(Some(p));
            assert!(
                read(
                    &*doc,
                    &report,
                    report.object_id,
                    &OperationBudget::new(8 * 1024 * 1024)
                )
                .is_err()
            );
        }
    }
}
