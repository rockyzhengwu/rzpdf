use pdf::{
    document::owned::Document,
    objects::{PdfObject, pdf_stream::PdfStream},
    parser::syntax::ParseMode,
};

#[test]
fn extracted_source_selects_single_cross_stream_draw_and_rejects_stale_snapshot() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(0, format!("% head\nq /{name} Do /{name} D").into_bytes())
        .unwrap();
    doc.append_page_contents(0, b"o Q\n% tail\n".to_vec())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let raw = doc
        .get_page(0)
        .unwrap()
        .extract_image_source(&name)
        .unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            raw.stream.dict().clone(),
            vec![0, 0, 255],
        )))
        .unwrap();
    let handle = doc.object_handle(id).unwrap();
    let mut page = doc.get_page(0).unwrap();
    let snapshot = page.content_program().unwrap();
    let draws = page.extract_images_paint_order().unwrap();
    let source = draws[1].source().unwrap();
    assert_eq!(source.operation_index, 2);
    assert_eq!(
        Some(source.operator_byte_offset),
        snapshot.operations()[source.operation_index]
            .operator
            .byte_offset()
    );
    let original = snapshot.source_bytes().to_vec();
    doc.replace_page_image_draw(0, &snapshot, source.operation_index, &handle)
        .unwrap();
    let revision = doc.revision();
    assert!(
        doc.replace_page_image_draw(0, &snapshot, source.operation_index, &handle)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let mut page = document.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        assert_eq!(program.stream_ranges().len(), 2);
        assert!(program.source_bytes().starts_with(b"% head\nq "));
        assert!(program.source_bytes().ends_with(b" Q\n% tail\n"));
        let images = page.extract_images_paint_order().unwrap();
        assert_eq!(images[0].image.rgba_buffer().unwrap(), [255, 0, 0, 255]);
        assert_eq!(images[1].image.rgba_buffer().unwrap(), [0, 0, 255, 255]);
        for (image, index) in images.iter().zip([1, 2]) {
            let fresh = image.source().unwrap();
            assert_eq!(fresh.operation_index, index);
            assert_eq!(
                Some(fresh.operator_byte_offset),
                program.operations()[index].operator.byte_offset()
            );
        }
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            original
        );
    }
}

#[test]
fn image_draw_reuses_and_escapes_existing_resource_name() {
    use pdf::objects::pdf_reference::PdfReference;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let old_name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("q /{old_name} Do /{old_name} Do Q\n").into_bytes(),
    )
    .unwrap();
    let page_id = doc.copy_page(0, 1).unwrap();
    let source = doc
        .get_page(0)
        .unwrap()
        .extract_image_source(&old_name)
        .unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            source.stream.dict().clone(),
            vec![0, 0, 255],
        )))
        .unwrap();
    let handle = doc.object_handle(id).unwrap();
    let mut page = doc.resolve_object(page_id).unwrap().into_dict().unwrap();
    let mut resources = page.get("Resources").unwrap().as_dict().unwrap().clone();
    let mut images = resources.get("XObject").unwrap().as_dict().unwrap().clone();
    images.insert(
        "A /#".into(),
        PdfObject::PdfReference(PdfReference::from_id(id)),
    );
    resources.insert("XObject".into(), PdfObject::PdfDict(images));
    page.insert("Resources".into(), PdfObject::PdfDict(resources));
    doc.update_object(page_id, PdfObject::PdfDict(page))
        .unwrap();
    let names = doc.get_page(1).unwrap().image_resource_names().unwrap();
    let snapshot = doc.get_page(1).unwrap().content_program().unwrap();
    doc.replace_page_image_draw(1, &snapshot, 1, &handle)
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let program = document.get_page(1).unwrap().content_program().unwrap();
        assert_eq!(program.operations().len(), snapshot.operations().len());
        assert_eq!(
            program.operations()[1]
                .named_resource_use()
                .unwrap()
                .unwrap()
                .name,
            "A /#"
        );
        assert!(
            program
                .source_bytes()
                .windows(b"/A#20#2F#23 Do".len())
                .any(|bytes| bytes == b"/A#20#2F#23 Do")
        );
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .image_resource_names()
                .unwrap(),
            names
        );
        let draws = document
            .get_page(1)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        assert_eq!(draws[0].image.rgba_buffer().unwrap(), [0, 0, 255, 255]);
        assert_eq!(draws[1].image.rgba_buffer().unwrap(), [255, 0, 0, 255]);
        let original = document
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        assert!(
            original
                .iter()
                .all(|draw| draw.image.rgba_buffer().unwrap() == [255, 0, 0, 255])
        );
    }
}

#[test]
fn image_draw_content_limit_rolls_back_registered_resource() {
    use pdf::parser::options::{Limits, ParseOptions};
    let source = b"q /Im1 Do Q\n";
    let doc = Document::new_with_options(ParseOptions {
        limits: Limits {
            max_decoded_bytes: source.len(),
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    assert_eq!(name, "Im1");
    doc.replace_page_contents(0, source.to_vec()).unwrap();
    let image = doc
        .get_page(0)
        .unwrap()
        .extract_image_source(&name)
        .unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            image.stream.dict().clone(),
            vec![0, 0, 255],
        )))
        .unwrap();
    let handle = doc.object_handle(id).unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let names = doc.get_page(0).unwrap().image_resource_names().unwrap();
    let revision = doc.revision();
    // Im2 has equal spelling length but operation replacement adds its required
    // separator newline, exceeding the exact source budget by one byte.
    assert!(matches!(
        doc.replace_page_image_draw(0, &snapshot, 1, &handle),
        Err(pdf::error::PdfError::ResourceLimit(_))
    ));
    assert!(doc.revision() > revision);
    assert_eq!(
        doc.get_page(0).unwrap().image_resource_names().unwrap(),
        names
    );
    let restored = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(restored.source_bytes(), source);
    assert_eq!(restored.stream_object_id(0), snapshot.stream_object_id(0));
    let revision = doc.revision();
    assert!(
        doc.replace_page_image_draw(0, &snapshot, 1, &handle)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .image_resource_names()
            .unwrap(),
        names
    );
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        source
    );
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .extract_decoded_image(&name)
            .unwrap()
            .rgba_buffer()
            .unwrap(),
        [255, 0, 0, 255]
    );
}

#[test]
fn one_image_draw_replacement_preserves_other_draws_and_shared_page() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("% keep\nq /{name} Do /{name} Do Q\n").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 1).unwrap();
    let source = doc
        .get_page(0)
        .unwrap()
        .extract_image_source(&name)
        .unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            source.stream.dict().clone(),
            vec![0, 0, 255],
        )))
        .unwrap();
    let handle = doc.object_handle(id).unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    assert!(
        doc.replace_page_image_draw(1, &snapshot, 1, &handle)
            .is_err()
    );
    assert!(
        doc.replace_page_image_draw(0, &snapshot, 0, &handle)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    let names_before = doc.get_page(0).unwrap().image_resource_names().unwrap();
    let failed = doc.edit_transaction(|doc| {
        doc.replace_page_image_draw(0, &snapshot, 1, &handle)?;
        Err::<(), _>(pdf::error::PdfError::DocumentError(
            "injected image draw rollback".into(),
        ))
    });
    assert!(failed.is_err());
    assert_eq!(
        doc.get_page(0).unwrap().image_resource_names().unwrap(),
        names_before
    );
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        snapshot.source_bytes()
    );
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    doc.replace_page_image_draw(0, &snapshot, 1, &handle)
        .unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes()
            .starts_with(b"% keep\n")
    );
    let revision = doc.revision();
    assert!(
        doc.replace_page_image_draw(0, &snapshot, 2, &handle)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let images = document
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        assert_eq!(images[0].image.rgba_buffer().unwrap(), [0, 0, 255, 255]);
        assert_eq!(images[1].image.rgba_buffer().unwrap(), [255, 0, 0, 255]);
        let other = document
            .get_page(1)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        assert!(
            other
                .iter()
                .all(|image| image.image.rgba_buffer().unwrap() == [255, 0, 0, 255])
        );
    }
}

#[test]
fn image_replacement_rejects_wrong_object_types_without_page_mutation() {
    use pdf::objects::{pdf_dict::PdfDict, pdf_name::PdfName};
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    let page_id = doc.copy_page(0, 1).unwrap();
    let page = doc.resolve_object(page_id).unwrap();
    let old_id = page
        .as_dict()
        .unwrap()
        .get("Resources")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(&name)
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let old_image = doc.resolve_object(old_id).unwrap();
    let mut form = PdfDict::default();
    form.insert(
        "Subtype".into(),
        PdfObject::PdfName(PdfName::new("Form".into())),
    );
    for object in [
        PdfObject::PdfNull,
        PdfObject::PdfDict(PdfDict::default()),
        PdfObject::PdfStream(PdfStream::new(PdfDict::default(), Vec::new())),
        PdfObject::PdfStream(PdfStream::new(form.clone(), Vec::new())),
    ] {
        let id = doc.try_insert_object(object).unwrap();
        let handle = doc.object_handle(id).unwrap();
        let revision = doc.revision();
        assert!(
            doc.resources()
                .replace_image_handle(1, &name, &handle)
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.resolve_object(page_id).unwrap(), page);
        assert_eq!(doc.resolve_object(old_id).unwrap(), old_image);
    }
    let new_id = doc.try_insert_object(old_image).unwrap();
    let new_handle = doc.object_handle(new_id).unwrap();
    doc.update_object(
        old_id,
        PdfObject::PdfStream(PdfStream::new(form, Vec::new())),
    )
    .unwrap();
    let revision = doc.revision();
    assert!(
        doc.resources()
            .replace_image_handle(1, &name, &new_handle)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.resolve_object(page_id).unwrap(), page);
}

#[test]
fn replacing_inherited_image_copies_resources_without_changing_parent() {
    use pdf::objects::pdf_reference::PdfReference;
    for indirect in [false, true] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        let font_name = doc
            .resources()
            .register_standard_font(0, pdf::document::resource_manager::StandardFont::Helvetica)
            .unwrap();
        doc.replace_page_contents(0, format!("q /{name} Do Q").into_bytes())
            .unwrap();
        let copied_id = doc.copy_page(0, 1).unwrap();
        let copied = doc.resolve_object(copied_id).unwrap().into_dict().unwrap();
        let parent_id = copied.get("Parent").unwrap().as_reference().unwrap().id();
        let mut parent = doc.resolve_object(parent_id).unwrap().into_dict().unwrap();
        let page_ids: Vec<_> = parent
            .get("Kids")
            .unwrap()
            .as_array()
            .unwrap()
            .into_iter()
            .map(|value| value.as_reference().unwrap().id())
            .collect();
        let mut first = doc
            .resolve_object(page_ids[0])
            .unwrap()
            .into_dict()
            .unwrap();
        let mut resources = first.get("Resources").unwrap().as_dict().unwrap().clone();
        let source = doc
            .get_page(0)
            .unwrap()
            .extract_image_source(&name)
            .unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                source.stream.dict().clone(),
                vec![0, 0, 255],
            )))
            .unwrap();
        let handle = doc.object_handle(id).unwrap();
        let resources = if indirect {
            let xobjects = resources.get("XObject").unwrap().clone();
            let id = doc.try_insert_object(xobjects).unwrap();
            resources.insert(
                "XObject".into(),
                PdfObject::PdfReference(PdfReference::from_id(id)),
            );
            let id = doc
                .try_insert_object(PdfObject::PdfDict(resources))
                .unwrap();
            PdfObject::PdfReference(PdfReference::from_id(id))
        } else {
            PdfObject::PdfDict(resources)
        };
        parent.insert("Resources".into(), resources);
        doc.update_object(parent_id, PdfObject::PdfDict(parent.clone()))
            .unwrap();
        first.remove("Resources");
        doc.update_object(page_ids[0], PdfObject::PdfDict(first))
            .unwrap();
        let mut second = doc
            .resolve_object(page_ids[1])
            .unwrap()
            .into_dict()
            .unwrap();
        second.remove("Resources");
        doc.update_object(page_ids[1], PdfObject::PdfDict(second.clone()))
            .unwrap();
        let contents_before = doc
            .resolve_object(page_ids[0])
            .unwrap()
            .as_dict()
            .unwrap()
            .get("Contents")
            .unwrap()
            .clone();
        doc.resources()
            .replace_image_handle(0, &name, &handle)
            .unwrap();
        assert_eq!(
            doc.resolve_object(parent_id).unwrap(),
            PdfObject::PdfDict(parent)
        );
        assert_eq!(
            doc.resolve_object(page_ids[1]).unwrap(),
            PdfObject::PdfDict(second)
        );
        let page = doc
            .resolve_object(page_ids[0])
            .unwrap()
            .into_dict()
            .unwrap();
        assert_eq!(page.get("Contents").unwrap(), &contents_before);
        assert!(
            page.get("Resources")
                .unwrap()
                .as_dict()
                .unwrap()
                .get("Font")
                .unwrap()
                .as_dict()
                .unwrap()
                .contains_key(&font_name)
        );
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            assert_eq!(
                document
                    .get_page(0)
                    .unwrap()
                    .extract_decoded_image(&name)
                    .unwrap()
                    .rgba_buffer()
                    .unwrap(),
                [0, 0, 255, 255]
            );
            assert_eq!(
                document
                    .get_page(1)
                    .unwrap()
                    .extract_decoded_image(&name)
                    .unwrap()
                    .rgba_buffer()
                    .unwrap(),
                [255, 0, 0, 255]
            );
        }
    }
}

#[test]
fn page_image_binding_replacement_isolated_and_transactional() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(0, format!("q /{name} Do /{name} Do Q\n").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let source = doc
        .get_page(0)
        .unwrap()
        .extract_image_source(&name)
        .unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            source.stream.dict().clone(),
            vec![0, 0, 255],
        )))
        .unwrap();
    let handle = doc.object_handle(id).unwrap();
    let foreign = Document::new().unwrap();
    let foreign_id = foreign.try_insert_object(PdfObject::PdfNull).unwrap();
    let foreign_handle = foreign.object_handle(foreign_id).unwrap();
    let revision = doc.revision();
    assert!(
        doc.resources()
            .replace_image_handle(0, &name, &foreign_handle)
            .is_err()
    );
    assert!(
        doc.resources()
            .replace_image_handle(0, "Missing", &handle)
            .is_err()
    );
    assert!(
        doc.resources()
            .replace_image_handle(2, &name, &handle)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    let error = doc.edit_transaction(|_| {
        doc.resources().replace_image_handle(0, &name, &handle)?;
        assert_eq!(
            doc.get_page(0)?
                .extract_decoded_image(&name)?
                .rgba_buffer()?,
            [0, 0, 255, 255]
        );
        Err::<(), _>(pdf::error::PdfError::DocumentError(
            "injected rollback".into(),
        ))
    });
    assert!(error.is_err());
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_decoded_image(&name)
            .unwrap()
            .rgba_buffer()
            .unwrap(),
        [255, 0, 0, 255]
    );
    doc.resources()
        .replace_image_handle(0, &name, &handle)
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for (page, expected) in [(0, [0, 0, 255, 255]), (1, [255, 0, 0, 255])] {
            assert_eq!(
                document
                    .get_page(page)
                    .unwrap()
                    .extract_decoded_image(&name)
                    .unwrap()
                    .rgba_buffer()
                    .unwrap(),
                expected
            );
            assert_eq!(
                document
                    .get_page(page)
                    .unwrap()
                    .extract_images_paint_order()
                    .unwrap()
                    .len(),
                2
            );
        }
    }
}
