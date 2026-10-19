//! Named destinations with full views and bounded balanced name trees.

use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference,
        pdf_string::PdfString,
    },
};
use std::io::{Read, Seek};

fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(super) fn destination_entries(&self) -> PdfResult<super::trees::NameTreeSnapshot> {
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
        let mut entries = Vec::new();
        let mut decoded_names = std::collections::HashSet::new();
        for (key, value) in self.read_tree(value, super::trees::TreeKind::Name)? {
            let super::trees::TreeKey::Name(bytes) = key else {
                unreachable!()
            };
            let key = PdfString::new(bytes, false);
            if !decoded_names.insert(key.text_content_with_limits(&self.ctx().limits())?) {
                return Err(PdfError::DocumentError(
                    "duplicate decoded destination name".into(),
                ));
            }
            entries.push((key, value));
        }
        Ok((names, tree, entries))
    }

    /// Full named targets, including coordinates and null view parameters.
    pub fn named_destination_views(
        &self,
    ) -> PdfResult<Vec<(String, super::destination::Destination)>> {
        self.destination_entries()?
            .2
            .into_iter()
            .map(|(key, value)| {
                Ok((
                    key.text_content_with_limits(&self.ctx().limits())?,
                    self.decode_destination(&value)?,
                ))
            })
            .collect()
    }

    /// Compatibility projection. Use named_destination_views to retain the view.
    pub fn named_destinations(&self) -> PdfResult<Vec<(String, u32)>> {
        self.named_destination_views()?
            .into_iter()
            .map(|(key, dest)| Ok((key, self.destination_page_index(&dest)?)))
            .collect()
    }

    pub fn set_named_destination(&self, name: &str, page_index: u32) -> PdfResult<()> {
        self.set_named_destination_view(
            name,
            &self.destination(page_index, super::destination::DestinationView::Fit)?,
        )
    }

    pub fn set_named_destination_view(
        &self,
        name: &str,
        destination: &super::destination::Destination,
    ) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
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
            let new_dest = doc.encode_destination(destination)?;
            let (names, tree, mut entries) = doc.destination_entries()?;
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
        self.ctx().require_assembly_permission()?;
        self.edit_transaction(|doc| {
            let (names, tree, mut entries) = doc.destination_entries()?;
            let mut index = None;
            for (i, (key, _)) in entries.iter().enumerate() {
                if key.text_content_with_limits(&doc.ctx().limits())? == name {
                    index = Some(i);
                    break;
                }
            }
            let Some(index) = index else {
                return Ok(false);
            };
            if !doc.named_navigation_uses(name)?.is_empty() {
                return Err(PdfError::UnsupportedFeature(
                    "named destination is referenced by navigation".into(),
                ));
            }
            entries.remove(index);
            doc.write_destination_entries(names, tree, entries)?;
            Ok(true)
        })
    }

    /// Rename the tree key and supported Link/outline GoTo references in one
    /// transaction. Collisions fail; additional action graphs are not rewritten.
    pub fn rename_named_destination(&self, old: &str, new: &str) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        if new.is_empty() {
            return Err(PdfError::DocumentError("destination name is empty".into()));
        }
        let key = PdfString::from_text_with_limits(new, &self.ctx().limits())?;
        self.edit_transaction(|doc| {
            let (names, tree, mut entries) = doc.destination_entries()?;
            let mut found = None;
            for (i, (name, _)) in entries.iter().enumerate() {
                let name = name.text_content_with_limits(&doc.ctx().limits())?;
                if name == old {
                    found = Some(i);
                }
                if old != new && name == new {
                    return Err(PdfError::DocumentError(
                        "destination name already exists".into(),
                    ));
                }
            }
            let index =
                found.ok_or_else(|| PdfError::DocumentError("named destination missing".into()))?;
            if old == new {
                return Ok(());
            }
            let references = doc.named_navigation_uses(old)?;
            if references.iter().any(|(_, dict, _)| {
                dict.get("Subtype")
                    .and_then(PdfObject::as_name)
                    .map(|n| n.name())
                    == Some("Link")
            }) {
                doc.ctx().require_annotation_permission()?;
            }
            for (id, mut dict, in_action) in references {
                if in_action {
                    let mut action = doc
                        .ctx()
                        .resolve(dict.get("A").unwrap())?
                        .into_dict()
                        .unwrap();
                    action.try_insert("D".into(), PdfObject::PdfString(key.clone()))?;
                    dict.try_insert("A".into(), PdfObject::PdfDict(action))?;
                } else {
                    dict.try_insert("Dest".into(), PdfObject::PdfString(key.clone()))?;
                }
                doc.update_object(id, PdfObject::PdfDict(dict))?;
            }
            entries[index].0 = key;
            doc.write_destination_entries(names, tree, entries)
        })
    }

    fn named_navigation_uses(&self, name: &str) -> PdfResult<Vec<(ObjectId, PdfDict, bool)>> {
        let root = self.ctx().get_root_owned()?;
        if root.get("OpenAction").is_some() || root.get("AA").is_some() {
            return Err(PdfError::UnsupportedFeature(
                "named target edit with catalog actions".into(),
            ));
        }
        let mut objects = Vec::new();
        let mut owners = std::collections::HashMap::new();
        for (page_index, page) in self.collect_page_entries()?.into_iter().enumerate() {
            if page.page_dict.get("AA").is_some() {
                return Err(PdfError::UnsupportedFeature(
                    "named target edit with page additional actions".into(),
                ));
            }
            if let Some(annots) = page.page_dict.get("Annots") {
                let annots = self
                    .ctx()
                    .resolve(annots)?
                    .into_array()
                    .ok_or_else(|| PdfError::DocumentError("Annots must be an array".into()))?;
                if annots.len() > self.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("annotation count".into()));
                }
                for annot in &annots {
                    if objects.len() >= self.ctx().limits().max_objects {
                        return Err(PdfError::ResourceLimit("navigation object count".into()));
                    }
                    if let Some(id) = annot.as_reference().map(PdfReference::id)
                        && owners
                            .insert(
                                id,
                                u32::try_from(page_index)
                                    .map_err(|_| PdfError::ResourceLimit("page index".into()))?,
                            )
                            .is_some()
                    {
                        return Err(PdfError::UnsupportedFeature(
                            "shared annotation ownership".into(),
                        ));
                    }
                    objects.push((annot.clone(), self.ctx().resolve(annot)?));
                }
            }
        }
        for item in self.outline_items()? {
            if objects.len() >= self.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("navigation object count".into()));
            }
            let id = item.handle.object_id();
            objects.push((reference(id), self.resolve_object(id)?));
        }
        if objects.len() > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("navigation object count".into()));
        }
        let mut uses = Vec::new();
        for (object, value) in objects {
            let dict = value.into_dict().ok_or_else(|| {
                PdfError::DocumentError("navigation item must be a dictionary".into())
            })?;
            if dict.get("AA").is_some() {
                return Err(PdfError::UnsupportedFeature(
                    "named target edit with annotation additional actions".into(),
                ));
            }
            let mut in_action = false;
            let mut target = dict.get("Dest").cloned();
            if let Some(action) = dict.get("A") {
                if target.is_some() {
                    return Err(PdfError::DocumentError("navigation has Dest and A".into()));
                }
                let action =
                    self.ctx().resolve(action)?.into_dict().ok_or_else(|| {
                        PdfError::DocumentError("action must be a dictionary".into())
                    })?;
                if action.get("Next").is_some() {
                    return Err(PdfError::UnsupportedFeature(
                        "named target edit with chained actions".into(),
                    ));
                }
                if action
                    .get("S")
                    .and_then(PdfObject::as_name)
                    .map(|n| n.name())
                    == Some("GoTo")
                {
                    target = action.get("D").cloned();
                    in_action = true;
                }
            }
            if let Some(target) = target {
                let target = self.ctx().resolve(&target)?;
                let text = if let Some(s) = target.as_string() {
                    Some(s.text_content_with_limits(&self.ctx().limits())?)
                } else {
                    target.as_name().map(|n| n.name().to_owned())
                };
                if text.as_deref() == Some(name) {
                    let id = object.as_reference().map(PdfReference::id).ok_or_else(|| {
                        PdfError::UnsupportedFeature("editing direct navigation references".into())
                    })?;
                    if dict
                        .get("Subtype")
                        .and_then(PdfObject::as_name)
                        .map(|n| n.name())
                        == Some("Link")
                    {
                        self.check_link_edit_owner(
                            *owners.get(&id).ok_or_else(|| {
                                PdfError::DocumentError("link page owner missing".into())
                            })?,
                            id,
                        )?;
                    }
                    uses.push((id, dict, in_action));
                }
            }
        }
        Ok(uses)
    }

    pub(super) fn write_destination_entries(
        &self,
        mut names: PdfDict,
        mut tree: PdfDict,
        mut entries: Vec<(PdfString, PdfObject)>,
    ) -> PdfResult<()> {
        if entries.len() > self.ctx().limits().max_objects / 2 {
            return Err(PdfError::ResourceLimit("named destination entries".into()));
        }
        entries.sort_by(|a, b| a.0.bytes().cmp(b.0.bytes()));
        let entries: Vec<_> = entries
            .into_iter()
            .map(|(key, value)| (super::trees::TreeKey::Name(key.bytes().to_vec()), value))
            .collect();
        tree = self.build_tree(super::trees::TreeKind::Name, &entries, tree)?;
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
