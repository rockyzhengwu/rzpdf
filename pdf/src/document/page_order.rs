use super::{PdfDocument, page_tree::PageTree};
use crate::{
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_reference::PdfReference},
};
use std::io::{Read, Seek};

impl<R: Read + Seek> PdfDocument<R> {
    /// Reorder pages by a complete permutation of their current zero-based indices.
    /// Keeps page IDs, tree slots and Counts; materializes inherited attributes
    /// when a page moves to a different parent. Does not rewrite outlines/labels.
    pub fn reorder_pages(&self, order: &[u32]) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        let entries = self.collect_page_entries()?;
        if order.len() != entries.len() {
            return Err(PdfError::DocumentError(
                "Page order must be a complete permutation".into(),
            ));
        }
        let mut seen = Vec::new();
        seen.try_reserve_exact(entries.len())
            .map_err(|_| PdfError::ResourceLimit("Page order allocation".into()))?;
        seen.resize(entries.len(), false);
        for &index in order {
            let flag = seen.get_mut(index as usize).ok_or(PdfError::PageNotExist)?;
            if *flag {
                return Err(PdfError::DocumentError("Duplicate page in order".into()));
            }
            *flag = true;
        }
        if order
            .iter()
            .enumerate()
            .all(|(slot, &index)| slot == index as usize)
        {
            return Ok(());
        }
        let mut replacements = Vec::new();
        replacements
            .try_reserve_exact(entries.len())
            .map_err(|_| PdfError::ResourceLimit("Page reorder snapshots".into()))?;
        // Resolve all inherited values before changing any parent links.
        for (slot, &index) in order.iter().enumerate() {
            let source = &entries[index as usize];
            let target = &entries[slot];
            let mut page = source.page_dict.clone();
            if source.parent_id != target.parent_id {
                for key in ["Resources", "MediaBox", "CropBox", "Rotate"] {
                    if let Some(value) = PageTree::inherited_attribute(
                        &self.ctx,
                        &PdfObject::PdfReference(PdfReference::from_id(source.page_id)),
                        key,
                    )? {
                        page.try_insert(key.into(), value)?;
                    } else if key == "CropBox" {
                        // Missing CropBox means MediaBox, not the new parent's crop.
                        if let Some(media) = page.get("MediaBox").cloned() {
                            page.try_insert(key.into(), media)?;
                        }
                    } else if key == "Rotate" {
                        page.try_insert(
                            key.into(),
                            PdfObject::PdfNumber(crate::objects::pdf_number::PdfNumber::new(0.0)),
                        )?;
                    } else if key == "Resources" {
                        page.try_insert(key.into(), PdfObject::PdfDict(Default::default()))?;
                    } else if key == "MediaBox" {
                        return Err(PdfError::DocumentError(
                            "Moved page has no inherited MediaBox".into(),
                        ));
                    }
                }
                page.try_insert(
                    "Parent".into(),
                    PdfObject::PdfReference(PdfReference::from_id(target.parent_id)),
                )?;
            }
            replacements.push((source.page_id, page));
        }
        self.edit_transaction(|doc| {
            for (slot, (id, page)) in replacements.into_iter().enumerate() {
                let target = &entries[slot];
                doc.update_pages_kids(
                    target.parent_id,
                    target.kid_index,
                    PdfObject::PdfReference(PdfReference::from_id(id)),
                    true,
                )?;
                doc.update_pages_kids(
                    target.parent_id,
                    target.kid_index,
                    PdfObject::PdfReference(PdfReference::from_id(id)),
                    false,
                )?;
                doc.update_object(id, PdfObject::PdfDict(page))?;
            }
            Ok(())
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn moving_between_nested_parents_preserves_effective_attributes() {
        use crate::objects::{
            pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber,
        };
        let doc = PdfDocument::new().unwrap();
        for index in 0..3 {
            doc.add_page(index).unwrap();
        }
        let entries = doc.collect_page_entries().unwrap();
        let (root_id, mut root) = doc.root_pages().unwrap();
        let media = entries[0].page_dict.get("MediaBox").unwrap().clone();
        let mut parent_ids = Vec::new();
        for (group, indices) in [vec![0, 1], vec![2]].into_iter().enumerate() {
            let mut parent = PdfDict::default();
            parent
                .try_insert(
                    "Type".into(),
                    PdfObject::PdfName(PdfName::new("Pages".into())),
                )
                .unwrap();
            parent
                .try_insert(
                    "Parent".into(),
                    PdfObject::PdfReference(PdfReference::from_id(root_id)),
                )
                .unwrap();
            parent.try_insert("MediaBox".into(), media.clone()).unwrap();
            let mut resources = PdfDict::default();
            resources
                .try_insert(
                    "Marker".into(),
                    PdfObject::PdfNumber(PdfNumber::new(group as f32)),
                )
                .unwrap();
            parent
                .try_insert("Resources".into(), PdfObject::PdfDict(resources))
                .unwrap();
            if group == 1 {
                parent
                    .try_insert("Rotate".into(), PdfObject::PdfNumber(PdfNumber::new(90.0)))
                    .unwrap();
                let mut crop = PdfArray::default();
                for number in [0.0, 0.0, 100.0, 100.0] {
                    crop.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(number)))
                        .unwrap();
                }
                parent
                    .try_insert("CropBox".into(), PdfObject::PdfArray(crop))
                    .unwrap();
            }
            let mut kids = PdfArray::default();
            for &index in &indices {
                kids.try_add_obj(PdfObject::PdfReference(PdfReference::from_id(
                    entries[index].page_id,
                )))
                .unwrap();
            }
            parent
                .try_insert("Kids".into(), PdfObject::PdfArray(kids))
                .unwrap();
            parent
                .try_insert(
                    "Count".into(),
                    PdfObject::PdfNumber(PdfNumber::new(indices.len() as f32)),
                )
                .unwrap();
            let id = doc.try_insert_object(PdfObject::PdfDict(parent)).unwrap();
            parent_ids.push(id);
            for index in indices {
                let mut page = entries[index].page_dict.clone();
                for key in ["Resources", "MediaBox", "CropBox", "Rotate"] {
                    page.remove(key);
                }
                page.try_insert(
                    "Parent".into(),
                    PdfObject::PdfReference(PdfReference::from_id(id)),
                )
                .unwrap();
                doc.update_object(entries[index].page_id, PdfObject::PdfDict(page))
                    .unwrap();
            }
        }
        let mut kids = PdfArray::default();
        for id in parent_ids {
            kids.try_add_obj(PdfObject::PdfReference(PdfReference::from_id(id)))
                .unwrap();
        }
        root.try_insert("Kids".into(), PdfObject::PdfArray(kids))
            .unwrap();
        doc.update_object(root_id, PdfObject::PdfDict(root))
            .unwrap();
        doc.reorder_pages(&[2, 0, 1]).unwrap();
        let reordered = doc.collect_page_entries().unwrap();
        assert_eq!(
            reordered[0]
                .page_dict
                .get("Rotate")
                .unwrap()
                .as_number()
                .unwrap()
                .as_i32_checked()
                .unwrap(),
            90
        );
        assert_eq!(
            reordered[2]
                .page_dict
                .get("Rotate")
                .unwrap()
                .as_number()
                .unwrap()
                .as_i32_checked()
                .unwrap(),
            0
        );
        assert_eq!(reordered[2].page_dict.get("CropBox"), Some(&media));
        assert_eq!(
            reordered[2]
                .page_dict
                .get("Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .get("Marker")
                .unwrap()
                .as_number()
                .unwrap()
                .as_i32_checked()
                .unwrap(),
            0
        );
        assert_eq!(doc.try_page_count().unwrap(), 3);
    }
    #[test]
    fn page_permutation_preserves_ids_and_invalid_orders_do_not_edit() {
        let doc = PdfDocument::new().unwrap();
        for index in 0..3 {
            doc.add_page(index).unwrap();
        }
        let original = doc.collect_page_entries().unwrap();
        let revision = doc.revision();
        for invalid in [vec![0, 1], vec![0, 0, 2], vec![0, 1, 3]] {
            assert!(doc.reorder_pages(&invalid).is_err());
            assert_eq!(doc.revision(), revision);
        }
        doc.reorder_pages(&[0, 1, 2]).unwrap();
        assert_eq!(doc.revision(), revision);
        let rollback: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.reorder_pages(&[2, 0, 1])?;
            Err(PdfError::DocumentError("test rollback".into()))
        });
        assert!(rollback.is_err());
        for (before, after) in original.iter().zip(doc.collect_page_entries().unwrap()) {
            assert_eq!(before.page_id, after.page_id);
            assert_eq!(before.page_dict, after.page_dict);
        }
        doc.reorder_pages(&[2, 0, 1]).unwrap();
        let reordered = doc.collect_page_entries().unwrap();
        for (slot, index) in [2, 0, 1].into_iter().enumerate() {
            assert_eq!(reordered[slot].page_id, original[index].page_id);
        }
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 3);
        for (slot, entry) in reopened.collect_page_entries().unwrap().iter().enumerate() {
            assert_eq!(entry.page_id, reordered[slot].page_id);
        }
    }
}
