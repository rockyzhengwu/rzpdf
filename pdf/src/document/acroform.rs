//! AcroForm field ownership, inherited attributes and widget appearances.

mod edit;
mod graph;
mod model;
mod text;
mod types;
mod write;
mod transfer;
pub use graph::{FormFieldNode, FormWidget};
pub use model::{
    ChoiceOption, FormFieldDefinition, FormFieldDetails, FormFieldKind, FormTextStyle,
    FormWidgetSpec,
};

use std::io::{Read, Seek};

use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_string::PdfString,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormValue {
    Text(String),
    Checkbox(bool),
    ButtonState(Option<String>),
    Radio(Option<String>),
    Choice(Vec<String>),
    ChoiceIndices(Vec<u32>),
    PushButton,
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormField {
    pub name: String,
    /// Compatibility projection: the first widget's current page. Use
    /// `form_field_tree` for all presentations and stable object identities.
    pub page_index: u32,
    pub value: FormValue,
}

fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn name(value: &str) -> PdfObject {
    PdfObject::PdfName(PdfName::new(value.into()))
}
fn number(value: f32) -> PdfObject {
    PdfObject::PdfNumber(PdfNumber::new(value))
}
fn numbers(values: &[f32]) -> PdfResult<PdfArray> {
    let mut result = PdfArray::default();
    for &value in values {
        result.try_add_obj(number(value))?;
    }
    Ok(result)
}
fn field_text(value: &str) -> PdfResult<()> {
    if !value.is_ascii() || value.bytes().any(|b| !(0x20..=0x7e).contains(&b)) {
        return Err(PdfError::UnsupportedFeature(
            "form appearance currently supports printable ASCII text".into(),
        ));
    }
    Ok(())
}
fn rect_size(rect: [f32; 4]) -> PdfResult<(f32, f32)> {
    if rect.iter().any(|v| !v.is_finite())
        || !(rect[2] - rect[0]).is_finite()
        || !(rect[3] - rect[1]).is_finite()
        || rect[2] - rect[0] < 18.0
        || rect[3] - rect[1] < 18.0
    {
        return Err(PdfError::DocumentError(
            "form widget rectangle is too small or invalid".into(),
        ));
    }
    Ok((rect[2] - rect[0], rect[3] - rect[1]))
}

impl<R: Read + Seek> PdfDocument<R> {
    fn compatibility_value(&self, node: &graph::FieldNode) -> PdfResult<FormValue> {
        self.field_value(node)
    }

    fn compatibility_fields(&self) -> PdfResult<Vec<(ObjectId, FormField)>> {
        let graph = self.field_graph()?;
        let mut result = Vec::new();
        let mut names = std::collections::HashSet::new();
        for node in &graph.nodes {
            if !node.children.is_empty() || node.widgets.is_empty() {
                continue;
            }
            if !names.insert(node.name.clone()) {
                return Err(PdfError::UnsupportedFeature(
                    "multiple terminal field dictionaries with one name; use form_field_tree"
                        .into(),
                ));
            }
            result.push((
                node.id,
                FormField {
                    name: node.name.clone(),
                    page_index: node.widgets[0].page_index,
                    value: self.compatibility_value(node)?,
                },
            ));
        }
        Ok(result)
    }

    pub fn form_fields(&self) -> PdfResult<Vec<FormField>> {
        Ok(self
            .compatibility_fields()?
            .into_iter()
            .map(|(_, field)| field)
            .collect())
    }

    fn validate_text_value(&self, node: &graph::FieldNode, text: &str) -> PdfResult<()> {
        PdfString::from_text_with_limits(text, &self.ctx().limits())?;
        if node.dict.get("RV").is_some() || node.dict.get("DS").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "rich/styled field text".into(),
            ));
        }
        let multiline = matches!(
            self.field_kind(node)?,
            FormFieldKind::Text { multiline: true }
        );
        if text
            .chars()
            .any(|ch| ch.is_control() && !(multiline && matches!(ch, '\n' | '\r')))
        {
            return Err(graph::invalid("unsupported field text control character"));
        }
        if let Some(maximum) = node.effective.get("MaxLen") {
            let maximum = maximum
                .as_number()
                .ok_or_else(|| graph::invalid("MaxLen integer"))?
                .as_u32_checked()?;
            if text.chars().count() > maximum as usize {
                return Err(graph::invalid("text exceeds field MaxLen"));
            }
        }
        Ok(())
    }

    fn appearance_stream(
        &self,
        width: f32,
        height: f32,
        content: Vec<u8>,
        text_font: bool,
    ) -> PdfResult<ObjectId> {
        if content.len() > self.ctx().limits().max_raw_stream_bytes
            || content.len() > self.ctx().limits().max_decoded_bytes
        {
            return Err(PdfError::ResourceLimit("form appearance bytes".into()));
        }
        let mut resources = PdfDict::default();
        if text_font {
            let mut font = PdfDict::default();
            font.try_insert("Type".into(), name("Font"))?;
            font.try_insert("Subtype".into(), name("Type1"))?;
            font.try_insert("BaseFont".into(), name("Helvetica"))?;
            let mut fonts = PdfDict::default();
            fonts.try_insert("Helv".into(), PdfObject::PdfDict(font))?;
            resources.try_insert("Font".into(), PdfObject::PdfDict(fonts))?;
        }
        self.normal_appearance(width, height, resources, content, 1., "Normal")
    }

    fn checkbox_appearance(&self, width: f32, height: f32) -> PdfResult<PdfDict> {
        let frame = format!(
            "q 1 1 1 rg 0 0 {width} {height} re f 0 0 0 RG 1 w 0.5 0.5 {} {} re S",
            width - 1.0,
            height - 1.0
        );
        let off =
            self.appearance_stream(width, height, format!("{frame} Q").into_bytes(), false)?;
        let on = self.appearance_stream(
            width,
            height,
            format!(
                "{frame} 2 w 3 3 m {} {} l S 3 {} m {} 3 l S Q",
                width - 3.0,
                height - 3.0,
                height - 3.0,
                width - 3.0
            )
            .into_bytes(),
            false,
        )?;
        let mut states = PdfDict::default();
        states.try_insert("Off".into(), reference(off))?;
        states.try_insert("Yes".into(), reference(on))?;
        let mut ap = PdfDict::default();
        ap.try_insert("N".into(), PdfObject::PdfDict(states))?;
        Ok(ap)
    }

    fn replace_normal_appearance(&self, field: &mut PdfDict, generated: PdfDict) -> PdfResult<()> {
        let normal = generated
            .get("N")
            .cloned()
            .ok_or_else(|| PdfError::DocumentError("generated normal appearance missing".into()))?;
        let mut appearance =
            match field.get("AP") {
                Some(value) => self.ctx().resolve(value)?.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("field AP must be a dictionary".into())
                })?,
                None => PdfDict::default(),
            };
        appearance.try_insert("N".into(), normal)?;
        field.try_insert("AP".into(), PdfObject::PdfDict(appearance))
    }

    pub fn add_text_field(
        &self,
        page_index: u32,
        rect: [f32; 4],
        field_name: &str,
        value: &str,
    ) -> PdfResult<()> {
        let _ = rect_size(rect)?;
        field_text(value)?;
        self.add_field(page_index, rect, field_name, FormValue::Text(value.into()))
    }
    pub fn add_checkbox_field(
        &self,
        page_index: u32,
        rect: [f32; 4],
        field_name: &str,
        checked: bool,
    ) -> PdfResult<()> {
        let _ = rect_size(rect)?;
        self.add_field(page_index, rect, field_name, FormValue::Checkbox(checked))
    }

    fn add_field(
        &self,
        page_index: u32,
        rect: [f32; 4],
        field_name: &str,
        value: FormValue,
    ) -> PdfResult<()> {
        let kind = match &value {
            FormValue::Text(_) => FormFieldKind::Text { multiline: false },
            FormValue::Checkbox(_) => FormFieldKind::Checkbox,
            _ => return Err(graph::invalid("legacy field creation type")),
        };
        self.add_form_field(
            field_name,
            &FormFieldDefinition::new(kind, value),
            &[FormWidgetSpec::new(page_index, rect)],
        )
        .map(|_| ())
    }

    pub fn set_form_value(&self, field_name: &str, value: FormValue) -> PdfResult<()> {
        self.ctx()
            .with_form_edit(|| self.set_form_value_inner(field_name, value))
    }

    fn set_form_value_inner(&self, field_name: &str, value: FormValue) -> PdfResult<()> {
        self.edit_transaction(|doc| {
            let graph = doc.field_graph()?;
            let matches: Vec<_> = graph
                .nodes
                .iter()
                .filter(|n| n.children.is_empty() && !n.widgets.is_empty() && n.name == field_name)
                .collect();
            if matches.len() != 1 {
                return Err(graph::invalid("field name missing or ambiguous"));
            }
            let node = matches[0];
            doc.form_ancestor_guard(&graph, node)?;
            doc.apply_field_value(node, &graph.form, &value, None, false)
        })
    }

    fn form_ancestor_guard(
        &self,
        graph: &graph::FieldGraph,
        node: &graph::FieldNode,
    ) -> PdfResult<()> {
        let mut parent = node.parent;
        while let Some(id) = parent {
            let ancestor = graph.node(id)?;
            if ancestor.dict.get("AA").is_some() || ancestor.dict.get("A").is_some() {
                return Err(PdfError::UnsupportedFeature(
                    "field edit with ancestor actions".into(),
                ));
            }
            parent = ancestor.parent;
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn basic_fields_roundtrip_and_fill() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_text_field(0, [10.0, 10.0, 150.0, 35.0], "name", "Alice")
            .unwrap();
        doc.add_checkbox_field(0, [10.0, 45.0, 35.0, 70.0], "ok", false)
            .unwrap();
        assert_eq!(doc.form_fields().unwrap().len(), 2);
        doc.set_form_value("name", FormValue::Text("Bob".into()))
            .unwrap();
        doc.set_form_value("ok", FormValue::Checkbox(true)).unwrap();
        let before = doc.commit_revision();
        assert!(
            doc.set_form_value("name", FormValue::Checkbox(false))
                .is_err()
        );
        assert_eq!(doc.commit_revision(), before);
        let reopened = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(
            reopened.form_fields().unwrap(),
            vec![
                FormField {
                    name: "name".into(),
                    page_index: 0,
                    value: FormValue::Text("Bob".into())
                },
                FormField {
                    name: "ok".into(),
                    page_index: 0,
                    value: FormValue::Checkbox(true)
                },
            ]
        );
    }
}
