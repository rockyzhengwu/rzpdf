use pdf::{
    document::{owned::Document, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_object::PageObject,
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

#[test]
fn nested_path_edit_rejects_unknown_or_malformed_prefix_without_writes() {
    use pdf::page::{
        content_program::ContentProgram,
        source_provenance::{FormInvocation, NestedDrawSource},
    };
    for body in [
        b"0 0 m Vendor 1 1 l S".as_slice(),
        b"1 q 0 0 m 1 1 l S".as_slice(),
    ] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let dict = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Subtype /Form /BBox [0 0 50 50] /Resources << >> >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let form = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, body.to_vec()).unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, form)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
            .unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let leaf = ContentProgram::from_decoded_bytes(body.to_vec(), Default::default()).unwrap();
        let paint = leaf.operations().len() - 1;
        let geometry: Vec<_> = leaf
            .operations()
            .iter()
            .enumerate()
            .filter_map(|(index, operation)| {
                matches!(operation.operator.name(), "m" | "l").then_some(index)
            })
            .collect();
        let source = NestedDrawSource {
            invocations: vec![FormInvocation {
                form_id: Some(form),
                operation_index: 0,
                operator_byte_offset: root.operations()[0].operator.byte_offset(),
            }],
            operation_index: paint,
            operator_byte_offset: leaf.operations()[paint].operator.byte_offset(),
            geometry_operation_indices: geometry,
        };
        let revision = doc.revision();
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        assert!(
            doc.replace_nested_path_numbers(
                0,
                &root,
                &source,
                source.geometry_operation_indices[0],
                &[2., 2.]
            )
            .is_err()
        );
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(unchanged.source_bytes(), root.source_bytes());
        assert_eq!(unchanged.stream_object_id(0), root.stream_object_id(0));
    }
}

#[test]
fn nested_path_membership_respects_discard_and_graphics_state_boundaries() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let dict = SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            b"<< /Subtype /Form /BBox [0 0 50 50] /Resources << >> >>".to_vec(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    let body = b"0 0 1 1 re n 2 2 m q 1 0 0 rg 8 2 l Q 8 8 l h S";
    let form = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, body.to_vec()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("path")
    };
    let source = path.nested_source().unwrap();
    assert_eq!(source.geometry_operation_indices, [2, 5, 7, 8]);
    let revision = doc.revision();
    for indices in [vec![0, 2, 5, 7, 8], vec![7, 8], vec![2, 5, 7]] {
        let mut incomplete = source.clone();
        incomplete.geometry_operation_indices = indices;
        assert!(
            doc.replace_nested_path_numbers(0, &root, &incomplete, 7, &[9., 9.])
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
    }
    assert!(
        doc.replace_nested_path_numbers(0, &root, source, 8, &[])
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    doc.replace_nested_path_numbers(0, &root, source, 2, &[3., 3.])
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for page in [0, 1] {
            let root = document.get_page(page).unwrap().content_program().unwrap();
            let list = document.get_page(page).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 2);
            assert!(list.diagnostics().is_empty());
            for (call, object) in list.objects().iter().enumerate() {
                let PageObject::Path(path) = object else {
                    panic!("path")
                };
                let source = path.nested_source().unwrap();
                assert_eq!(source.geometry_operation_indices, [2, 5, 7, 8]);
                let leaf = document
                    .get_page(page)
                    .unwrap()
                    .nested_content_program(&root, source)
                    .unwrap();
                let expected = if page == 0 && call == 0 { 3. } else { 2. };
                assert_eq!(
                    leaf.operations()[2].operator.get_as_f32(0).unwrap(),
                    expected
                );
                assert_eq!(
                    leaf.operations()[2].operator.get_as_f32(1).unwrap(),
                    expected
                );
                for index in [0, 1, 3, 4, 5, 6, 7, 8, 9] {
                    let original = pdf::page::content_program::ContentProgram::from_decoded_bytes(
                        body.to_vec(),
                        Default::default(),
                    )
                    .unwrap();
                    let old = &original.operations()[index].operator;
                    let new = &leaf.operations()[index].operator;
                    assert_eq!(new.name(), old.name());
                    assert_eq!(new.num_operands(), old.num_operands());
                    for operand in 0..old.num_operands() {
                        assert_eq!(
                            new.get_as_f32(operand).unwrap(),
                            old.get_as_f32(operand).unwrap()
                        );
                    }
                }
            }
        }
    }
}

#[test]
fn nested_path_numbers_cover_all_numeric_geometry_operators() {
    let body = b"% geometry\n0 0 m 10 0 l 10 2 8 4 6 4 c 4 6 2 6 v 0 4 0 2 y 12 12 2 2 re S";
    for (index, name, values) in [
        (0, "m", vec![1., 1.]),
        (1, "l", vec![11., 1.]),
        (2, "c", vec![11., 3., 9., 5., 7., 5.]),
        (3, "v", vec![5., 7., 3., 7.]),
        (4, "y", vec![1., 5., 1., 3.]),
        (5, "re", vec![13., 13., -3., 4.]),
    ] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let dict = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Subtype /Form /BBox [0 0 50 50] /Resources << >> >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let form = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, body.to_vec()).unwrap(),
            ))
            .unwrap();
        let name_in_page = doc
            .resources()
            .register_existing(0, ResourceKind::Form, form)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("/{name_in_page} Do /{name_in_page} Do").into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let PageObject::Path(path) = &list.objects()[0] else {
            panic!("path")
        };
        let source = path.nested_source().unwrap();
        assert_eq!(source.geometry_operation_indices, [0, 1, 2, 3, 4, 5]);
        let before = doc
            .get_page(0)
            .unwrap()
            .nested_content_program(&root, source)
            .unwrap();
        doc.replace_nested_path_numbers(0, &root, source, index, &values)
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            for page in [0, 1] {
                let root = document.get_page(page).unwrap().content_program().unwrap();
                let list = document.get_page(page).unwrap().display_list().unwrap();
                assert_eq!(list.objects().len(), 2);
                assert!(list.diagnostics().is_empty());
                for (call, object) in list.objects().iter().enumerate() {
                    let PageObject::Path(path) = object else {
                        panic!("path")
                    };
                    let program = document
                        .get_page(page)
                        .unwrap()
                        .nested_content_program(&root, path.nested_source().unwrap())
                        .unwrap();
                    assert_eq!(program.operations().len(), before.operations().len());
                    for (operation, original) in
                        program.operations().iter().zip(before.operations())
                    {
                        assert_eq!(operation.operator.name(), original.operator.name());
                    }
                    if page == 0 && call == 0 {
                        let target = &program.operations()[index].operator;
                        assert_eq!(target.name(), name);
                        for (operand, value) in values.iter().enumerate() {
                            assert_eq!(target.get_as_f32(operand).unwrap(), *value);
                        }
                        for neighbor in 0..before.operations().len() {
                            if neighbor == index {
                                continue;
                            }
                            let old = &before.operations()[neighbor].operator;
                            let new = &program.operations()[neighbor].operator;
                            assert_eq!(new.num_operands(), old.num_operands());
                            for operand in 0..old.num_operands() {
                                assert_eq!(
                                    new.get_as_f32(operand).unwrap(),
                                    old.get_as_f32(operand).unwrap()
                                );
                            }
                        }
                    } else {
                        assert_eq!(program.source_bytes(), body);
                    }
                    assert!(program.source_bytes().starts_with(b"% geometry\n"));
                }
            }
            assert_eq!(
                document
                    .resolve_object(form)
                    .unwrap()
                    .into_stream()
                    .unwrap()
                    .raw_data(),
                body
            );
        }
    }
}

#[test]
fn nested_path_numbers_change_only_selected_invocation() {
    fn insert(doc: &Document, dictionary: &str, body: &[u8]) -> pdf::objects::object_id::ObjectId {
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
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let leaf = insert(
        &doc,
        "<< /Subtype /Form /BBox [0 0 50 50] /Resources << >> >>",
        b"% keep\n0 0 2 2 re f 10 10 3 3 re f",
    );
    let outer = insert(
        &doc,
        &format!(
            "<< /Subtype /Form /BBox [0 0 50 50] /Resources << /XObject << /Child {} 0 R >> >> >>",
            leaf.obj_num()
        ),
        b"/Child Do /Child Do",
    );
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, outer)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("path")
    };
    let source = path.nested_source().unwrap();
    assert_eq!(source.geometry_operation_indices, [0]);
    let revision = doc.revision();
    let PageObject::Path(second) = &list.objects()[1] else {
        panic!("path")
    };
    let mut forged = second.nested_source().unwrap().clone();
    // Sorted, in-range geometry is insufficient: operation 0 belongs to an
    // already-painted path, not the second event.
    forged.geometry_operation_indices = vec![0, 2];
    assert!(
        doc.replace_nested_path_numbers(0, &root, &forged, 0, &[1., 2., 3., 4.])
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    for (operation, values) in [
        (0, vec![f32::NAN, 0., 2., 2.]),
        (0, vec![1., 2.]),
        (2, vec![1., 2., 3., 4.]),
    ] {
        assert!(
            doc.replace_nested_path_numbers(0, &root, source, operation, &values)
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
    }
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let rollback: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
        editing.replace_nested_path_numbers(0, &root, source, 0, &[5., 6., -2., 4.])?;
        Err(pdf::error::PdfError::ObjectError(
            "injected geometry rollback".into(),
        ))
    });
    assert!(rollback.is_err());
    let refreshed = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(refreshed.source_bytes(), root.source_bytes());
    assert_eq!(refreshed.stream_object_id(0), root.stream_object_id(0));
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    assert!(
        doc.replace_nested_path_numbers(0, &root, source, 0, &[5., 6., -2., 4.])
            .is_err()
    );
    doc.replace_nested_path_numbers(0, &refreshed, source, 0, &[5., 6., -2., 4.])
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for page in [0, 1] {
            let root = document.get_page(page).unwrap().content_program().unwrap();
            let list = document.get_page(page).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 4);
            assert!(list.diagnostics().is_empty());
            for (index, object) in list.objects().iter().enumerate() {
                let PageObject::Path(path) = object else {
                    panic!("path")
                };
                let source = path.nested_source().unwrap();
                let program = document
                    .get_page(page)
                    .unwrap()
                    .nested_content_program(&root, source)
                    .unwrap();
                let operation =
                    &program.operations()[source.geometry_operation_indices[0]].operator;
                let expected = if page == 0 && index == 0 {
                    [5., 6., -2., 4.]
                } else if index % 2 == 0 {
                    [0., 0., 2., 2.]
                } else {
                    [10., 10., 3., 3.]
                };
                for (operand, value) in expected.iter().enumerate() {
                    assert_eq!(operation.get_as_f32(operand).unwrap(), *value);
                }
                assert!(program.source_bytes().starts_with(b"% keep\n"));
            }
        }
        assert_eq!(
            document
                .resolve_object(leaf)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"% keep\n0 0 2 2 re f 10 10 3 3 re f"
        );
    }
}

#[test]
fn nested_final_root_byte_limit_rolls_back_created_form_and_resource() {
    use pdf::{
        page::{
            content_program::ContentProgram,
            source_provenance::{FormInvocation, NestedDrawSource},
        },
        parser::options::{Limits, ParseOptions},
    };
    for limit in [512, 513, 514] {
        let options = ParseOptions {
            limits: Limits {
                max_decoded_bytes: limit,
                ..Limits::default()
            },
            ..ParseOptions::default()
        };
        let doc = Document::new_with_options(options).unwrap();
        doc.add_page(0).unwrap();
        let dict = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Subtype /Form /BBox [0 0 10 10] /Resources << >> >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let mut body = b"%".to_vec();
        body.extend(std::iter::repeat_n(b'x', 491));
        body.extend_from_slice(b"\n0 0 2 2 re f");
        assert_eq!(body.len(), 505);
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dict, body.clone()).unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
            .unwrap();
        let root = doc.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(root.source_bytes().len() + body.len(), 512);
        let leaf = ContentProgram::from_decoded_bytes(body.clone(), options).unwrap();
        let source = NestedDrawSource {
            invocations: vec![FormInvocation {
                form_id: Some(id),
                operation_index: 0,
                operator_byte_offset: root.operations()[0].operator.byte_offset(),
            }],
            operation_index: 1,
            operator_byte_offset: leaf.operations()[1].operator.byte_offset(),
            geometry_operation_indices: vec![0],
        };
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let revision = doc.revision();
        let result = doc.remove_nested_path_draw(0, &root, &source);
        if limit == 514 {
            result.unwrap();
            let edited = doc.get_page(0).unwrap().content_program().unwrap();
            let name = edited.operations()[0]
                .operator
                .get_as_name(0)
                .unwrap()
                .name();
            let copied = doc
                .get_page(0)
                .unwrap()
                .get_xobject(name)
                .unwrap()
                .into_stream()
                .unwrap();
            assert_eq!(edited.source_bytes().len() + copied.raw_data().len(), 514);
            let program =
                ContentProgram::from_decoded_bytes(copied.raw_data().to_vec(), options).unwrap();
            assert_eq!(program.operations()[1].operator.name(), "n");
            assert_ne!(edited.stream_object_id(0), root.stream_object_id(0));
        } else {
            let error = result.unwrap_err();
            assert!(
                matches!(error,pdf::error::PdfError::ResourceLimit(ref message) if message=="Nested edited decoded bytes")
            );
            assert!(doc.revision() > revision);
            assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
            let restored = doc.get_page(0).unwrap().content_program().unwrap();
            assert_eq!(restored.source_bytes(), root.source_bytes());
            assert_eq!(restored.stream_object_id(0), root.stream_object_id(0));
            assert_eq!(
                doc.get_page(0)
                    .unwrap()
                    .nested_content_program(&restored, &source)
                    .unwrap()
                    .source_bytes(),
                body
            );
        }
        assert_eq!(
            doc.resolve_object(id)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            body
        );
    }
}

#[test]
fn nested_copy_object_limit_failure_restores_store_capacity_and_page_bindings() {
    use pdf::parser::options::{Limits, ParseOptions};
    let doc = Document::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 100,
            ..Limits::default()
        },
        ..ParseOptions::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    let parse = |text: &str| {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap()
    };
    let leaf = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                parse("<< /Subtype /Form /BBox [0 0 10 10] /Resources << >> >>"),
                b"0 0 2 2 re f".to_vec(),
            )
            .unwrap(),
        ))
        .unwrap();
    let outer = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(parse(&format!("<< /Subtype /Form /BBox [0 0 10 10] /Resources << /XObject << /Child {} 0 R >> >> >>",leaf.obj_num())),b"/Child Do".to_vec()).unwrap())).unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, outer)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
        .unwrap();
    // Fresh owned document IDs are contiguous; leave exactly one store slot.
    loop {
        let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
        if id.obj_num() == 99 {
            break;
        }
        assert!(id.obj_num() < 99);
    }
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("missing nested path")
    };
    let revision = doc.revision();
    let error = doc
        .remove_nested_path_draw(0, &root, path.nested_source().unwrap())
        .unwrap_err();
    assert!(
        matches!(error,pdf::error::PdfError::ResourceLimit(ref message) if message=="document object count")
    );
    assert!(doc.revision() > revision);
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    let restored = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(restored.source_bytes(), root.source_bytes());
    assert_eq!(restored.stream_object_id(0), root.stream_object_id(0));
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .objects()
            .len(),
        1
    );
    // The rolled-back leaf copy did not consume the final slot or object ID.
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        100
    );
    assert!(matches!(
        doc.try_insert_object(PdfObject::PdfNull),
        Err(pdf::error::PdfError::ResourceLimit(_))
    ));
}

#[test]
fn nested_edit_materializes_page_fallback_without_losing_font_bindings() {
    use pdf::document::resource_manager::StandardFont;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let font = doc
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)
        .unwrap();
    let dictionary = SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            b"<< /Subtype /Form /BBox [0 0 50 50] >>".to_vec(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    let body = format!("0 0 2 2 re f BT /{font} 12 Tf (AB) Tj ET");
    let id = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dictionary, body.as_bytes().to_vec()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, id)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("missing fallback path")
    };
    doc.remove_nested_path_draw(0, &root, path.nested_source().unwrap())
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let mut page = document.get_page(0).unwrap();
        let list = page.display_list().unwrap();
        assert_eq!(list.objects().len(), 3);
        let PageObject::Text(text) = &list.objects()[0] else {
            panic!("missing copied Form text")
        };
        let copied = text.nested_source().unwrap().invocations[0]
            .form_id
            .unwrap();
        assert_ne!(copied, id);
        let stream = document
            .resolve_object(copied)
            .unwrap()
            .into_stream()
            .unwrap();
        assert!(stream.dict().get("Resources").is_some());
        let report = page.extract_characters_paint_order_report().unwrap();
        assert_eq!(
            report
                .characters
                .iter()
                .filter_map(|ch| ch.unicode.as_deref())
                .collect::<String>(),
            "ABAB"
        );
        assert!(
            report
                .characters
                .iter()
                .all(|ch| ch.font_name.as_deref() == Some("Helvetica"))
        );
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .display_list()
                .unwrap()
                .objects()
                .len(),
            4
        );
        let original = document.resolve_object(id).unwrap().into_stream().unwrap();
        assert!(original.dict().get("Resources").is_none());
        assert_eq!(original.raw_data(), body.as_bytes());
    }
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Text(text) = &list.objects()[0] else {
        panic!("missing nested text")
    };
    let source = text.nested_source().unwrap().clone();
    let result: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
        editing.remove_nested_text_draw(0, &root, &source)?;
        Err(pdf::error::PdfError::ObjectError(
            "injected nested text rollback".into(),
        ))
    });
    assert!(result.is_err());
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap()
            .characters
            .len(),
        4
    );
    doc.remove_nested_text_draw(0, &root, &source).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        for (index, expected) in [(0, "AB"), (1, "ABAB")] {
            assert_eq!(
                document
                    .get_page(index)
                    .unwrap()
                    .extract_characters_paint_order_report()
                    .unwrap()
                    .characters
                    .iter()
                    .filter_map(|ch| ch.unicode.as_deref())
                    .collect::<String>(),
                expected
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
    }
}

#[test]
fn nested_path_removal_copies_selected_chain_and_rolls_back_whole_edit() {
    fn insert(doc: &Document, dictionary: &str, body: &[u8]) -> pdf::objects::object_id::ObjectId {
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
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let leaf = insert(
        &doc,
        "<< /Subtype /Form /BBox [0 0 20 20] /Resources << >> >>",
        b"0 0 2 2 re f",
    );
    let outer = insert(
        &doc,
        &format!(
            "<< /Subtype /Form /BBox [0 0 50 50] /Resources << /XObject << /FmEdit1 {} 0 R >> >> >>",
            leaf.obj_num()
        ),
        b"/FmEdit1 Do /FmEdit1 Do",
    );
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, outer)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    assert_eq!(list.objects().len(), 4);
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("missing nested path")
    };
    let source = path.nested_source().unwrap().clone();
    let revision = doc.revision();
    let mut invalid = source.clone();
    invalid.invocations[1].operator_byte_offset = None;
    assert!(doc.remove_nested_path_draw(0, &root, &invalid).is_err());
    let mut invalid = source.clone();
    invalid.invocations[0].form_id = Some(leaf);
    assert!(doc.remove_nested_path_draw(0, &root, &invalid).is_err());
    let leaf_program = doc
        .get_page(0)
        .unwrap()
        .nested_content_program(&root, &source)
        .unwrap();
    let mut invalid = source.clone();
    invalid.operation_index = 0;
    invalid.operator_byte_offset = leaf_program.operations()[0].operator.byte_offset();
    invalid.geometry_operation_indices.clear();
    assert!(matches!(
        doc.remove_nested_path_draw(0, &root, &invalid),
        Err(pdf::error::PdfError::ObjectError(_))
    ));
    assert!(doc.remove_nested_path_draw(1, &root, &source).is_err());
    assert!(
        doc.remove_nested_path_draw(0, &leaf_program, &source)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(unchanged.source_bytes(), root.source_bytes());
    assert_eq!(unchanged.stream_object_id(0), root.stream_object_id(0));
    let result: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
        editing.remove_nested_path_draw(0, &root, &source)?;
        assert_eq!(editing.get_page(0)?.display_list()?.objects().len(), 3);
        Err(pdf::error::PdfError::ObjectError(
            "injected nested edit rollback".into(),
        ))
    });
    assert!(result.is_err());
    let fresh = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(fresh.source_bytes(), root.source_bytes());
    assert_eq!(fresh.stream_object_id(0), root.stream_object_id(0));
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .objects()
            .len(),
        4
    );
    assert!(doc.remove_nested_path_draw(0, &root, &source).is_err());
    doc.remove_nested_path_draw(0, &fresh, &source).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let list = document.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(list.objects().len(), 3);
        let PageObject::Path(first) = &list.objects()[0] else {
            panic!("missing unselected inner call")
        };
        let first = first.nested_source().unwrap();
        assert_ne!(first.invocations[0].form_id, Some(outer));
        assert_eq!(first.invocations[1].form_id, Some(leaf));
        let copied_outer = document
            .resolve_object(first.invocations[0].form_id.unwrap())
            .unwrap()
            .into_stream()
            .unwrap();
        assert_eq!(copied_outer.raw_data(), b"/FmEdit2 Do\n /FmEdit1 Do");
        for object in &list.objects()[1..] {
            let PageObject::Path(path) = object else {
                panic!("missing unselected outer call")
            };
            assert_eq!(
                path.nested_source().unwrap().invocations[0].form_id,
                Some(outer)
            );
        }
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .display_list()
                .unwrap()
                .objects()
                .len(),
            4
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
                .resolve_object(leaf)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"0 0 2 2 re f"
        );
        assert_eq!(
            document
                .resolve_object(outer)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"/FmEdit1 Do /FmEdit1 Do"
        );
    }
}
