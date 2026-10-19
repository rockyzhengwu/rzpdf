use pdf::{
    document::{owned::Document, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_object::PageObject,
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

#[test]
fn form_transform_checks_graphics_stack_headroom_before_mutating_shared_contents() {
    use pdf::parser::options::{Limits, ParseOptions};
    for depth in [7, 8] {
        let options = ParseOptions {
            limits: Limits {
                max_depth: 8,
                ..Limits::default()
            },
            ..ParseOptions::default()
        };
        let doc = Document::new_with_options(options).unwrap();
        doc.add_page(0).unwrap();
        let dictionary = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Subtype /Form /BBox [0 0 10 10] /Resources << >> >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dictionary, b"0 0 2 2 re f".to_vec()).unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("{}/{name} Do {}", "q ".repeat(depth), "Q ".repeat(depth)).into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        assert!(doc.get_page(0).unwrap().display_list().is_ok());
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let revision = doc.revision();
        let result = doc.transform_page_form_draw(0, &snapshot, depth, [1., 0., 0., 1., 3., 4.]);
        if depth == 7 {
            result.unwrap();
            assert!(doc.get_page(0).unwrap().display_list().is_ok());
        } else {
            assert!(matches!(
                result,
                Err(pdf::error::PdfError::ResourceLimit(_))
            ));
            assert_eq!(doc.revision(), revision);
            let unchanged = doc.get_page(0).unwrap().content_program().unwrap();
            assert_eq!(unchanged.source_bytes(), snapshot.source_bytes());
            assert_eq!(unchanged.stream_object_id(0), snapshot.stream_object_id(0));
            assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
        }
        let reopened = Document::from_bytes_with_options(
            doc.save_to_bytes().unwrap(),
            ParseOptions {
                mode: ParseMode::Strict,
                ..options
            },
        )
        .unwrap();
        for document in [&doc, &reopened] {
            assert!(document.get_page(0).unwrap().display_list().is_ok());
            assert_eq!(
                document
                    .get_page(1)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                snapshot.source_bytes()
            );
        }
    }
}

#[test]
fn form_affine_edits_compose_form_matrix_and_parent_ctm_without_changing_second_call() {
    for (matrix, expected) in [
        ([0., 1., -1., 0., 4., 5.], [0., -3., -2., 0., 14., 38.]),
        ([4., 0., 0., 0.5, 1., 2.], [8., 0., 0., -1.5, 20., 29.]),
        ([-1., 0., 0., 1., 0., 0.], [-2., 0., 0., -3., 8., 26.]),
    ] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let dictionary = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(
                b"<< /Subtype /Form /BBox [0 0 10 10] /Matrix [1 0 0 1 1 2] /Resources << >> >>"
                    .to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(dictionary, b"0 0 2 2 re f".to_vec()).unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("q 2 0 0 3 10 20 cm /{name} Do /{name} Do Q").into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        doc.transform_page_form_draw(0, &snapshot, 2, matrix)
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let height = document
                .get_page(0)
                .unwrap()
                .geometry()
                .unwrap()
                .display_height;
            let list = document.get_page(0).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 2);
            for (index, values) in [expected, [2., 0., 0., -3., 12., 26.]]
                .into_iter()
                .enumerate()
            {
                let PageObject::Path(path) = &list.objects()[index] else {
                    panic!("missing Form path")
                };
                let actual = path.matrix();
                assert_eq!(
                    [
                        actual.a(),
                        actual.b(),
                        actual.c(),
                        actual.d(),
                        actual.e(),
                        height - actual.f()
                    ],
                    values
                );
                assert_eq!(
                    path.nested_source().unwrap().invocations[0].form_id,
                    Some(id)
                );
            }
            assert_eq!(
                document
                    .get_page(1)
                    .unwrap()
                    .content_program()
                    .unwrap()
                    .source_bytes(),
                snapshot.source_bytes()
            );
            assert_eq!(
                document
                    .resolve_object(id)
                    .unwrap()
                    .into_stream()
                    .unwrap()
                    .raw_data(),
                b"0 0 2 2 re f"
            );
        }
    }
}

#[test]
fn removing_one_form_call_preserves_repeated_and_shared_draws_and_rejects_leaf_snapshot() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let dictionary = SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            b"<< /Type /XObject /Subtype /Form /BBox [0 0 10 10] /Resources << >> >>".to_vec(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dictionary, b"0 0 2 2 re f".to_vec()).unwrap(),
        ))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, id)
        .unwrap();
    let image = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("/{name} Do /{name} Do /{image} Do 20 20 2 2 re f").into_bytes(),
    )
    .unwrap();
    doc.copy_page(0, 1).unwrap();
    let page = doc.get_page(0).unwrap();
    let root = page.content_program().unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[0] else {
        panic!("missing Form path")
    };
    let leaf = page
        .nested_content_program(&root, path.nested_source().unwrap())
        .unwrap();
    let revision = doc.revision();
    assert!(doc.remove_page_form_draw(0, &leaf, 1).is_err());
    assert!(doc.remove_page_form_draw(0, &root, 2).is_err());
    assert!(doc.remove_page_form_draw(0, &root, 4).is_err());
    assert_eq!(doc.revision(), revision);
    let original_stream = root.stream_object_id(0);
    let rollback: pdf::error::PdfResult<()> = doc.edit_transaction(|editing| {
        editing.remove_page_form_draw(0, &root, 0)?;
        assert_eq!(editing.get_page(0)?.display_list()?.objects().len(), 3);
        Err(pdf::error::PdfError::ObjectError(
            "injected Form removal rollback".into(),
        ))
    });
    assert!(rollback.is_err());
    let root = doc.get_page(0).unwrap().content_program().unwrap();
    assert_eq!(root.stream_object_id(0), original_stream);
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .objects()
            .len(),
        4
    );
    doc.remove_page_form_draw(0, &root, 0).unwrap();
    assert!(doc.remove_page_form_draw(0, &root, 1).is_err());
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let edited = document.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(edited.objects().len(), 3);
        let PageObject::Path(path) = &edited.objects()[0] else {
            panic!("missing remaining Form")
        };
        assert_eq!(
            path.nested_source().unwrap().invocations[0].form_id,
            Some(id)
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
                .resolve_object(id)
                .unwrap()
                .into_stream()
                .unwrap()
                .raw_data(),
            b"0 0 2 2 re f"
        );
    }
    let before = doc.get_page(0).unwrap().display_list().unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    assert!(
        doc.transform_page_form_draw(0, &snapshot, 0, [f32::NAN, 0., 0., 1., 0., 0.])
            .is_err()
    );
    assert!(
        doc.transform_page_form_draw(0, &snapshot, 1, [1., 0., 0., 1., 3., 4.])
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    doc.transform_page_form_draw(0, &snapshot, 0, [1., 0., 0., 1., 3., 4.])
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let after = document.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(after.objects().len(), before.objects().len());
        let (PageObject::Path(original), PageObject::Path(changed)) =
            (&before.objects()[0], &after.objects()[0])
        else {
            panic!("missing transformed Form path")
        };
        assert_eq!(changed.matrix().e(), original.matrix().e() + 3.);
        assert_eq!(changed.matrix().f(), original.matrix().f() - 4.);
        let (PageObject::Path(original), PageObject::Path(neighbor)) =
            (&before.objects()[2], &after.objects()[2])
        else {
            panic!("missing neighboring path")
        };
        assert_eq!(neighbor.matrix().e(), original.matrix().e());
        assert_eq!(neighbor.matrix().f(), original.matrix().f());
        let (PageObject::Image(original), PageObject::Image(neighbor)) =
            (&before.objects()[1], &after.objects()[1])
        else {
            panic!("missing neighboring image")
        };
        assert_eq!(neighbor.matrix.e(), original.matrix.e());
        assert_eq!(neighbor.matrix.f(), original.matrix.f());
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            root.source_bytes()
        );
    }
}
