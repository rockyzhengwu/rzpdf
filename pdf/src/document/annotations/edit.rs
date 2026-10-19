use super::{
    Annotation, AnnotationKind as Kind, AnnotationRecord, AnnotationRemoval, TextAlignment,
};
use crate::{
    document::{PdfDocument, appearance::reference, identity::ObjectHandle},
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict,
        pdf_reference::PdfReference,
    },
};
use std::{
    collections::{HashMap, HashSet},
    io::{Read, Seek},
};

fn invalid(s: &str) -> PdfError {
    PdfError::DocumentError(s.into())
}
struct Entry {
    page: u32,
    id: Option<ObjectId>,
    dict: PdfDict,
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(crate) fn validate_annotation_ownership(&self) -> PdfResult<()> {
        self.annotation_graph().map(|_| ())
    }
    fn annotation_page_entries(&self, page: u32) -> PdfResult<Vec<Entry>> {
        let location = self.page_location(page)?;
        let Some(value) = location.page_dict.get("Annots") else {
            return Ok(vec![]);
        };
        let array = self
            .ctx()
            .resolve(value)?
            .into_array()
            .ok_or_else(|| invalid("Annots must be array"))?;
        if array.len() > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("annotation count".into()));
        }
        let mut result = Vec::new();
        for value in &array {
            let dict = self
                .ctx()
                .resolve(value)?
                .into_dict()
                .ok_or_else(|| invalid("annotation must be dictionary"))?;
            if dict
                .get("P")
                .is_some_and(|p| p.as_reference().map(PdfReference::id) != Some(location.page_id))
            {
                return Err(invalid("annotation P does not match page"));
            }
            result.push(Entry {
                page,
                id: value.as_reference().map(PdfReference::id),
                dict,
            });
        }
        Ok(result)
    }

    fn annotation_graph(&self) -> PdfResult<Vec<Entry>> {
        let mut result = Vec::new();
        let mut ids = HashMap::new();
        for i in 0..self.try_page_count()? {
            let page =
                u32::try_from(i).map_err(|_| PdfError::ResourceLimit("page index".into()))?;
            for entry in self.annotation_page_entries(page)? {
                if result.len() >= self.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("annotation graph count".into()));
                }
                if let Some(id) = entry.id
                    && ids.insert(id, result.len()).is_some()
                {
                    return Err(PdfError::UnsupportedFeature(
                        "shared annotation page ownership".into(),
                    ));
                }
                result.push(entry);
            }
        }
        for entry in &result {
            let subtype = self
                .annot_name(&entry.dict, "Subtype")?
                .ok_or_else(|| invalid("annotation Subtype missing"))?;
            for key in if subtype == "Popup" {
                vec!["Parent"]
            } else {
                vec!["Popup", "IRT"]
            } {
                let Some(value) = entry.dict.get(key) else {
                    continue;
                };
                let id = value
                    .as_reference()
                    .map(PdfReference::id)
                    .ok_or_else(|| invalid("annotation relationship must be indirect"))?;
                let target = &result[*ids
                    .get(&id)
                    .ok_or_else(|| invalid("annotation relationship target missing from pages"))?];
                if target.page != entry.page {
                    return Err(PdfError::UnsupportedFeature(
                        "cross-page annotation relationship".into(),
                    ));
                }
                let own = entry.id.ok_or_else(|| {
                    PdfError::UnsupportedFeature("direct annotation relationship owner".into())
                })?;
                if own == id {
                    return Err(invalid("self annotation relationship"));
                }
                if key == "Popup" {
                    if self.annot_name(&target.dict, "Subtype")?.as_deref() != Some("Popup")
                        || target
                            .dict
                            .get("Parent")
                            .and_then(PdfObject::as_reference)
                            .map(PdfReference::id)
                            != Some(own)
                    {
                        return Err(invalid("nonreciprocal Popup relationship"));
                    }
                } else if key == "Parent"
                    && (self.annot_name(&target.dict, "Subtype")?.as_deref() == Some("Popup")
                        || target
                            .dict
                            .get("Popup")
                            .and_then(PdfObject::as_reference)
                            .map(PdfReference::id)
                            != Some(own))
                {
                    return Err(invalid("nonreciprocal Popup Parent relationship"));
                }
            }
            let mut current = entry;
            let mut seen = HashSet::new();
            while let Some(value) = current.dict.get("IRT") {
                let id = value
                    .as_reference()
                    .map(PdfReference::id)
                    .ok_or_else(|| invalid("IRT must be indirect"))?;
                if seen.len() >= self.ctx().limits().max_depth || !seen.insert(id) {
                    return Err(PdfError::ResourceLimit(
                        "annotation reply depth or cycle".into(),
                    ));
                }
                current = &result[*ids
                    .get(&id)
                    .ok_or_else(|| invalid("reply target missing"))?];
            }
        }
        Ok(result)
    }

    fn annot_name(&self, dict: &PdfDict, key: &str) -> PdfResult<Option<String>> {
        dict.get(key)
            .map(|v| {
                self.ctx()
                    .resolve(v)?
                    .as_name()
                    .map(|n| n.name().to_owned())
                    .ok_or_else(|| invalid("annotation name expected"))
            })
            .transpose()
    }
    fn annot_numbers(&self, dict: &PdfDict, key: &str) -> PdfResult<Vec<f32>> {
        let value = dict
            .get(key)
            .ok_or_else(|| invalid("annotation numeric array missing"))?;
        let array = self
            .ctx()
            .resolve(value)?
            .into_array()
            .ok_or_else(|| invalid("annotation numeric array expected"))?;
        if array.len()
            > self
                .ctx()
                .limits()
                .max_path_segments
                .saturating_mul(8)
                .min(self.ctx().limits().max_objects)
        {
            return Err(PdfError::ResourceLimit("annotation coordinates".into()));
        }
        (&array)
            .into_iter()
            .map(|v| {
                self.ctx()
                    .resolve(v)?
                    .as_f32()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| invalid("annotation coordinate must be finite"))
            })
            .collect()
    }
    fn annot_scalar(&self, dict: &PdfDict, key: &str, default: f32) -> PdfResult<f32> {
        dict.get(key)
            .map(|v| {
                self.ctx()
                    .resolve(v)?
                    .as_f32()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| invalid("annotation number expected"))
            })
            .transpose()
            .map(|v| v.unwrap_or(default))
    }
    fn annot_bool(&self, dict: &PdfDict, key: &str) -> PdfResult<bool> {
        dict.get(key)
            .map(|v| {
                self.ctx()
                    .resolve(v)?
                    .as_bool()
                    .map(|v| v.value())
                    .ok_or_else(|| invalid("annotation boolean expected"))
            })
            .transpose()
            .map(|v| v.unwrap_or(false))
    }
    fn annot_text(&self, dict: &PdfDict, key: &str) -> PdfResult<String> {
        dict.get(key)
            .map(|v| {
                self.ctx()
                    .resolve(v)?
                    .as_string()
                    .ok_or_else(|| invalid("annotation string expected"))?
                    .text_content_with_limits(&self.ctx().limits())
            })
            .transpose()
            .map(|v| v.unwrap_or_default())
    }

    fn decode_annotation(&self, dict: &PdfDict) -> PdfResult<Option<Annotation>> {
        let subtype = self
            .annot_name(dict, "Subtype")?
            .ok_or_else(|| invalid("annotation Subtype missing"))?;
        let points = |key: &str, count: usize| -> PdfResult<Vec<f32>> {
            let n = self.annot_numbers(dict, key)?;
            if n.len() != count {
                return Err(invalid("annotation coordinate count"));
            }
            Ok(n)
        };
        let vertices = || -> PdfResult<Vec<[f32; 2]>> {
            let n = self.annot_numbers(dict, "Vertices")?;
            if n.len() % 2 != 0 {
                return Err(invalid("odd Vertices"));
            }
            Ok(n.as_chunks::<2>().0.iter().map(|p| [p[0], p[1]]).collect())
        };
        let quads = || -> PdfResult<Vec<[f32; 8]>> {
            let n = self.annot_numbers(dict, "QuadPoints")?;
            if n.is_empty() || n.len() % 8 != 0 {
                return Err(invalid("invalid QuadPoints"));
            }
            Ok(n.as_chunks::<8>().0.to_vec())
        };
        let kind = match subtype.as_str() {
            "Square" => Kind::Square,
            "Circle" => Kind::Circle,
            "Text" => Kind::Text {
                icon: self
                    .annot_name(dict, "Name")?
                    .unwrap_or_else(|| "Note".into()),
                open: self.annot_bool(dict, "Open")?,
            },
            "Popup" => Kind::Popup {
                open: self.annot_bool(dict, "Open")?,
            },
            "FreeText" => {
                let da = self.annot_text(dict, "DA")?;
                let words: Vec<_> = da.split_ascii_whitespace().collect();
                let font_size = if words.len() == 3 && words[0] == "/F0" && words[2] == "Tf" {
                    words[1]
                        .parse::<f32>()
                        .map_err(|_| invalid("FreeText DA size"))?
                } else {
                    return Err(PdfError::UnsupportedFeature(
                        "FreeText typed view requires plain Courier /F0 DA".into(),
                    ));
                };
                let q = self.annot_scalar(dict, "Q", 0.)?;
                if !font_size.is_finite() || font_size <= 0. {
                    return Err(invalid("FreeText font size must be finite and positive"));
                }
                Kind::FreeText {
                    font_size,
                    alignment: match q {
                        0. => TextAlignment::Left,
                        1. => TextAlignment::Center,
                        2. => TextAlignment::Right,
                        _ => return Err(invalid("invalid FreeText alignment")),
                    },
                }
            }
            "Line" => Kind::Line {
                points: points("L", 4)?.try_into().expect("four coordinates"),
            },
            "Polygon" => Kind::Polygon {
                vertices: vertices()?,
            },
            "PolyLine" => Kind::PolyLine {
                vertices: vertices()?,
            },
            "Highlight" => Kind::Highlight { quads: quads()? },
            "Underline" => Kind::Underline { quads: quads()? },
            "Squiggly" => Kind::Squiggly { quads: quads()? },
            "StrikeOut" => Kind::StrikeOut { quads: quads()? },
            "Ink" => {
                let array = self
                    .ctx()
                    .resolve(
                        dict.get("InkList")
                            .ok_or_else(|| invalid("InkList missing"))?,
                    )?
                    .into_array()
                    .ok_or_else(|| invalid("InkList must be array"))?;
                if array.len() > self.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("InkList stroke count".into()));
                }
                let mut strokes = Vec::new();
                let mut total = 0usize;
                for v in &array {
                    let mut d = PdfDict::default();
                    d.try_insert("points".into(), v.clone())?;
                    let n = self.annot_numbers(&d, "points")?;
                    total = total
                        .checked_add(n.len() / 2)
                        .ok_or_else(|| invalid("InkList overflow"))?;
                    if n.len() % 2 != 0 || total > self.ctx().limits().max_path_segments {
                        return Err(PdfError::ResourceLimit("InkList coordinates".into()));
                    }
                    strokes.push(n.as_chunks::<2>().0.iter().map(|p| [p[0], p[1]]).collect());
                }
                Kind::Ink { strokes }
            }
            "Stamp" => Kind::Stamp {
                name: self
                    .annot_name(dict, "Name")?
                    .unwrap_or_else(|| "Draft".into()),
            },
            "FileAttachment" => {
                let id = dict
                    .get("FS")
                    .and_then(PdfObject::as_reference)
                    .map(PdfReference::id)
                    .ok_or_else(|| {
                        PdfError::UnsupportedFeature("direct/missing FileAttachment FS".into())
                    })?;
                Kind::FileAttachment {
                    file: self.object_handle(id)?,
                    icon: self
                        .annot_name(dict, "Name")?
                        .unwrap_or_else(|| "PushPin".into()),
                }
            }
            _ => return Ok(None),
        };
        let rect: [f32; 4] = points("Rect", 4)?.try_into().expect("four coordinates");
        let color = if dict.get("C").is_none() {
            [0., 0., 0.]
        } else {
            let n = self.annot_numbers(dict, "C")?;
            match n.len() {
                1 => [n[0]; 3],
                3 => n.try_into().expect("RGB"),
                _ => {
                    return Err(PdfError::UnsupportedFeature(
                        "annotation color is not Gray/RGB".into(),
                    ));
                }
            }
        };
        let interior_color = if dict.get("IC").is_some() {
            Some(points("IC", 3)?.try_into().expect("RGB"))
        } else {
            None
        };
        let flags = dict
            .get("F")
            .map(|v| {
                self.ctx()
                    .resolve(v)?
                    .as_number()
                    .ok_or_else(|| invalid("annotation F must be integer"))?
                    .as_u32_checked()
            })
            .transpose()?
            .unwrap_or(0);
        let stroke_width = if let Some(bs) = dict.get("BS") {
            let bs = self
                .ctx()
                .resolve(bs)?
                .into_dict()
                .ok_or_else(|| invalid("BS must be dictionary"))?;
            self.annot_scalar(&bs, "W", 1.)?
        } else {
            1.
        };
        Ok(Some(Annotation {
            kind,
            rect,
            color,
            interior_color,
            flags,
            opacity: self.annot_scalar(dict, "CA", 1.)?,
            stroke_width,
            contents: self.annot_text(dict, "Contents")?,
        }))
    }

    pub fn page_annotations(&self, page: u32) -> PdfResult<Vec<AnnotationRecord>> {
        let mut result = Vec::new();
        for entry in self.annotation_page_entries(page)? {
            let relation = |key: &str| -> PdfResult<Option<ObjectHandle>> {
                entry
                    .dict
                    .get(key)
                    .map(|v| {
                        self.object_handle(
                            v.as_reference()
                                .map(PdfReference::id)
                                .ok_or_else(|| invalid("annotation relation must be indirect"))?,
                        )
                    })
                    .transpose()
            };
            let subtype = self
                .annot_name(&entry.dict, "Subtype")?
                .ok_or_else(|| invalid("annotation Subtype missing"))?;
            let (annotation, unsupported_reason) = match self.decode_annotation(&entry.dict) {
                Ok(value) => (value, None),
                Err(PdfError::UnsupportedFeature(reason)) => (None, Some(reason)),
                Err(error) => return Err(error),
            };
            result.push(AnnotationRecord {
                handle: entry.id.map(|id| self.object_handle(id)).transpose()?,
                annotation,
                unsupported_reason,
                popup: relation("Popup")?,
                popup_parent: if subtype == "Popup" {
                    relation("Parent")?
                } else {
                    None
                },
                reply_to: relation("IRT")?,
                subtype,
            });
        }
        Ok(result)
    }

    /// Decode the embedded payload of a FileAttachment. Never executes or
    /// opens it. Detached objects remain readable; removal is not erasure.
    pub fn annotation_file_data(&self, handle: &ObjectHandle) -> PdfResult<Vec<u8>> {
        let dict = self
            .resolve_handle(handle)?
            .into_dict()
            .ok_or_else(|| invalid("annotation dictionary expected"))?;
        if self.annot_name(&dict, "Subtype")?.as_deref() != Some("FileAttachment") {
            return Err(invalid("annotation is not FileAttachment"));
        }
        let fs = self
            .ctx()
            .resolve(
                dict.get("FS")
                    .ok_or_else(|| invalid("FileAttachment FS missing"))?,
            )?
            .into_dict()
            .ok_or_else(|| invalid("Filespec dictionary expected"))?;
        let ef = self
            .ctx()
            .resolve(
                fs.get("EF")
                    .ok_or_else(|| invalid("embedded Filespec EF missing"))?,
            )?
            .into_dict()
            .ok_or_else(|| invalid("Filespec EF dictionary expected"))?;
        let stream = self
            .ctx()
            .resolve(crate::document::embedded_file::payload_reference(
                &ef, true,
            )?)?
            .into_stream()
            .ok_or_else(|| invalid("embedded payload must be stream"))?;
        stream.decode_data(self.ctx())
    }

    fn annotation_edit_guard(&self, entry: &Entry) -> PdfResult<()> {
        if self
            .annot_name(&entry.dict, "RT")?
            .is_some_and(|s| s != "R")
        {
            return Err(PdfError::UnsupportedFeature(
                "grouped annotation relationship editing".into(),
            ));
        }
        if self.decode_annotation(&entry.dict)?.is_none() {
            return Err(PdfError::UnsupportedFeature(
                "use Link/Widget-specific editing API".into(),
            ));
        }
        let flags = entry
            .dict
            .get("F")
            .map(|v| {
                self.ctx()
                    .resolve(v)?
                    .as_number()
                    .ok_or_else(|| invalid("annotation F integer"))?
                    .as_u32_checked()
            })
            .transpose()?
            .unwrap_or(0);
        if flags & (64 | 128 | 512) != 0 {
            return Err(PdfError::UnsupportedFeature(
                "annotation is read-only or locked".into(),
            ));
        }
        for key in [
            "AA",
            "A",
            "OC",
            "StructParent",
            "BE",
            "RD",
            "RC",
            "DS",
            "CL",
            "LE",
            "LL",
            "LLE",
            "LLO",
            "Measure",
            "IT",
            "Cap",
            "CP",
        ] {
            if entry.dict.get(key).is_some() {
                return Err(PdfError::UnsupportedFeature(format!(
                    "annotation edit with {key}"
                )));
            }
        }
        if let Some(bs) = entry.dict.get("BS") {
            let bs = self
                .ctx()
                .resolve(bs)?
                .into_dict()
                .ok_or_else(|| invalid("BS dictionary"))?;
            if self.annot_name(&bs, "S")?.is_some_and(|s| s != "S") || bs.get("D").is_some() {
                return Err(PdfError::UnsupportedFeature(
                    "non-solid annotation border".into(),
                ));
            }
        }
        if entry.dict.get("Border").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "legacy Border edit; preserve original appearance".into(),
            ));
        }
        if self.annot_name(&entry.dict, "Subtype")?.as_deref() == Some("FreeText") {
            let da = self.annot_text(&entry.dict, "DA")?;
            let words: Vec<_> = da.split_ascii_whitespace().collect();
            if words.len() != 3 || words[0] != "/F0" || words[2] != "Tf" {
                return Err(PdfError::UnsupportedFeature(
                    "editing external FreeText default appearance".into(),
                ));
            }
        }
        Ok(())
    }

    pub fn add_annotation(&self, page: u32, annotation: &Annotation) -> PdfResult<ObjectHandle> {
        self.ctx().require_annotation_permission()?;
        let minimum = if matches!(
            annotation.kind,
            Kind::Polygon { .. } | Kind::PolyLine { .. }
        ) {
            crate::document::save_options::PdfVersion::V1_5
        } else {
            crate::document::save_options::PdfVersion::V1_4
        };
        if self.effective_version()? < minimum {
            return Err(PdfError::UnsupportedFeature(
                "annotation writing requires newer PDF version".into(),
            ));
        }
        self.edit_transaction(|doc| {
            doc.annotation_graph()?;
            let location = doc.page_location(page)?;
            let mut dict = doc.encode_annotation(annotation, PdfDict::default())?;
            dict.try_insert("P".into(), reference(location.page_id))?;
            let id = doc.try_insert_object(PdfObject::PdfDict(dict))?;
            let mut annots = match location.page_dict.get("Annots") {
                Some(v) => doc
                    .ctx()
                    .resolve(v)?
                    .into_array()
                    .ok_or_else(|| invalid("Annots array"))?,
                None => PdfArray::default(),
            };
            if annots.len() >= doc.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("annotation count".into()));
            }
            annots.try_add_obj(reference(id))?;
            let mut page = location.page_dict;
            page.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
            doc.update_object(location.page_id, PdfObject::PdfDict(page))?;
            doc.object_handle(id)
        })
    }

    pub fn update_annotation(
        &self,
        page: u32,
        handle: &ObjectHandle,
        annotation: &Annotation,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let id = self.validate_handle(handle)?;
        self.edit_transaction(|doc| {
            let graph = doc.annotation_graph()?;
            let entry = graph
                .iter()
                .find(|e| e.id == Some(id) && e.page == page)
                .ok_or_else(|| invalid("annotation not owned by page"))?;
            doc.annotation_edit_guard(entry)?;
            if doc.annot_name(&entry.dict, "Subtype")?.as_deref() != Some(annotation.kind.subtype())
            {
                return Err(PdfError::UnsupportedFeature(
                    "changing annotation subtype".into(),
                ));
            }
            let dict = doc.encode_annotation(annotation, entry.dict.clone())?;
            doc.update_object(id, PdfObject::PdfDict(dict))
        })
    }

    pub fn set_annotation_reply(
        &self,
        handle: &ObjectHandle,
        target: Option<&ObjectHandle>,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        if target.is_some()
            && self.effective_version()? < crate::document::save_options::PdfVersion::V1_5
        {
            return Err(PdfError::UnsupportedFeature(
                "annotation replies require PDF 1.5".into(),
            ));
        }
        let id = self.validate_handle(handle)?;
        let target = target.map(|h| self.validate_handle(h)).transpose()?;
        self.edit_transaction(|doc| {
            let graph = doc.annotation_graph()?;
            let entry = graph
                .iter()
                .find(|e| e.id == Some(id))
                .ok_or_else(|| invalid("annotation missing"))?;
            doc.annotation_edit_guard(entry)?;
            if doc.annot_name(&entry.dict, "Subtype")?.as_deref() == Some("Popup") {
                return Err(invalid("Popup cannot be a reply"));
            }
            if let Some(target) = target {
                let other = graph
                    .iter()
                    .find(|e| e.id == Some(target))
                    .ok_or_else(|| invalid("reply target missing"))?;
                if other.page != entry.page
                    || doc.decode_annotation(&other.dict)?.is_none()
                    || doc.annot_name(&other.dict, "Subtype")?.as_deref() == Some("Popup")
                {
                    return Err(PdfError::UnsupportedFeature(
                        "reply target must be supported markup on same page".into(),
                    ));
                }
            }
            let mut dict = entry.dict.clone();
            dict.remove("IRT");
            dict.remove("RT");
            if let Some(target) = target {
                dict.try_insert("IRT".into(), reference(target))?;
                // R is the default; omitting RT keeps ordinary replies valid
                // in PDF 1.5 (the optional RT entry starts with PDF 1.6).
            }
            doc.update_object(id, PdfObject::PdfDict(dict))?;
            doc.annotation_graph()?;
            Ok(())
        })
    }

    pub fn set_annotation_popup(
        &self,
        parent: &ObjectHandle,
        popup: Option<&ObjectHandle>,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let parent = self.validate_handle(parent)?;
        let popup = popup.map(|h| self.validate_handle(h)).transpose()?;
        self.edit_transaction(|doc| {
            let graph = doc.annotation_graph()?;
            let entry = graph
                .iter()
                .find(|e| e.id == Some(parent))
                .ok_or_else(|| invalid("popup parent missing"))?;
            doc.annotation_edit_guard(entry)?;
            if doc.annot_name(&entry.dict, "Subtype")?.as_deref() == Some("Popup") {
                return Err(invalid("Popup cannot own Popup"));
            }
            if let Some(old) = entry
                .dict
                .get("Popup")
                .and_then(PdfObject::as_reference)
                .map(PdfReference::id)
            {
                let previous = graph
                    .iter()
                    .find(|e| e.id == Some(old))
                    .ok_or_else(|| invalid("Popup missing"))?;
                doc.annotation_edit_guard(previous)?;
                let mut dict = previous.dict.clone();
                dict.remove("Parent");
                doc.update_object(old, PdfObject::PdfDict(dict))?;
            }
            let mut dict = entry.dict.clone();
            dict.remove("Popup");
            if let Some(popup) = popup {
                let target = graph
                    .iter()
                    .find(|e| e.id == Some(popup))
                    .ok_or_else(|| invalid("Popup missing"))?;
                doc.annotation_edit_guard(target)?;
                if target.page != entry.page
                    || doc.annot_name(&target.dict, "Subtype")?.as_deref() != Some("Popup")
                {
                    return Err(invalid("Popup must be on parent page"));
                }
                if target
                    .dict
                    .get("Parent")
                    .and_then(PdfObject::as_reference)
                    .is_some_and(|p| p.id() != parent)
                {
                    return Err(invalid("Popup already belongs to another annotation"));
                }
                let mut target = target.dict.clone();
                target.try_insert("Parent".into(), reference(parent))?;
                doc.update_object(popup, PdfObject::PdfDict(target))?;
                dict.try_insert("Popup".into(), reference(popup))?;
            }
            doc.update_object(parent, PdfObject::PdfDict(dict))?;
            doc.annotation_graph()?;
            Ok(())
        })
    }

    /// Detach, not erase. Cascade follows owned Popups and replies, not arbitrary
    /// object references; all affected entries must be editable before any write.
    pub fn remove_annotation(
        &self,
        page: u32,
        handle: &ObjectHandle,
        policy: AnnotationRemoval,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let id = self.validate_handle(handle)?;
        self.edit_transaction(|doc| {
            let graph = doc.annotation_graph()?;
            let entry = graph
                .iter()
                .find(|e| e.id == Some(id) && e.page == page)
                .ok_or_else(|| invalid("annotation not owned by page"))?;
            doc.annotation_edit_guard(entry)?;
            let mut remove = HashSet::from([id]);
            loop {
                let old = remove.len();
                for e in &graph {
                    let related = e
                        .dict
                        .get("IRT")
                        .or_else(|| {
                            if doc.annot_name(&e.dict, "Subtype").ok().flatten().as_deref()
                                == Some("Popup")
                            {
                                e.dict.get("Parent")
                            } else {
                                None
                            }
                        })
                        .and_then(PdfObject::as_reference)
                        .is_some_and(|r| remove.contains(&r.id()));
                    if related {
                        let id =
                            e.id.ok_or_else(|| invalid("related annotation must be indirect"))?;
                        if policy == AnnotationRemoval::RejectRelated {
                            return Err(PdfError::UnsupportedFeature(
                                "annotation has replies/Popup; explicit cascade required".into(),
                            ));
                        }
                        doc.annotation_edit_guard(e)?;
                        remove.insert(id);
                    }
                }
                if remove.len() == old {
                    break;
                }
            }
            for e in &graph {
                if !e.id.is_some_and(|i| remove.contains(&i))
                    && e.dict
                        .get("Popup")
                        .and_then(PdfObject::as_reference)
                        .is_some_and(|r| remove.contains(&r.id()))
                {
                    doc.annotation_edit_guard(e)?;
                    let mut dict = e.dict.clone();
                    dict.remove("Popup");
                    doc.update_object(
                        e.id.ok_or_else(|| invalid("Popup owner must be indirect"))?,
                        PdfObject::PdfDict(dict),
                    )?;
                }
            }
            let location = doc.page_location(page)?;
            let old = doc
                .ctx()
                .resolve(
                    location
                        .page_dict
                        .get("Annots")
                        .ok_or_else(|| invalid("Annots missing"))?,
                )?
                .into_array()
                .ok_or_else(|| invalid("Annots array"))?;
            let mut annots = PdfArray::default();
            for value in &old {
                if !value
                    .as_reference()
                    .is_some_and(|r| remove.contains(&r.id()))
                {
                    annots.try_add_obj(value.clone())?;
                }
            }
            let mut dict = location.page_dict;
            dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
            doc.update_object(location.page_id, PdfObject::PdfDict(dict))?;
            doc.annotation_graph()?;
            Ok(())
        })
    }
}
