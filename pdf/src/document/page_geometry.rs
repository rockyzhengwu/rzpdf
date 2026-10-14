use super::{PdfDocument, page_tree::PageTree};
use crate::{
    error::{PdfError, PdfResult},
    geom::rectangle::Rectangle,
    objects::{PdfObject, pdf_number::PdfNumber, pdf_reference::PdfReference},
};
use std::io::{Read, Seek};

impl<R: Read + Seek> PdfDocument<R> {
    /// Set the leaf page's default user-space unit size (multiples of 1/72 inch).
    /// Some requires effective PDF 1.6; None removes it and restores default 1.
    /// Does not rewrite content, annotations or boxes in their user coordinates.
    pub fn set_page_user_unit(&self, page_index: u32, unit: Option<f32>) -> PdfResult<()> {
        if let Some(unit) = unit {
            if !unit.is_finite() || unit <= 0. {
                return Err(PdfError::DocumentError(
                    "UserUnit must be a finite positive number".into(),
                ));
            }
            if self.effective_version()? < super::save_options::PdfVersion::V1_6 {
                return Err(PdfError::UnsupportedFeature(
                    "UserUnit requires PDF 1.6 or later".into(),
                ));
            }
        }
        let location = self.page_location(page_index)?;
        let mut page = location.page_dict;
        match unit {
            Some(unit) => page.try_insert(
                "UserUnit".into(),
                PdfObject::PdfNumber(PdfNumber::new(unit)),
            )?,
            None => {
                page.remove("UserUnit");
            }
        }
        // Validate the proposed page without storing it or decoding its Contents.
        crate::page::PdfPage::new(PdfObject::PdfDict(page.clone()), &self.ctx).geometry()?;
        self.edit_transaction(|doc| doc.update_object(location.page_id, PdfObject::PdfDict(page)))
    }

    /// Rotate clockwise relative to the effective (possibly inherited) rotation.
    /// Changes only the leaf page; content, annotations and parent stay unchanged.
    pub fn rotate_page(&self, page_index: u32, degrees: i32) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        if degrees % 90 != 0 {
            return Err(PdfError::DocumentError(
                "Page rotation must be a multiple of 90".into(),
            ));
        }
        let location = self.page_location(page_index)?;
        let rotation = PageTree::inherited_attribute(
            &self.ctx,
            &PdfObject::PdfReference(PdfReference::from_id(location.page_id)),
            "Rotate",
        )?;
        let current = rotation
            .as_ref()
            .map(crate::page::normalized_page_rotation)
            .transpose()?
            .unwrap_or(0);
        // Normalize before adding so even large valid angles cannot overflow i32.
        self.set_page_rotation(
            page_index,
            current.rem_euclid(360) + degrees.rem_euclid(360),
        )
    }

    /// Set absolute clockwise rotation, normalized to 0/90/180/270 degrees.
    /// Changes only the leaf page dictionary, not its inherited parent.
    pub fn set_page_rotation(&self, page_index: u32, degrees: i32) -> PdfResult<()> {
        self.ctx().require_assembly_permission()?;
        if degrees % 90 != 0 {
            return Err(PdfError::DocumentError(
                "Page rotation must be a multiple of 90".into(),
            ));
        }
        let location = self.page_location(page_index)?;
        let mut page = location.page_dict;
        page.try_insert(
            "Rotate".into(),
            PdfObject::PdfNumber(PdfNumber::new(degrees.rem_euclid(360) as f32)),
        )?;
        self.edit_transaction(|doc| doc.update_object(location.page_id, PdfObject::PdfDict(page)))
    }

    /// Set a leaf CropBox in unrotated PDF user-space coordinates.
    /// None resets cropping to the effective MediaBox, overriding parent crops.
    /// This does not transform content or rewrite other boxes/annotations.
    pub fn set_page_crop_box(&self, page_index: u32, crop: Option<Rectangle>) -> PdfResult<()> {
        let location = self.page_location(page_index)?;
        let media_object = PageTree::inherited_attribute(
            &self.ctx,
            &PdfObject::PdfReference(PdfReference::from_id(location.page_id)),
            "MediaBox",
        )?
        .ok_or_else(|| PdfError::DocumentError("Page has no inherited MediaBox".into()))?;
        let media = Rectangle::try_from_with_context(&media_object, &self.ctx)?;
        let crop = crop.unwrap_or_else(|| media.clone());
        crate::page::page_builder::PageBuilder::new()
            .media_box(media)
            .crop_box(crop.clone())
            .validate()?;
        let mut page = location.page_dict;
        page.try_insert(
            "CropBox".into(),
            PdfObject::PdfArray(super::rect_to_array(crop)?),
        )?;
        self.edit_transaction(|doc| doc.update_object(location.page_id, PdfObject::PdfDict(page)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn user_unit_builder_validates_before_allocating_and_round_trips() {
        use crate::{
            document::save_options::PdfVersion, page::page_builder::PageBuilder,
            parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new_with_version(PdfVersion::V1_6).unwrap();
        let revision = doc.revision();
        for unit in [0., -1., f32::NAN, f32::INFINITY, f32::MAX] {
            assert!(
                doc.add_page_with_builder(0, PageBuilder::new().user_unit(unit))
                    .is_err()
            );
            assert_eq!(doc.revision(), revision);
            assert_eq!(doc.try_page_count().unwrap(), 0);
            let unchanged =
                PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(unchanged.try_page_count().unwrap(), 0);
            assert_eq!(unchanged.effective_version().unwrap(), PdfVersion::V1_6);
        }
        // Failed preflight must not consume an object identifier.
        assert_eq!(
            doc.try_insert_object(crate::objects::PdfObject::PdfNull)
                .unwrap(),
            crate::objects::object_id::ObjectId::new(3, 0)
        );
        doc.add_page_with_builder(
            0,
            PageBuilder::new()
                .media_box(Rectangle::new(10., 20., 110., 80.))
                .crop_box(Rectangle::new(20., 30., 100., 70.))
                .rotation(90)
                .user_unit(1.5),
        )
        .unwrap();
        let geometry = doc.get_page(0).unwrap().geometry().unwrap();
        assert_eq!(
            (
                geometry.user_unit,
                geometry.display_width,
                geometry.display_height
            ),
            (1.5, 60., 120.)
        );
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let geometry = reopened.get_page(0).unwrap().geometry().unwrap();
        assert_eq!(
            (
                geometry.user_unit,
                geometry.display_width,
                geometry.display_height
            ),
            (1.5, 60., 120.)
        );
        let old = PdfDocument::new_with_version(PdfVersion::V1_5).unwrap();
        let revision = old.revision();
        assert!(
            old.add_page_with_builder(0, PageBuilder::new().user_unit(1.))
                .is_err()
        );
        assert_eq!(old.revision(), revision);
        assert_eq!(old.try_page_count().unwrap(), 0);
        old.add_page(0).unwrap();
        assert_eq!(old.get_page(0).unwrap().geometry().unwrap().user_unit, 1.);
    }

    #[test]
    fn user_unit_setter_is_page_local_validated_and_transactional() {
        use crate::document::save_options::PdfVersion;
        let doc = PdfDocument::new_with_version(PdfVersion::V1_6).unwrap();
        doc.add_page(0).unwrap();
        doc.copy_page(0, 1).unwrap();
        let revision = doc.revision();
        for value in [0., -1., f32::NAN, f32::INFINITY, f32::MAX] {
            assert!(doc.set_page_user_unit(0, Some(value)).is_err());
            assert_eq!(doc.revision(), revision);
        }
        assert!(doc.set_page_user_unit(2, Some(2.)).is_err());
        assert_eq!(doc.revision(), revision);
        doc.set_page_user_unit(0, Some(3.)).unwrap();
        assert_eq!(
            doc.get_page(0).unwrap().geometry().unwrap().display_width,
            1836.
        );
        assert_eq!(doc.get_page(1).unwrap().geometry().unwrap().user_unit, 1.);
        let result: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.set_page_user_unit(0, None)?;
            assert_eq!(doc.get_page(0)?.geometry()?.user_unit, 1.);
            Err(PdfError::DocumentError("rollback UserUnit".into()))
        });
        assert!(result.is_err());
        assert_eq!(doc.get_page(0).unwrap().geometry().unwrap().user_unit, 3.);
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(
            reopened.get_page(0).unwrap().geometry().unwrap().user_unit,
            3.
        );
        doc.set_page_user_unit(0, None).unwrap();
        assert!(
            !doc.page_location(0)
                .unwrap()
                .page_dict
                .contains_key("UserUnit")
        );
        assert_eq!(
            doc.get_page(0).unwrap().geometry().unwrap().display_width,
            612.
        );
        let old = PdfDocument::new_with_version(PdfVersion::V1_5).unwrap();
        old.add_page(0).unwrap();
        let revision = old.revision();
        assert!(old.set_page_user_unit(0, Some(1.)).is_err());
        assert_eq!(old.revision(), revision);
        old.set_page_user_unit(0, None).unwrap();
        old.save_to_bytes().unwrap();
    }

    #[test]
    fn user_unit_scales_geometry_and_requires_save_version_upgrade() {
        use crate::document::save_options::{PdfVersion, SaveOptions};
        let doc = PdfDocument::new_with_version(PdfVersion::V1_5).unwrap();
        doc.add_page(0).unwrap();
        let location = doc.page_location(0).unwrap();
        let mut page = location.page_dict;
        let unit_id = doc
            .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(2.5)))
            .unwrap();
        page.try_insert(
            "UserUnit".into(),
            PdfObject::PdfReference(PdfReference::from_id(unit_id)),
        )
        .unwrap();
        doc.update_object(location.page_id, PdfObject::PdfDict(page))
            .unwrap();
        let mut retained = doc.get_page(0).unwrap();
        let geometry = retained.geometry().unwrap();
        assert_eq!(geometry.user_unit, 2.5);
        assert_eq!(
            (geometry.display_width, geometry.display_height),
            (1530., 1980.)
        );
        let point = geometry
            .user_to_display
            .transform(&crate::geom::point::Point::new(10., 20.));
        assert_eq!((point.x(), point.y()), (25., 1930.));
        let restored = geometry.display_to_user_point(&point).unwrap();
        assert_eq!((restored.x(), restored.y()), (10., 20.));
        assert!(doc.save_to_bytes().is_err());
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                version: Some(PdfVersion::V1_6),
                ..Default::default()
            })
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(bytes, crate::parser::syntax::ParseMode::Strict)
                .unwrap();
        assert_eq!(
            reopened.get_page(0).unwrap().geometry().unwrap().user_unit,
            2.5
        );
        for value in [0., -1., f32::INFINITY, f32::MAX] {
            doc.update_object(unit_id, PdfObject::PdfNumber(PdfNumber::new(value)))
                .unwrap();
            let revision = doc.revision();
            assert!(retained.geometry().is_err());
            assert_eq!(doc.revision(), revision);
        }
    }

    #[test]
    fn relative_rotation_uses_exact_large_integer_lexemes() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        for (token, expected) in [
            ("2147483700", 180),
            ("-2147483700", 180),
            ("9223372036854775800", 0),
            ("-9223372036854775800", 0),
        ] {
            let location = doc.page_location(0).unwrap();
            let mut page = location.page_dict;
            page.try_insert(
                "Rotate".into(),
                PdfObject::PdfNumber(PdfNumber::new_from_lexeme(token.as_bytes()).unwrap()),
            )
            .unwrap();
            doc.update_object(location.page_id, PdfObject::PdfDict(page))
                .unwrap();
            let before = doc.get_page(0).unwrap().geometry().unwrap().rotation;
            assert_eq!(before, expected);
            doc.rotate_page(0, 90).unwrap();
            assert_eq!(
                doc.get_page(0).unwrap().geometry().unwrap().rotation,
                (before + 90) % 360
            );
            let reopened = PdfDocument::from_bytes_with_mode(
                doc.save_to_bytes().unwrap(),
                crate::parser::syntax::ParseMode::Strict,
            )
            .unwrap();
            assert_eq!(
                reopened.get_page(0).unwrap().geometry().unwrap().rotation,
                (before + 90) % 360
            );
        }
        for token in ["9223372036854775808", "2147483701", "90.0"] {
            let location = doc.page_location(0).unwrap();
            let mut page = location.page_dict;
            page.try_insert(
                "Rotate".into(),
                PdfObject::PdfNumber(PdfNumber::new_from_lexeme(token.as_bytes()).unwrap()),
            )
            .unwrap();
            doc.update_object(location.page_id, PdfObject::PdfDict(page))
                .unwrap();
            let revision = doc.revision();
            assert!(doc.get_page(0).unwrap().geometry().is_err());
            assert!(doc.rotate_page(0, 90).is_err());
            assert_eq!(doc.revision(), revision);
        }
    }

    #[test]
    fn indirect_box_coordinates_support_read_crop_reset_and_strict_save() {
        use crate::objects::pdf_array::PdfArray;
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let mut array = PdfArray::default();
        let mut ids = Vec::new();
        for value in [10., 20., 210., 120.] {
            let id = doc
                .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(value)))
                .unwrap();
            ids.push(id);
            array
                .try_add_obj(PdfObject::PdfReference(PdfReference::from_id(id)))
                .unwrap();
        }
        let box_id = doc.try_insert_object(PdfObject::PdfArray(array)).unwrap();
        let location = doc.page_location(0).unwrap();
        let mut page = location.page_dict;
        page.try_insert(
            "MediaBox".into(),
            PdfObject::PdfReference(PdfReference::from_id(box_id)),
        )
        .unwrap();
        page.try_insert(
            "CropBox".into(),
            PdfObject::PdfReference(PdfReference::from_id(box_id)),
        )
        .unwrap();
        doc.update_object(location.page_id, PdfObject::PdfDict(page))
            .unwrap();
        let mut retained = doc.get_page(0).unwrap();
        let list = retained.display_list().unwrap();
        assert_eq!((list.page_width(), list.page_height()), (200., 100.));
        doc.set_page_crop_box(0, None).unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        let list = reopened.get_page(0).unwrap().display_list().unwrap();
        assert_eq!((list.page_width(), list.page_height()), (200., 100.));
        doc.update_object(ids[2], PdfObject::PdfNumber(PdfNumber::new(310.)))
            .unwrap();
        // Reset the explicit crop to adopt the newly resolved media width.
        doc.set_page_crop_box(0, None).unwrap();
        assert_eq!(retained.display_list().unwrap().page_width(), 300.);
        doc.update_object(ids[0], PdfObject::PdfNull).unwrap();
        let revision = doc.revision();
        assert!(doc.set_page_crop_box(0, None).is_err());
        assert!(retained.display_list().is_err());
        assert_eq!(doc.revision(), revision);
    }

    #[test]
    fn relative_rotation_uses_inheritance_and_preserves_neighbors() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        for index in 0..2 {
            let location = doc.page_location(index).unwrap();
            let mut page = location.page_dict;
            page.remove("Rotate");
            doc.update_object(location.page_id, PdfObject::PdfDict(page))
                .unwrap();
        }
        let (root_id, mut root) = doc.root_pages().unwrap();
        let angle_id = doc
            .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(-90.0)))
            .unwrap();
        root.try_insert(
            "Rotate".into(),
            PdfObject::PdfReference(PdfReference::from_id(angle_id)),
        )
        .unwrap();
        doc.update_object(root_id, PdfObject::PdfDict(root.clone()))
            .unwrap();
        doc.rotate_page(0, 180).unwrap();
        let edited = doc.page_location(0).unwrap().page_dict;
        assert_eq!(edited.get("Rotate").unwrap().as_f32(), Some(90.0));
        assert!(
            !doc.page_location(1)
                .unwrap()
                .page_dict
                .contains_key("Rotate")
        );
        assert_eq!(doc.resolve_object(root_id).unwrap().as_dict(), Some(&root));
        let revision = doc.revision();
        assert!(doc.rotate_page(0, 45).is_err());
        assert!(doc.rotate_page(2, 90).is_err());
        assert_eq!(doc.revision(), revision);
        let result: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.rotate_page(0, -180)?;
            Err(PdfError::DocumentError("rollback".into()))
        });
        assert!(result.is_err());
        assert_eq!(doc.page_location(0).unwrap().page_dict, edited);
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(reopened.page_location(0).unwrap().page_dict, edited);
        assert_eq!(
            reopened.resolve_object(root_id).unwrap().as_dict(),
            Some(&root)
        );
        doc.rotate_page(0, 2_147_483_610).unwrap();
        assert_eq!(
            doc.page_location(0)
                .unwrap()
                .page_dict
                .get("Rotate")
                .unwrap()
                .as_f32(),
            Some(180.0)
        );
    }

    #[test]
    fn relative_rotation_rejects_invalid_inherited_value_without_mutation() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let location = doc.page_location(0).unwrap();
        let mut page = location.page_dict;
        page.remove("Rotate");
        doc.update_object(location.page_id, PdfObject::PdfDict(page))
            .unwrap();
        let (root_id, mut root) = doc.root_pages().unwrap();
        for angle in [45.0, 90.5, f32::INFINITY] {
            root.try_insert("Rotate".into(), PdfObject::PdfNumber(PdfNumber::new(angle)))
                .unwrap();
            doc.update_object(root_id, PdfObject::PdfDict(root.clone()))
                .unwrap();
            let before = doc.page_location(0).unwrap().page_dict;
            let revision = doc.revision();
            assert!(doc.rotate_page(0, 90).is_err());
            assert_eq!(doc.revision(), revision);
            assert_eq!(doc.page_location(0).unwrap().page_dict, before);
        }
    }
    #[test]
    fn geometry_edits_are_visible_persistent_and_atomic() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let revision = doc.revision();
        assert!(doc.set_page_rotation(0, 45).is_err());
        for crop in [
            Rectangle::new(0.0, 0.0, 0.0, 1.0),
            Rectangle::new(f32::NAN, 0.0, 1.0, 1.0),
            Rectangle::new(1000.0, 1000.0, 1100.0, 1100.0),
        ] {
            assert!(doc.set_page_crop_box(0, Some(crop)).is_err());
            assert_eq!(doc.revision(), revision);
        }
        assert!(doc.set_page_rotation(1, 90).is_err());
        assert!(doc.set_page_crop_box(1, None).is_err());
        doc.set_page_rotation(0, -90).unwrap();
        doc.set_page_crop_box(0, Some(Rectangle::new(10.0, 20.0, 100.0, 200.0)))
            .unwrap();
        let before = doc.page_location(0).unwrap().page_dict;
        assert_eq!(
            before
                .get("Rotate")
                .unwrap()
                .as_number()
                .unwrap()
                .as_i32_checked()
                .unwrap(),
            270
        );
        let rollback: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.set_page_rotation(0, 180)?;
            doc.set_page_crop_box(0, None)?;
            Err(PdfError::DocumentError("test rollback".into()))
        });
        assert!(rollback.is_err());
        assert_eq!(doc.page_location(0).unwrap().page_dict, before);
        let reopened = PdfDocument::from_bytes_with_mode(
            doc.save_to_bytes().unwrap(),
            crate::parser::syntax::ParseMode::Strict,
        )
        .unwrap();
        assert_eq!(reopened.page_location(0).unwrap().page_dict, before);
    }

    #[test]
    fn crop_reset_overrides_inherited_crop_without_changing_parent() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let (id, mut root) = doc.root_pages().unwrap();
        root.try_insert(
            "CropBox".into(),
            PdfObject::PdfArray(
                super::super::rect_to_array(Rectangle::new(0.0, 0.0, 100.0, 100.0)).unwrap(),
            ),
        )
        .unwrap();
        root.try_insert("Rotate".into(), PdfObject::PdfNumber(PdfNumber::new(90.0)))
            .unwrap();
        doc.update_object(id, PdfObject::PdfDict(root.clone()))
            .unwrap();
        doc.set_page_crop_box(0, None).unwrap();
        doc.set_page_rotation(0, 0).unwrap();
        let page = doc.page_location(0).unwrap().page_dict;
        assert_eq!(page.get("CropBox"), page.get("MediaBox"));
        assert_eq!(doc.resolve_object(id).unwrap().as_dict(), Some(&root));
        assert!(
            !doc.page_location(1)
                .unwrap()
                .page_dict
                .contains_key("CropBox")
        );
    }
}
