use super::{PdfDocument, identity::ObjectHandle, resource_manager::ResourceKind};
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict,
        pdf_reference::PdfReference, pdf_stream::PdfStream,
    },
};
use std::{
    collections::HashMap,
    io::{Read, Seek},
};

struct Importer<'a, S: Read + Seek, T: Read + Seek> {
    source: &'a PdfDocument<S>,
    target: &'a PdfDocument<T>,
    mapped: HashMap<ObjectId, ObjectId>,
    nodes: usize,
    retained_bytes: usize,
}

impl<S: Read + Seek, T: Read + Seek> Importer<'_, S, T> {
    fn charge_bytes(&mut self, size: usize, token: bool) -> PdfResult<()> {
        let limits = self.target.ctx.limits();
        if (token && size > limits.max_token_bytes)
            || self
                .retained_bytes
                .checked_add(size)
                .is_none_or(|total| total > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit(
                "Imported token or cumulative payload bytes".into(),
            ));
        }
        self.retained_bytes += size;
        Ok(())
    }
    fn reference(&mut self, id: ObjectId, depth: usize) -> PdfResult<ObjectId> {
        if let Some(&mapped) = self.mapped.get(&id) {
            return Ok(mapped);
        }
        self.mapped
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("Imported object map allocation".into()))?;
        let object = self.source.resolve_object(id)?;
        let mapped = self.target.try_insert_object(PdfObject::PdfNull)?;
        self.mapped.insert(id, mapped);
        let object = self.object(object, depth)?;
        self.target.update_object(mapped, object)?;
        Ok(mapped)
    }
    fn dictionary(&mut self, dict: PdfDict, depth: usize) -> PdfResult<PdfDict> {
        let kind = dict
            .get("Type")
            .map(|value| self.source.ctx.resolve_owned(value))
            .transpose()?;
        if kind
            .as_ref()
            .and_then(PdfObject::as_name)
            .is_some_and(|name| matches!(name.name(), "Page" | "Pages" | "Catalog"))
        {
            return Err(PdfError::UnsupportedFeature(
                "Resource graph refers to document/page ownership".into(),
            ));
        }
        let mut output = PdfDict::default();
        for (key, value) in &dict {
            self.charge_bytes(key.len(), true)?;
            output.try_insert(key.clone(), self.object(value.clone(), depth + 1)?)?;
        }
        Ok(output)
    }
    fn object(&mut self, object: PdfObject, depth: usize) -> PdfResult<PdfObject> {
        let limits = self.target.ctx.limits();
        if depth > limits.max_depth.min(self.source.ctx.limits().max_depth)
            || self.nodes >= limits.max_objects
        {
            return Err(PdfError::ResourceLimit(
                "Imported graph depth or nodes".into(),
            ));
        }
        self.nodes += 1;
        let token_bytes = match &object {
            PdfObject::PdfString(value) => value.bytes().len(),
            PdfObject::PdfName(value) => value.bytes().len(),
            PdfObject::PdfNumber(value) => value.lexeme().len(),
            _ => 0,
        };
        self.charge_bytes(token_bytes, true)?;
        match object {
            PdfObject::PdfReference(reference) => Ok(PdfObject::PdfReference(
                PdfReference::from_id(self.reference(reference.id(), depth + 1)?),
            )),
            PdfObject::PdfDict(dict) => Ok(PdfObject::PdfDict(self.dictionary(dict, depth)?)),
            PdfObject::PdfArray(array) => {
                let mut output = PdfArray::default();
                for value in &array {
                    output.try_add_obj(self.object(value.clone(), depth + 1)?)?;
                }
                Ok(PdfObject::PdfArray(output))
            }
            PdfObject::PdfStream(stream) => {
                let (dict, data) = stream.into_raw_parts();
                if ["F", "FFilter", "FDecodeParms"]
                    .iter()
                    .any(|key| dict.contains_key(key))
                {
                    return Err(PdfError::UnsupportedFeature(
                        "Import of external-file streams".into(),
                    ));
                }
                if data.len() > limits.max_raw_stream_bytes {
                    return Err(PdfError::ResourceLimit(
                        "Imported cumulative stream bytes".into(),
                    ));
                }
                self.charge_bytes(data.len(), false)?;
                Ok(PdfObject::PdfStream(PdfStream::new(
                    self.dictionary(dict, depth)?,
                    data,
                )))
            }
            value => Ok(value),
        }
    }
}

impl<T: Read + Seek> PdfDocument<T> {
    /// Append all source pages atomically. Catalog metadata/outlines are not merged.
    pub fn merge_document<S: Read + Seek>(
        &self,
        source: &PdfDocument<S>,
    ) -> PdfResult<Vec<ObjectHandle>> {
        let count = source.try_page_count()?;
        let insertion = u32::try_from(self.try_page_count()?)
            .map_err(|_| PdfError::ResourceLimit("Merge insertion index".into()))?;
        if count > self.ctx.limits().max_objects || count > u32::MAX as usize {
            return Err(PdfError::ResourceLimit("Merge page selection".into()));
        }
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("Merge selection allocation".into()))?;
        indices.extend(0..count as u32);
        self.import_pages_from(source, &indices, insertion)
    }

    /// Extract a unique ordered page selection, keeping the source effective
    /// version and ParseOptions. Does not copy Info/XMP, outlines or named targets.
    pub fn extract_pages(
        &self,
        indices: &[u32],
    ) -> PdfResult<PdfDocument<std::io::Cursor<Vec<u8>>>> {
        let result = PdfDocument::new_with_version_and_options(
            self.effective_version()?,
            self.ctx.options(),
        )?;
        result.import_pages_from(self, indices, 0)?;
        Ok(result)
    }

    /// Import a unique ordered selection in one transaction and shared ID map.
    /// Cross-page references to selected pages are remapped; other pages reject.
    /// Use copy_page for same-document copying. Annotation/tag restrictions match
    /// import_page_from; outlines and named destination trees are not imported.
    pub fn import_pages_from<S: Read + Seek>(
        &self,
        source: &PdfDocument<S>,
        indices: &[u32],
        insertion_index: u32,
    ) -> PdfResult<Vec<ObjectHandle>> {
        if insertion_index as usize > self.try_page_count()? {
            return Err(PdfError::PageNotExist);
        }
        if indices.is_empty() {
            return Ok(Vec::new());
        }
        if self.identity() == source.identity() {
            return Err(PdfError::UnsupportedFeature(
                "Batch import requires a distinct source document; use copy_page".into(),
            ));
        }
        if source
            .ctx
            .trailer()
            .get("Encrypt")
            .is_some_and(|value| !matches!(value, PdfObject::PdfNull))
            || source.effective_version()? > self.effective_version()?
        {
            return Err(PdfError::UnsupportedFeature(
                "Page import from encrypted or newer-version source".into(),
            ));
        }
        let delta = i32::try_from(indices.len())
            .map_err(|_| PdfError::ResourceLimit("Imported page count".into()))?;
        if indices.len() > self.ctx.limits().max_objects {
            return Err(PdfError::ResourceLimit("Imported page selection".into()));
        }
        let entries = source.collect_page_entries()?;
        let mut selected = std::collections::HashSet::new();
        selected
            .try_reserve(indices.len())
            .map_err(|_| PdfError::ResourceLimit("Imported page selection allocation".into()))?;
        let mut prepared = Vec::new();
        prepared
            .try_reserve_exact(indices.len())
            .map_err(|_| PdfError::ResourceLimit("Imported page snapshots".into()))?;
        for &index in indices {
            let location = entries.get(index as usize).ok_or(PdfError::PageNotExist)?;
            if !selected.insert(location.page_id) {
                return Err(PdfError::DocumentError(
                    "Duplicate page in import selection".into(),
                ));
            }
            for key in ["StructParents", "B"] {
                if location.page_dict.contains_key(key) {
                    return Err(PdfError::UnsupportedFeature(format!(
                        "Page import with {key} ownership"
                    )));
                }
            }
            if let Some(value) = location.page_dict.get("Annots") {
                match source.ctx.resolve_owned(value)? {
                    PdfObject::PdfNull => {}
                    PdfObject::PdfArray(array) if array.len() == 0 => {}
                    _ => {
                        return Err(PdfError::UnsupportedFeature(
                            "Page import with annotations".into(),
                        ));
                    }
                }
            }
            let mut page = location.page_dict.clone();
            let node = PdfObject::PdfReference(PdfReference::from_id(location.page_id));
            for key in ["Resources", "MediaBox", "CropBox", "Rotate"] {
                let value =
                    match super::page_tree::PageTree::inherited_attribute(&source.ctx, &node, key)?
                    {
                        Some(value) => value,
                        None => match key {
                            "MediaBox" => {
                                return Err(PdfError::DocumentError(
                                    "Imported page has no MediaBox".into(),
                                ));
                            }
                            "CropBox" => page.get("MediaBox").cloned().ok_or_else(|| {
                                PdfError::DocumentError("Imported page has no MediaBox".into())
                            })?,
                            "Rotate" => PdfObject::PdfNumber(
                                crate::objects::pdf_number::PdfNumber::new(0.0),
                            ),
                            _ => PdfObject::PdfDict(PdfDict::default()),
                        },
                    };
                page.try_insert(key.into(), value)?;
            }
            for key in ["Type", "Parent", "Annots"] {
                page.remove(key);
            }
            prepared.push((location.page_id, page));
        }
        let insertion = self.page_insertion_point(insertion_index)?;
        self.edit_transaction(|target| {
            let mut importer = Importer {
                source,
                target,
                mapped: HashMap::new(),
                nodes: 0,
                retained_bytes: 0,
            };
            importer
                .mapped
                .try_reserve(prepared.len())
                .map_err(|_| PdfError::ResourceLimit("Page import map allocation".into()))?;
            let mut handles = Vec::new();
            handles
                .try_reserve_exact(prepared.len())
                .map_err(|_| PdfError::ResourceLimit("Page import handles allocation".into()))?;
            for (source_id, _) in &prepared {
                let id = target.try_insert_object(PdfObject::PdfNull)?;
                importer.mapped.insert(*source_id, id);
                handles.push(target.object_handle(id)?);
            }
            for (offset, ((_, page), handle)) in prepared.into_iter().zip(&handles).enumerate() {
                let mut page = importer
                    .object(PdfObject::PdfDict(page), 0)?
                    .into_dict()
                    .ok_or_else(|| {
                        PdfError::DocumentError("Imported page is not a dictionary".into())
                    })?;
                page.try_insert(
                    "Type".into(),
                    PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("Page".into())),
                )?;
                page.try_insert(
                    "Parent".into(),
                    PdfObject::PdfReference(PdfReference::from_id(insertion.parent_id)),
                )?;
                target.update_object(handle.object_id(), PdfObject::PdfDict(page))?;
                let slot = insertion
                    .kid_index
                    .checked_add(offset)
                    .ok_or_else(|| PdfError::ResourceLimit("Imported page slot".into()))?;
                target.update_pages_kids(
                    insertion.parent_id,
                    slot,
                    PdfObject::PdfReference(PdfReference::from_id(handle.object_id())),
                    false,
                )?;
            }
            target.adjust_page_counts(&insertion.ancestor_ids, delta)?;
            Ok(handles)
        })
    }

    /// Import one unannotated, untagged page and its content/resource graph.
    /// The source is unchanged. Does not import outlines, labels or structure trees.
    pub fn import_page_from<S: Read + Seek>(
        &self,
        source: &PdfDocument<S>,
        source_index: u32,
        insertion_index: u32,
    ) -> PdfResult<ObjectHandle> {
        if self.identity() == source.identity() {
            return self.object_handle(self.copy_page(source_index, insertion_index)?);
        }
        let mut handles = self.import_pages_from(source, &[source_index], insertion_index)?;
        handles
            .pop()
            .ok_or_else(|| PdfError::DocumentError("Single page import returned no handle".into()))
    }

    /// Import one font/image/Form resource and its reachable graph. IDs are remapped
    /// once per call, preserving sharing and cycles; later calls are not deduplicated.
    /// Does not bind a page resource name; use resources().register_handle afterward.
    pub fn import_resource_from<S: Read + Seek>(
        &self,
        source: &PdfDocument<S>,
        handle: &ObjectHandle,
        kind: ResourceKind,
    ) -> PdfResult<ObjectHandle> {
        let id = source.validate_handle(handle)?;
        if source
            .ctx
            .trailer()
            .get("Encrypt")
            .is_some_and(|value| !matches!(value, PdfObject::PdfNull))
        {
            return Err(PdfError::UnsupportedFeature(
                "Resource import from encrypted document".into(),
            ));
        }
        if source.effective_version()? > self.effective_version()? {
            return Err(PdfError::UnsupportedFeature(
                "Import into an older effective PDF version".into(),
            ));
        }
        let object = source.resolve_object(id)?;
        let dict = || match &object {
            PdfObject::PdfDict(dict) => Ok(dict),
            PdfObject::PdfStream(stream) => Ok(stream.dict()),
            _ => Err(PdfError::ObjectError(
                "Imported resource must be dictionary or stream".into(),
            )),
        };
        let valid = match kind {
            ResourceKind::Shading => {
                crate::color::shading::Shading::try_new(&object, source.ctx())?;
                true
            }
            ResourceKind::ColorSpace => {
                crate::color::colorspace::ColorSpace::try_new(&object, source.ctx())?;
                true
            }
            ResourceKind::Form => {
                dict()?;
                crate::page::pdf_form::PdfForm::try_new(&object, source.ctx())?;
                true
            }
            ResourceKind::Font => dict()?
                .get("Type")
                .and_then(PdfObject::as_name)
                .is_some_and(|name| name.name() == "Font"),
            ResourceKind::Image => dict()?
                .get("Subtype")
                .and_then(PdfObject::as_name)
                .is_some_and(|name| name.name() == "Image"),
        };
        if !valid {
            return Err(PdfError::ObjectError(
                "Imported resource kind mismatch".into(),
            ));
        }
        if self.identity() == source.identity() {
            return self.object_handle(id);
        }
        self.edit_transaction(|target| {
            let mut importer = Importer {
                source,
                target,
                mapped: HashMap::new(),
                nodes: 0,
                retained_bytes: 0,
            };
            let imported = importer.reference(id, 0)?;
            target.object_handle(imported)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::objects::pdf_name::PdfName;
    #[test]
    fn extraction_preserves_header_and_catalog_effective_versions() {
        use super::super::save_options::{PdfVersion, SaveOptions};
        for version in [PdfVersion::V1_4, PdfVersion::V2_0] {
            let source = PdfDocument::new_with_version(version).unwrap();
            source.add_page(0).unwrap();
            let extracted = source.extract_pages(&[0]).unwrap();
            assert_eq!(extracted.version(), version.as_str());
            let reopened = PdfDocument::from_bytes_with_mode(
                extracted.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            assert_eq!(reopened.effective_version().unwrap(), version);
        }
        let source = PdfDocument::new().unwrap();
        source.add_page(0).unwrap();
        let root_id = source
            .ctx
            .trailer()
            .get("Root")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let mut root = source.resolve_object(root_id).unwrap().into_dict().unwrap();
        root.try_insert(
            "Version".into(),
            PdfObject::PdfName(PdfName::new("2.0".into())),
        )
        .unwrap();
        source
            .update_object(root_id, PdfObject::PdfDict(root))
            .unwrap();
        assert_eq!(source.version(), "1.7");
        let extracted = source.extract_pages(&[0]).unwrap();
        assert_eq!(extracted.version(), "2.0");
        assert!(
            extracted
                .save_to_bytes_with_options(SaveOptions {
                    version: Some(PdfVersion::V1_7),
                    ..Default::default()
                })
                .is_err()
        );
    }
    #[test]
    fn merge_and_extract_keep_order_identity_and_source_options() {
        let source = PdfDocument::new_with_options(crate::parser::options::ParseOptions {
            limits: crate::parser::options::Limits {
                max_text_chars: 1234,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        source.add_page(0).unwrap();
        source.add_page(1).unwrap();
        source.set_page_rotation(1, 90).unwrap();
        let revision = source.revision();
        let target = PdfDocument::new().unwrap();
        target.add_page(0).unwrap();
        let imported = target.merge_document(&source).unwrap();
        assert_eq!(imported.len(), 2);
        assert_eq!(target.try_page_count().unwrap(), 3);
        assert_eq!(
            target
                .page_location(2)
                .unwrap()
                .page_dict
                .get("Rotate")
                .unwrap()
                .as_i32(),
            Some(90)
        );
        let extracted = source.extract_pages(&[1, 0]).unwrap();
        assert_eq!(extracted.try_page_count().unwrap(), 2);
        assert_ne!(extracted.identity(), source.identity());
        assert_eq!(extracted.ctx.limits().max_text_chars, 1234);
        assert_eq!(
            extracted
                .page_location(0)
                .unwrap()
                .page_dict
                .get("Rotate")
                .unwrap()
                .as_i32(),
            Some(90)
        );
        assert_eq!(source.revision(), revision);
        assert!(source.extract_pages(&[0, 0]).is_err());
        let reopened = PdfDocument::from_bytes_with_mode(
            extracted.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 2);
    }
    #[test]
    fn batch_preserves_order_shared_fonts_and_selected_page_cycles() {
        let source = PdfDocument::new().unwrap();
        source.add_page(0).unwrap();
        source
            .resources()
            .register_standard_font(0, super::super::resource_manager::StandardFont::Helvetica)
            .unwrap();
        source.copy_page(0, 1).unwrap();
        let entries = source.collect_page_entries().unwrap();
        for index in 0..2 {
            let mut page = entries[index].page_dict.clone();
            page.try_insert(
                "Peer".into(),
                PdfObject::PdfReference(PdfReference::from_id(entries[1 - index].page_id)),
            )
            .unwrap();
            source
                .update_object(entries[index].page_id, PdfObject::PdfDict(page))
                .unwrap();
        }
        let source_revision = source.revision();
        let target = PdfDocument::new().unwrap();
        target.add_page(0).unwrap();
        let handles = target.import_pages_from(&source, &[1, 0], 0).unwrap();
        assert_eq!(target.try_page_count().unwrap(), 3);
        let pages = target.collect_page_entries().unwrap();
        for index in 0..2 {
            assert_eq!(pages[index].page_id, handles[index].object_id());
            assert_eq!(
                pages[index]
                    .page_dict
                    .get("Peer")
                    .unwrap()
                    .as_reference()
                    .unwrap()
                    .id(),
                handles[1 - index].object_id()
            );
        }
        let resource = |index: usize| {
            pages[index]
                .page_dict
                .get("Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .get("Font")
                .unwrap()
                .as_dict()
                .unwrap()
                .get("F1")
                .unwrap()
                .clone()
        };
        assert_eq!(resource(0), resource(1));
        assert_eq!(source.revision(), source_revision);
        let reopened = PdfDocument::from_bytes_with_mode(
            target.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 3);
        let revision = target.revision();
        assert!(target.import_pages_from(&source, &[0, 0], 0).is_err());
        assert!(target.import_pages_from(&source, &[2], 0).is_err());
        assert!(target.import_pages_from(&source, &[0], 4).is_err());
        assert!(
            target
                .import_pages_from(&source, &[], 0)
                .unwrap()
                .is_empty()
        );
        assert_eq!(target.revision(), revision);
        let isolated = PdfDocument::new().unwrap();
        assert!(isolated.import_page_from(&source, 0, 0).is_err());
        assert_eq!(isolated.try_page_count().unwrap(), 0);
        assert_eq!(
            isolated
                .try_insert_object(PdfObject::PdfNull)
                .unwrap()
                .obj_num(),
            3
        );
    }

    #[test]
    fn batch_late_failure_restores_all_pages_and_allocations() {
        let source = PdfDocument::new().unwrap();
        source.add_page(0).unwrap();
        source.add_page(1).unwrap();
        source.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        source
            .replace_page_contents(1, b"q q Q Q".to_vec())
            .unwrap();
        let target = PdfDocument::new_with_options(crate::parser::options::ParseOptions {
            limits: crate::parser::options::Limits {
                max_raw_stream_bytes: 4,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let root = target.root_pages().unwrap();
        assert!(target.import_pages_from(&source, &[0, 1], 0).is_err());
        assert_eq!(target.root_pages().unwrap(), root);
        assert_eq!(target.try_page_count().unwrap(), 0);
        assert_eq!(
            target
                .try_insert_object(PdfObject::PdfNull)
                .unwrap()
                .obj_num(),
            3
        );
    }
    #[test]
    fn imported_page_is_independent_and_can_be_interpreted_after_strict_reopen() {
        let source = PdfDocument::new().unwrap();
        source.add_page(0).unwrap();
        source
            .resources()
            .register_standard_font(0, super::super::resource_manager::StandardFont::Helvetica)
            .unwrap();
        source
            .replace_page_contents(0, b"BT /F1 12 Tf 10 20 Td <4142> Tj ET".to_vec())
            .unwrap();
        source.set_page_rotation(0, -90).unwrap();
        let original = source.page_location(0).unwrap();
        let revision = source.revision();
        let target = PdfDocument::new().unwrap();
        target.add_page(0).unwrap();
        let imported = target.import_page_from(&source, 0, 0).unwrap();
        assert_eq!(target.try_page_count().unwrap(), 2);
        let page = target.page_location(0).unwrap();
        assert_eq!(page.page_id, imported.object_id());
        let imported_contents = page
            .page_dict
            .get("Contents")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let original_contents = original
            .page_dict
            .get("Contents")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        assert_ne!(
            target.object_handle(imported_contents).unwrap(),
            source.object_handle(original_contents).unwrap()
        );
        assert_eq!(
            page.page_dict.get("Rotate"),
            original.page_dict.get("Rotate")
        );
        target.get_page(0).unwrap().display_list().unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(
            target.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        reopened.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 2);
        target.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        assert_eq!(source.revision(), revision);
        assert_eq!(
            source.page_location(0).unwrap().page_dict,
            original.page_dict
        );
    }

    #[test]
    fn failed_page_import_rolls_back_tree_and_objects() {
        let source = PdfDocument::new().unwrap();
        source.add_page(0).unwrap();
        source.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        let target = PdfDocument::new_with_options(crate::parser::options::ParseOptions {
            limits: crate::parser::options::Limits {
                max_raw_stream_bytes: 2,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        assert!(target.import_page_from(&source, 0, 0).is_err());
        assert_eq!(target.try_page_count().unwrap(), 0);
        assert_eq!(
            target
                .try_insert_object(PdfObject::PdfNull)
                .unwrap()
                .obj_num(),
            3
        );
        let location = source.page_location(0).unwrap();
        let mut page = location.page_dict;
        page.try_insert("StructParents".into(), PdfObject::PdfNull)
            .unwrap();
        source
            .update_object(location.page_id, PdfObject::PdfDict(page))
            .unwrap();
        let revision = target.revision();
        assert!(target.import_page_from(&source, 0, 0).is_err());
        assert!(target.import_page_from(&source, 1, 0).is_err());
        assert!(target.import_page_from(&source, 0, 1).is_err());
        assert_eq!(target.revision(), revision);
    }
    fn font() -> PdfDict {
        let mut dict = PdfDict::default();
        for (key, value) in [
            ("Type", "Font"),
            ("Subtype", "Type1"),
            ("BaseFont", "Helvetica"),
        ] {
            dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))
                .unwrap();
        }
        dict
    }
    #[test]
    fn remaps_shared_cycles_preserves_raw_stream_and_source() {
        let source = PdfDocument::new().unwrap();
        let root_id = source.try_insert_object(PdfObject::PdfNull).unwrap();
        let mut stream_dict = PdfDict::default();
        stream_dict
            .try_insert(
                "Back".into(),
                PdfObject::PdfReference(PdfReference::from_id(root_id)),
            )
            .unwrap();
        stream_dict
            .try_insert(
                "Filter".into(),
                PdfObject::PdfName(PdfName::new("ASCIIHexDecode".into())),
            )
            .unwrap();
        let stream_id = source
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                stream_dict,
                b"4142>".to_vec(),
            )))
            .unwrap();
        let mut dict = font();
        for key in ["A", "B"] {
            dict.try_insert(
                key.into(),
                PdfObject::PdfReference(PdfReference::from_id(stream_id)),
            )
            .unwrap();
        }
        source
            .update_object(root_id, PdfObject::PdfDict(dict))
            .unwrap();
        let source_revision = source.revision();
        let handle = source.object_handle(root_id).unwrap();
        let limited = PdfDocument::new_with_options(crate::parser::options::ParseOptions {
            limits: crate::parser::options::Limits {
                max_raw_stream_bytes: 4,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        assert!(
            limited
                .import_resource_from(&source, &handle, ResourceKind::Font)
                .is_err()
        );
        assert_eq!(
            limited
                .try_insert_object(PdfObject::PdfNull)
                .unwrap()
                .obj_num(),
            3
        );
        let target = PdfDocument::new().unwrap();
        target.add_page(0).unwrap();
        let imported = target
            .import_resource_from(&source, &handle, ResourceKind::Font)
            .unwrap();
        assert_ne!(imported.document_identity(), handle.document_identity());
        let imported_dict = target
            .resolve_handle(&imported)
            .unwrap()
            .into_dict()
            .unwrap();
        assert_eq!(imported_dict.get("A"), imported_dict.get("B"));
        let mapped_stream = imported_dict.get("A").unwrap().as_reference().unwrap().id();
        let stream_object = target.resolve_object(mapped_stream).unwrap();
        let stream = stream_object.as_stream().unwrap();
        assert_eq!(stream.raw_data(), b"4142>");
        assert_eq!(
            stream
                .dict()
                .get("Back")
                .unwrap()
                .as_reference()
                .unwrap()
                .id(),
            imported.object_id()
        );
        assert_eq!(source.revision(), source_revision);
        target
            .resources()
            .register_handle(0, ResourceKind::Font, &imported)
            .unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(
            target.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 1);
    }

    #[test]
    fn failures_roll_back_target_and_reject_foreign_source_handles() {
        let source = PdfDocument::new().unwrap();
        let id = source
            .try_insert_object(PdfObject::PdfDict(font()))
            .unwrap();
        let handle = source.object_handle(id).unwrap();
        let target = PdfDocument::new_with_options(crate::parser::options::ParseOptions {
            limits: crate::parser::options::Limits {
                max_objects: 3,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let result = target.import_resource_from(&source, &handle, ResourceKind::Font);
        assert!(result.is_err());
        assert_eq!(
            target
                .try_insert_object(PdfObject::PdfNull)
                .unwrap()
                .obj_num(),
            3
        );
        let other = PdfDocument::new().unwrap();
        let revision = other.revision();
        assert!(
            other
                .import_resource_from(&target, &handle, ResourceKind::Font)
                .is_err()
        );
        assert!(
            other
                .import_resource_from(&source, &handle, ResourceKind::Image)
                .is_err()
        );
        assert_eq!(other.revision(), revision);
        let small_token = PdfDocument::new_with_options(crate::parser::options::ParseOptions {
            limits: crate::parser::options::Limits {
                max_token_bytes: 5,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        assert!(
            small_token
                .import_resource_from(&source, &handle, ResourceKind::Font)
                .is_err()
        );
        assert_eq!(
            small_token
                .try_insert_object(PdfObject::PdfNull)
                .unwrap()
                .obj_num(),
            3
        );
    }
}
