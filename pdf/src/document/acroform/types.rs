use super::{
    graph::{FieldNode, invalid},
    *,
};
use std::collections::HashSet;

pub(super) const MULTILINE: u32 = 1 << 12;
pub(super) const NO_TOGGLE: u32 = 1 << 14;
pub(super) const RADIO: u32 = 1 << 15;
pub(super) const PUSH: u32 = 1 << 16;
pub(super) const COMBO: u32 = 1 << 17;
pub(super) const EDIT: u32 = 1 << 18;
pub(super) const MULTI: u32 = 1 << 21;
pub(super) const UNISON: u32 = 1 << 25;

impl<R: Read + Seek> PdfDocument<R> {
    pub(super) fn field_kind(&self, node: &FieldNode) -> PdfResult<FormFieldKind> {
        let flags = self.form_flags(&node.effective, "Ff")?;
        let (kind, allowed) = match node
            .effective
            .get("FT")
            .and_then(PdfObject::as_name)
            .map(PdfName::name)
        {
            Some("Tx") => (
                FormFieldKind::Text {
                    multiline: flags & MULTILINE != 0,
                },
                7 | MULTILINE | (1 << 22) | (1 << 23),
            ),
            Some("Btn") if flags & PUSH != 0 => {
                if flags & (RADIO | NO_TOGGLE | UNISON) != 0 {
                    return Err(invalid("conflicting button flags"));
                }
                (FormFieldKind::PushButton, 7 | PUSH)
            }
            Some("Btn") if flags & RADIO != 0 => (
                FormFieldKind::Radio {
                    no_toggle_to_off: flags & NO_TOGGLE != 0,
                    in_unison: flags & UNISON != 0,
                },
                7 | RADIO | NO_TOGGLE | UNISON,
            ),
            Some("Btn") => (FormFieldKind::Checkbox, 7),
            Some("Ch") => {
                let combo = flags & COMBO != 0;
                let editable = flags & EDIT != 0;
                let multiple = flags & MULTI != 0;
                if editable && !combo
                    || combo && multiple
                    || flags & (1 << 22) != 0 && !(combo && editable)
                {
                    return Err(invalid("invalid Choice flags"));
                }
                (
                    FormFieldKind::Choice {
                        combo,
                        editable,
                        multiple,
                    },
                    7 | COMBO | EDIT | MULTI | (1 << 19) | (1 << 22) | (1 << 26),
                )
            }
            _ => {
                return Err(PdfError::UnsupportedFeature(
                    "unknown or signature field type".into(),
                ));
            }
        };
        if flags & !allowed != 0 {
            return Err(PdfError::UnsupportedFeature(
                "unsupported field flags (password, comb, rich text or other variant)".into(),
            ));
        }
        Ok(kind)
    }

    pub(super) fn field_string(&self, object: &PdfObject) -> PdfResult<String> {
        self.ctx()
            .resolve(object)?
            .as_string()
            .ok_or_else(|| invalid("field text string"))?
            .text_content_with_limits(&self.ctx().limits())
    }

    pub(super) fn choice_options(&self, node: &FieldNode) -> PdfResult<Vec<ChoiceOption>> {
        let mut result = Vec::new();
        for option in &self.form_array(&node.dict, "Opt")? {
            let option = self.ctx().resolve(option)?;
            let pair = match &option {
                PdfObject::PdfArray(a) if a.len() == 2 => ChoiceOption {
                    export: self.field_string(a.get(0).unwrap())?,
                    display: self.field_string(a.get(1).unwrap())?,
                },
                PdfObject::PdfString(_) => {
                    let text = self.field_string(&option)?;
                    ChoiceOption {
                        export: text.clone(),
                        display: text,
                    }
                }
                _ => {
                    return Err(invalid(
                        "Choice Opt must contain strings or export/display pairs",
                    ));
                }
            };
            result.push(pair);
        }
        Ok(result)
    }

    pub(super) fn button_states(&self, node: &FieldNode) -> PdfResult<Vec<String>> {
        let mut result = Vec::new();
        for widget in &node.widgets {
            let dict = self.form_dict(widget.handle.object_id())?;
            let ap = dict
                .get("AP")
                .map(|v| self.ctx().resolve(v))
                .transpose()?
                .and_then(PdfObject::into_dict)
                .ok_or_else(|| invalid("button AP dictionary missing"))?;
            let states = ap
                .get("N")
                .map(|v| self.ctx().resolve(v))
                .transpose()?
                .and_then(PdfObject::into_dict)
                .ok_or_else(|| invalid("button AP N state dictionary missing"))?;
            let on: Vec<_> = (&states)
                .into_iter()
                .filter(|(key, _)| key.as_str() != "Off")
                .collect();
            if on.len() != 1 {
                return Err(PdfError::UnsupportedFeature(
                    "button widget requires one on-state".into(),
                ));
            }
            for (_, value) in &states {
                if !matches!(self.ctx().resolve(value)?, PdfObject::PdfStream(_)) {
                    return Err(invalid("button appearance state must be a stream"));
                }
            }
            result.push(on[0].0.clone());
        }
        if let Some(exports) = node.effective.get("Opt") {
            let exports = self
                .ctx()
                .resolve(exports)?
                .into_array()
                .ok_or_else(|| invalid("button Opt array"))?;
            if exports.len() != result.len() {
                return Err(invalid("button Opt/widget count mismatch"));
            }
            for (i, value) in (&exports).into_iter().enumerate() {
                self.field_string(value)?;
                if result[i] != i.to_string() {
                    return Err(PdfError::UnsupportedFeature(
                        "button Opt requires index-named on-states".into(),
                    ));
                }
            }
        }
        Ok(result)
    }

    pub(super) fn choice_selection(
        &self,
        node: &FieldNode,
        value: &FormValue,
    ) -> PdfResult<(Vec<usize>, Vec<String>)> {
        let FormFieldKind::Choice {
            editable, multiple, ..
        } = self.field_kind(node)?
        else {
            return Err(invalid("not a choice field"));
        };
        let options = self.choice_options(node)?;
        let mut indices = Vec::new();
        let mut exports = Vec::new();
        match value {
            FormValue::Choice(values) => {
                for value in values {
                    let matches: Vec<_> = options
                        .iter()
                        .enumerate()
                        .filter(|(_, o)| &o.export == value)
                        .collect();
                    if matches.len() > 1 {
                        return Err(PdfError::UnsupportedFeature(
                            "duplicate Choice export requires ChoiceIndices".into(),
                        ));
                    }
                    if let Some((i, _)) = matches.first() {
                        indices.push(*i);
                    } else if !editable {
                        return Err(invalid("Choice value is not in Opt"));
                    }
                    exports.push(value.clone());
                }
            }
            FormValue::ChoiceIndices(values) => {
                for &i in values {
                    let i = i as usize;
                    exports.push(
                        options
                            .get(i)
                            .ok_or_else(|| invalid("Choice index out of bounds"))?
                            .export
                            .clone(),
                    );
                    indices.push(i);
                }
            }
            _ => return Err(invalid("Choice value type mismatch")),
        }
        if !multiple && exports.len() > 1 {
            return Err(invalid("single Choice cannot select multiple options"));
        }
        let mut seen = HashSet::new();
        if indices.iter().any(|i| !seen.insert(*i)) {
            return Err(invalid("duplicate selected Choice index"));
        }
        if indices.len() == exports.len() {
            let mut order: Vec<_> = indices.iter().copied().zip(exports).collect();
            order.sort_by_key(|pair| pair.0);
            (indices, exports) = order.into_iter().unzip();
        }
        Ok((indices, exports))
    }

    pub(super) fn field_value(&self, node: &FieldNode) -> PdfResult<FormValue> {
        match self.field_kind(node)? {
            FormFieldKind::Text { .. } => Ok(FormValue::Text(match node.effective.get("V") {
                None | Some(PdfObject::PdfNull) => String::new(),
                Some(PdfObject::PdfStream(stream)) => {
                    PdfString::new(stream.decode_data(self.ctx())?.to_vec(), false)
                        .text_content_with_limits(&self.ctx().limits())?
                }
                Some(value) => self.field_string(value)?,
            })),
            FormFieldKind::PushButton => {
                if node.effective.get("V").is_some() || node.effective.get("DV").is_some() {
                    return Err(invalid("PDF 1.x PushButton cannot retain V/DV"));
                }
                Ok(FormValue::PushButton)
            }
            kind @ (FormFieldKind::Checkbox | FormFieldKind::Radio { .. }) => {
                let states = self.button_states(node)?;
                let state = match node.effective.get("V") {
                    None | Some(PdfObject::PdfNull) => None,
                    Some(v) => Some(
                        v.as_name()
                            .ok_or_else(|| invalid("button V must be a name"))?
                            .name()
                            .to_owned(),
                    ),
                }
                .filter(|s| s != "Off");
                if state.as_ref().is_some_and(|s| !states.contains(s)) {
                    return Err(invalid("button V has no matching widget on-state"));
                }
                match kind {
                    FormFieldKind::Radio { .. } => Ok(FormValue::Radio(state)),
                    _ if states.iter().all(|s| s == &states[0]) => {
                        Ok(FormValue::Checkbox(state.is_some()))
                    }
                    _ => Ok(FormValue::ButtonState(state)),
                }
            }
            FormFieldKind::Choice { .. } => {
                let raw = match node.effective.get("V") {
                    None | Some(PdfObject::PdfNull) => Vec::new(),
                    Some(PdfObject::PdfArray(array)) => array
                        .into_iter()
                        .map(|v| self.field_string(v))
                        .collect::<PdfResult<Vec<_>>>()?,
                    Some(v) => vec![self.field_string(v)?],
                };
                // A consistent /I disambiguates duplicate exports. If /I
                // disagrees with /V, ISO requires /V to take precedence.
                if node.dict.get("I").is_some() {
                    let mut indices = Vec::new();
                    for v in &self.form_array(&node.dict, "I")? {
                        indices.push(
                            self.ctx()
                                .resolve(v)?
                                .as_number()
                                .ok_or_else(|| invalid("Choice I integer"))?
                                .as_u32_checked()?,
                        );
                    }
                    if indices.windows(2).any(|p| p[0] >= p[1]) {
                        return Err(invalid("Choice I must be strictly increasing"));
                    }
                    let value = FormValue::ChoiceIndices(indices);
                    if self.choice_selection(node, &value)?.1 == raw {
                        return Ok(value);
                    }
                }
                let value = FormValue::Choice(raw);
                self.choice_selection(node, &value)?;
                Ok(value)
            }
        }
    }

    pub fn form_field_details(&self) -> PdfResult<Vec<FormFieldDetails>> {
        self.field_graph()?
            .nodes
            .iter()
            .filter(|n| n.children.is_empty() && !n.widgets.is_empty())
            .map(|n| self.field_details(n))
            .collect()
    }
    fn field_details(&self, n: &FieldNode) -> PdfResult<FormFieldDetails> {
        let kind = self.field_kind(n)?;
        let buttons = matches!(kind, FormFieldKind::Checkbox | FormFieldKind::Radio { .. });
        Ok(FormFieldDetails {
            handle: self.object_handle(n.id)?,
            name: n.name.clone(),
            kind: kind.clone(),
            value: self.field_value(n)?,
            options: if matches!(kind, FormFieldKind::Choice { .. }) {
                self.choice_options(n)?
            } else {
                Vec::new()
            },
            button_states: if buttons {
                self.button_states(n)?
            } else {
                Vec::new()
            },
            button_exports: if buttons {
                n.effective
                    .get("Opt")
                    .map(|_| {
                        self.form_array(&n.effective, "Opt")?
                            .into_iter()
                            .map(|v| self.field_string(v))
                            .collect()
                    })
                    .transpose()?
            } else {
                None
            },
        })
    }

    /// Read each terminal independently; unsupported types/flags retain a record.
    /// Invalid ownership, malformed data and resource errors still fail the query.
    pub fn form_field_records(&self) -> PdfResult<Vec<FormFieldRecord>> {
        self.field_graph()?
            .nodes
            .iter()
            .filter(|n| n.children.is_empty())
            .map(|n| {
                let (details, unsupported_reason) = match self.field_details(n) {
                    Ok(value) => (Some(value), None),
                    Err(PdfError::UnsupportedFeature(reason)) => (None, Some(reason)),
                    Err(error) => return Err(error),
                };
                Ok(FormFieldRecord {
                    handle: self.object_handle(n.id)?,
                    name: n.name.clone(),
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
                    widgets: n.widgets.clone(),
                    details,
                    unsupported_reason,
                })
            })
            .collect()
    }
}
