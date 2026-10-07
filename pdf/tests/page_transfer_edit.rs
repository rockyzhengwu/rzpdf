use pdf::{
    document::{PdfDocument, resource_manager::StandardFont},
    geom::rectangle::Rectangle,
    parser::syntax::ParseMode,
};

#[test]
fn user_unit_is_leaf_only_even_after_copy_and_import() {
    use pdf::{
        document::save_options::PdfVersion,
        objects::{
            PdfObject, object_id::ObjectId, pdf_number::PdfNumber, pdf_reference::PdfReference,
        },
        page::page_builder::PageBuilder,
    };
    let fixture = PdfDocument::new_with_version(PdfVersion::V1_6).unwrap();
    for index in 0..4 {
        fixture
            .add_page_with_builder(index, PageBuilder::new().size(80., 40.))
            .unwrap();
    }
    // Object 1 is the Pages root of this explicitly new-document fixture only.
    let root_id = ObjectId::new(1, 0);
    let mut root = fixture
        .resolve_object(root_id)
        .unwrap()
        .into_dict()
        .unwrap();
    let ids = root
        .get("Kids")
        .unwrap()
        .as_array()
        .unwrap()
        .into_iter()
        .map(|value| value.as_reference().unwrap().id())
        .collect::<Vec<_>>();
    root.try_insert("UserUnit".into(), PdfObject::PdfNumber(PdfNumber::new(9.)))
        .unwrap();
    fixture
        .update_object(root_id, PdfObject::PdfDict(root))
        .unwrap();
    let null_id = fixture.try_insert_object(PdfObject::PdfNull).unwrap();
    let unit_id = fixture
        .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(2.)))
        .unwrap();
    for (index, value) in [
        (1, PdfObject::PdfNull),
        (2, PdfObject::PdfReference(PdfReference::from_id(null_id))),
        (3, PdfObject::PdfReference(PdfReference::from_id(unit_id))),
    ] {
        let mut leaf = fixture
            .resolve_object(ids[index])
            .unwrap()
            .into_dict()
            .unwrap();
        leaf.try_insert("UserUnit".into(), value).unwrap();
        fixture
            .update_object(ids[index], PdfObject::PdfDict(leaf))
            .unwrap();
    }
    let source =
        PdfDocument::from_bytes_with_mode(fixture.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    for index in 0..4 {
        source.copy_page(index, 4 + index).unwrap();
    }
    let stamp = source.stamp();
    let target = PdfDocument::new_with_version(PdfVersion::V1_6).unwrap();
    target
        .import_pages_from(&source, &[0, 1, 2, 3, 4, 5, 6, 7], 0)
        .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(target.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    for doc in [&source, &target, &reopened] {
        for index in 0..8 {
            let geometry = doc.get_page(index).unwrap().geometry().unwrap();
            let unit = if index % 4 == 3 { 2. } else { 1. };
            assert_eq!(
                (
                    geometry.user_unit,
                    geometry.display_width,
                    geometry.display_height
                ),
                (unit, 80. * unit, 40. * unit)
            );
        }
    }
    // Removing a real leaf value restores 1 even while its parent declares 9.
    source.set_page_user_unit(3, None).unwrap();
    assert_eq!(
        source.get_page(3).unwrap().geometry().unwrap().user_unit,
        1.
    );
    assert_eq!(
        source.get_page(7).unwrap().geometry().unwrap().user_unit,
        2.
    );
    assert!(!source.is_stamp_current(&stamp));
    assert_eq!(
        target.get_page(3).unwrap().geometry().unwrap().user_unit,
        2.
    );
}

#[test]
fn user_unit_import_merge_and_extract_preserve_units_and_version_boundary() {
    use pdf::{document::save_options::PdfVersion, page::page_builder::PageBuilder};
    let source = PdfDocument::new_with_version(PdfVersion::V1_6).unwrap();
    for (index, unit) in [0.5, 1.5, 2.5].into_iter().enumerate() {
        source
            .add_page_with_builder(
                index as u32,
                PageBuilder::new()
                    .size(80., 40.)
                    .rotation(index as i32 * 90)
                    .user_unit(unit),
            )
            .unwrap();
    }
    let source =
        PdfDocument::from_bytes_with_mode(source.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    let stamp = source.stamp();
    let old = PdfDocument::new_with_version(PdfVersion::V1_5).unwrap();
    old.add_page(0).unwrap();
    let old_stamp = old.stamp();
    assert!(old.import_pages_from(&source, &[2, 0], 1).is_err());
    assert!(old.merge_document(&source).is_err());
    assert!(old.is_stamp_current(&old_stamp));
    assert_eq!(old.try_page_count().unwrap(), 1);
    assert_eq!(old.effective_version().unwrap(), PdfVersion::V1_5);
    let old =
        PdfDocument::from_bytes_with_mode(old.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(old.try_page_count().unwrap(), 1);
    assert_eq!(old.get_page(0).unwrap().geometry().unwrap().user_unit, 1.);

    let target = PdfDocument::new_with_version(PdfVersion::V1_6).unwrap();
    target.import_pages_from(&source, &[2, 0], 0).unwrap();
    target.merge_document(&source).unwrap();
    let extracted = target.extract_pages(&[4, 1, 3]).unwrap();
    assert_eq!(extracted.effective_version().unwrap(), PdfVersion::V1_6);
    let reopened =
        PdfDocument::from_bytes_with_mode(extracted.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    for doc in [&extracted, &reopened] {
        for (index, unit, width, height, rotation) in [
            (0, 2.5, 200., 100., 180),
            (1, 0.5, 40., 20., 0),
            (2, 1.5, 60., 120., 90),
        ] {
            let geometry = doc.get_page(index).unwrap().geometry().unwrap();
            assert_eq!(
                (
                    geometry.user_unit,
                    geometry.display_width,
                    geometry.display_height,
                    geometry.rotation
                ),
                (unit, width, height, rotation)
            );
        }
    }
    target.set_page_user_unit(0, None).unwrap();
    assert_eq!(
        target.get_page(0).unwrap().geometry().unwrap().user_unit,
        1.
    );
    assert_eq!(
        target.get_page(4).unwrap().geometry().unwrap().user_unit,
        2.5
    );
    assert_eq!(
        source.get_page(2).unwrap().geometry().unwrap().user_unit,
        2.5
    );
    assert!(source.is_stamp_current(&stamp));
}

#[test]
fn inherited_indirect_boxes_import_and_crop_reset_are_page_local() {
    use pdf::objects::{
        PdfObject, pdf_array::PdfArray, pdf_number::PdfNumber, pdf_reference::PdfReference,
    };
    fn box_reference(
        doc: &PdfDocument<std::io::Cursor<Vec<u8>>>,
        coordinates: [f32; 4],
    ) -> PdfObject {
        let mut array = PdfArray::default();
        for coordinate in coordinates {
            let id = doc
                .try_insert_object(PdfObject::PdfNumber(PdfNumber::new(coordinate)))
                .unwrap();
            array
                .try_add_obj(PdfObject::PdfReference(PdfReference::from_id(id)))
                .unwrap();
        }
        let id = doc.try_insert_object(PdfObject::PdfArray(array)).unwrap();
        PdfObject::PdfReference(PdfReference::from_id(id))
    }
    let fixture = PdfDocument::new().unwrap();
    fixture.add_page(0).unwrap();
    fixture.add_page(1).unwrap();
    // The new-document fixture creates /Pages as object 1 (not a loaded-file assumption).
    let root_id = pdf::objects::object_id::ObjectId::new(1, 0);
    let mut root = fixture
        .resolve_object(root_id)
        .unwrap()
        .into_dict()
        .unwrap();
    let page_ids = root
        .get("Kids")
        .unwrap()
        .as_array()
        .unwrap()
        .into_iter()
        .map(|page| page.as_reference().unwrap().id())
        .collect::<Vec<_>>();
    root.try_insert(
        "MediaBox".into(),
        box_reference(&fixture, [10., 20., 210., 120.]),
    )
    .unwrap();
    root.try_insert(
        "CropBox".into(),
        box_reference(&fixture, [20., 30., 100., 90.]),
    )
    .unwrap();
    root.try_insert("Rotate".into(), PdfObject::PdfNumber(PdfNumber::new(90.)))
        .unwrap();
    fixture
        .update_object(root_id, PdfObject::PdfDict(root))
        .unwrap();
    for id in page_ids {
        let mut page = fixture.resolve_object(id).unwrap().into_dict().unwrap();
        for key in ["MediaBox", "CropBox", "Rotate"] {
            page.remove(key);
        }
        fixture.update_object(id, PdfObject::PdfDict(page)).unwrap();
    }
    // Start from actual parsed indirect objects, not just the in-memory store.
    let source =
        PdfDocument::from_bytes_with_mode(fixture.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    let source_stamp = source.stamp();
    let target = PdfDocument::new().unwrap();
    target.import_pages_from(&source, &[1, 0], 0).unwrap();
    for doc in [&source, &target] {
        for index in 0..2 {
            let list = doc.get_page(index).unwrap().display_list().unwrap();
            assert_eq!((list.page_width(), list.page_height()), (60., 80.));
        }
    }
    let mut retained = target.get_page(0).unwrap();
    retained.display_list().unwrap();
    target.set_page_crop_box(0, None).unwrap();
    let list = retained.display_list().unwrap();
    assert_eq!((list.page_width(), list.page_height()), (100., 200.));
    let reopened =
        PdfDocument::from_bytes_with_mode(target.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    for doc in [&target, &reopened] {
        for (index, expected) in [(0, (100., 200.)), (1, (60., 80.))] {
            let list = doc.get_page(index).unwrap().display_list().unwrap();
            assert_eq!((list.page_width(), list.page_height()), expected);
        }
    }
    for index in 0..2 {
        let list = source.get_page(index).unwrap().display_list().unwrap();
        assert_eq!((list.page_width(), list.page_height()), (60., 80.));
    }
    assert!(source.is_stamp_current(&source_stamp));
}

#[test]
fn rotated_page_extract_merge_and_content_edit_preserve_sources() {
    let source = PdfDocument::new().unwrap();
    source.add_page(0).unwrap();
    source
        .set_page_crop_box(0, Some(Rectangle::new(10., 20., 220., 200.)))
        .unwrap();
    let font = source
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let image = source
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    for page in 0..4 {
        if page != 0 {
            source.copy_page(0, page).unwrap();
        }
        source.set_page_rotation(page, (page * 90) as i32).unwrap();
        source.replace_page_contents(page, format!("BT /{font} 12 Tf 1 0 0 1 40 80 Tm (Page{page}) Tj ET q 20 0 0 10 30 50 cm /{image} Do Q").into_bytes()).unwrap();
    }
    let source_stamp = source.stamp();
    let selected = source.extract_pages(&[3, 1]).unwrap();
    let selected_stamp = selected.stamp();
    let target = PdfDocument::new().unwrap();
    let handles = target.merge_document(&selected).unwrap();
    assert_eq!(handles.len(), 2);
    assert!(source.is_stamp_current(&source_stamp));
    assert!(selected.is_stamp_current(&selected_stamp));
    let before = target.save_to_bytes().unwrap();
    let reopened = PdfDocument::from_bytes_with_mode(before, ParseMode::Strict).unwrap();
    for (target_index, source_index) in [(0, 3), (1, 1)] {
        let mut original = source.get_page(source_index).unwrap();
        let original_chars = original.extract_characters_paint_order().unwrap();
        let original_images = original.extract_images_paint_order().unwrap();
        let original_list = original.display_list().unwrap();
        for document in [&target, &reopened] {
            let mut page = document.get_page(target_index).unwrap();
            let list = page.display_list().unwrap();
            assert_eq!(
                (list.page_width(), list.page_height()),
                (original_list.page_width(), original_list.page_height())
            );
            let chars = page.extract_characters_paint_order().unwrap();
            assert_eq!(chars.len(), original_chars.len());
            for (actual, expected) in chars.iter().zip(&original_chars) {
                assert_eq!(actual.unicode, expected.unicode);
                assert_eq!(
                    (actual.position.x(), actual.position.y()),
                    (expected.position.x(), expected.position.y())
                );
                assert_eq!(actual.writing_axis(), expected.writing_axis());
            }
            let images = page.extract_images_paint_order().unwrap();
            assert_eq!(images.len(), 1);
            assert_eq!(images[0].image.rgba_buffer().unwrap(), [255, 0, 0, 255]);
            for (x, y) in [(0., 0.), (1., 0.), (0., 1.), (1., 1.)] {
                let point = pdf::geom::point::Point::new(x, y);
                let actual = images[0].matrix.transform(&point);
                let expected = original_images[0].matrix.transform(&point);
                assert_eq!((actual.x(), actual.y()), (expected.x(), expected.y()));
            }
        }
    }
    let snapshot = target.get_page(0).unwrap().content_program().unwrap();
    let operation = snapshot
        .operations()
        .iter()
        .position(|op| op.operator.name() == "Tj")
        .unwrap();
    target
        .remove_page_text_draw(0, &snapshot, operation)
        .unwrap();
    let edited =
        PdfDocument::from_bytes_with_mode(target.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    assert!(
        edited
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()
            .is_empty()
    );
    assert_eq!(
        edited
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()
            .len(),
        1
    );
    for (document, index, expected) in [
        (&source, 3, "Page3"),
        (&selected, 0, "Page3"),
        (&edited, 1, "Page1"),
    ] {
        let text = document
            .get_page(index)
            .unwrap()
            .extract_characters_paint_order()
            .unwrap()
            .iter()
            .map(|ch| ch.unicode.as_deref().unwrap())
            .collect::<String>();
        assert_eq!(text, expected);
    }
    assert!(source.is_stamp_current(&source_stamp));
    assert!(selected.is_stamp_current(&selected_stamp));
}
