//! Limited flat AcroForm: merged text and checkbox widget fields.

use std::io::{Read, Seek};

use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream,
        pdf_string::PdfString,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormValue {
    Text(String),
    Checkbox(bool),
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FormField {
    pub name: String,
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
    if rect.iter().any(|v| !v.is_finite()) || rect[2] - rect[0] < 18.0 || rect[3] - rect[1] < 18.0 {
        return Err(PdfError::DocumentError(
            "form widget rectangle is too small or invalid".into(),
        ));
    }
    Ok((rect[2] - rect[0], rect[3] - rect[1]))
}

impl<R: Read + Seek> PdfDocument<R> {
    fn flat_fields(&self) -> PdfResult<(PdfDict, Vec<(ObjectId, FormField)>)> {
        let catalog = self.ctx().get_root_owned()?;
        let Some(value) = catalog.get("AcroForm") else {
            return Ok((PdfDict::default(), Vec::new()));
        };
        let form = self
            .ctx()
            .resolve(value)?
            .into_dict()
            .ok_or_else(|| PdfError::DocumentError("AcroForm must be a dictionary".into()))?;
        if form.get("XFA").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "XFA forms are not supported".into(),
            ));
        }
        let Some(value) = form.get("Fields") else {
            return Ok((form, Vec::new()));
        };
        let fields =
            self.ctx().resolve(value)?.into_array().ok_or_else(|| {
                PdfError::DocumentError("AcroForm Fields must be an array".into())
            })?;
        if fields.len() > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("form field count".into()));
        }
        let pages = self.collect_page_entries()?;
        let mut result = Vec::new();
        let mut names = std::collections::HashSet::new();
        for entry in &fields {
            let id = entry.as_reference().map(PdfReference::id).ok_or_else(|| {
                PdfError::UnsupportedFeature("form field must be indirect".into())
            })?;
            let field = self
                .resolve_object(id)?
                .into_dict()
                .ok_or_else(|| PdfError::DocumentError("form field must be a dictionary".into()))?;
            if field.get("Kids").is_some()
                || field.get("Parent").is_some()
                || field.get("AA").is_some()
            {
                return Err(PdfError::UnsupportedFeature(
                    "nested or scripted fields are not supported".into(),
                ));
            }
            if field
                .get("Subtype")
                .and_then(PdfObject::as_name)
                .map(PdfName::name)
                != Some("Widget")
            {
                return Err(PdfError::UnsupportedFeature(
                    "only merged Widget fields are supported".into(),
                ));
            }
            let field_name = field
                .get("T")
                .and_then(PdfObject::as_string)
                .ok_or_else(|| PdfError::DocumentError("field T missing".into()))?
                .text_content_with_limits(&self.ctx().limits())?;
            if !names.insert(field_name.clone()) {
                return Err(PdfError::DocumentError("duplicate form field name".into()));
            }
            let page_id = field
                .get("P")
                .and_then(PdfObject::as_reference)
                .map(PdfReference::id)
                .ok_or_else(|| {
                    PdfError::UnsupportedFeature("field P must be an indirect page".into())
                })?;
            let index = pages
                .iter()
                .position(|page| page.page_id == page_id)
                .ok_or_else(|| PdfError::DocumentError("field page is missing".into()))?;
            let page_annots = pages[index].page_dict.get("Annots").ok_or_else(|| {
                PdfError::DocumentError("field Widget is missing from page Annots".into())
            })?;
            let page_annots = self
                .ctx()
                .resolve(page_annots)?
                .into_array()
                .ok_or_else(|| PdfError::DocumentError("page Annots must be an array".into()))?;
            if !(&page_annots)
                .into_iter()
                .any(|value| value.as_reference().map(PdfReference::id) == Some(id))
            {
                return Err(PdfError::DocumentError(
                    "field Widget is missing from page Annots".into(),
                ));
            }
            let rect = crate::geom::rectangle::Rectangle::try_from_with_context(
                field
                    .get("Rect")
                    .ok_or_else(|| PdfError::DocumentError("field Rect missing".into()))?,
                self.ctx(),
            )?;
            rect_size([rect.llx(), rect.lly(), rect.urx(), rect.ury()])?;
            if field.get("AP").is_none() {
                return Err(PdfError::UnsupportedFeature(
                    "field appearance is missing".into(),
                ));
            }
            let flags = field
                .get("Ff")
                .map(|value| self.ctx().resolve(value))
                .transpose()?
                .map(|value| {
                    value
                        .as_number()
                        .ok_or_else(|| PdfError::DocumentError("field Ff must be a number".into()))
                        .and_then(|number| number.as_u32_checked())
                })
                .transpose()?
                .unwrap_or(0);
            if flags & !0b111 != 0 {
                return Err(PdfError::UnsupportedFeature(
                    "field flags beyond ReadOnly/Required/NoExport are not supported".into(),
                ));
            }
            let kind = field
                .get("FT")
                .and_then(PdfObject::as_name)
                .map(PdfName::name);
            let value = match kind {
                Some("Tx") => FormValue::Text(
                    field
                        .get("V")
                        .and_then(PdfObject::as_string)
                        .ok_or_else(|| PdfError::DocumentError("text field V missing".into()))?
                        .text_content_with_limits(&self.ctx().limits())?,
                ),
                Some("Btn") => FormValue::Checkbox(
                    match field
                        .get("V")
                        .and_then(PdfObject::as_name)
                        .map(PdfName::name)
                    {
                        Some("Yes") => true,
                        Some("Off") => false,
                        _ => {
                            return Err(PdfError::UnsupportedFeature(
                                "only Yes/Off checkbox states are supported".into(),
                            ));
                        }
                    },
                ),
                _ => {
                    return Err(PdfError::UnsupportedFeature(
                        "only Tx and checkbox Btn fields are supported".into(),
                    ));
                }
            };
            result.push((
                id,
                FormField {
                    name: field_name,
                    page_index: u32::try_from(index)
                        .map_err(|_| PdfError::ResourceLimit("page index".into()))?,
                    value,
                },
            ));
        }
        Ok((form, result))
    }

    pub fn form_fields(&self) -> PdfResult<Vec<FormField>> {
        Ok(self
            .flat_fields()?
            .1
            .into_iter()
            .map(|(_, field)| field)
            .collect())
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
        let mut dict = PdfDict::default();
        dict.try_insert("Type".into(), name("XObject"))?;
        dict.try_insert("Subtype".into(), name("Form"))?;
        dict.try_insert(
            "BBox".into(),
            PdfObject::PdfArray(numbers(&[0.0, 0.0, width, height])?),
        )?;
        if text_font {
            let mut font = PdfDict::default();
            font.try_insert("Type".into(), name("Font"))?;
            font.try_insert("Subtype".into(), name("Type1"))?;
            font.try_insert("BaseFont".into(), name("Helvetica"))?;
            let mut fonts = PdfDict::default();
            fonts.try_insert("Helv".into(), PdfObject::PdfDict(font))?;
            let mut resources = PdfDict::default();
            resources.try_insert("Font".into(), PdfObject::PdfDict(fonts))?;
            dict.try_insert("Resources".into(), PdfObject::PdfDict(resources))?;
        }
        self.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dict, content,
        )?))
    }

    fn text_appearance(&self, width: f32, height: f32, value: &str) -> PdfResult<PdfDict> {
        field_text(value)?;
        let mut escaped = String::new();
        for ch in value.chars() {
            if ch == '(' || ch == ')' || ch == '\\' {
                escaped.push('\\');
            }
            escaped.push(ch);
        }
        let content = format!("q 1 1 1 rg 0 0 {width} {height} re f 0 0 0 RG 1 w 0.5 0.5 {} {} re S 0 0 0 rg BT /Helv 12 Tf 3 4 Td ({escaped}) Tj ET Q", width-1.0,height-1.0).into_bytes();
        let stream_id = self.appearance_stream(width, height, content, true)?;
        let mut ap = PdfDict::default();
        ap.try_insert("N".into(), reference(stream_id))?;
        Ok(ap)
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
        self.ctx().require_annotation_permission()?;
        if self.effective_version()? < super::save_options::PdfVersion::V1_2 {
            return Err(PdfError::UnsupportedFeature(
                "AcroForm requires PDF 1.2 or later".into(),
            ));
        }
        if field_name.is_empty() {
            return Err(PdfError::DocumentError("form field name is empty".into()));
        }
        let title = PdfString::from_text_with_limits(field_name, &self.ctx().limits())?;
        self.edit_transaction(|doc| {
            let page = doc.page_location(page_index)?;
            let (mut form, fields) = doc.flat_fields()?;
            if fields.len() >= doc.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("form field count".into()));
            }
            if fields.iter().any(|(_, field)| field.name == field_name) {
                return Err(PdfError::DocumentError(
                    "form field name already exists".into(),
                ));
            }
            if matches!(&value, FormValue::Text(_)) {
                let mut resources = match form.get("DR") {
                    Some(value) => doc.ctx().resolve(value)?.into_dict().ok_or_else(|| {
                        PdfError::DocumentError("AcroForm DR must be a dictionary".into())
                    })?,
                    None => PdfDict::default(),
                };
                let mut fonts = match resources.get("Font") {
                    Some(value) => doc.ctx().resolve(value)?.into_dict().ok_or_else(|| {
                        PdfError::DocumentError("AcroForm DR Font must be a dictionary".into())
                    })?,
                    None => PdfDict::default(),
                };
                if fonts.get("Helv").is_none() {
                    let mut font = PdfDict::default();
                    font.try_insert("Type".into(), name("Font"))?;
                    font.try_insert("Subtype".into(), name("Type1"))?;
                    font.try_insert("BaseFont".into(), name("Helvetica"))?;
                    fonts.try_insert("Helv".into(), PdfObject::PdfDict(font))?;
                }
                resources.try_insert("Font".into(), PdfObject::PdfDict(fonts))?;
                form.try_insert("DR".into(), PdfObject::PdfDict(resources))?;
                if form.get("DA").is_none() {
                    form.try_insert(
                        "DA".into(),
                        PdfObject::PdfString(PdfString::new(b"/Helv 12 Tf 0 g".to_vec(), false)),
                    )?;
                }
            }
            let mut annots = match page.page_dict.get("Annots") {
                Some(value) => doc
                    .ctx()
                    .resolve(value)?
                    .into_array()
                    .ok_or_else(|| PdfError::DocumentError("Annots must be an array".into()))?,
                None => PdfArray::default(),
            };
            if annots.len() >= doc.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("annotation count".into()));
            }
            let (width, height) = rect_size(rect)?;
            let mut field = PdfDict::default();
            field.try_insert("Type".into(), name("Annot"))?;
            field.try_insert("Subtype".into(), name("Widget"))?;
            field.try_insert("T".into(), PdfObject::PdfString(title))?;
            field.try_insert("P".into(), reference(page.page_id))?;
            field.try_insert("Rect".into(), PdfObject::PdfArray(numbers(&rect)?))?;
            field.try_insert("F".into(), number(4.0))?;
            match value {
                FormValue::Text(text) => {
                    field.try_insert("FT".into(), name("Tx"))?;
                    field.try_insert(
                        "V".into(),
                        PdfObject::PdfString(PdfString::from_text_with_limits(
                            &text,
                            &doc.ctx().limits(),
                        )?),
                    )?;
                    field.try_insert(
                        "DA".into(),
                        PdfObject::PdfString(PdfString::new(b"/Helv 12 Tf 0 g".to_vec(), false)),
                    )?;
                    doc.replace_normal_appearance(
                        &mut field,
                        doc.text_appearance(width, height, &text)?,
                    )?;
                }
                FormValue::Checkbox(checked) => {
                    field.try_insert("FT".into(), name("Btn"))?;
                    field.try_insert("V".into(), name(if checked { "Yes" } else { "Off" }))?;
                    field.try_insert("AS".into(), name(if checked { "Yes" } else { "Off" }))?;
                    doc.replace_normal_appearance(
                        &mut field,
                        doc.checkbox_appearance(width, height)?,
                    )?;
                }
            }
            let id = doc.try_insert_object(PdfObject::PdfDict(field))?;
            annots.try_add_obj(reference(id))?;
            let mut page_dict = page.page_dict;
            page_dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
            doc.update_object(page.page_id, PdfObject::PdfDict(page_dict))?;
            let mut field_ids = match form.get("Fields") {
                Some(value) => doc
                    .ctx()
                    .resolve(value)?
                    .into_array()
                    .ok_or_else(|| PdfError::DocumentError("Fields must be an array".into()))?,
                None => PdfArray::default(),
            };
            field_ids.try_add_obj(reference(id))?;
            form.try_insert("Fields".into(), PdfObject::PdfArray(field_ids))?;
            let form_id = doc.try_insert_object(PdfObject::PdfDict(form))?;
            let catalog_id = doc
                .ctx()
                .trailer()
                .get("Root")
                .and_then(PdfObject::as_reference)
                .map(PdfReference::id)
                .ok_or_else(|| PdfError::DocumentError("Root must be indirect".into()))?;
            let mut catalog = doc.ctx().get_root_owned()?;
            catalog.try_insert("AcroForm".into(), reference(form_id))?;
            doc.update_object(catalog_id, PdfObject::PdfDict(catalog))
        })
    }

    pub fn set_form_value(&self, field_name: &str, value: FormValue) -> PdfResult<()> {
        self.ctx()
            .with_form_edit(|| self.set_form_value_inner(field_name, value))
    }

    fn set_form_value_inner(&self, field_name: &str, value: FormValue) -> PdfResult<()> {
        if self.effective_version()? < super::save_options::PdfVersion::V1_2 {
            return Err(PdfError::UnsupportedFeature(
                "AcroForm requires PDF 1.2 or later".into(),
            ));
        }
        self.edit_transaction(|doc| {
            let (_, fields) = doc.flat_fields()?;
            let (id, field_info) = fields
                .into_iter()
                .find(|(_, field)| field.name == field_name)
                .ok_or_else(|| PdfError::DocumentError("form field not found".into()))?;
            let mut field = doc
                .resolve_object(id)?
                .into_dict()
                .ok_or_else(|| PdfError::DocumentError("form field must be dictionary".into()))?;
            let flags = field
                .get("Ff")
                .map(|value| doc.ctx().resolve(value))
                .transpose()?
                .map(|value| {
                    value
                        .as_number()
                        .ok_or_else(|| PdfError::DocumentError("field Ff must be a number".into()))
                        .and_then(|number| number.as_u32_checked())
                })
                .transpose()?
                .unwrap_or(0);
            if flags & 1 != 0 {
                return Err(PdfError::UnsupportedFeature(
                    "read-only form field cannot be filled".into(),
                ));
            }
            let rect = crate::geom::rectangle::Rectangle::try_from_with_context(
                field
                    .get("Rect")
                    .ok_or_else(|| PdfError::DocumentError("field Rect missing".into()))?,
                doc.ctx(),
            )?;
            let (width, height) = rect_size([rect.llx(), rect.lly(), rect.urx(), rect.ury()])?;
            match (field_info.value, value) {
                (FormValue::Text(_), FormValue::Text(text)) => {
                    field_text(&text)?;
                    if let Some(maximum) = field.get("MaxLen") {
                        let maximum = doc.ctx().resolve(maximum)?;
                        let maximum = maximum
                            .as_number()
                            .ok_or_else(|| {
                                PdfError::DocumentError("text field MaxLen must be a number".into())
                            })?
                            .as_u32_checked()?;
                        if text.len() > maximum as usize {
                            return Err(PdfError::DocumentError(
                                "text exceeds field MaxLen".into(),
                            ));
                        }
                    }
                    field.try_insert(
                        "V".into(),
                        PdfObject::PdfString(PdfString::from_text_with_limits(
                            &text,
                            &doc.ctx().limits(),
                        )?),
                    )?;
                    doc.replace_normal_appearance(
                        &mut field,
                        doc.text_appearance(width, height, &text)?,
                    )?;
                }
                (FormValue::Checkbox(_), FormValue::Checkbox(checked)) => {
                    field.try_insert("V".into(), name(if checked { "Yes" } else { "Off" }))?;
                    field.try_insert("AS".into(), name(if checked { "Yes" } else { "Off" }))?;
                    doc.replace_normal_appearance(
                        &mut field,
                        doc.checkbox_appearance(width, height)?,
                    )?;
                }
                _ => {
                    return Err(PdfError::UnsupportedFeature(
                        "form value type mismatch".into(),
                    ));
                }
            }
            doc.update_object(id, PdfObject::PdfDict(field))
        })
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
