use pdf::{
    document::{owned::Document, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_object::PageObject,
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

#[test]
fn imported_form_keeps_private_font_and_image_scope_after_source_release() {
    use pdf::document::resource_manager::StandardFont;
    let target = Document::new().unwrap();
    target.add_page(0).unwrap();
    // Page resources have different payloads from the Form's private scope.
    target
        .resources()
        .register_standard_font(0, StandardFont::Courier)
        .unwrap();
    target
        .resources()
        .embed_rgb_image(0, 1, 1, vec![0, 255, 0])
        .unwrap();
    let imported = {
        let source = Document::new().unwrap();
        source.add_page(0).unwrap();
        let font = source
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        let image = source
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
            PdfObject::PdfDict(source.get_page(0).unwrap().resource().unwrap()),
        );
        let body = format!("BT /{font} 12 Tf (AB) Tj ET q 5 0 0 5 20 20 cm /{image} Do Q");
        let id = source
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, body.into_bytes()).unwrap(),
            ))
            .unwrap();
        target
            .import_resource_from(
                &source,
                &source.object_handle(id).unwrap(),
                ResourceKind::Form,
            )
            .unwrap()
    };
    let name = target
        .resources()
        .register_handle(0, ResourceKind::Form, &imported)
        .unwrap();
    target
        .replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    target.copy_page(0, 1).unwrap();
    let reopened =
        Document::from_bytes_with_mode(target.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&target, &reopened] {
        for index in [0, 1] {
            let mut page = document.get_page(index).unwrap();
            let root = page.content_program().unwrap();
            let text = page.extract_characters_paint_order_report().unwrap();
            assert_eq!(
                text.characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                "ABAB"
            );
            assert!(
                text.characters
                    .iter()
                    .all(|ch| ch.font_name.as_deref() == Some("Helvetica"))
            );
            assert!(text.characters.iter().all(|ch| {
                ch.nested_source.as_ref().unwrap().invocations[0].form_id
                    == Some(imported.object_id())
            }));
            let images = page.extract_images_paint_order().unwrap();
            assert_eq!(images.len(), 2);
            for image in images {
                assert_eq!(image.image.rgba_buffer().unwrap(), [255, 0, 0, 255]);
                assert!(image.source().is_none());
                let nested = image.nested_source().unwrap();
                assert_eq!(nested.invocations[0].form_id, Some(imported.object_id()));
                let leaf = page.nested_content_program(&root, nested).unwrap();
                assert_eq!(
                    leaf.operations()[nested.operation_index].operator.name(),
                    "Do"
                );
            }
        }
    }
}

#[test]
fn imported_nested_form_replaces_one_call_after_source_is_released() {
    fn insert_form(
        doc: &Document,
        dictionary: &str,
        body: &[u8],
    ) -> pdf::objects::object_id::ObjectId {
        let dict = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(dictionary.as_bytes().to_vec())).unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        doc.try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, body.to_vec()).unwrap(),
        ))
        .unwrap()
    }
    let target = Document::new().unwrap();
    target.add_page(0).unwrap();
    let old = insert_form(
        &target,
        "<< /Subtype /Form /BBox [0 0 50 50] /Resources << >> >>",
        b"0 0 2 2 re f",
    );
    let name = target
        .resources()
        .register_existing(0, ResourceKind::Form, old)
        .unwrap();
    target
        .replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    target.copy_page(0, 1).unwrap();
    let original = target
        .get_page(0)
        .unwrap()
        .content_program()
        .unwrap()
        .source_bytes()
        .to_vec();
    let imported = {
        let source = Document::new().unwrap();
        let leaf = insert_form(
            &source,
            "<< /Subtype /Form /BBox [0 0 50 50] /Matrix [1 0 0 1 2 3] /Resources << >> >>",
            b"0 0 3 3 re f",
        );
        let outer = insert_form(
            &source,
            &format!(
                "<< /Subtype /Form /BBox [0 0 50 50] /Matrix [1 0 0 1 7 8] /Resources << /XObject << /Child {} 0 R >> >> >>",
                leaf.obj_num()
            ),
            b"/Child Do",
        );
        target
            .import_resource_from(
                &source,
                &source.object_handle(outer).unwrap(),
                ResourceKind::Form,
            )
            .unwrap()
    };
    let snapshot = target.get_page(0).unwrap().content_program().unwrap();
    target
        .replace_page_form_draw(0, &snapshot, 1, &imported)
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(target.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&target, &reopened] {
        let mut page = document.get_page(0).unwrap();
        let root = page.content_program().unwrap();
        let list = document.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(list.objects().len(), 2);
        let PageObject::Path(first) = &list.objects()[0] else {
            panic!("missing original Form")
        };
        assert_eq!(
            first.nested_source().unwrap().invocations[0].form_id,
            Some(old)
        );
        let PageObject::Path(second) = &list.objects()[1] else {
            panic!("missing imported nested Form")
        };
        let source = second.nested_source().unwrap();
        assert_eq!(source.invocations.len(), 2);
        assert_eq!(source.invocations[0].form_id, Some(imported.object_id()));
        assert_eq!(second.matrix().e(), 9.);
        assert_eq!(
            second.matrix().f(),
            page.geometry().unwrap().display_height - 11.
        );
        assert_eq!(
            page.nested_content_program(&root, source)
                .unwrap()
                .source_bytes(),
            b"0 0 3 3 re f"
        );
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            original
        );
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .display_list()
                .unwrap()
                .objects()
                .len(),
            2
        );
    }
}

#[test]
fn form_replacement_isolates_one_call_and_rolls_back_resource_registration() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let dictionary = SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            b"<< /Subtype /Form /BBox [0 0 20 20] /Resources << >> >>".to_vec(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    let old = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dictionary.clone(), b"0 0 2 2 re f".to_vec()).unwrap(),
        ))
        .unwrap();
    let new = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dictionary.clone(), b"0 0 3 3 re f".to_vec()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, old)
        .unwrap();
    doc.replace_page_contents(0, format!("q /{name} Do /{name} D").into_bytes())
        .unwrap();
    doc.append_page_contents(0, b"o Q".to_vec()).unwrap();
    doc.copy_page(0, 1).unwrap();
    let handle = doc.object_handle(new).unwrap();
    let non_stream = doc
        .try_insert_object(PdfObject::PdfDict(Default::default()))
        .unwrap();
    let mut malformed = dictionary;
    malformed.remove("BBox");
    let malformed = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(malformed, Vec::new()).unwrap(),
        ))
        .unwrap();
    let original = doc.get_page(0).unwrap().content_program().unwrap();
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let revision = doc.revision();
    let other_page = doc.get_page(1).unwrap().content_program().unwrap();
    let page = doc.get_page(0).unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("missing Form path")
    };
    let leaf = page
        .nested_content_program(&original, path.nested_source().unwrap())
        .unwrap();
    assert!(
        doc.replace_page_form_draw(0, &other_page, 2, &handle)
            .is_err()
    );
    assert!(doc.replace_page_form_draw(0, &leaf, 1, &handle).is_err());
    for index in [0, 3, usize::MAX] {
        assert!(
            doc.replace_page_form_draw(0, &original, index, &handle)
                .is_err()
        );
    }
    for id in [non_stream, malformed] {
        assert!(
            doc.replace_page_form_draw(0, &original, 2, &doc.object_handle(id).unwrap())
                .is_err()
        );
    }
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(unchanged.source_bytes(), original.source_bytes());
    for index in 0..2 {
        assert_eq!(
            unchanged.stream_object_id(index),
            original.stream_object_id(index)
        );
    }
    let result: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_page_form_draw(0, &original, 2, &handle)?;
        assert_ne!(editing.get_page(0)?.resource()?, resources);
        Err(pdf::error::PdfError::ObjectError(
            "injected Form replacement rollback".into(),
        ))
    });
    assert!(result.is_err());
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(snapshot.source_bytes(), original.source_bytes());
    for index in 0..2 {
        assert_eq!(
            snapshot.stream_object_id(index),
            original.stream_object_id(index)
        );
    }
    let foreign =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let foreign_handle = foreign.object_handle(new).unwrap();
    let revision = doc.revision();
    assert!(
        doc.replace_page_form_draw(0, &snapshot, 2, &foreign_handle)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    doc.replace_page_form_draw(0, &snapshot, 2, &handle)
        .unwrap();
    assert!(
        doc.replace_page_form_draw(0, &snapshot, 2, &handle)
            .is_err()
    );
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for (page, expected) in [(0, [old, new]), (1, [old, old])] {
            let list = document.get_page(page).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 2);
            for (object, id) in list.objects().iter().zip(expected) {
                let PageObject::Path(path) = object else {
                    panic!("missing Form path")
                };
                assert_eq!(
                    path.nested_source().unwrap().invocations[0].form_id,
                    Some(id)
                );
            }
        }
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            original.source_bytes()
        );
        assert_eq!(
            document
                .resolve_object(old)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"0 0 2 2 re f"
        );
        assert_eq!(
            document
                .resolve_object(new)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"0 0 3 3 re f"
        );
    }
}
