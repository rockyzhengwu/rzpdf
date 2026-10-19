use pdf::{
    document::{owned::Document, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

#[test]
fn nested_image_transform_is_relative_and_restores_neighbor_state() {
    for (transform, linear, delta) in [
        ([1., 0., 0., 1., 1., 2.], [4., 0., 0., 5.], [4., -10.]),
        ([0., 1., -1., 0., 4., 5.], [0., -5., 4., 0.], [12., -20.]),
        ([4., 0., 0., 0.5, 1., 2.], [16., 0., 0., 2.5], [4., -7.5]),
        ([-1., 0., 0., 1., 0., 0.], [-4., 0., 0., 5.], [0., 0.]),
    ] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let image = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        let mut dict = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Subtype /Form /BBox [0 0 100 100] >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        dict.insert(
            "Resources".into(),
            PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
        );
        let body = format!("q 4 0 0 5 10 10 cm /{image} Do /{image} Do Q");
        let form = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, body.as_bytes().to_vec()).unwrap(),
            ))
            .unwrap();
        let outer_dict = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                format!(
        "<< /Subtype /Form /BBox [0 0 100 100] /Resources << /XObject << /Child {} 0 R >> >> >>",
        form.obj_num())
                .into_bytes(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let outer_body = b"/Child Do";
        let outer = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(outer_dict, outer_body.to_vec()).unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, outer)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
            .unwrap();
        doc.copy_page(0, 1).unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let before = doc
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        let source = before[0].nested_source().unwrap();
        assert_eq!(source.invocations.len(), 2);
        let revision = doc.revision();
        assert!(
            doc.transform_nested_image_draw(0, &root, source, [1., 0., 0., 1., f32::NAN, 0.])
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let rollback: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
            editing.transform_nested_image_draw(0, &root, source, [1., 0., 0., 1., 1., 2.])?;
            let temporary = editing.get_page(0)?.extract_images_paint_order()?;
            assert_eq!(temporary[0].matrix.e(), before[0].matrix.e() + 4.);
            assert_eq!(temporary[0].matrix.f(), before[0].matrix.f() - 10.);
            assert_eq!(temporary[1].matrix.e(), before[1].matrix.e());
            Err(pdf::error::PdfError::ObjectError(
                "injected image transform rollback".into(),
            ))
        });
        assert!(rollback.is_err());
        let refreshed = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(refreshed.source_bytes(), root.source_bytes());
        assert_eq!(refreshed.stream_object_id(0), root.stream_object_id(0));
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        let restored = doc
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        for (image, original) in restored.iter().zip(&before) {
            let matrix = &image.matrix;
            let old = &original.matrix;
            assert_eq!(
                [
                    matrix.a(),
                    matrix.b(),
                    matrix.c(),
                    matrix.d(),
                    matrix.e(),
                    matrix.f()
                ],
                [old.a(), old.b(), old.c(), old.d(), old.e(), old.f()]
            );
        }
        assert!(
            doc.transform_nested_image_draw(0, &root, source, [1., 0., 0., 1., 1., 2.])
                .is_err()
        );
        doc.transform_nested_image_draw(0, &refreshed, source, transform)
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            for page in [0, 1] {
                let images = document
                    .get_page(page)
                    .unwrap()
                    .extract_images_paint_order()
                    .unwrap();
                assert_eq!(images.len(), 4);
                for (index, image) in images.iter().enumerate() {
                    let matrix = &image.matrix;
                    let original = &before[index].matrix;
                    let changed = page == 0 && index == 0;
                    assert_eq!(
                        [matrix.a(), matrix.b(), matrix.c(), matrix.d()],
                        if changed {
                            linear
                        } else {
                            [original.a(), original.b(), original.c(), original.d()]
                        }
                    );
                    let calls = &image.nested_source().unwrap().invocations;
                    assert_eq!(calls.len(), 2);
                    if page == 0 && index < 2 {
                        assert_ne!(calls[0].form_id, Some(outer));
                        assert_ne!(calls[1].form_id, Some(form));
                    } else {
                        assert_eq!(calls[0].form_id, Some(outer));
                        assert_eq!(calls[1].form_id, Some(form));
                    }
                    assert_eq!(
                        matrix.e(),
                        original.e() + if changed { delta[0] } else { 0. }
                    );
                    // Extracted coordinates are display-space, with Y pointing down.
                    assert_eq!(
                        matrix.f(),
                        original.f() + if changed { delta[1] } else { 0. }
                    );
                    assert_eq!(image.image.rgba_buffer().unwrap(), [255, 0, 0, 255]);
                }
            }
            assert_eq!(
                document
                    .resolve_object(outer)
                    .unwrap()
                    .into_stream()
                    .unwrap()
                    .raw_data(),
                outer_body
            );
            assert_eq!(
                document
                    .resolve_object(form)
                    .unwrap()
                    .into_stream()
                    .unwrap()
                    .raw_data(),
                body.as_bytes()
            );
        }
        let blue_dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(
            b"<< /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB >>".to_vec()
        )).unwrap()).get_object().unwrap().into_dict().unwrap();
        let blue = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(blue_dict, vec![0, 0, 255]).unwrap(),
            ))
            .unwrap();
        let non_stream = doc.try_insert_object(PdfObject::PdfNull).unwrap();
        let foreign = Document::new().unwrap();
        let foreign_image = doc.resolve_object(blue).unwrap();
        let mut foreign_id = foreign.try_insert_object(foreign_image.clone()).unwrap();
        while foreign_id.obj_num() < blue.obj_num() {
            foreign_id = foreign.try_insert_object(foreign_image.clone()).unwrap();
        }
        assert_eq!(foreign_id, blue);
        let foreign_handle = foreign.object_handle(foreign_id).unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let original = doc
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        let source = original[0].nested_source().unwrap();
        let wrong = doc.object_handle(form).unwrap();
        let revision = doc.revision();
        assert!(
            doc.replace_nested_image_draw(0, &root, source, &wrong)
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        for invalid in [&foreign_handle, &doc.object_handle(non_stream).unwrap()] {
            assert!(
                doc.replace_nested_image_draw(0, &root, source, invalid)
                    .is_err()
            );
            assert_eq!(doc.revision(), revision);
            assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
            let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
            assert_eq!(unchanged.source_bytes(), root.source_bytes());
            assert_eq!(unchanged.stream_object_id(0), root.stream_object_id(0));
        }
        let imported = doc
            .import_resource_from(&foreign, &foreign_handle, ResourceKind::Image)
            .unwrap();
        drop(foreign);
        assert!(
            doc.replace_nested_image_draw(0, &root, source, &imported)
                .is_err()
        );
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let handle = imported;
        let rollback: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
            editing.replace_nested_image_draw(0, &root, source, &handle)?;
            assert_eq!(
                editing.get_page(0)?.extract_images_paint_order()?[0]
                    .image
                    .rgba_buffer()?,
                [0, 0, 255, 255]
            );
            Err(pdf::error::PdfError::ObjectError(
                "injected replacement rollback".into(),
            ))
        });
        assert!(rollback.is_err());
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        let refreshed = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(refreshed.source_bytes(), root.source_bytes());
        assert_eq!(refreshed.stream_object_id(0), root.stream_object_id(0));
        doc.replace_nested_image_draw(0, &refreshed, source, &handle)
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            for page in [0, 1] {
                let images = document
                    .get_page(page)
                    .unwrap()
                    .extract_images_paint_order()
                    .unwrap();
                assert_eq!(images.len(), 4);
                for (index, image) in images.iter().enumerate() {
                    let expected = if page == 0 && index == 0 {
                        [0, 0, 255, 255]
                    } else {
                        [255, 0, 0, 255]
                    };
                    assert_eq!(image.image.rgba_buffer().unwrap(), expected);
                    let expected_matrix = if page == 0 {
                        &original[index].matrix
                    } else {
                        &before[index].matrix
                    };
                    let matrix = &image.matrix;
                    assert_eq!(
                        [
                            matrix.a(),
                            matrix.b(),
                            matrix.c(),
                            matrix.d(),
                            matrix.e(),
                            matrix.f()
                        ],
                        [
                            expected_matrix.a(),
                            expected_matrix.b(),
                            expected_matrix.c(),
                            expected_matrix.d(),
                            expected_matrix.e(),
                            expected_matrix.f()
                        ]
                    );
                }
            }
        }
    }
}

#[test]
fn nested_image_removal_isolates_one_call_keeps_shared_pixels_and_rolls_back() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let image = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    let mut dict = SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            b"<< /Subtype /Form /BBox [0 0 50 50] >>".to_vec(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    let empty = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict.clone(), Vec::new()).unwrap(),
        ))
        .unwrap();
    let empty_name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, empty)
        .unwrap();
    dict.insert(
        "Resources".into(),
        PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
    );
    let body =
        format!("q 5 0 0 5 10 10 cm /{image} Do /{image} Do Q /{empty_name} Do 20 20 2 2 re f");
    let id = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, body.as_bytes().to_vec()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, id)
        .unwrap();
    let outer_body = format!("/{name} Do");
    let mut outer_dict = SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            b"<< /Subtype /Form /BBox [0 0 50 50] >>".to_vec(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    outer_dict.insert(
        "Resources".into(),
        PdfObject::PdfDict(doc.get_page(0).unwrap().resource().unwrap()),
    );
    let outer = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(outer_dict, outer_body.as_bytes().to_vec()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, outer)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let images = doc
        .get_page(0)
        .unwrap()
        .extract_images_paint_order()
        .unwrap();
    assert_eq!(images.len(), 4);
    let source = images[0].nested_source().unwrap().clone();
    assert_eq!(source.invocations.len(), 2);
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let revision = doc.revision();
    let leaf = doc
        .get_page(0)
        .unwrap()
        .nested_content_program(&root, &source)
        .unwrap();
    for index in [0, 5] {
        let mut invalid = source.clone();
        invalid.operation_index = index;
        invalid.operator_byte_offset = leaf.operations()[index].operator.byte_offset();
        assert!(matches!(
            doc.remove_nested_image_draw(0, &root, &invalid),
            Err(pdf::error::PdfError::ObjectError(_))
        ));
        assert!(matches!(
            doc.transform_nested_image_draw(0, &root, &invalid, [1., 0., 0., 1., 1., 0.]),
            Err(pdf::error::PdfError::ObjectError(_))
        ));
    }
    let mut invalid = source.clone();
    invalid.invocations[0].form_id = None;
    assert!(doc.remove_nested_image_draw(0, &root, &invalid).is_err());
    assert!(doc.remove_nested_image_draw(1, &root, &source).is_err());
    assert!(doc.remove_nested_image_draw(0, &leaf, &source).is_err());
    for (page, snapshot, source) in [
        (0, &root, &invalid),
        (1, &root, &source),
        (0, &leaf, &source),
    ] {
        assert!(
            doc.transform_nested_image_draw(page, snapshot, source, [1., 0., 0., 1., 1., 0.])
                .is_err()
        );
    }
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .stream_object_id(0),
        root.stream_object_id(0)
    );
    let result: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
        editing.remove_nested_image_draw(0, &root, &source)?;
        assert_eq!(editing.get_page(0)?.extract_images_paint_order()?.len(), 3);
        Err(pdf::error::PdfError::ObjectError(
            "injected nested image rollback".into(),
        ))
    });
    assert!(result.is_err());
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    let fresh = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(fresh.stream_object_id(0), root.stream_object_id(0));
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap()
            .len(),
        4
    );
    doc.remove_nested_image_draw(0, &fresh, &source).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for (page, count) in [(0, 3), (1, 4)] {
            let images = document
                .get_page(page)
                .unwrap()
                .extract_images_paint_order()
                .unwrap();
            assert_eq!(images.len(), count);
            for (index, image) in images.iter().enumerate() {
                let chain = &image.nested_source().unwrap().invocations;
                assert_eq!(chain.len(), 2);
                if page == 0 && index == 0 {
                    assert_ne!(chain[0].form_id, Some(outer));
                    assert_ne!(chain[1].form_id, Some(id));
                } else {
                    assert_eq!(chain[0].form_id, Some(outer));
                    assert_eq!(chain[1].form_id, Some(id));
                }
            }
            assert!(
                images
                    .iter()
                    .all(|draw| draw.image.rgba_buffer().unwrap() == [255, 0, 0, 255])
            );
            assert_eq!(
                document
                    .get_page(page)
                    .unwrap()
                    .display_list()
                    .unwrap()
                    .objects()
                    .len(),
                count + 2
            );
        }
        assert_eq!(
            document
                .resolve_object(id)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            body.as_bytes()
        );
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
        assert_eq!(
            document
                .resolve_object(outer)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            outer_body.as_bytes()
        );
    }
}
