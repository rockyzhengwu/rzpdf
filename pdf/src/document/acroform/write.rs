use super::{
    graph::{FieldNode, invalid, partial_name},
    types::*,
    *,
};
use crate::document::{identity::ObjectHandle, save_options::PdfVersion};

fn string(text: &str, limits: &crate::parser::options::Limits) -> PdfResult<PdfObject> {
    Ok(PdfObject::PdfString(PdfString::from_text_with_limits(
        text, limits,
    )?))
}
pub(super) fn state_name(state: &str) -> PdfResult<()> {
    if state.is_empty()
        || state == "Off"
        || !state.is_ascii()
        || state.bytes().any(|b| !(32..=126).contains(&b))
    {
        return Err(invalid(
            "button on-state must be a nonempty printable ASCII name other than Off",
        ));
    }
    Ok(())
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(super) fn form_version_guard(&self, kind: &FormFieldKind, exports: bool) -> PdfResult<()> {
        let mut minimum = PdfVersion::V1_2;
        if exports || matches!(kind, FormFieldKind::Choice { multiple: true, .. }) {
            minimum = PdfVersion::V1_4;
        }
        if matches!(
            kind,
            FormFieldKind::Radio {
                in_unison: true,
                ..
            }
        ) {
            minimum = PdfVersion::V1_5;
        }
        if self.effective_version()? < minimum {
            return Err(PdfError::UnsupportedFeature(format!(
                "field feature requires PDF {minimum:?}"
            )));
        }
        Ok(())
    }

    pub(super) fn button_appearance(
        &self,
        width: f32,
        height: f32,
        state: &str,
        radio: bool,
    ) -> PdfResult<PdfDict> {
        if state.len() > self.ctx().limits().max_token_bytes {
            return Err(PdfError::ResourceLimit("button state name bytes".into()));
        }
        state_name(state)?;
        if !radio {
            let mut ap = self.checkbox_appearance(width, height)?;
            let mut states = ap.remove("N").unwrap().into_dict().unwrap();
            let on = states.remove("Yes").unwrap();
            states.try_insert(state.into(), on)?;
            ap.try_insert("N".into(), PdfObject::PdfDict(states))?;
            return Ok(ap);
        }
        let cx = width / 2.;
        let cy = height / 2.;
        let radius = width.min(height) / 2. - 1.;
        let circle = |r: f32| {
            let k = r * 0.5522848;
            format!(
                "{} {cy} m {} {} {} {} {cx} {} c {} {} {} {} {} {cy} c {} {} {} {} {cx} {} c {} {} {} {} {} {cy} c h",
                cx + r,
                cx + r,
                cy + k,
                cx + k,
                cy + r,
                cy + r,
                cx - k,
                cy + r,
                cx - r,
                cy + k,
                cx - r,
                cx - r,
                cy - k,
                cx - k,
                cy - r,
                cy - r,
                cx + k,
                cy - r,
                cx + r,
                cy - k,
                cx + r
            )
        };
        let frame = format!(
            "q 1 g 0 0 {width} {height} re f 0 G 1 w {} S",
            circle(radius)
        );
        let off =
            self.appearance_stream(width, height, format!("{frame} Q").into_bytes(), false)?;
        let on = self.appearance_stream(
            width,
            height,
            format!("{frame} 0 g {} f Q", circle(radius * 0.5)).into_bytes(),
            false,
        )?;
        let mut states = PdfDict::default();
        states.try_insert("Off".into(), reference(off))?;
        states.try_insert(state.into(), reference(on))?;
        let mut ap = PdfDict::default();
        ap.try_insert("N".into(), PdfObject::PdfDict(states))?;
        Ok(ap)
    }

    /// Unified field creation. Radio/Opt fields always use separate Widgets;
    /// a single ordinary widget may retain the v0.4 merged representation.
    pub fn add_form_field(
        &self,
        full_name: &str,
        definition: &FormFieldDefinition,
        widgets: &[FormWidgetSpec],
    ) -> PdfResult<ObjectHandle> {
        self.ctx().require_annotation_permission()?;
        self.form_version_guard(&definition.kind, definition.button_exports.is_some())?;
        let limits = self.ctx().limits();
        PdfString::from_text_with_limits(full_name, &limits)?;
        for part in full_name.split('.') {
            partial_name(part)?;
        }
        if widgets.is_empty()
            || widgets.len() > limits.max_objects
            || definition.options.len() > limits.max_objects
        {
            return Err(PdfError::ResourceLimit("field widget/option count".into()));
        }
        for widget in widgets {
            rect_size(widget.rect)?;
        }
        let buttons = matches!(
            definition.kind,
            FormFieldKind::Checkbox | FormFieldKind::Radio { .. }
        );
        if !buttons && definition.button_exports.is_some() {
            return Err(invalid("Opt export array belongs to a button field"));
        }
        if !matches!(definition.kind, FormFieldKind::Choice { .. })
            && !definition.options.is_empty()
        {
            return Err(invalid("choice options on a non-Choice field"));
        }
        if !matches!(definition.kind, FormFieldKind::Text { .. }) && definition.max_len.is_some() {
            return Err(invalid("MaxLen on a non-Text field"));
        }
        if let Some(exports) = &definition.button_exports {
            if exports.len() != widgets.len() {
                return Err(invalid("button Opt/widget count mismatch"));
            }
        }
        self.edit_transaction(|doc| {
            let graph = doc.field_graph()?;
            if graph.nodes.iter().any(|n| n.name == full_name) {
                return Err(invalid("field name already exists"));
            }
            let mut form = graph.form.clone();
            let parent = doc.ensure_form_path(&graph, &mut form, full_name)?;
            let mut field = PdfDict::default();
            field.try_insert(
                "T".into(),
                string(full_name.rsplit('.').next().unwrap(), &limits)?,
            )?;
            if let Some(parent) = parent {
                field.try_insert("Parent".into(), reference(parent))?;
            }
            let (ft, flags) = match definition.kind {
                FormFieldKind::Text { multiline } => ("Tx", if multiline { MULTILINE } else { 0 }),
                FormFieldKind::Checkbox => ("Btn", 0),
                FormFieldKind::Radio {
                    no_toggle_to_off,
                    in_unison,
                } => (
                    "Btn",
                    RADIO
                        | if no_toggle_to_off { NO_TOGGLE } else { 0 }
                        | if in_unison { UNISON } else { 0 },
                ),
                FormFieldKind::Choice {
                    combo,
                    editable,
                    multiple,
                } => (
                    "Ch",
                    if combo { COMBO } else { 0 }
                        | if editable { EDIT } else { 0 }
                        | if multiple { MULTI } else { 0 },
                ),
                FormFieldKind::PushButton => ("Btn", PUSH),
            };
            // Inherit common flags; type-specific flags come from the explicit definition.
            let mut ancestor_name = full_name.rsplit_once('.').map(|p| p.0);
            let mut inherited_flags = 0;
            while let Some(prefix) = ancestor_name {
                if let Some(node) = graph.nodes.iter().find(|n| n.name == prefix) {
                    inherited_flags = doc.form_flags(&node.effective, "Ff")?;
                    break;
                }
                ancestor_name = prefix.rsplit_once('.').map(|p| p.0);
            }
            let common = inherited_flags & 7;
            field.try_insert("FT".into(), name(ft))?;
            if flags != 0 || inherited_flags & !7 != 0 {
                field.try_insert(
                    "Ff".into(),
                    PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                        (flags | common).to_string().as_bytes(),
                    )?),
                )?;
            }
            if let Some(max) = definition.max_len {
                field.try_insert(
                    "MaxLen".into(),
                    PdfObject::PdfNumber(PdfNumber::new_from_lexeme(max.to_string().as_bytes())?),
                )?;
            }
            if !buttons {
                doc.install_form_style(&mut form, &mut field, &definition.text_style)?;
            }
            if let Some(exports) = &definition.button_exports {
                let mut array = PdfArray::default();
                for text in exports {
                    array.try_add_obj(string(text, &limits)?)?;
                }
                field.try_insert("Opt".into(), PdfObject::PdfArray(array))?;
            }
            if matches!(definition.kind, FormFieldKind::Choice { .. }) {
                let mut options = PdfArray::default();
                for option in &definition.options {
                    let mut pair = PdfArray::default();
                    pair.try_add_obj(string(&option.export, &limits)?)?;
                    pair.try_add_obj(string(&option.display, &limits)?)?;
                    options.try_add_obj(PdfObject::PdfArray(pair))?;
                }
                field.try_insert("Opt".into(), PdfObject::PdfArray(options))?;
            }
            let merged = widgets.len() == 1
                && !matches!(definition.kind, FormFieldKind::Radio { .. })
                && definition.button_exports.is_none();
            let id = doc.try_insert_object(PdfObject::PdfDict(field.clone()))?;
            let mut kids = PdfArray::default();
            for (index, spec) in widgets.iter().enumerate() {
                let page = doc.page_location(spec.page_index)?;
                let mut widget = if merged {
                    field.clone()
                } else {
                    PdfDict::default()
                };
                widget.try_insert("Type".into(), name("Annot"))?;
                widget.try_insert("Subtype".into(), name("Widget"))?;
                widget.try_insert("P".into(), reference(page.page_id))?;
                widget.try_insert("Rect".into(), PdfObject::PdfArray(numbers(&spec.rect)?))?;
                widget.try_insert("F".into(), number(4.))?;
                if !merged {
                    widget.try_insert("Parent".into(), reference(id))?;
                }
                if buttons {
                    let state = spec.on_state.as_deref().unwrap_or("Yes");
                    if matches!(definition.kind, FormFieldKind::Radio { .. })
                        && spec.on_state.is_none()
                    {
                        return Err(invalid("Radio widgets need explicit on-state names"));
                    }
                    if definition.button_exports.is_some() && state != index.to_string() {
                        return Err(invalid("button Opt requires index-named states"));
                    }
                    let (w, h) = rect_size(spec.rect)?;
                    widget.try_insert("AS".into(), name("Off"))?;
                    widget.try_insert(
                        "AP".into(),
                        PdfObject::PdfDict(doc.button_appearance(
                            w,
                            h,
                            state,
                            matches!(definition.kind, FormFieldKind::Radio { .. }),
                        )?),
                    )?;
                } else if spec.on_state.is_some() {
                    return Err(invalid("on-state on a non-button widget"));
                }
                if matches!(definition.kind, FormFieldKind::PushButton) {
                    let mut mk = PdfDict::default();
                    mk.try_insert("CA".into(), string(&definition.caption, &limits)?)?;
                    widget.try_insert("MK".into(), PdfObject::PdfDict(mk))?;
                }
                let widget_id = if merged {
                    doc.update_object(id, PdfObject::PdfDict(widget))?;
                    id
                } else {
                    doc.try_insert_object(PdfObject::PdfDict(widget))?
                };
                if !merged {
                    kids.try_add_obj(reference(widget_id))?;
                }
                let mut page_dict = page.page_dict;
                let mut annots = doc.form_array(&page_dict, "Annots")?;
                if annots.len() >= limits.max_objects {
                    return Err(PdfError::ResourceLimit("field page annotations".into()));
                }
                annots.try_add_obj(reference(widget_id))?;
                page_dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
                doc.update_object(page.page_id, PdfObject::PdfDict(page_dict))?;
            }
            if !merged {
                field.try_insert("Kids".into(), PdfObject::PdfArray(kids))?;
                doc.update_object(id, PdfObject::PdfDict(field))?;
            }
            doc.append_field_child(&mut form, parent, id)?;
            doc.publish_form(form.clone())?;
            let candidate = doc.field_graph()?;
            let node = candidate.node(id)?;
            doc.apply_field_value(node, &form, &definition.value, None, true)?;
            doc.field_graph()?;
            doc.object_handle(id)
        })
    }

    pub(super) fn apply_field_value(
        &self,
        node: &FieldNode,
        form: &PdfDict,
        value: &FormValue,
        selected_widget: Option<ObjectId>,
        initialize_push: bool,
    ) -> PdfResult<()> {
        self.form_node_guard(node)?;
        let kind = self.field_kind(node)?;
        let flags = self.form_flags(&node.effective, "Ff")?;
        if flags & ((1 << 22) | (1 << 23)) != 0 && self.effective_version()? < PdfVersion::V1_4
            || flags & (1 << 26) != 0 && self.effective_version()? < PdfVersion::V1_5
        {
            return Err(PdfError::UnsupportedFeature(
                "field flags require an explicit PDF version upgrade".into(),
            ));
        }
        self.form_version_guard(
            &kind,
            node.effective.get("Opt").is_some()
                && matches!(kind, FormFieldKind::Checkbox | FormFieldKind::Radio { .. }),
        )?;
        let budget = self.ctx().limits().operation_budget();
        let mut field = node.dict.clone();
        let mut selected = None;
        let mut choice_indices = Vec::new();
        match (&kind, value) {
            (FormFieldKind::Text { multiline }, FormValue::Text(text)) => {
                self.validate_text_value(node, text)?;
                if !multiline && (text.contains('\n') || text.contains('\r')) {
                    return Err(invalid("single-line Text cannot contain line breaks"));
                }
                field.try_insert("V".into(), string(text, &self.ctx().limits())?)?;
            }
            (FormFieldKind::Checkbox | FormFieldKind::Radio { .. }, _) => {
                let states = self.button_states(node)?;
                if matches!(value, FormValue::ButtonState(Some(s)) | FormValue::Radio(Some(s)) if s.len() > self.ctx().limits().max_token_bytes)
                {
                    return Err(PdfError::ResourceLimit("button value name bytes".into()));
                }
                selected = match (&kind, value) {
                    (FormFieldKind::Checkbox, FormValue::Checkbox(false)) => None,
                    (FormFieldKind::Checkbox, FormValue::Checkbox(true))
                        if states.iter().all(|s| s == &states[0]) =>
                    {
                        Some(states[0].clone())
                    }
                    (FormFieldKind::Checkbox, FormValue::ButtonState(state))
                    | (FormFieldKind::Radio { .. }, FormValue::Radio(state)) => state.clone(),
                    _ => return Err(invalid("button value type mismatch or ambiguous on-state")),
                };
                if selected.as_deref() == Some("Off") {
                    return Err(invalid("use None to clear a button"));
                }
                if selected.as_ref().is_some_and(|s| !states.contains(s)) {
                    return Err(invalid("button selection is not an on-state"));
                }
                if matches!(
                    kind,
                    FormFieldKind::Radio {
                        no_toggle_to_off: true,
                        ..
                    }
                ) && selected.is_none()
                {
                    return Err(invalid("NoToggleToOff Radio must retain a selection"));
                }
                if matches!(
                    kind,
                    FormFieldKind::Radio {
                        in_unison: false,
                        ..
                    }
                ) && selected
                    .as_ref()
                    .is_some_and(|s| states.iter().filter(|v| *v == s).count() > 1)
                    && selected_widget.is_none()
                {
                    return Err(PdfError::UnsupportedFeature(
                        "duplicate Radio state requires set_radio_widget".into(),
                    ));
                }
                field.try_insert("V".into(), name(selected.as_deref().unwrap_or("Off")))?;
            }
            (
                FormFieldKind::Choice { multiple, .. },
                FormValue::Choice(_) | FormValue::ChoiceIndices(_),
            ) => {
                let (indices, exports) = self.choice_selection(node, value)?;
                choice_indices = indices;
                let v = match exports.len() {
                    0 => PdfObject::PdfNull,
                    1 => string(&exports[0], &self.ctx().limits())?,
                    _ => {
                        let mut a = PdfArray::default();
                        for export in &exports {
                            a.try_add_obj(string(export, &self.ctx().limits())?)?;
                        }
                        PdfObject::PdfArray(a)
                    }
                };
                field.try_insert("V".into(), v)?;
                let options = self.choice_options(node)?;
                let ambiguous = exports
                    .iter()
                    .any(|export| options.iter().filter(|o| &o.export == export).count() > 1);
                if *multiple || ambiguous {
                    if self.effective_version()? < PdfVersion::V1_4 {
                        return Err(PdfError::UnsupportedFeature(
                            "Choice indices require PDF 1.4".into(),
                        ));
                    }
                    let mut indices = PdfArray::default();
                    for &index in &choice_indices {
                        indices.try_add_obj(PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                            index.to_string().as_bytes(),
                        )?))?;
                    }
                    field.try_insert("I".into(), PdfObject::PdfArray(indices))?;
                } else {
                    field.remove("I");
                }
            }
            (FormFieldKind::PushButton, FormValue::PushButton) if initialize_push => {
                if node.effective.get("V").is_some() || node.effective.get("DV").is_some() {
                    return Err(invalid("PushButton cannot retain V/DV"));
                }
            }
            _ => {
                return Err(invalid(
                    "form value type mismatch or PushButton has no fillable value",
                ));
            }
        }
        let context = if matches!(
            kind,
            FormFieldKind::Text { .. } | FormFieldKind::Choice { .. } | FormFieldKind::PushButton
        ) {
            Some(self.form_text_context(form, node, &budget)?)
        } else {
            None
        };
        let states = if context.is_none() {
            self.button_states(node)?
        } else {
            Vec::new()
        };
        let mut appearance_work = text::AppearanceWork::default();
        for (index, widget) in node.widgets.iter().enumerate() {
            let widget_id = widget.handle.object_id();
            let mut annot = if widget_id == node.id {
                field.clone()
            } else {
                self.form_dict(widget_id)?
            };
            let (width, height) = rect_size(widget.rect)?;
            if let Some(context) = &context {
                let (lines, multiline) = match (&kind, value) {
                    (FormFieldKind::Text { multiline }, FormValue::Text(text)) => (
                        text.replace("\r\n", "\n")
                            .replace('\r', "\n")
                            .split('\n')
                            .map(|line| (line.to_owned(), false))
                            .collect(),
                        *multiline,
                    ),
                    (
                        FormFieldKind::Choice {
                            combo, editable, ..
                        },
                        _,
                    ) => {
                        let options = self.choice_options(node)?;
                        if *combo {
                            let line = if let Some(&i) = choice_indices.first() {
                                options[i].display.clone()
                            } else if *editable {
                                match value {
                                    FormValue::Choice(v) => v.first().cloned().unwrap_or_default(),
                                    _ => String::new(),
                                }
                            } else {
                                String::new()
                            };
                            (vec![(line, false)], false)
                        } else {
                            let rows =
                                ((height - 6.) / (context.size * 1.2)).floor().max(1.) as usize;
                            let mut top = self.form_flags(&node.dict, "TI")? as usize;
                            if top > options.len() || !options.is_empty() && top == options.len() {
                                return Err(invalid("Choice TI outside Opt"));
                            }
                            if let Some(&first) = choice_indices.first() {
                                if first < top || first >= top.saturating_add(rows) {
                                    top = first;
                                }
                            }
                            field.try_insert(
                                "TI".into(),
                                PdfObject::PdfNumber(PdfNumber::new_from_lexeme(
                                    top.to_string().as_bytes(),
                                )?),
                            )?;
                            (
                                options
                                    .iter()
                                    .enumerate()
                                    .skip(top)
                                    .take(rows)
                                    .map(|(i, o)| (o.display.clone(), choice_indices.contains(&i)))
                                    .collect(),
                                true,
                            )
                        }
                    }
                    (FormFieldKind::PushButton, _) => {
                        let mk = annot
                            .get("MK")
                            .map(|v| self.ctx().resolve(v))
                            .transpose()?
                            .and_then(PdfObject::into_dict)
                            .unwrap_or_default();
                        let caption = mk
                            .get("CA")
                            .map(|v| self.field_string(v))
                            .transpose()?
                            .unwrap_or_default();
                        (vec![(caption, false)], false)
                    }
                    _ => unreachable!(),
                };
                let ap = self.field_text_appearance(
                    context,
                    width,
                    height,
                    &lines,
                    multiline,
                    &budget,
                    &mut appearance_work,
                )?;
                self.replace_normal_appearance(&mut annot, ap)?;
                annot.remove("AS");
            } else {
                // Off appearance is optional in imported button fields. Make
                // the implicit blank state explicit for the shared renderer,
                // retaining the authored on artwork and R/D dictionaries.
                let mut ap = annot
                    .get("AP")
                    .map(|v| self.ctx().resolve(v))
                    .transpose()?
                    .and_then(PdfObject::into_dict)
                    .ok_or_else(|| invalid("button AP missing"))?;
                let mut normal = ap
                    .get("N")
                    .map(|v| self.ctx().resolve(v))
                    .transpose()?
                    .and_then(PdfObject::into_dict)
                    .ok_or_else(|| invalid("button state dictionary missing"))?;
                if normal.get("Off").is_none() {
                    let off = self.normal_appearance(
                        width,
                        height,
                        PdfDict::default(),
                        b"q Q".to_vec(),
                        1.,
                        "Normal",
                    )?;
                    normal.try_insert("Off".into(), reference(off))?;
                    ap.try_insert("N".into(), PdfObject::PdfDict(normal))?;
                    annot.try_insert("AP".into(), PdfObject::PdfDict(ap))?;
                }
                let on = selected.as_ref() == Some(&states[index])
                    && selected_widget.is_none_or(|id| {
                        id == widget_id
                            || matches!(
                                kind,
                                FormFieldKind::Radio {
                                    in_unison: true,
                                    ..
                                }
                            )
                    });
                annot.try_insert("AS".into(), name(if on { &states[index] } else { "Off" }))?;
            }
            if widget_id == node.id {
                // Choice list layout may have changed /TI on the field.
                if let Some(top) = field.get("TI") {
                    annot.try_insert("TI".into(), top.clone())?;
                }
                field = annot;
            } else {
                self.update_object(widget_id, PdfObject::PdfDict(annot))?;
            }
        }
        self.update_object(node.id, PdfObject::PdfDict(field))
    }

    pub fn set_radio_widget(&self, widget: &ObjectHandle) -> PdfResult<()> {
        let id = self.validate_handle(widget)?;
        self.ctx().with_form_edit(|| {
            self.edit_transaction(|doc| {
                let graph = doc.field_graph()?;
                let node = graph
                    .nodes
                    .iter()
                    .find(|n| n.widgets.iter().any(|w| w.handle.object_id() == id))
                    .ok_or_else(|| invalid("Radio widget not in field tree"))?;
                if !matches!(doc.field_kind(node)?, FormFieldKind::Radio { .. }) {
                    return Err(invalid("not a Radio widget"));
                }
                doc.form_ancestor_guard(&graph, node)?;
                let index = node
                    .widgets
                    .iter()
                    .position(|w| w.handle.object_id() == id)
                    .unwrap();
                let value = FormValue::Radio(Some(doc.button_states(node)?[index].clone()));
                doc.apply_field_value(node, &graph.form, &value, Some(id), false)
            })
        })
    }

    pub fn set_form_text_style(
        &self,
        field: &ObjectHandle,
        style: &FormTextStyle,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let id = self.validate_handle(field)?;
        self.edit_transaction(|doc| {
            let mut graph = doc.field_graph()?;
            let node = graph.node(id)?;
            doc.form_node_guard(node)?;
            doc.form_ancestor_guard(&graph, node)?;
            let value = doc.field_value(node)?;
            if !matches!(
                doc.field_kind(node)?,
                FormFieldKind::Text { .. }
                    | FormFieldKind::Choice { .. }
                    | FormFieldKind::PushButton
            ) {
                return Err(invalid("field has no variable text appearance"));
            }
            let mut field = node.dict.clone();
            doc.install_form_style(&mut graph.form, &mut field, style)?;
            doc.update_object(id, PdfObject::PdfDict(field))?;
            doc.publish_form(graph.form)?;
            let graph = doc.field_graph()?;
            doc.apply_field_value(graph.node(id)?, &graph.form, &value, None, true)
        })
    }

    pub fn set_pushbutton_caption(&self, field: &ObjectHandle, caption: &str) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let id = self.validate_handle(field)?;
        let caption = string(caption, &self.ctx().limits())?;
        self.edit_transaction(|doc| {
            let graph = doc.field_graph()?;
            let node = graph.node(id)?;
            doc.form_node_guard(node)?;
            doc.form_ancestor_guard(&graph, node)?;
            if doc.field_kind(node)? != FormFieldKind::PushButton {
                return Err(invalid("not a PushButton field"));
            }
            for widget in &node.widgets {
                let mut annot = doc.form_dict(widget.handle.object_id())?;
                let mut mk = annot
                    .get("MK")
                    .map(|v| doc.ctx().resolve(v))
                    .transpose()?
                    .and_then(PdfObject::into_dict)
                    .unwrap_or_default();
                mk.try_insert("CA".into(), caption.clone())?;
                annot.try_insert("MK".into(), PdfObject::PdfDict(mk))?;
                doc.update_object(widget.handle.object_id(), PdfObject::PdfDict(annot))?;
            }
            let graph = doc.field_graph()?;
            doc.apply_field_value(
                graph.node(id)?,
                &graph.form,
                &FormValue::PushButton,
                None,
                true,
            )
        })
    }
}
