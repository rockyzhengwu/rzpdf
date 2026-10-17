//! Conservative semantic projection of actual revision objects, not API names.
use super::*;
use crate::document::{PdfDocument, acroform::FormFieldNode};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModificationKind {
    FormFill,
    Appearance,
    Signature,
    Annotation,
    Unknown,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ModificationAuthorization {
    Allowed,
    Forbidden,
    Unsupported,
}
#[derive(Debug, Clone)]
pub struct RevisionModification {
    pub revision: usize,
    pub kinds: Vec<ModificationKind>,
    pub authorization: ModificationAuthorization,
    pub reason: Option<String>,
}

struct Analysis {
    operations: Vec<(ModificationKind, Option<ObjectId>)>,
    added_fields: bool,
}

fn object<R: Read + Seek>(
    doc: &PdfDocument<R>,
    id: ObjectId,
    budget: &OperationBudget,
) -> PdfResult<PdfObject> {
    // Called under a whole-comparison envelope; temporary payloads still use
    // the caller's pool during source resolution.
    Ok(doc.ctx().resolve_source_owned_with_budget(id, budget)?.0)
}
fn dictionary<R: Read + Seek>(
    doc: &PdfDocument<R>,
    id: ObjectId,
    budget: &OperationBudget,
) -> PdfResult<PdfDict> {
    object(doc, id, budget)?
        .into_dict()
        .ok_or_else(|| malformed("change dictionary"))
}
fn strip(mut dict: PdfDict, keys: &[&str]) -> PdfDict {
    for key in keys {
        dict.remove(key);
    }
    dict
}
fn name_of<'a>(dict: &'a PdfDict, key: &str) -> Option<&'a str> {
    dict.get(key).and_then(PdfObject::as_name).map(|n| n.name())
}
fn ids<R: Read + Seek>(doc: &PdfDocument<R>) -> HashSet<ObjectId> {
    doc.ctx()
        .source_xref()
        .objects()
        .values()
        .filter(|info| {
            !matches!(
                info.state(),
                crate::document::cross_ref_table::ObjectState::Free
            )
        })
        .map(|i| i.object_id())
        .collect()
}
fn same_node(a: &FormFieldNode, b: &FormFieldNode) -> bool {
    a.handle.object_id() == b.handle.object_id()
        && a.name == b.name
        && a.partial_name == b.partial_name
        && a.parent.as_ref().map(|h| h.object_id()) == b.parent.as_ref().map(|h| h.object_id())
        && a.children
            .iter()
            .map(|h| h.object_id())
            .eq(b.children.iter().map(|h| h.object_id()))
        && a.flags == b.flags
        && a.field_type == b.field_type
        && a.widgets.len() == b.widgets.len()
        && a.widgets.iter().zip(&b.widgets).all(|(x, y)| {
            x.handle.object_id() == y.handle.object_id()
                && x.page_index == y.page_index
                && x.rect == y.rect
        })
}

fn appearance<R: Read + Seek>(
    doc: &PdfDocument<R>,
    dict: &PdfDict,
    old_ids: &HashSet<ObjectId>,
    allowed: &mut HashSet<ObjectId>,
    budget: &OperationBudget,
) -> PdfResult<()> {
    let ap = dict
        .get("AP")
        .and_then(PdfObject::as_dict)
        .ok_or_else(|| invalid("new direct AP dictionary required"))?;
    if ap.into_iter().any(|(k, _)| k != "N") {
        return Err(invalid("changed rollover/down appearances unsupported"));
    }
    let id = ap
        .get("N")
        .and_then(PdfObject::as_object_id)
        .ok_or_else(|| invalid("new indirect normal AP required"))?;
    if old_ids.contains(&id) || !allowed.insert(id) {
        return Err(invalid("new AP must be fresh and unshared"));
    }
    let value = object(doc, id, budget)?;
    let stream = value
        .as_stream()
        .ok_or_else(|| malformed("normal AP stream"))?;
    let dict = stream.dict();
    if name_of(dict, "Type") != Some("XObject")
        || name_of(dict, "Subtype") != Some("Form")
        || dict.into_iter().any(|(key, _)| {
            ![
                "Type",
                "Subtype",
                "BBox",
                "Matrix",
                "Resources",
                "Length",
                "Filter",
                "DecodeParms",
            ]
            .contains(&key.as_str())
        })
    {
        return Err(invalid("normal AP profile"));
    }
    let matrix = dict
        .get("Matrix")
        .and_then(PdfObject::as_array)
        .ok_or_else(|| invalid("explicit AP Matrix required"))?;
    let identity = [1., 0., 0., 1., 0., 0.];
    if matrix.len() != 6
        || matrix
            .into_iter()
            .zip(identity)
            .any(|(a, b)| a.as_number().is_none_or(|n| n.value() != b))
    {
        return Err(invalid("changed AP Matrix must be identity"));
    }
    let bbox = dict
        .get("BBox")
        .and_then(PdfObject::as_array)
        .ok_or_else(|| malformed("AP BBox"))?;
    if bbox.len() != 4
        || bbox
            .into_iter()
            .any(|v| v.as_number().is_none_or(|n| !n.value().is_finite()))
    {
        return Err(malformed("AP BBox numbers"));
    }
    let coords: Vec<_> = bbox
        .into_iter()
        .map(|v| v.as_number().unwrap().value())
        .collect();
    if coords[0] != 0. || coords[1] != 0. || coords[2] <= 0. || coords[3] <= 0. {
        return Err(invalid(
            "changed AP BBox must have positive local dimensions",
        ));
    }
    let resources = dict
        .get("Resources")
        .and_then(PdfObject::as_dict)
        .ok_or_else(|| invalid("self-contained AP Resources"))?;
    if resources.into_iter().any(|(key, _)| key != "Font") {
        return Err(invalid("AP resources outside font-only profile"));
    }
    if let Some(fonts) = resources.get("Font") {
        let fonts = fonts
            .as_dict()
            .ok_or_else(|| malformed("AP Font resources"))?;
        for (_, font) in fonts {
            if let Some(id) = font.as_object_id() {
                if !old_ids.contains(&id) {
                    return Err(invalid("AP may only reuse an unchanged source font"));
                }
                let font = dictionary(doc, id, budget)?;
                if name_of(&font, "Type") != Some("Font") {
                    return Err(malformed("AP font reference"));
                }
            } else {
                let font = font.as_dict().ok_or_else(|| malformed("AP direct font"))?;
                if name_of(font, "Subtype") != Some("Type1")
                    || !matches!(
                        name_of(font, "BaseFont"),
                        Some("Helvetica" | "Courier" | "Times-Roman")
                    )
                    || font.into_iter().any(|(k, _)| {
                        !["Type", "Subtype", "BaseFont", "Encoding"].contains(&k.as_str())
                    })
                {
                    return Err(invalid("direct AP font profile"));
                }
            }
        }
    }
    Ok(())
}

fn analyze<R: Read + Seek, S: Read + Seek>(
    before: &PdfDocument<R>,
    after: &PdfDocument<S>,
    budget: &OperationBudget,
) -> PdfResult<Analysis> {
    let old_ids = ids(before);
    let new_ids = ids(after);
    if old_ids.len().saturating_add(new_ids.len()) > before.ctx().limits().max_objects {
        return Err(PdfError::ResourceLimit("revision comparison visits".into()));
    }
    if !old_ids.is_subset(&new_ids) {
        return Err(invalid("objects removed/reused in signed revision"));
    }
    let old_size = before
        .ctx()
        .trailer()
        .get("Size")
        .and_then(PdfObject::as_u32)
        .ok_or_else(|| malformed("source Size"))?;
    if new_ids
        .difference(&old_ids)
        .any(|id| id.obj_num() < old_size || id.gen_num() != 0)
    {
        return Err(invalid("new signed object number reuse"));
    }
    let old_tree = before.form_field_tree()?;
    let new_tree = after.form_field_tree()?;
    if old_tree.len() > 256 || new_tree.len() > 256 {
        return Err(PdfError::ResourceLimit("signed field count".into()));
    }
    if old_tree.len() > new_tree.len()
        || old_tree
            .iter()
            .zip(&new_tree)
            .any(|(a, b)| !same_node(a, b))
    {
        return Err(invalid("signed field ownership/name/geometry changed"));
    }
    let mut old_trailer = before.ctx().trailer().clone();
    let mut new_trailer = after.ctx().trailer().clone();
    for key in [
        "Size",
        "Prev",
        "XRefStm",
        "Type",
        "W",
        "Index",
        "Length",
        "Filter",
        "DecodeParms",
    ] {
        old_trailer.remove(key);
        new_trailer.remove(key);
    }
    if old_trailer != new_trailer {
        return Err(invalid("signed trailer roots/identity changed"));
    }
    let mut allowed_old = HashSet::new();
    let mut allowed_new = HashSet::new();
    let mut operations = Vec::new();
    let added = &new_tree[old_tree.len()..];
    let mut added_widgets = HashSet::new();
    for node in added {
        // Recognize exactly the V11-07 flat invisible approval shape. New field
        // creation under an MDP declaration remains outside this profile.
        let id = node.handle.object_id();
        if old_tree
            .iter()
            .any(|n| n.parent.is_some() || !n.children.is_empty())
            || node.parent.is_some()
            || !node.children.is_empty()
            || node.field_type.as_deref() != Some("Sig")
            || node.widgets.len() != 1
            || node.widgets[0].handle.object_id() != id
            || node.widgets[0].page_index != 0
            || node.widgets[0].rect != [0.; 4]
            || node.name.contains('.')
            || old_ids.contains(&id)
            || !allowed_new.insert(id)
        {
            return Err(invalid(
                "new signature field outside flat invisible approval profile",
            ));
        }
        let field = dictionary(after, id, budget)?;
        if field.into_iter().any(|(k, _)| {
            !["Type", "Subtype", "FT", "T", "V", "P", "F", "Rect", "AP"].contains(&k.as_str())
        }) || name_of(&field, "Type") != Some("Annot")
            || name_of(&field, "Subtype") != Some("Widget")
            || field.get("F").and_then(PdfObject::as_u32) != Some(132)
        {
            return Err(invalid("new approval Widget properties"));
        }
        let sid = field
            .get("V")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| malformed("new approval V"))?;
        if old_ids.contains(&sid) || !allowed_new.insert(sid) {
            return Err(invalid("new approval signature identity"));
        }
        let signature = dictionary(after, sid, budget)?;
        if name_of(&signature, "Type") != Some("Sig")
            || name_of(&signature, "SubFilter") != Some("adbe.pkcs7.detached")
            || signature.contains_key("Reference")
        {
            return Err(invalid("new approval signature profile"));
        }
        let ap = field
            .get("AP")
            .and_then(PdfObject::as_dict)
            .ok_or_else(|| malformed("new approval AP"))?;
        if ap.into_iter().count() != 1 {
            return Err(invalid("new approval AP states"));
        }
        let aid = ap
            .get("N")
            .and_then(PdfObject::as_object_id)
            .ok_or_else(|| malformed("new approval AP reference"))?;
        if old_ids.contains(&aid) || !allowed_new.insert(aid) {
            return Err(invalid("new approval AP ownership"));
        }
        let value = object(after, aid, budget)?;
        let stream = value
            .as_stream()
            .ok_or_else(|| malformed("new approval AP stream"))?;
        let d = stream.dict();
        if !stream.raw_data().is_empty()
            || name_of(d, "Type") != Some("XObject")
            || name_of(d, "Subtype") != Some("Form")
            || d.get("Resources")
                .and_then(PdfObject::as_dict)
                .is_none_or(|r| r.into_iter().count() != 0)
            || d.into_iter().any(|(k, _)| {
                !["Type", "Subtype", "Resources", "BBox", "Length"].contains(&k.as_str())
            })
        {
            return Err(invalid("new approval empty AP profile"));
        }
        let bbox = d
            .get("BBox")
            .and_then(PdfObject::as_array)
            .ok_or_else(|| malformed("new approval AP BBox"))?;
        if bbox.len() != 4
            || bbox
                .into_iter()
                .zip([0., 0., 1., 1.])
                .any(|(v, n)| v.as_number().is_none_or(|v| v.value() != n))
        {
            return Err(invalid("new approval AP dimensions"));
        }
        added_widgets.insert(id);
        operations.push((ModificationKind::Signature, Some(id)));
    }
    for node in &old_tree {
        let id = node.handle.object_id();
        let a = dictionary(before, id, budget)?;
        let b = dictionary(after, id, budget)?;
        let mut widget_changed = false;
        let value_changed = a.get("V") != b.get("V");
        for widget in &node.widgets {
            let wid = widget.handle.object_id();
            let wa = dictionary(before, wid, budget)?;
            let wb = dictionary(after, wid, budget)?;
            let keys: &[&str] = if wid == id {
                &["V", "AP", "AS"]
            } else {
                &["AP", "AS"]
            };
            if strip(wa.clone(), keys) != strip(wb.clone(), keys) {
                return Err(invalid("Widget properties/relationships changed"));
            }
            if wa != wb {
                if wa.get("AP") != wb.get("AP") {
                    appearance(after, &wb, &old_ids, &mut allowed_new, budget)?;
                }
                if wa.get("AS") != wb.get("AS") && node.field_type.as_deref() != Some("Btn") {
                    return Err(invalid("non-button AS change"));
                }
                allowed_old.insert(wid);
                widget_changed = true;
            }
        }
        if !value_changed && !widget_changed {
            continue;
        }
        if !node.children.is_empty()
            || node.widgets.is_empty()
            || node.flags & 1 != 0
            || ["A", "AA", "SV", "Lock"]
                .iter()
                .any(|key| a.contains_key(key))
        {
            return Err(invalid("nonterminal/read-only/seeded/locked field changed"));
        }
        let merged = node.widgets.iter().any(|w| w.handle.object_id() == id);
        let keys: &[&str] = if merged { &["V", "AP", "AS"] } else { &["V"] };
        if strip(a.clone(), keys) != strip(b.clone(), keys) {
            return Err(invalid("field properties changed"));
        }
        for widget in &node.widgets {
            let w = dictionary(before, widget.handle.object_id(), budget)?;
            if ["A", "AA", "OC", "StructParent", "Popup", "IRT"]
                .iter()
                .any(|key| w.contains_key(key))
                || w.get("F")
                    .is_some_and(|v| v.as_u32().is_none_or(|f| f & 512 != 0 || f & !1023 != 0))
            {
                return Err(invalid("locked or unsupported Widget changed"));
            }
        }
        let kind = match node.field_type.as_deref() {
            Some("Sig") => {
                let mut parent = node.parent.as_ref();
                while let Some(handle) = parent {
                    let ancestor = old_tree
                        .iter()
                        .find(|n| n.handle.object_id() == handle.object_id())
                        .ok_or_else(|| malformed("signature ancestor"))?;
                    if dictionary(before, handle.object_id(), budget)?
                        .get("V")
                        .is_some_and(|v| !matches!(v, PdfObject::PdfNull))
                    {
                        return Err(invalid("inherited filled signature value overwritten"));
                    }
                    parent = ancestor.parent.as_ref();
                }
                if a.get("V").is_some_and(|v| !matches!(v, PdfObject::PdfNull)) {
                    return Err(invalid("existing signature overwritten"));
                }
                let sid = b
                    .get("V")
                    .and_then(PdfObject::as_object_id)
                    .ok_or_else(|| malformed("new signature V"))?;
                if old_ids.contains(&sid) || !allowed_new.insert(sid) {
                    return Err(invalid("signature not fresh"));
                }
                let signature = dictionary(after, sid, budget)?;
                if name_of(&signature, "Type") != Some("Sig")
                    || name_of(&signature, "SubFilter") != Some("adbe.pkcs7.detached")
                {
                    return Err(invalid("new signature profile"));
                }
                if signature.contains_key("Reference") {
                    return Err(invalid(
                        "adding new MDP constraints requires another profile",
                    ));
                }
                ModificationKind::Signature
            }
            Some("Tx") => {
                if !matches!(b.get("V"), Some(PdfObject::PdfString(_))) {
                    return Err(malformed("text value must be direct string"));
                }
                if value_changed {
                    for widget in &node.widgets {
                        if dictionary(before, widget.handle.object_id(), budget)?.get("AP")
                            == dictionary(after, widget.handle.object_id(), budget)?.get("AP")
                        {
                            return Err(invalid(
                                "text value change requires regenerated widget appearances",
                            ));
                        }
                    }
                }
                if value_changed {
                    ModificationKind::FormFill
                } else {
                    ModificationKind::Appearance
                }
            }
            Some("Btn") if node.flags & ((1 << 15) | (1 << 16)) == 0 => {
                let state = name_of(&b, "V").ok_or_else(|| malformed("checkbox V"))?;
                for widget in &node.widgets {
                    let dict = dictionary(after, widget.handle.object_id(), budget)?;
                    if name_of(&dict, "AS") != Some(state) {
                        return Err(malformed("checkbox V/AS mismatch"));
                    }
                    let ap = dict
                        .get("AP")
                        .and_then(PdfObject::as_dict)
                        .and_then(|d| d.get("N"))
                        .and_then(PdfObject::as_dict)
                        .ok_or_else(|| invalid("checkbox state AP"))?;
                    if !ap.contains_key(state) {
                        return Err(malformed("checkbox state absent from AP"));
                    }
                }
                if value_changed {
                    ModificationKind::FormFill
                } else {
                    ModificationKind::Appearance
                }
            }
            _ => return Err(invalid("unsupported signed field type")),
        };
        allowed_old.insert(id);
        operations.push((kind, Some(id)));
    }
    let root = before.ctx().get_root_owned()?;
    if let Some(fid) = root.get("AcroForm").and_then(PdfObject::as_object_id) {
        let a = dictionary(before, fid, budget)?;
        let b = dictionary(after, fid, budget)?;
        if a != b {
            if operations
                .iter()
                .all(|(k, _)| *k != ModificationKind::Signature)
                || b.get("SigFlags").and_then(PdfObject::as_u32) != Some(3)
                || a.get("SigFlags")
                    .is_some_and(|v| v.as_u32().is_none_or(|n| n & !3 != 0))
                || strip(
                    a.clone(),
                    if added.is_empty() {
                        &["SigFlags"]
                    } else {
                        &["SigFlags", "Fields"]
                    },
                ) != strip(
                    b.clone(),
                    if added.is_empty() {
                        &["SigFlags"]
                    } else {
                        &["SigFlags", "Fields"]
                    },
                )
            {
                return Err(invalid("AcroForm changed outside signature flags"));
            }
            if !added.is_empty() {
                let old = before
                    .ctx()
                    .resolve(a.get("Fields").ok_or_else(|| malformed("Fields"))?)?
                    .into_array()
                    .ok_or_else(|| malformed("Fields"))?;
                let new = after
                    .ctx()
                    .resolve(b.get("Fields").ok_or_else(|| malformed("Fields"))?)?
                    .into_array()
                    .ok_or_else(|| malformed("Fields"))?;
                let old: Vec<_> = old.into_iter().collect();
                let new: Vec<_> = new.into_iter().collect();
                if !new.starts_with(&old)
                    || new.len() != old.len() + added.len()
                    || new[old.len()..]
                        .iter()
                        .zip(added)
                        .any(|(v, n)| v.as_object_id() != Some(n.handle.object_id()))
                {
                    return Err(invalid("new approval Fields projection"));
                }
            }
            allowed_old.insert(fid);
        }
    }
    // Square annotations are an explicit narrow P=3 operation; page content,
    // geometry, resources, structure and optional-content keys never change.
    let old_pages = crate::document::page_tree::PageTree::entries(before.ctx())?;
    let new_pages = crate::document::page_tree::PageTree::entries(after.ctx())?;
    if old_pages.len() != new_pages.len() {
        return Err(invalid("signed page count changed"));
    }
    for (a, b) in old_pages.iter().zip(&new_pages) {
        if a.page_id != b.page_id {
            return Err(invalid("signed pages reordered"));
        }
        if a.page_dict == b.page_dict {
            continue;
        }
        if strip(a.page_dict.clone(), &["Annots"]) != strip(b.page_dict.clone(), &["Annots"]) {
            return Err(invalid("signed page content/resources/geometry changed"));
        }
        let aa = before
            .ctx()
            .resolve(
                a.page_dict
                    .get("Annots")
                    .unwrap_or(&PdfObject::PdfArray(PdfArray::default())),
            )?
            .into_array()
            .ok_or_else(|| malformed("Annots"))?;
        let bb = after
            .ctx()
            .resolve(
                b.page_dict
                    .get("Annots")
                    .ok_or_else(|| malformed("Annots"))?,
            )?
            .into_array()
            .ok_or_else(|| malformed("Annots"))?;
        let aa: Vec<_> = aa.into_iter().collect();
        let bb: Vec<_> = bb.into_iter().collect();
        if bb.len() <= aa.len() || !bb.starts_with(&aa) {
            return Err(invalid("old annotations changed/removed"));
        }
        for value in &bb[aa.len()..] {
            let id = value
                .as_object_id()
                .ok_or_else(|| invalid("indirect new annotation"))?;
            if added_widgets.remove(&id) {
                if a.page_id != old_pages[0].page_id {
                    return Err(invalid("new approval page ownership"));
                }
                let field = dictionary(after, id, budget)?;
                if field.get("P").and_then(PdfObject::as_object_id) != Some(a.page_id) {
                    return Err(malformed("new approval P reference"));
                }
                continue;
            }
            if old_ids.contains(&id) || !allowed_new.insert(id) {
                return Err(invalid("annotation fresh unique ownership"));
            }
            let d = dictionary(after, id, budget)?;
            if name_of(&d, "Subtype") != Some("Square")
                || d.get("P").and_then(PdfObject::as_object_id) != Some(b.page_id)
                || d.into_iter().any(|(key, _)| {
                    ![
                        "Type", "Subtype", "P", "Rect", "Contents", "F", "AP", "C", "BS",
                    ]
                    .contains(&key.as_str())
                })
            {
                return Err(invalid(
                    "unsupported signed annotation relationships/profile",
                ));
            }
            let rect = d
                .get("Rect")
                .and_then(PdfObject::as_array)
                .ok_or_else(|| malformed("annotation Rect"))?;
            if rect.len() != 4
                || rect
                    .into_iter()
                    .any(|v| v.as_number().is_none_or(|n| !n.value().is_finite()))
            {
                return Err(malformed("annotation rectangle numbers"));
            }
            let r: Vec<_> = rect
                .into_iter()
                .map(|v| v.as_number().unwrap().value())
                .collect();
            if r[2] <= r[0]
                || r[3] <= r[1]
                || !(r[2] - r[0]).is_finite()
                || !(r[3] - r[1]).is_finite()
            {
                return Err(malformed("annotation rectangle dimensions"));
            }
            if d.get("Type")
                .is_some_and(|v| v.as_name().is_none_or(|n| n.name() != "Annot"))
                || d.get("Contents").is_some_and(|v| v.as_string().is_none())
                || d.get("F")
                    .is_some_and(|v| v.as_u32().is_none_or(|n| n & !1023 != 0))
            {
                return Err(malformed("annotation type/contents/flags"));
            }
            appearance(after, &d, &old_ids, &mut allowed_new, budget)?;
            operations.push((ModificationKind::Annotation, None));
        }
        allowed_old.insert(a.page_id);
    }
    if !added_widgets.is_empty() {
        return Err(invalid("new approval Widget missing from Annots"));
    }
    for id in &old_ids {
        if object(before, *id, budget)? != object(after, *id, budget)? && !allowed_old.contains(id)
        {
            return Err(invalid("unclassified old object changed"));
        }
    }
    if new_ids
        .difference(&old_ids)
        .any(|id| !allowed_new.contains(id))
    {
        return Err(invalid("unclassified new object"));
    }
    Ok(Analysis {
        operations,
        added_fields: !added.is_empty(),
    })
}

pub(super) fn candidate(
    source: &Document,
    output: &Document,
    budget: &OperationBudget,
) -> PdfResult<()> {
    let analysis = analyze(source, output, budget)?;
    let policies = source.signature_constraints_with_budget(budget)?;
    if policies.iter().any(|p| {
        p.doc_mdp == Some(policy::DocMdpPermission::NoChanges)
            || (analysis.added_fields && (p.doc_mdp.is_some() || !p.field_locks.is_empty()))
    }) || analysis
        .operations
        .iter()
        .any(|(kind, field)| !policy::permits(&policies, *kind, *field))
    {
        return Err(PdfError::PermissionDenied(
            "signature modification constraints".into(),
        ));
    }
    Ok(())
}

impl Document {
    pub fn signature_modifications(&self) -> PdfResult<Vec<RevisionModification>> {
        self.signature_modifications_with_budget(&self.ctx().limits().operation_budget())
    }
    pub fn signature_modifications_with_budget(
        &self,
        budget: &OperationBudget,
    ) -> PdfResult<Vec<RevisionModification>> {
        let Some(source) = self.signature_source()? else {
            return Ok(Vec::new());
        };
        let _envelope = budget.reserve(
            source
                .len()
                .checked_mul(16)
                .and_then(|n| n.checked_add(4 * 1024 * 1024))
                .ok_or_else(|| {
                    PdfError::ResourceLimit("modification comparison envelope".into())
                })?,
        )?;
        policy::safe_history(self, budget)?;
        let reports = self.verify_signatures_with_budget(budget)?;
        let policies = self.signature_constraints_with_budget(budget)?;
        let _policy_tables =
            budget.reserve(policies.len().checked_mul(4 * 1024 * 1024).ok_or_else(|| {
                PdfError::ResourceLimit("MDP declarations and active policy copies".into())
            })?)?;
        let Some(first) = reports.first() else {
            return Ok(Vec::new());
        };
        let ends = history::revision_ends(source, self.ctx().options())?;
        let mut result = Vec::new();
        for revision in first.signed_revision + 1..ends.len() {
            let before = PdfDocument::from_reader_with_options(
                Cursor::new(&source[..ends[revision - 1]]),
                self.ctx().options(),
            )?;
            let after = PdfDocument::from_reader_with_options(
                Cursor::new(&source[..ends[revision]]),
                self.ctx().options(),
            )?;
            let active: Vec<_> = policies
                .iter()
                .filter(|p| p.signed_revision < revision)
                .cloned()
                .collect();
            let analyzed = analyze(&before, &after, budget);
            let (kinds, authorization, reason) = match analyzed {
                Ok(analysis) => {
                    let mut kinds = Vec::new();
                    for (kind, _) in &analysis.operations {
                        if !kinds.contains(kind) {
                            kinds.push(*kind);
                        }
                    }
                    let allowed = reports.iter().all(|r| {
                        r.verification == SignatureVerification::Valid
                            && r.source_object_state == SignatureObjectState::Unchanged
                            && r.field.is_some()
                    }) && !active.iter().any(|p| {
                        p.doc_mdp == Some(policy::DocMdpPermission::NoChanges)
                            || (analysis.added_fields
                                && (p.doc_mdp.is_some() || !p.field_locks.is_empty()))
                    }) && analysis
                        .operations
                        .iter()
                        .all(|(kind, field)| policy::permits(&active, *kind, *field));
                    (
                        kinds,
                        if allowed {
                            ModificationAuthorization::Allowed
                        } else {
                            ModificationAuthorization::Forbidden
                        },
                        if allowed {
                            None
                        } else {
                            Some("signature bits/state or signed modification constraints deny change".into())
                        },
                    )
                }
                Err(PdfError::ResourceLimit(message)) => {
                    return Err(PdfError::ResourceLimit(message));
                }
                Err(error) => (
                    vec![ModificationKind::Unknown],
                    ModificationAuthorization::Unsupported,
                    Some(error.to_string()),
                ),
            };
            result.push(RevisionModification {
                revision,
                kinds,
                authorization,
                reason,
            });
        }
        Ok(result)
    }
}
