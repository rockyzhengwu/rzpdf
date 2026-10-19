//! Limited flat outline bookmarks. Unsupported outline shapes are rejected on
//! edit, so an existing document's navigation tree is never silently replaced.

use std::{
    collections::HashSet,
    io::{Read, Seek},
};

use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_string::PdfString,
    },
};

type FlatBookmarkNodes = (Option<ObjectId>, Vec<(ObjectId, Bookmark)>);

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Bookmark {
    pub title: String,
    /// Zero-based page index at the time of reading.
    pub page_index: u32,
}

fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}

fn required_ref(value: Option<&PdfObject>, label: &str) -> PdfResult<ObjectId> {
    value
        .and_then(PdfObject::as_reference)
        .map(PdfReference::id)
        .ok_or_else(|| {
            PdfError::UnsupportedFeature(format!("{label} must be an indirect reference"))
        })
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Read the supported flat outline subset: a linked list of leaf items with
    /// direct `[page /Fit]` destinations. Other forms return UnsupportedFeature.
    pub fn flat_bookmarks(&self) -> PdfResult<Vec<Bookmark>> {
        Ok(self
            .flat_bookmark_nodes()?
            .1
            .into_iter()
            .map(|(_, bookmark)| bookmark)
            .collect())
    }

    fn flat_bookmark_nodes(&self) -> PdfResult<FlatBookmarkNodes> {
        let root = self.ctx().get_root_owned()?;
        let Some(outlines) = root.get("Outlines") else {
            return Ok((None, Vec::new()));
        };
        let root_id = required_ref(Some(outlines), "Outlines")?;
        let outline_root = self
            .resolve_object(root_id)?
            .into_dict()
            .ok_or_else(|| PdfError::DocumentError("Outlines root must be a dictionary".into()))?;
        let first = outline_root.get("First");
        let last = outline_root.get("Last");
        if first.is_none() && last.is_none() {
            if outline_root
                .get("Count")
                .is_some_and(|value| value.as_number().is_none_or(|n| n.value() != 0.0))
            {
                return Err(PdfError::UnsupportedFeature(
                    "inconsistent empty outline root".into(),
                ));
            }
            return Ok((Some(root_id), Vec::new()));
        }
        let mut next = Some(required_ref(first, "Outlines First")?);
        let last = required_ref(last, "Outlines Last")?;
        let pages = self.collect_page_entries()?;
        let mut seen = HashSet::new();
        let mut found = Vec::new();
        let mut previous = None;
        while let Some(id) = next {
            if found.len() >= self.ctx().limits().max_objects || !seen.insert(id) {
                return Err(PdfError::ResourceLimit(
                    "outline item count or cycle".into(),
                ));
            }
            let item = self.resolve_object(id)?.into_dict().ok_or_else(|| {
                PdfError::DocumentError("outline item must be a dictionary".into())
            })?;
            if required_ref(item.get("Parent"), "outline Parent")? != root_id
                || item.get("First").is_some()
                || item.get("Last").is_some()
                || item.get("A").is_some()
            {
                return Err(PdfError::UnsupportedFeature(
                    "nested or action outline".into(),
                ));
            }
            let actual_prev = item
                .get("Prev")
                .map(|v| required_ref(Some(v), "outline Prev"))
                .transpose()?;
            if actual_prev != previous {
                return Err(PdfError::DocumentError("broken outline Prev chain".into()));
            }
            let title = item
                .get("Title")
                .and_then(PdfObject::as_string)
                .ok_or_else(|| {
                    PdfError::UnsupportedFeature("outline Title must be a string".into())
                })?
                .text_content_with_limits(&self.ctx().limits())?;
            let dest = item
                .get("Dest")
                .and_then(PdfObject::as_array)
                .ok_or_else(|| {
                    PdfError::UnsupportedFeature("outline Dest must be an array".into())
                })?;
            if dest.len() != 2
                || self
                    .ctx()
                    .resolve(dest.get(1).unwrap())?
                    .as_name()
                    .map(PdfName::name)
                    != Some("Fit")
            {
                return Err(PdfError::UnsupportedFeature(
                    "only /Fit outline destinations are supported".into(),
                ));
            }
            let page_id = required_ref(dest.get(0), "outline destination page")?;
            let page_index = pages
                .iter()
                .position(|page| page.page_id == page_id)
                .ok_or_else(|| {
                    PdfError::DocumentError("outline destination page is missing".into())
                })?;
            found.push((
                id,
                Bookmark {
                    title,
                    page_index: u32::try_from(page_index)
                        .map_err(|_| PdfError::ResourceLimit("page index exceeds u32".into()))?,
                },
            ));
            previous = Some(id);
            next = item
                .get("Next")
                .map(|v| required_ref(Some(v), "outline Next"))
                .transpose()?;
        }
        if previous != Some(last) {
            return Err(PdfError::DocumentError("broken outline Last link".into()));
        }
        if outline_root.get("Count").is_some_and(|value| {
            value
                .as_number()
                .is_none_or(|n| n.value() != found.len() as f32)
        }) {
            return Err(PdfError::UnsupportedFeature(
                "non-flat outline Count".into(),
            ));
        }
        Ok((Some(root_id), found))
    }

    /// Append a top-level bookmark pointing at the entire page (`/Fit`).
    /// Existing nested/action outlines are left untouched and rejected.
    pub fn add_flat_bookmark(&self, title: &str, page_index: u32) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        let title = PdfString::from_text_with_limits(title, &self.ctx().limits())?;
        self.edit_transaction(|doc| {
            let page = doc.page_location(page_index)?;
            let (existing_root, nodes) = doc.flat_bookmark_nodes()?;
            let root_id = if let Some(id) = existing_root {
                id
            } else {
                let mut dict = PdfDict::default();
                dict.try_insert(
                    "Type".into(),
                    PdfObject::PdfName(PdfName::new("Outlines".into())),
                )?;
                doc.try_insert_object(PdfObject::PdfDict(dict))?
            };
            let mut dest = PdfArray::default();
            dest.try_add_obj(reference(page.page_id))?;
            dest.try_add_obj(PdfObject::PdfName(PdfName::new("Fit".into())))?;
            let mut item = PdfDict::default();
            item.try_insert("Title".into(), PdfObject::PdfString(title))?;
            item.try_insert("Parent".into(), reference(root_id))?;
            item.try_insert("Dest".into(), PdfObject::PdfArray(dest))?;
            if let Some((last, _)) = nodes.last() {
                item.try_insert("Prev".into(), reference(*last))?;
            }
            let item_id = doc.try_insert_object(PdfObject::PdfDict(item))?;
            if let Some((last, _)) = nodes.last() {
                let mut previous = doc.resolve_object(*last)?.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("outline item must be a dictionary".into())
                })?;
                previous.try_insert("Next".into(), reference(item_id))?;
                doc.update_object(*last, PdfObject::PdfDict(previous))?;
            }
            let mut outline = doc.resolve_object(root_id)?.into_dict().ok_or_else(|| {
                PdfError::DocumentError("Outlines root must be a dictionary".into())
            })?;
            if nodes.is_empty() {
                outline.try_insert("First".into(), reference(item_id))?;
            }
            outline.try_insert("Last".into(), reference(item_id))?;
            let count = nodes
                .len()
                .checked_add(1)
                .and_then(|v| u32::try_from(v).ok())
                .filter(|&v| v <= (1 << 24))
                .ok_or_else(|| PdfError::ResourceLimit("outline count".into()))?;
            outline.try_insert(
                "Count".into(),
                PdfObject::PdfNumber(PdfNumber::new(count as f32)),
            )?;
            doc.update_object(root_id, PdfObject::PdfDict(outline))?;
            if existing_root.is_none() {
                let catalog_id = required_ref(doc.ctx().trailer().get("Root"), "catalog Root")?;
                let mut catalog = doc.ctx().get_root_owned()?;
                catalog.try_insert("Outlines".into(), reference(root_id))?;
                doc.update_object(catalog_id, PdfObject::PdfDict(catalog))?;
            }
            Ok(())
        })
    }

    /// Rename one supported top-level bookmark by zero-based outline position.
    pub fn rename_flat_bookmark(&self, index: usize, title: &str) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        let title = PdfString::from_text_with_limits(title, &self.ctx().limits())?;
        self.edit_transaction(|doc| {
            let (_, nodes) = doc.flat_bookmark_nodes()?;
            let id = nodes
                .get(index)
                .map(|(id, _)| *id)
                .ok_or_else(|| PdfError::DocumentError("bookmark index out of range".into()))?;
            let mut item = doc.resolve_object(id)?.into_dict().ok_or_else(|| {
                PdfError::DocumentError("outline item must be a dictionary".into())
            })?;
            item.try_insert("Title".into(), PdfObject::PdfString(title))?;
            doc.update_object(id, PdfObject::PdfDict(item))
        })
    }

    /// Unlink one supported top-level bookmark. Old object bytes may remain in
    /// the file; this is not a secure deletion operation.
    pub fn remove_flat_bookmark(&self, index: usize) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        self.edit_transaction(|doc| {
            let (root_id, nodes) = doc.flat_bookmark_nodes()?;
            let root_id = root_id.ok_or_else(|| PdfError::DocumentError("no Outlines".into()))?;
            if index >= nodes.len() {
                return Err(PdfError::DocumentError(
                    "bookmark index out of range".into(),
                ));
            }
            let previous = index.checked_sub(1).map(|i| nodes[i].0);
            let next = nodes.get(index + 1).map(|(id, _)| *id);
            if let Some(id) = previous {
                let mut item = doc.resolve_object(id)?.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("outline item must be a dictionary".into())
                })?;
                if let Some(next) = next {
                    item.try_insert("Next".into(), reference(next))?;
                } else {
                    item.remove("Next");
                }
                doc.update_object(id, PdfObject::PdfDict(item))?;
            }
            if let Some(id) = next {
                let mut item = doc.resolve_object(id)?.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("outline item must be a dictionary".into())
                })?;
                if let Some(previous) = previous {
                    item.try_insert("Prev".into(), reference(previous))?;
                } else {
                    item.remove("Prev");
                }
                doc.update_object(id, PdfObject::PdfDict(item))?;
            }
            let mut root = doc.resolve_object(root_id)?.into_dict().ok_or_else(|| {
                PdfError::DocumentError("Outlines root must be a dictionary".into())
            })?;
            if let Some(first) = nodes
                .first()
                .filter(|_| index != 0)
                .map(|(id, _)| *id)
                .or(next)
            {
                root.try_insert("First".into(), reference(first))?;
            } else {
                root.remove("First");
            }
            if let Some(last) = nodes
                .last()
                .filter(|_| index + 1 != nodes.len())
                .map(|(id, _)| *id)
                .or(previous)
            {
                root.try_insert("Last".into(), reference(last))?;
            } else {
                root.remove("Last");
            }
            root.try_insert(
                "Count".into(),
                PdfObject::PdfNumber(PdfNumber::new((nodes.len() - 1) as f32)),
            )?;
            doc.update_object(root_id, PdfObject::PdfDict(root))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn flat_bookmarks_roundtrip_and_failure_do_not_mutate() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        doc.add_flat_bookmark("开始", 0).unwrap();
        doc.add_flat_bookmark("End", 1).unwrap();
        let expected = vec![
            Bookmark {
                title: "开始".into(),
                page_index: 0,
            },
            Bookmark {
                title: "End".into(),
                page_index: 1,
            },
        ];
        assert_eq!(doc.flat_bookmarks().unwrap(), expected);
        let revision = doc.commit_revision();
        assert!(doc.add_flat_bookmark("bad", 2).is_err());
        assert_eq!(doc.commit_revision(), revision);
        assert!(doc.delete_page(0).is_err());
        assert_eq!(doc.try_page_count().unwrap(), 2);
        assert_eq!(doc.commit_revision(), revision);
        assert_eq!(doc.flat_bookmarks().unwrap(), expected);
        let reopened = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.flat_bookmarks().unwrap(), expected);
    }

    #[test]
    fn outline_cycle_is_rejected_without_write() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_flat_bookmark("One", 0).unwrap();
        let root = doc.ctx().get_root_owned().unwrap();
        let outline_id = root.get("Outlines").unwrap().as_reference().unwrap().id();
        let outline = doc.resolve_object(outline_id).unwrap().into_dict().unwrap();
        let item_id = outline.get("First").unwrap().as_reference().unwrap().id();
        let mut item = doc.resolve_object(item_id).unwrap().into_dict().unwrap();
        item.try_insert("Next".into(), reference(item_id)).unwrap();
        doc.update_object(item_id, PdfObject::PdfDict(item))
            .unwrap();
        let revision = doc.commit_revision();
        assert!(doc.flat_bookmarks().is_err());
        assert!(doc.add_flat_bookmark("Two", 0).is_err());
        assert_eq!(doc.commit_revision(), revision);
    }

    #[test]
    fn rename_and_remove_flat_bookmarks_keep_links_consistent() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        for title in ["a", "b", "c"] {
            doc.add_flat_bookmark(title, 0).unwrap();
        }
        doc.rename_flat_bookmark(1, "中").unwrap();
        doc.remove_flat_bookmark(1).unwrap();
        assert_eq!(
            doc.flat_bookmarks()
                .unwrap()
                .iter()
                .map(|item| item.title.as_str())
                .collect::<Vec<_>>(),
            vec!["a", "c"]
        );
        doc.remove_flat_bookmark(0).unwrap();
        doc.remove_flat_bookmark(0).unwrap();
        assert!(doc.flat_bookmarks().unwrap().is_empty());
        doc.add_flat_bookmark("new", 0).unwrap();
        let reopened = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.flat_bookmarks().unwrap()[0].title, "new");
    }
}
