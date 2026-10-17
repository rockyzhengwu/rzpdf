use super::{PdfDocument, page_tree::PageTree};
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_number::PdfNumber,
        pdf_reference::PdfReference,
    },
};
use std::io::{Read, Seek};

impl<R: Read + Seek> PdfDocument<R> {
    /// Copy a page within this document, sharing content and resource objects.
    /// Both indices refer to the pre-insertion order; insertion accepts page_count.
    /// Returns the new page ID. Supported annotations are cloned; fields must
    /// have unique names (use copy_page_with_options for a prefix). Supported
    /// structure branches migrate with isolated ownership; article graphs reject.
    /// Low-level mutation of shared streams/resources affects both pages; use
    /// page-scoped replacement/resource APIs for copy-on-write editing.
    pub fn copy_page(&self, source_index: u32, insertion_index: u32) -> PdfResult<ObjectId> {
        self.copy_page_with_options(source_index, insertion_index, &Default::default())
    }

    pub fn copy_page_with_options(
        &self,
        source_index: u32,
        insertion_index: u32,
        options: &super::page_transfer::PageTransferOptions,
    ) -> PdfResult<ObjectId> {
        self.ctx().require_assembly_permission()?;
        self.ctx().require_content_edit_permission()?;
        let source = self.page_location(source_index)?;
        if insertion_index as usize > self.try_page_count()? {
            return Err(PdfError::PageNotExist);
        }
        for key in ["B"] {
            if source.page_dict.contains_key(key) {
                return Err(PdfError::UnsupportedFeature(format!(
                    "Page copy with {key} ownership"
                )));
            }
        }
        self.page_relationship_guard()?;
        let structure_plan=self.plan_structure_transfer(self,&std::collections::HashSet::from([source.page_id]))?;
        let insertion = self.page_insertion_point(insertion_index)?;
        let mut page = source.page_dict;
        let node = PdfObject::PdfReference(PdfReference::from_id(source.page_id));
        for key in ["Resources", "MediaBox", "CropBox", "Rotate"] {
            if let Some(value) = PageTree::inherited_attribute(&self.ctx, &node, key)? {
                page.try_insert(key.into(), value)?;
            } else {
                let value = match key {
                    "MediaBox" => {
                        return Err(PdfError::DocumentError(
                            "Copied page has no MediaBox".into(),
                        ));
                    }
                    "CropBox" => page.get("MediaBox").cloned().ok_or_else(|| {
                        PdfError::DocumentError("Copied page has no MediaBox".into())
                    })?,
                    "Rotate" => PdfObject::PdfNumber(PdfNumber::new(0.0)),
                    _ => PdfObject::PdfDict(PdfDict::default()),
                };
                page.try_insert(key.into(), value)?;
            }
        }
        page.try_insert(
            "Parent".into(),
            PdfObject::PdfReference(PdfReference::from_id(insertion.parent_id)),
        )?;
        let annotations = page.get("Annots").cloned();
        page.remove("Annots");
        self.edit_transaction(|doc| {
            let count = u32::try_from(doc.try_page_count()?)
                .map_err(|_| PdfError::ResourceLimit("page count".into()))?;
            let labels = doc.labels_after_page_mapping(
                (0..insertion_index)
                    .map(Some)
                    .chain(std::iter::once(if options.preserve_labels {
                        Some(source_index)
                    } else {
                        None
                    }))
                    .chain((insertion_index..count).map(Some)),
            )?;
            let id = doc.try_insert_object(PdfObject::PdfDict(page))?;
            let mut importer = super::resource_import::Importer::new(doc, doc);
            importer.mapped.insert(source.page_id, id);
            structure_plan.prepare(&mut importer);
            let (roots, resources) = doc.prepare_page_relationships(
                &mut importer,
                &std::collections::HashSet::from([source.page_id]),
                options,
            )?;
            if importer.isolate_structure {
                let mut page = doc.resolve_object(id)?.into_dict().unwrap();
                let parent = page.remove("Parent");
                page.remove("Type");
                let mut page = importer.object(PdfObject::PdfDict(page), 0)?.into_dict().unwrap();
                page.try_insert("Type".into(), PdfObject::PdfName(crate::objects::pdf_name::PdfName::new("Page".into())))?;
                if let Some(parent) = parent { page.try_insert("Parent".into(), parent)?; }
                doc.update_object(id, PdfObject::PdfDict(page))?;
            }
            if let Some(annotations) = annotations {
                // Resolve the array itself, so a shared indirect Annots container
                // is not reused by a same-document copy.
                let annotations = importer.object(doc.ctx().resolve(&annotations)?, 0)?;
                let mut page = doc.resolve_object(id)?.into_dict().unwrap();
                page.try_insert("Annots".into(), annotations)?;
                doc.update_object(id, PdfObject::PdfDict(page))?;
            }
            doc.update_pages_kids(
                insertion.parent_id,
                insertion.kid_index,
                PdfObject::PdfReference(PdfReference::from_id(id)),
                false,
            )?;
            doc.adjust_page_counts(&insertion.ancestor_ids, 1)?;
            doc.attach_page_fields(roots, resources)?;
            doc.transfer_navigation(&mut importer, options)?;
            doc.validate_annotation_ownership()?;
            doc.apply_structure_transfer(&mut importer,structure_plan)?;
            if let Some(labels) = labels {
                doc.set_page_label_ranges(&labels)?;
            }
            Ok(id)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn copied_page_shares_content_until_page_scoped_replacement() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
        let original = doc.page_location(0).unwrap();
        let (root_id, mut root) = doc.root_pages().unwrap();
        let crop = PdfObject::PdfArray(
            super::super::rect_to_array(crate::geom::rectangle::Rectangle::new(
                10.0, 20.0, 300.0, 400.0,
            ))
            .unwrap(),
        );
        root.try_insert("CropBox".into(), crop.clone()).unwrap();
        doc.update_object(root_id, PdfObject::PdfDict(root))
            .unwrap();
        let id = doc.copy_page(0, 0).unwrap();
        assert_ne!(id, original.page_id);
        assert_eq!(doc.try_page_count().unwrap(), 2);
        let copy = doc.page_location(0).unwrap();
        assert_eq!(copy.page_id, id);
        assert_eq!(copy.page_dict.get("CropBox"), Some(&crop));
        assert_eq!(
            copy.page_dict.get("Contents"),
            original.page_dict.get("Contents")
        );
        doc.replace_page_contents(0, b"q q Q Q".to_vec()).unwrap();
        assert_ne!(
            doc.page_location(0).unwrap().page_dict.get("Contents"),
            original.page_dict.get("Contents")
        );
        assert_eq!(doc.page_location(1).unwrap().page_dict, original.page_dict);
        let saved = doc.save_to_bytes().unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(saved, crate::parser::syntax::ParseMode::Strict)
                .unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 2);
        assert_eq!(reopened.page_location(0).unwrap().page_id, id);
    }

    #[test]
    fn outer_rollback_restores_copy_and_next_object_number() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let original = doc.page_location(0).unwrap();
        let result: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.copy_page(0, 1)?;
            Err(PdfError::DocumentError("test rollback".into()))
        });
        assert!(result.is_err());
        assert_eq!(doc.try_page_count().unwrap(), 1);
        assert_eq!(doc.page_location(0).unwrap().page_dict, original.page_dict);
        assert_eq!(doc.copy_page(0, 1).unwrap().obj_num(), 5);
    }

    #[test]
    fn unsupported_ownership_and_invalid_indices_are_rejected_without_edits() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let source = doc.page_location(0).unwrap();
        let revision = doc.revision();
        assert!(doc.copy_page(1, 0).is_err());
        assert!(doc.copy_page(0, 2).is_err());
        assert_eq!(doc.revision(), revision);
        for key in ["StructParents", "B", "Annots"] {
            let mut page = source.page_dict.clone();
            page.try_insert(key.into(), PdfObject::PdfNumber(PdfNumber::new(0.0)))
                .unwrap();
            doc.update_object(source.page_id, PdfObject::PdfDict(page))
                .unwrap();
            let revision = doc.revision();
            assert!(doc.copy_page(0, 1).is_err());
            assert_eq!(doc.revision(), revision);
            assert_eq!(doc.try_page_count().unwrap(), 1);
        }
    }
}
