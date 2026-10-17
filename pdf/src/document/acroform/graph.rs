//! The field tree and page /Annots are two views of the same widget ownership.
use super::*;
use crate::document::identity::ObjectHandle;
use std::collections::{HashMap, HashSet};

/// A preorder field-tree record. A group has children; a terminal field has
/// widgets (possibly none). Names are fully qualified, not object identities.
#[derive(Debug, Clone, PartialEq)]
pub struct FormFieldNode {
    pub handle: ObjectHandle,
    pub parent: Option<ObjectHandle>,
    pub partial_name: Option<String>,
    pub name: String,
    pub children: Vec<ObjectHandle>,
    pub widgets: Vec<FormWidget>,
    /// Effective inherited /FT; unknown types remain inspectable.
    pub field_type: Option<String>,
    pub flags: u32,
}

#[derive(Debug, Clone, PartialEq)]
pub struct FormWidget {
    pub handle: ObjectHandle,
    pub page_index: u32,
    pub rect: [f32; 4],
}

pub(super) struct FieldNode {
    pub id: ObjectId,
    pub dict: PdfDict,
    pub effective: PdfDict,
    pub parent: Option<ObjectId>,
    pub children: Vec<ObjectId>,
    pub widgets: Vec<FormWidget>,
    pub partial: Option<String>,
    pub name: String,
}

pub(super) struct FieldGraph {
    pub form: PdfDict,
    pub nodes: Vec<FieldNode>,
}

pub(super) fn invalid(message: &str) -> PdfError {
    PdfError::DocumentError(format!("AcroForm: {message}"))
}

pub(super) fn indirect(value: &PdfObject) -> PdfResult<ObjectId> {
    value
        .as_reference()
        .map(PdfReference::id)
        .ok_or_else(|| invalid("ownership reference must be indirect"))
}

pub(super) fn partial_name(value: &str) -> PdfResult<()> {
    if value.is_empty() || value.contains('.') || value.chars().any(char::is_control) {
        return Err(invalid(
            "partial name must be nonempty, without periods or control characters",
        ));
    }
    Ok(())
}

impl FieldGraph {
    pub fn node(&self, id: ObjectId) -> PdfResult<&FieldNode> {
        self.nodes
            .iter()
            .find(|n| n.id == id)
            .ok_or_else(|| invalid("field not in tree"))
    }
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(super) fn form_array(&self, dict: &PdfDict, key: &str) -> PdfResult<PdfArray> {
        let array = dict
            .get(key)
            .map(|v| self.ctx().resolve(v))
            .transpose()?
            .map(|v| {
                v.into_array()
                    .ok_or_else(|| invalid("expected ownership array"))
            })
            .transpose()?
            .unwrap_or_default();
        if array.len() > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("form ownership entries".into()));
        }
        Ok(array)
    }

    pub(super) fn form_dict(&self, id: ObjectId) -> PdfResult<PdfDict> {
        self.resolve_object(id)?
            .into_dict()
            .ok_or_else(|| invalid("expected dictionary"))
    }

    pub(super) fn form_flags(&self, dict: &PdfDict, key: &str) -> PdfResult<u32> {
        dict.get(key)
            .map(|v| self.ctx().resolve(v))
            .transpose()?
            .map(|v| {
                v.as_number()
                    .ok_or_else(|| invalid("flags must be an integer"))?
                    .as_u32_checked()
            })
            .transpose()
            .map(|v| v.unwrap_or(0))
    }

    pub(super) fn field_graph(&self) -> PdfResult<FieldGraph> {
        let root = self.ctx().get_root_owned()?;
        let Some(value) = root.get("AcroForm") else {
            return Ok(FieldGraph {
                form: PdfDict::default(),
                nodes: Vec::new(),
            });
        };
        let form = self
            .ctx()
            .resolve(value)?
            .into_dict()
            .ok_or_else(|| invalid("expected form dictionary"))?;
        if form.get("XFA").is_some() {
            return Err(PdfError::UnsupportedFeature("XFA field graph".into()));
        }
        let limits = self.ctx().limits();
        let roots = self.form_array(&form, "Fields")?;
        let mut owners = HashMap::new();
        let mut annotation_count = 0usize;
        // /P is optional. Derive it from page membership, then check it when present.
        for (page_index, page) in self.collect_page_entries()?.iter().enumerate() {
            for entry in &self.form_array(&page.page_dict, "Annots")? {
                annotation_count = annotation_count
                    .checked_add(1)
                    .filter(|n| *n <= limits.max_objects)
                    .ok_or_else(|| PdfError::ResourceLimit("form page annotation count".into()))?;
                let dict = self
                    .ctx()
                    .resolve(entry)?
                    .into_dict()
                    .ok_or_else(|| invalid("annotation dictionary"))?;
                if dict
                    .get("Subtype")
                    .and_then(PdfObject::as_name)
                    .map(PdfName::name)
                    != Some("Widget")
                {
                    continue;
                }
                let id = indirect(entry)?;
                if owners
                    .insert(id, (page_index as u32, page.page_id))
                    .is_some()
                {
                    return Err(invalid("widget has multiple page owners"));
                }
            }
        }
        let mut pending = Vec::new();
        let mut seen = HashSet::new();
        for entry in &roots {
            let id = indirect(entry)?;
            if !seen.insert(id) {
                return Err(invalid("shared root field"));
            }
            pending.push((id, None, 0usize, String::new(), PdfDict::default()));
        }
        pending.reverse();
        let mut names = HashSet::new();
        let mut nodes = Vec::new();
        let mut name_bytes = 0usize;
        while let Some((id, parent, depth, prefix, mut effective)) = pending.pop() {
            if depth >= limits.max_depth {
                return Err(PdfError::ResourceLimit("form field depth".into()));
            }
            if seen.len() > limits.max_objects {
                return Err(PdfError::ResourceLimit("form graph nodes".into()));
            }
            let dict = self.form_dict(id)?;
            if dict.get("Parent").map(indirect).transpose()? != parent {
                return Err(invalid("field Parent/Kids mismatch"));
            }
            let partial = dict
                .get("T")
                .map(|v| {
                    self.ctx()
                        .resolve(v)?
                        .as_string()
                        .ok_or_else(|| invalid("field T must be a text string"))?
                        .text_content_with_limits(&limits)
                })
                .transpose()?;
            if let Some(partial) = &partial {
                partial_name(partial)?;
            }
            let full = match &partial {
                Some(p) if !prefix.is_empty() => format!("{prefix}.{p}"),
                Some(p) => p.clone(),
                None => prefix,
            };
            name_bytes = name_bytes
                .checked_add(full.len())
                .filter(|n| *n <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("form qualified name bytes".into()))?;
            // Only designated inheritable entries are copied; /T, /Kids,
            // /P and /AP never inherit. /Opt inherits for Btn, not for Ch.
            for key in ["FT", "Ff", "V", "DV", "DA", "Q", "MaxLen"] {
                if let Some(v) = dict.get(key) {
                    effective.try_insert(key.into(), self.ctx().resolve(v)?)?;
                }
            }
            if effective
                .get("FT")
                .and_then(PdfObject::as_name)
                .map(PdfName::name)
                != Some("Btn")
            {
                effective.remove("Opt");
            }
            if let Some(value) = dict.get("Opt") {
                effective.try_insert("Opt".into(), self.ctx().resolve(value)?)?;
            }
            let kids = self.form_array(&dict, "Kids")?;
            let merged = dict
                .get("Subtype")
                .and_then(PdfObject::as_name)
                .map(PdfName::name)
                == Some("Widget");
            if merged && dict.get("Kids").is_some() {
                return Err(invalid("merged widget field cannot have Kids"));
            }
            let mut children = Vec::new();
            let mut widgets = Vec::new();
            if merged {
                widgets.push(self.describe_form_widget(id, &dict, &mut owners,
                    effective.get("FT").and_then(PdfObject::as_name).is_some_and(|n| n.name() == "Sig"))?);
            }
            for kid in &kids {
                let kid_id = indirect(kid)?;
                if !seen.insert(kid_id) {
                    return Err(invalid("cycle or shared field/widget"));
                }
                if seen.len() > limits.max_objects {
                    return Err(PdfError::ResourceLimit("form graph nodes".into()));
                }
                let child = self.form_dict(kid_id)?;
                if child
                    .get("Subtype")
                    .and_then(PdfObject::as_name)
                    .map(PdfName::name)
                    == Some("Widget")
                    && child.get("T").is_none()
                    && child.get("FT").is_none()
                    && child.get("Kids").is_none()
                {
                    if child.get("Parent").map(indirect).transpose()? != Some(id) {
                        return Err(invalid("widget Parent/Kids mismatch"));
                    }
                    widgets.push(self.describe_form_widget(kid_id, &child, &mut owners,
                        effective.get("FT").and_then(PdfObject::as_name).is_some_and(|n| n.name() == "Sig"))?);
                } else {
                    children.push(kid_id);
                }
            }
            if !children.is_empty() && !widgets.is_empty() {
                return Err(invalid("Kids mixes fields and widgets"));
            }
            // An unnamed child may legally share its ancestor's name. Named
            // siblings and named field/group paths must nevertheless be unique.
            if partial.is_some() && !names.insert(full.clone()) {
                return Err(invalid("duplicate qualified field name"));
            }
            for child in children.iter().rev() {
                let mut inherited = effective.clone();
                if effective
                    .get("FT")
                    .and_then(PdfObject::as_name)
                    .map(PdfName::name)
                    != Some("Btn")
                {
                    inherited.remove("Opt");
                }
                pending.push((*child, Some(id), depth + 1, full.clone(), inherited));
            }
            nodes.push(FieldNode {
                id,
                dict,
                effective,
                parent,
                children,
                widgets,
                partial,
                name: full,
            });
        }
        if !owners.is_empty() {
            return Err(invalid("page widget is absent from the field tree"));
        }
        let mut terminal_values = HashMap::new();
        for node in &nodes {
            if !node.children.is_empty() || node.widgets.is_empty() {
                continue;
            }
            if node
                .effective
                .get("FT")
                .and_then(PdfObject::as_name)
                .is_none()
            {
                return Err(invalid("terminal field has no effective field type"));
            }
            let values: Vec<_> = ["FT", "V", "DV"]
                .into_iter()
                .map(|key| {
                    node.effective
                        .get(key)
                        .filter(|v| !matches!(v, PdfObject::PdfNull))
                        .cloned()
                })
                .collect();
            if let Some(old) = terminal_values.insert(node.name.as_str(), values.clone()) {
                if old != values {
                    return Err(invalid("same-named unnamed fields disagree on FT/V/DV"));
                }
            }
        }
        Ok(FieldGraph { form, nodes })
    }

    fn describe_form_widget(
        &self,
        id: ObjectId,
        dict: &PdfDict,
        owners: &mut HashMap<ObjectId, (u32, ObjectId)>,
        signature: bool,
    ) -> PdfResult<FormWidget> {
        let (page_index, page_id) = owners
            .remove(&id)
            .ok_or_else(|| invalid("widget is absent from page Annots or shared"))?;
        if dict
            .get("P")
            .map(indirect)
            .transpose()?
            .is_some_and(|p| p != page_id)
        {
            return Err(invalid("widget P disagrees with page Annots"));
        }
        let r = crate::geom::rectangle::Rectangle::try_from_with_context(
            dict.get("Rect")
                .ok_or_else(|| invalid("widget Rect missing"))?,
            self.ctx(),
        )?;
        let rect = [r.llx(), r.lly(), r.urx(), r.ury()];
        if !rect.iter().all(|v| v.is_finite())
            || rect[2] < rect[0]
            || rect[3] < rect[1]
            || (!signature && (rect[2] == rect[0] || rect[3] == rect[1]))
            || !(rect[2] - rect[0]).is_finite()
            || !(rect[3] - rect[1]).is_finite()
        {
            return Err(invalid("invalid widget rectangle"));
        }
        Ok(FormWidget {
            handle: self.object_handle(id)?,
            page_index,
            rect,
        })
    }

    /// Inspect all field nodes without executing actions or requiring an /AP.
    pub fn form_field_tree(&self) -> PdfResult<Vec<FormFieldNode>> {
        self.field_graph()?
            .nodes
            .into_iter()
            .map(|n| {
                Ok(FormFieldNode {
                    handle: self.object_handle(n.id)?,
                    parent: n.parent.map(|id| self.object_handle(id)).transpose()?,
                    partial_name: n.partial,
                    name: n.name,
                    children: n
                        .children
                        .into_iter()
                        .map(|id| self.object_handle(id))
                        .collect::<PdfResult<_>>()?,
                    widgets: n.widgets,
                    field_type: n
                        .effective
                        .get("FT")
                        .map(|v| {
                            v.as_name()
                                .map(|n| n.name().to_owned())
                                .ok_or_else(|| invalid("field FT must be a name"))
                        })
                        .transpose()?,
                    flags: self.form_flags(&n.effective, "Ff")?,
                })
            })
            .collect()
    }
}
