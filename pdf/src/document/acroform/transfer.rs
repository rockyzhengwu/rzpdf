//! Snapshots for page operations. Keep field identity inside one ID-map, not
//! reconstructed through the flattened compatibility API.
use super::*;
use std::collections::{HashMap, HashSet};

pub(crate) struct FieldTransfer {
    pub objects: HashMap<ObjectId, PdfObject>,
    pub roots: Vec<ObjectId>,
    pub form: PdfDict,
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(crate) fn page_field_snapshot(
        &self,
        pages: &HashSet<ObjectId>,
        prefix: &str,
    ) -> PdfResult<FieldTransfer> {
        let graph = self.field_graph()?;
        if !prefix.is_empty() {
            graph::partial_name(prefix)?;
        }
        for key in ["CO", "XFA", "SigFlags"] {
            if graph.form.contains_key(key) {
                return Err(PdfError::UnsupportedFeature(format!(
                    "page transfer with AcroForm {key}"
                )));
            }
        }
        for (key, _) in &graph.form {
            if !matches!(
                key.as_str(),
                "Fields" | "DR" | "DA" | "Q" | "NeedAppearances"
            ) {
                return Err(PdfError::UnsupportedFeature(format!(
                    "page transfer with unknown AcroForm key {key}"
                )));
            }
        }
        if graph
            .form
            .get("NeedAppearances")
            .map(|v| self.ctx().resolve(v))
            .transpose()?
            .is_some_and(|v| v.as_bool().is_none_or(|b| b.value()))
        {
            return Err(PdfError::UnsupportedFeature(
                "page transfer requiring reader-generated appearances".into(),
            ));
        }
        let entries = self.collect_page_entries()?;
        let mut keep = HashSet::new();
        let mut widgets = HashSet::new();
        for node in &graph.nodes {
            let selected: Vec<_> = node
                .widgets
                .iter()
                .filter(|w| pages.contains(&entries[w.page_index as usize].page_id))
                .collect();
            if selected.is_empty() {
                continue;
            }
            self.form_node_transfer_guard(node)?;
            self.field_value(node)?;
            self.check_page_button_states(node)?;
            let flags = self.form_flags(&node.effective, "Ff")?;
            if selected.len() != node.widgets.len()
                && node
                    .effective
                    .get("FT")
                    .and_then(PdfObject::as_name)
                    .map(PdfName::name)
                    == Some("Btn")
                && (flags & (1 << 15) != 0 || node.effective.contains_key("Opt"))
            {
                return Err(PdfError::UnsupportedFeature(
                    "partial page transfer of Radio or indexed button widgets".into(),
                ));
            }
            widgets.extend(selected.iter().map(|w| w.handle.object_id()));
            let mut id = Some(node.id);
            while let Some(current) = id {
                if !keep.insert(current) {
                    break;
                }
                let ancestor = graph.node(current)?;
                self.form_node_transfer_guard(ancestor)?;
                id = ancestor.parent;
            }
        }
        let mut objects = HashMap::new();
        let mut roots = Vec::new();
        for node in &graph.nodes {
            if !keep.contains(&node.id) {
                continue;
            }
            let mut dict = node.dict.clone();
            if node.parent.is_none() {
                roots.push(node.id);
                if !prefix.is_empty() {
                    let partial = node.partial.as_ref().ok_or_else(|| {
                        PdfError::UnsupportedFeature("prefixing anonymous field roots".into())
                    })?;
                    dict.try_insert(
                        "T".into(),
                        PdfObject::PdfString(PdfString::from_text_with_limits(
                            &format!("{prefix}_{partial}"),
                            &self.ctx().limits(),
                        )?),
                    )?;
                }
                for key in ["DA", "Q"] {
                    if !dict.contains_key(key) {
                        let value = graph.form.get(key).cloned().unwrap_or(if key == "Q" {
                            number(0.)
                        } else {
                            PdfObject::PdfString(PdfString::new(b"/Helv 12 Tf 0 g".to_vec(), false))
                        });
                        dict.try_insert(key.into(), value)?;
                    }
                }
            }
            if dict.contains_key("Kids") {
                let mut kids = PdfArray::default();
                for value in &self.form_array(&dict, "Kids")? {
                    let id = graph::indirect(value)?;
                    if keep.contains(&id) || widgets.contains(&id) {
                        kids.try_add_obj(reference(id))?;
                    }
                }
                dict.try_insert("Kids".into(), PdfObject::PdfArray(kids))?;
            }
            objects.insert(node.id, PdfObject::PdfDict(dict));
        }
        Ok(FieldTransfer {
            objects,
            roots,
            form: graph.form,
        })
    }

    pub(crate) fn attach_page_fields(
        &self,
        roots: Vec<ObjectId>,
        resources: Option<PdfObject>,
    ) -> PdfResult<()> {
        if roots.is_empty() {
            return Ok(());
        }
        let root = self.ctx().get_root_owned()?;
        let mut form = root
            .get("AcroForm")
            .map(|v| self.ctx().resolve(v))
            .transpose()?
            .map(|v| {
                v.into_dict()
                    .ok_or_else(|| graph::invalid("AcroForm dictionary"))
            })
            .transpose()?
            .unwrap_or_default();
        let mut fields = self.form_array(&form, "Fields")?;
        for id in roots {
            fields.try_add_obj(reference(id))?;
        }
        form.try_insert("Fields".into(), PdfObject::PdfArray(fields))?;
        if let Some(resources) = resources {
            let incoming = self
                .ctx()
                .resolve(&resources)?
                .into_dict()
                .ok_or_else(|| graph::invalid("DR dictionary"))?;
            let mut current = form
                .get("DR")
                .map(|v| self.ctx().resolve(v))
                .transpose()?
                .map(|v| v.into_dict().ok_or_else(|| graph::invalid("DR dictionary")))
                .transpose()?
                .unwrap_or_default();
            for (kind, value) in &incoming {
                let additions = self
                    .ctx()
                    .resolve(value)?
                    .into_dict()
                    .ok_or_else(|| graph::invalid("DR resource dictionary"))?;
                let mut existing = current
                    .get(kind)
                    .map(|v| self.ctx().resolve(v))
                    .transpose()?
                    .map(|v| {
                        v.into_dict()
                            .ok_or_else(|| graph::invalid("DR resource dictionary"))
                    })
                    .transpose()?
                    .unwrap_or_default();
                for (name, resource) in &additions {
                    if let Some(old) = existing.get(name) {
                        if self.ctx().resolve(old)? != self.ctx().resolve(resource)? {
                            return Err(PdfError::UnsupportedFeature(format!(
                                "AcroForm DR resource collision: {kind}/{name}"
                            )));
                        }
                    } else {
                        existing.try_insert(name.clone(), resource.clone())?;
                    }
                }
                current.try_insert(kind.clone(), PdfObject::PdfDict(existing))?;
            }
            form.try_insert("DR".into(), PdfObject::PdfDict(current))?;
        }
        self.publish_form(form)?;
        self.field_graph()?; // Includes duplicate qualified-name rejection.
        self.form_fields()?;
        Ok(())
    }

    pub(crate) fn detach_page_fields(&self, page: u32) -> PdfResult<()> {
        let graph = self.field_graph()?;
        let mut removed = HashSet::new();
        for node in graph.nodes.iter().rev() {
            let departing: Vec<_> = node
                .widgets
                .iter()
                .filter(|w| w.page_index == page)
                .collect();
            if departing.is_empty() && !node.children.iter().any(|id| removed.contains(id)) {
                continue;
            }
            self.form_node_transfer_guard(node)?;
            if !departing.is_empty() && departing.len() < node.widgets.len() {
                let flags = self.form_flags(&node.effective, "Ff")?;
                if node
                    .effective
                    .get("FT")
                    .and_then(PdfObject::as_name)
                    .map(PdfName::name)
                    == Some("Btn")
                    && (flags & (1 << 15) != 0 || node.effective.contains_key("Opt"))
                {
                    return Err(PdfError::UnsupportedFeature(
                        "deleting part of Radio or indexed button widgets".into(),
                    ));
                }
            }
            let mut removed_here = removed.clone();
            removed_here.extend(departing.iter().map(|w| w.handle.object_id()));
            let kids = self.form_array(&node.dict, "Kids")?;
            let mut remaining = PdfArray::default();
            for value in &kids {
                if !removed_here.contains(&graph::indirect(value)?) {
                    remaining.try_add_obj(value.clone())?;
                }
            }
            if (node.widgets.len() == departing.len() && !departing.is_empty())
                || (!node.children.is_empty() && remaining.is_empty())
            {
                removed.insert(node.id);
            } else {
                let mut dict = node.dict.clone();
                dict.try_insert("Kids".into(), PdfObject::PdfArray(remaining))?;
                self.update_object(node.id, PdfObject::PdfDict(dict))?;
            }
        }
        if !removed.is_empty() {
            let mut form = graph.form;
            let mut roots = PdfArray::default();
            for value in &self.form_array(&form, "Fields")? {
                if !removed.contains(&graph::indirect(value)?) {
                    roots.try_add_obj(value.clone())?;
                }
            }
            form.try_insert("Fields".into(), PdfObject::PdfArray(roots))?;
            self.publish_form(form)?;
        }
        Ok(())
    }

    fn check_page_button_states(&self, node: &graph::FieldNode) -> PdfResult<()> {
        let kind = self.field_kind(node)?;
        if !matches!(kind, FormFieldKind::Checkbox | FormFieldKind::Radio { .. }) {
            return Ok(());
        }
        let states = self.button_states(node)?;
        let selected = node
            .effective
            .get("V")
            .and_then(PdfObject::as_name)
            .map(PdfName::name)
            .filter(|s| *s != "Off");
        let mut active = 0usize;
        for (widget, on) in node.widgets.iter().zip(&states) {
            let dict = self.form_dict(widget.handle.object_id())?;
            let state = dict
                .get("AS")
                .and_then(PdfObject::as_name)
                .map(PdfName::name)
                .unwrap_or("Off");
            let exclusive = matches!(
                kind,
                FormFieldKind::Radio {
                    in_unison: false,
                    ..
                }
            );
            if state != "Off" {
                if state != on || Some(state) != selected {
                    return Err(graph::invalid("button V/AS/AP mismatch"));
                }
                active += 1;
            }
            if !exclusive && (state != "Off") != (Some(on.as_str()) == selected) {
                return Err(graph::invalid("button widget AS disagrees with logical V"));
            }
        }
        if matches!(
            kind,
            FormFieldKind::Radio {
                in_unison: false,
                ..
            }
        ) && active != usize::from(selected.is_some())
        {
            return Err(graph::invalid("exclusive Radio selection count"));
        }
        Ok(())
    }
}
