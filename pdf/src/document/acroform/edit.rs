//! Atomic field and widget edits; graph validation precedes publication.
use super::graph::{FieldGraph, FieldNode, indirect, invalid, partial_name};
use super::*;
use crate::document::identity::ObjectHandle;
use std::collections::HashSet;

impl<R: Read + Seek> PdfDocument<R> {
    pub(super) fn publish_form(&self, form: PdfDict) -> PdfResult<()> {
        let catalog_id = self
            .ctx()
            .trailer()
            .get("Root")
            .map(indirect)
            .transpose()?
            .ok_or_else(|| invalid("Root missing"))?;
        let mut root = self.ctx().get_root_owned()?;
        if let Some(old) = root.get("AcroForm").and_then(PdfObject::as_reference) {
            self.update_object(old.id(), PdfObject::PdfDict(form))
        } else {
            let id = self.try_insert_object(PdfObject::PdfDict(form))?;
            root.try_insert("AcroForm".into(), reference(id))?;
            self.update_object(catalog_id, PdfObject::PdfDict(root))
        }
    }

    pub(super) fn form_node_guard(&self, node: &FieldNode) -> PdfResult<()> {
        self.form_node_guard_with_structure(node, false)
    }

    /// Page transactions validate and rebuild OBJR/ParentTree separately.
    pub(super) fn form_node_transfer_guard(&self, node: &FieldNode) -> PdfResult<()> {
        self.form_node_guard_with_structure(node, true)
    }

    fn form_node_guard_with_structure(
        &self,
        node: &FieldNode,
        allow_structure: bool,
    ) -> PdfResult<()> {
        if node
            .effective
            .get("FT")
            .and_then(PdfObject::as_name)
            .is_some_and(|n| !["Tx", "Btn", "Ch"].contains(&n.name()))
        {
            return Err(PdfError::UnsupportedFeature(
                "unknown or signature field editing".into(),
            ));
        }
        if self.form_flags(&node.effective, "Ff")? & 1 != 0 {
            return Err(PdfError::UnsupportedFeature("read-only form field".into()));
        }
        for key in ["A", "AA", "Lock", "SV"] {
            if node.dict.get(key).is_some() {
                return Err(PdfError::UnsupportedFeature(format!(
                    "field edit with {key}"
                )));
            }
        }
        for widget in &node.widgets {
            self.form_widget_guard_with_structure(
                &self.form_dict(widget.handle.object_id())?,
                allow_structure,
            )?;
        }
        Ok(())
    }

    fn form_widget_guard_with_structure(
        &self,
        dict: &PdfDict,
        allow_structure: bool,
    ) -> PdfResult<()> {
        if self.form_flags(dict, "F")? & (64 | 128 | 512) != 0 {
            return Err(PdfError::UnsupportedFeature(
                "read-only or locked widget".into(),
            ));
        }
        for key in ["A", "AA", "OC", "StructParent", "Popup", "IRT"] {
            if dict.get(key).is_some() && !(allow_structure && key == "StructParent") {
                return Err(PdfError::UnsupportedFeature(format!(
                    "widget edit with {key}"
                )));
            }
        }
        Ok(())
    }

    /// Structural edits do not rewrite scripts, calculation order or actions
    /// that can name fields. Reject those graphs before publishing anything.
    pub(super) fn form_reference_guard(&self, graph: &FieldGraph) -> PdfResult<()> {
        if graph.form.get("CO").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "form edit with calculation order".into(),
            ));
        }
        let root = self.ctx().get_root_owned()?;
        if root.get("AA").is_some() || root.get("OpenAction").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "form structural edit with catalog actions".into(),
            ));
        }
        if let Some(names) = root.get("Names") {
            let names = self
                .ctx()
                .resolve(names)?
                .into_dict()
                .ok_or_else(|| invalid("Names dictionary"))?;
            if names.get("JavaScript").is_some() {
                return Err(PdfError::UnsupportedFeature(
                    "form edit with JavaScript name tree".into(),
                ));
            }
        }
        // The existing outline reader rejects additional and non-GoTo actions.
        self.outline_items()?;
        for page in self.collect_page_entries()? {
            if page.page_dict.get("AA").is_some() {
                return Err(PdfError::UnsupportedFeature(
                    "form edit with page actions".into(),
                ));
            }
            for annot in &self.form_array(&page.page_dict, "Annots")? {
                let dict = self
                    .ctx()
                    .resolve(annot)?
                    .into_dict()
                    .ok_or_else(|| invalid("annotation dictionary"))?;
                if dict.get("A").is_some() || dict.get("AA").is_some() {
                    return Err(PdfError::UnsupportedFeature(
                        "form structural edit with annotation actions".into(),
                    ));
                }
                for key in ["IRT", "Popup", "Parent"] {
                    if dict
                        .get("Subtype")
                        .and_then(PdfObject::as_name)
                        .map(PdfName::name)
                        == Some("Widget")
                    {
                        continue;
                    }
                    if let Some(target) = dict.get(key).and_then(PdfObject::as_reference)
                        && graph.nodes.iter().any(|n| {
                            n.id == target.id()
                                || n.widgets
                                    .iter()
                                    .any(|w| w.handle.object_id() == target.id())
                        })
                    {
                        return Err(PdfError::UnsupportedFeature(
                            "non-field annotation relationship to form".into(),
                        ));
                    }
                }
            }
        }
        for node in &graph.nodes {
            if node.dict.get("AA").is_some() || node.dict.get("A").is_some() {
                return Err(PdfError::UnsupportedFeature(
                    "form structural edit with field actions".into(),
                ));
            }
        }
        Ok(())
    }

    pub(super) fn append_field_child(
        &self,
        form: &mut PdfDict,
        parent: Option<ObjectId>,
        id: ObjectId,
    ) -> PdfResult<()> {
        let mut owner = parent
            .map(|p| self.form_dict(p))
            .transpose()?
            .unwrap_or_else(|| form.clone());
        let key = if parent.is_some() { "Kids" } else { "Fields" };
        let mut kids = self.form_array(&owner, key)?;
        if kids.len() >= self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("form child count".into()));
        }
        kids.try_add_obj(reference(id))?;
        owner.try_insert(key.into(), PdfObject::PdfArray(kids))?;
        if let Some(parent) = parent {
            self.update_object(parent, PdfObject::PdfDict(owner))?;
        } else {
            *form = owner;
        }
        Ok(())
    }

    pub(super) fn ensure_form_path(
        &self,
        graph: &FieldGraph,
        form: &mut PdfDict,
        full: &str,
    ) -> PdfResult<Option<ObjectId>> {
        let mut parent = None;
        let mut prefix = String::new();
        let parts: Vec<_> = full.split('.').collect();
        if parts.len() > self.ctx().limits().max_depth {
            return Err(PdfError::ResourceLimit("form field depth".into()));
        }
        for part in &parts[..parts.len() - 1] {
            if !prefix.is_empty() {
                prefix.push('.');
            }
            prefix.push_str(part);
            if let Some(node) = graph.nodes.iter().find(|n| n.name == prefix) {
                self.form_node_guard(node)?;
                if !node.widgets.is_empty() || node.dict.get("Kids").is_none() {
                    return Err(invalid("field path crosses a terminal field"));
                }
                parent = Some(node.id);
            } else {
                let mut dict = PdfDict::default();
                dict.try_insert(
                    "T".into(),
                    PdfObject::PdfString(PdfString::from_text_with_limits(
                        part,
                        &self.ctx().limits(),
                    )?),
                )?;
                dict.try_insert("Kids".into(), PdfObject::PdfArray(PdfArray::default()))?;
                if let Some(parent) = parent {
                    dict.try_insert("Parent".into(), reference(parent))?;
                }
                let id = self.try_insert_object(PdfObject::PdfDict(dict))?;
                self.append_field_child(form, parent, id)?;
                parent = Some(id);
            }
        }
        Ok(parent)
    }

    /// Add a named container. Empty groups can subsequently receive fields.
    /// Addresses returned inside an outer transaction must not escape rollback.
    pub fn add_form_group(
        &self,
        parent: Option<&ObjectHandle>,
        partial: &str,
    ) -> PdfResult<ObjectHandle> {
        self.ctx().require_annotation_permission()?;
        if self.effective_version()? < super::super::save_options::PdfVersion::V1_2 {
            return Err(PdfError::UnsupportedFeature(
                "AcroForm requires PDF 1.2".into(),
            ));
        }
        partial_name(partial)?;
        let title = PdfString::from_text_with_limits(partial, &self.ctx().limits())?;
        let parent = parent.map(|p| self.validate_handle(p)).transpose()?;
        self.edit_transaction(|doc| {
            let mut graph = doc.field_graph()?;
            if let Some(parent) = parent {
                let node = graph.node(parent)?;
                doc.form_node_guard(node)?;
                if !node.widgets.is_empty() || node.dict.get("Kids").is_none() {
                    return Err(invalid("parent is not a field group"));
                }
            }
            let mut dict = PdfDict::default();
            dict.try_insert("T".into(), PdfObject::PdfString(title))?;
            dict.try_insert("Kids".into(), PdfObject::PdfArray(PdfArray::default()))?;
            if let Some(parent) = parent {
                dict.try_insert("Parent".into(), reference(parent))?;
            }
            let id = doc.try_insert_object(PdfObject::PdfDict(dict))?;
            doc.append_field_child(&mut graph.form, parent, id)?;
            doc.publish_form(graph.form)?;
            doc.field_graph()?; // collision/depth checks include the candidate
            doc.object_handle(id)
        })
    }

    /// Rename one partial name; descendant qualified names change atomically.
    pub fn rename_form_field(&self, field: &ObjectHandle, partial: &str) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        partial_name(partial)?;
        let title = PdfString::from_text_with_limits(partial, &self.ctx().limits())?;
        let id = self.validate_handle(field)?;
        self.edit_transaction(|doc| {
            let graph = doc.field_graph()?;
            doc.form_reference_guard(&graph)?;
            let node = graph.node(id)?;
            doc.form_node_guard(node)?;
            // A group rename affects all descendants, including unnamed ones.
            for descendant in &graph.nodes {
                let mut p = descendant.parent;
                while let Some(parent) = p {
                    if parent == id {
                        doc.form_node_guard(descendant)?;
                        break;
                    }
                    p = graph.node(parent)?.parent;
                }
            }
            let mut dict = node.dict.clone();
            dict.try_insert("T".into(), PdfObject::PdfString(title))?;
            doc.update_object(id, PdfObject::PdfDict(dict))?;
            doc.field_graph()?;
            Ok(())
        })
    }

    /// Add another presentation of one terminal Text/Checkbox field. A merged
    /// field is split without changing its field handle. Existing /AP R and D
    /// entries are retained on its original presentation.
    pub fn add_form_widget(
        &self,
        field: &ObjectHandle,
        page_index: u32,
        rect: [f32; 4],
    ) -> PdfResult<ObjectHandle> {
        self.add_form_widget_with_state(field, page_index, rect, None)
    }

    /// Radio requires an explicit new on-state. Button /Opt membership changes
    /// remain conservative: export-index renumbering is not implicit.
    pub fn add_form_widget_with_state(
        &self,
        field: &ObjectHandle,
        page_index: u32,
        rect: [f32; 4],
        on_state: Option<&str>,
    ) -> PdfResult<ObjectHandle> {
        self.ctx().require_annotation_permission()?;
        let id = self.validate_handle(field)?;
        let (width, height) = rect_size(rect)?;
        self.edit_transaction(|doc| {
            let graph = doc.field_graph()?;
            let node = graph.node(id)?;
            doc.form_node_guard(node)?;
            if !node.children.is_empty() || node.widgets.is_empty() {
                return Err(invalid("adding a widget requires a visible terminal field"));
            }
            // Splitting changes the page address of the old merged Widget.
            // External annotation relationships/actions must not dangle.
            doc.form_reference_guard(&graph)?;
            let value = doc.compatibility_value(node)?;
            let kind = doc.field_kind(node)?;
            if matches!(kind, FormFieldKind::Checkbox | FormFieldKind::Radio { .. })
                && node.effective.get("Opt").is_some()
            {
                return Err(PdfError::UnsupportedFeature(
                    "button Opt widget membership/index renumbering".into(),
                ));
            }
            let selected_widget = if matches!(
                kind,
                FormFieldKind::Radio {
                    in_unison: false,
                    ..
                }
            ) {
                node.widgets
                    .iter()
                    .find(|w| {
                        doc.form_dict(w.handle.object_id()).is_ok_and(|d| {
                            d.get("AS")
                                .and_then(PdfObject::as_name)
                                .is_some_and(|n| n.name() != "Off")
                        })
                    })
                    .map(|w| w.handle.object_id())
            } else {
                None
            };
            if let FormValue::Text(text) = &value {
                doc.validate_text_value(node, text)?;
            }
            let mut field = node.dict.clone();
            if node.widgets[0].handle.object_id() == id {
                // Unknown merged keys have ambiguous field/widget ownership.
                // Refuse to move them rather than guess or silently drop them.
                const WIDGET_KEYS: &[&str] = &[
                    "Type", "Subtype", "Rect", "Contents", "P", "NM", "M", "F", "AP", "AS",
                    "Border", "C", "H", "MK", "BS",
                ];
                const FIELD_KEYS: &[&str] = &[
                    "FT", "Parent", "T", "TU", "TM", "Ff", "V", "DV", "DA", "Q", "MaxLen", "Opt",
                    "TI", "I", "DS", "RV",
                ];
                if (&field).into_iter().any(|(key, _)| {
                    !WIDGET_KEYS.contains(&key.as_str()) && !FIELD_KEYS.contains(&key.as_str())
                }) {
                    return Err(PdfError::UnsupportedFeature(
                        "split merged field with unknown ownership keys".into(),
                    ));
                }
                let mut widget = PdfDict::default();
                for key in WIDGET_KEYS {
                    if let Some(value) = field.remove(key) {
                        widget.try_insert((*key).into(), value)?;
                    }
                }
                widget.try_insert("Parent".into(), reference(id))?;
                let widget_id = doc.try_insert_object(PdfObject::PdfDict(widget))?;
                let old_page = doc.page_location(node.widgets[0].page_index)?;
                let mut page_dict = old_page.page_dict;
                let mut annots = doc.form_array(&page_dict, "Annots")?;
                let index = (&annots)
                    .into_iter()
                    .position(|v| v.as_reference().map(PdfReference::id) == Some(id))
                    .ok_or_else(|| invalid("merged widget owner missing"))?;
                annots.remove(index);
                annots.insert(index, reference(widget_id))?;
                page_dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
                doc.update_object(old_page.page_id, PdfObject::PdfDict(page_dict))?;
                let mut kids = PdfArray::default();
                kids.try_add_obj(reference(widget_id))?;
                field.try_insert("Kids".into(), PdfObject::PdfArray(kids))?;
            }
            let page = doc.page_location(page_index)?;
            let mut widget = PdfDict::default();
            widget.try_insert("Type".into(), name("Annot"))?;
            widget.try_insert("Subtype".into(), name("Widget"))?;
            widget.try_insert("Parent".into(), reference(id))?;
            widget.try_insert("P".into(), reference(page.page_id))?;
            widget.try_insert("Rect".into(), PdfObject::PdfArray(numbers(&rect)?))?;
            widget.try_insert("F".into(), number(4.))?;
            match &kind {
                FormFieldKind::Checkbox | FormFieldKind::Radio { .. } => {
                    let states = doc.button_states(node)?;
                    let state = match on_state {
                        Some(s) => s,
                        None if matches!(kind, FormFieldKind::Checkbox) => &states[0],
                        None => return Err(invalid("new Radio widget needs an explicit on-state")),
                    };
                    widget.try_insert("AS".into(), name("Off"))?;
                    doc.replace_normal_appearance(
                        &mut widget,
                        doc.button_appearance(
                            width,
                            height,
                            state,
                            matches!(kind, FormFieldKind::Radio { .. }),
                        )?,
                    )?;
                }
                FormFieldKind::PushButton => {
                    let old = doc.form_dict(node.widgets[0].handle.object_id())?;
                    if let Some(mk) = old.get("MK") {
                        widget.try_insert("MK".into(), mk.clone())?;
                    }
                }
                _ => {
                    if on_state.is_some() {
                        return Err(invalid("on-state on a non-button widget"));
                    }
                }
            }
            let widget_id = doc.try_insert_object(PdfObject::PdfDict(widget))?;
            let mut kids = doc.form_array(&field, "Kids")?;
            if kids.len() >= doc.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("form widget count".into()));
            }
            kids.try_add_obj(reference(widget_id))?;
            field.try_insert("Kids".into(), PdfObject::PdfArray(kids))?;
            doc.update_object(id, PdfObject::PdfDict(field))?;
            let mut page_dict = page.page_dict;
            let mut annots = doc.form_array(&page_dict, "Annots")?;
            if annots.len() >= doc.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("form page annotations".into()));
            }
            annots.try_add_obj(reference(widget_id))?;
            page_dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
            doc.update_object(page.page_id, PdfObject::PdfDict(page_dict))?;
            let candidate = doc.field_graph()?;
            doc.apply_field_value(
                candidate.node(id)?,
                &candidate.form,
                &value,
                selected_widget,
                true,
            )?;
            doc.object_handle(widget_id)
        })
    }

    /// Remove one split presentation, leaving at least one widget. To remove
    /// the last or a merged widget, explicitly remove the field instead.
    pub fn remove_form_widget(&self, widget: &ObjectHandle) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let id = self.validate_handle(widget)?;
        self.edit_transaction(|doc| {
            let graph = doc.field_graph()?;
            doc.form_reference_guard(&graph)?;
            let node = graph
                .nodes
                .iter()
                .find(|n| n.widgets.iter().any(|w| w.handle.object_id() == id))
                .ok_or_else(|| invalid("widget not in field tree"))?;
            doc.form_node_guard(node)?;
            if node.id == id || node.widgets.len() <= 1 {
                return Err(PdfError::UnsupportedFeature(
                    "remove the field to remove its last widget".into(),
                ));
            }
            let mut value = doc.field_value(node)?;
            let kind = doc.field_kind(node)?;
            if matches!(kind, FormFieldKind::Checkbox | FormFieldKind::Radio { .. })
                && node.effective.get("Opt").is_some()
            {
                return Err(PdfError::UnsupportedFeature(
                    "button Opt widget membership/index renumbering".into(),
                ));
            }
            let mut selected_widget = None;
            if matches!(kind, FormFieldKind::Radio { .. }) {
                for widget in &node.widgets {
                    let annot = doc.form_dict(widget.handle.object_id())?;
                    if annot
                        .get("AS")
                        .and_then(PdfObject::as_name)
                        .is_some_and(|s| s.name() != "Off")
                        && widget.handle.object_id() != id
                    {
                        selected_widget = Some(widget.handle.object_id());
                    }
                }
                if selected_widget.is_none() {
                    value = FormValue::Radio(None);
                }
            }
            let widget = node
                .widgets
                .iter()
                .find(|w| w.handle.object_id() == id)
                .unwrap();
            doc.unlink_widget_page(widget)?;
            let mut field = node.dict.clone();
            let mut kids = doc.form_array(&field, "Kids")?;
            let index = (&kids)
                .into_iter()
                .position(|v| v.as_reference().map(PdfReference::id) == Some(id))
                .unwrap();
            kids.remove(index);
            field.try_insert("Kids".into(), PdfObject::PdfArray(kids))?;
            doc.update_object(node.id, PdfObject::PdfDict(field))?;
            let candidate = doc.field_graph()?;
            doc.apply_field_value(
                candidate.node(node.id)?,
                &candidate.form,
                &value,
                selected_widget,
                true,
            )?;
            Ok(())
        })
    }

    pub(super) fn unlink_widget_page(&self, widget: &FormWidget) -> PdfResult<()> {
        let page = self.page_location(widget.page_index)?;
        let mut dict = page.page_dict;
        let mut annots = self.form_array(&dict, "Annots")?;
        let index = (&annots)
            .into_iter()
            .position(|v| v.as_reference().map(PdfReference::id) == Some(widget.handle.object_id()))
            .ok_or_else(|| invalid("widget page owner disappeared"))?;
        annots.remove(index);
        dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
        self.update_object(page.page_id, PdfObject::PdfDict(dict))
    }

    /// Unlink a field subtree and every widget from the owning pages. Objects
    /// are not securely erased; unrelated dictionary keys and annotations stay.
    pub fn remove_form_field(&self, field: &ObjectHandle) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let id = self.validate_handle(field)?;
        self.edit_transaction(|doc| {
            let mut graph = doc.field_graph()?;
            doc.form_reference_guard(&graph)?;
            let node = graph.node(id)?;
            let parent = node.parent;
            let mut removing = HashSet::from([id]);
            for node in &graph.nodes {
                if node.parent.is_some_and(|p| removing.contains(&p)) {
                    removing.insert(node.id);
                }
            }
            for node in &graph.nodes {
                if removing.contains(&node.id) {
                    doc.form_node_guard(node)?;
                    for widget in &node.widgets {
                        doc.unlink_widget_page(widget)?;
                    }
                }
            }
            let mut owner = parent
                .map(|p| doc.form_dict(p))
                .transpose()?
                .unwrap_or_else(|| graph.form.clone());
            let key = if parent.is_some() { "Kids" } else { "Fields" };
            let mut children = doc.form_array(&owner, key)?;
            let index = (&children)
                .into_iter()
                .position(|v| v.as_reference().map(PdfReference::id) == Some(id))
                .ok_or_else(|| invalid("field owner disappeared"))?;
            children.remove(index);
            owner.try_insert(key.into(), PdfObject::PdfArray(children))?;
            if let Some(parent) = parent {
                doc.update_object(parent, PdfObject::PdfDict(owner))?;
            } else {
                graph.form = owner;
            }
            doc.publish_form(graph.form)?;
            doc.field_graph()?;
            Ok(())
        })
    }
}
