//! Flat named `/Dests` name tree with in-document `/Fit` page targets.

use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_reference::PdfReference, pdf_string::PdfString,
    },
};
use std::io::{Read, Seek};

fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}

impl<R: Read + Seek> PdfDocument<R> {
    fn destination_entries(&self) -> PdfResult<(PdfDict, PdfDict, Vec<(PdfString, PdfObject)>)> {
        let root = self.ctx().get_root_owned()?;
        if root.get("Dests").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "legacy catalog Dests dictionary is not supported".into(),
            ));
        }
        let names = match root.get("Names") {
            Some(value) => self
                .ctx()
                .resolve(value)?
                .into_dict()
                .ok_or_else(|| PdfError::DocumentError("Names must be a dictionary".into()))?,
            None => PdfDict::default(),
        };
        let Some(value) = names.get("Dests") else {
            return Ok((names, PdfDict::default(), Vec::new()));
        };
        let tree = self
            .ctx()
            .resolve(value)?
            .into_dict()
            .ok_or_else(|| PdfError::DocumentError("Dests must be a dictionary".into()))?;
        if tree.get("Kids").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "nested Dests name tree".into(),
            ));
        }
        let Some(value) = tree.get("Names") else {
            return Ok((names, tree, Vec::new()));
        };
        let array = self
            .ctx()
            .resolve(value)?
            .into_array()
            .ok_or_else(|| PdfError::DocumentError("Dests Names must be an array".into()))?;
        if array.len() % 2 != 0 || array.len() > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("named destination entries".into()));
        }
        let mut entries = Vec::new();
        let mut previous: Option<Vec<u8>> = None;
        let mut decoded_names = std::collections::HashSet::new();
        for i in (0..array.len()).step_by(2) {
            let key = self
                .ctx()
                .resolve(array.get(i).unwrap())?
                .into_string()
                .ok_or_else(|| {
                    PdfError::DocumentError("destination name must be a string".into())
                })?;
            if previous.as_deref().is_some_and(|old| old >= key.bytes()) {
                return Err(PdfError::DocumentError(
                    "destination names must be unique and ordered".into(),
                ));
            }
            if !decoded_names.insert(key.text_content_with_limits(&self.ctx().limits())?) {
                return Err(PdfError::DocumentError(
                    "duplicate decoded destination name".into(),
                ));
            }
            previous = Some(key.bytes().to_vec());
            entries.push((key, array.get(i + 1).unwrap().clone()));
        }
        Ok((names, tree, entries))
    }

    pub fn named_destinations(&self) -> PdfResult<Vec<(String, u32)>> {
        let (_, _, entries) = self.destination_entries()?;
        let pages = self.collect_page_entries()?;
        let mut found = Vec::new();
        for (key, value) in entries {
            let destination = self.ctx().resolve(&value)?;
            let destination = if let Some(dict) = destination.as_dict() {
                if dict
                    .get("S")
                    .is_some_and(|kind| kind.as_name().map(PdfName::name) != Some("GoTo"))
                {
                    return Err(PdfError::UnsupportedFeature(
                        "named destination action is not GoTo".into(),
                    ));
                }
                let inner = dict.get("D").ok_or_else(|| {
                    PdfError::UnsupportedFeature("named destination dictionary has no D".into())
                })?;
                self.ctx().resolve(inner)?
            } else {
                destination
            };
            let destination = destination.as_array().ok_or_else(|| {
                PdfError::UnsupportedFeature("named destination must be an array".into())
            })?;
            let view = destination.get(1).ok_or_else(|| {
                PdfError::UnsupportedFeature("named destination view missing".into())
            })?;
            let view = self.ctx().resolve(view)?;
            let supported = matches!(
                (view.as_name().map(PdfName::name), destination.len()),
                (Some("Fit"), 2) | (Some("FitH"), 3)
            );
            if !supported {
                return Err(PdfError::UnsupportedFeature(
                    "only /Fit and /FitH named destinations are supported".into(),
                ));
            }
            if view.as_name().map(PdfName::name) == Some("FitH") {
                let top = self.ctx().resolve(
                    destination
                        .get(2)
                        .ok_or_else(|| PdfError::DocumentError("FitH top missing".into()))?,
                )?;
                if !matches!(&top, PdfObject::PdfNull)
                    && !top.as_number().is_some_and(|n| n.value().is_finite())
                {
                    return Err(PdfError::DocumentError(
                        "FitH top must be finite or null".into(),
                    ));
                }
            }
            let page_id = destination
                .get(0)
                .and_then(PdfObject::as_reference)
                .map(PdfReference::id)
                .ok_or_else(|| {
                    PdfError::UnsupportedFeature("named destination page must be indirect".into())
                })?;
            let page_index = pages
                .iter()
                .position(|page| page.page_id == page_id)
                .ok_or_else(|| PdfError::DocumentError("named destination page missing".into()))?;
            found.push((
                key.text_content_with_limits(&self.ctx().limits())?,
                u32::try_from(page_index)
                    .map_err(|_| PdfError::ResourceLimit("page index".into()))?,
            ));
        }
        Ok(found)
    }

    pub fn set_named_destination(&self, name: &str, page_index: u32) -> PdfResult<()> {
        if self.effective_version()? < super::save_options::PdfVersion::V1_4 {
            return Err(PdfError::UnsupportedFeature(
                "named destination tree writing requires PDF 1.4 or later".into(),
            ));
        }
        if name.is_empty() {
            return Err(PdfError::DocumentError("destination name is empty".into()));
        }
        let key = PdfString::from_text_with_limits(name, &self.ctx().limits())?;
        self.edit_transaction(|doc| {
            let page = doc.page_location(page_index)?;
            let (names, tree, mut entries) = doc.destination_entries()?;
            let mut dest = PdfArray::default();
            dest.try_add_obj(reference(page.page_id))?;
            dest.try_add_obj(PdfObject::PdfName(PdfName::new("Fit".into())))?;
            let new_dest = PdfObject::PdfArray(dest);
            let mut replacement = None;
            for (index, (existing, _)) in entries.iter().enumerate() {
                if existing.text_content_with_limits(&doc.ctx().limits())? == name {
                    replacement = Some(index);
                    break;
                }
            }
            if let Some(index) = replacement {
                entries[index].1 = new_dest;
            } else {
                if entries.len() >= doc.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("named destination count".into()));
                }
                entries.push((key, new_dest));
            }
            doc.write_destination_entries(names, tree, entries)
        })
    }

    pub fn remove_named_destination(&self, name: &str) -> PdfResult<bool> {
        self.edit_transaction(|doc| {
            let (names,tree,mut entries) = doc.destination_entries()?;
            let mut index = None;
            for (i,(key,_)) in entries.iter().enumerate() {
                if key.text_content_with_limits(&doc.ctx().limits())? == name { index=Some(i); break; }
            }
            let Some(index)=index else { return Ok(false); };
            for page in 0..doc.try_page_count()? {
                let page = u32::try_from(page).map_err(|_| PdfError::ResourceLimit("page index".into()))?;
                if doc.page_links(page)?.iter().any(|link| matches!(&link.target, super::links::LinkTarget::Named(target) if target == name)) {
                    return Err(PdfError::UnsupportedFeature("named destination is referenced by a link".into()));
                }
            }
            entries.remove(index);
            doc.write_destination_entries(names,tree,entries)?;
            Ok(true)
        })
    }

    fn write_destination_entries(
        &self,
        mut names: PdfDict,
        mut tree: PdfDict,
        mut entries: Vec<(PdfString, PdfObject)>,
    ) -> PdfResult<()> {
        if entries.len() > self.ctx().limits().max_objects / 2 {
            return Err(PdfError::ResourceLimit("named destination entries".into()));
        }
        entries.sort_by(|a, b| a.0.bytes().cmp(b.0.bytes()));
        let mut array = PdfArray::default();
        for (key, value) in entries {
            array.try_add_obj(PdfObject::PdfString(key))?;
            array.try_add_obj(value)?;
        }
        tree.try_insert("Names".into(), PdfObject::PdfArray(array))?;
        tree.remove("Limits");
        let tree_id = self.try_insert_object(PdfObject::PdfDict(tree))?;
        names.try_insert("Dests".into(), reference(tree_id))?;
        let catalog_id = self
            .ctx()
            .trailer()
            .get("Root")
            .and_then(PdfObject::as_reference)
            .map(PdfReference::id)
            .ok_or_else(|| PdfError::DocumentError("Root must be indirect".into()))?;
        let mut root = self.ctx().get_root_owned()?;
        root.try_insert("Names".into(), PdfObject::PdfDict(names))?;
        self.update_object(catalog_id, PdfObject::PdfDict(root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn destinations_roundtrip_replace_remove() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        doc.set_named_destination("章", 1).unwrap();
        doc.set_named_destination("章", 0).unwrap();
        assert_eq!(doc.named_destinations().unwrap(), vec![("章".into(), 0)]);
        let reopened = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(
            reopened.named_destinations().unwrap(),
            vec![("章".into(), 0)]
        );
        assert!(reopened.remove_named_destination("章").unwrap());
        assert!(reopened.named_destinations().unwrap().is_empty());
    }
}
