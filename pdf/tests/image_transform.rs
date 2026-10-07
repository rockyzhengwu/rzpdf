use pdf::{document::owned::Document, parser::syntax::ParseMode};

#[test]
fn image_relative_scale_rotation_and_reflection_compose_with_existing_ctm() {
    // Extracted image matrices also include image top-left normalization
    // [1 0 0 -1 0 1] and the page's top-left display conversion.
    for (matrix, expected_linear, delta) in [
        (
            [0.0, 1.0, -1.0, 0.0, 4.0, 5.0],
            [0.0, -3.0, 2.0, 0.0],
            [6.0, -12.0],
        ),
        (
            [4.0, 0.0, 0.0, 0.5, 1.0, 2.0],
            [8.0, 0.0, 0.0, 1.5],
            [2.0, -4.5],
        ),
        (
            [-1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
            [-2.0, 0.0, 0.0, 3.0],
            [0.0, 0.0],
        ),
    ] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("q 2 0 0 3 10 20 cm /{name} Do /{name} Do Q").into_bytes(),
        )
        .unwrap();
        let original = doc
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        doc.transform_page_image_draw(0, &snapshot, 2, matrix)
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let draws = document
                .get_page(0)
                .unwrap()
                .extract_images_paint_order()
                .unwrap();
            let transformed = &draws[0].matrix;
            assert_eq!(
                [
                    transformed.a(),
                    transformed.b(),
                    transformed.c(),
                    transformed.d()
                ],
                expected_linear
            );
            assert_eq!(transformed.e(), original[0].matrix.e() + delta[0]);
            assert_eq!(transformed.f(), original[0].matrix.f() + delta[1]);
            let unmodified = &draws[1].matrix;
            let before = &original[1].matrix;
            assert_eq!(
                [
                    unmodified.a(),
                    unmodified.b(),
                    unmodified.c(),
                    unmodified.d(),
                    unmodified.e(),
                    unmodified.f()
                ],
                [
                    before.a(),
                    before.b(),
                    before.c(),
                    before.d(),
                    before.e(),
                    before.f()
                ]
            );
        }
    }
}

#[test]
fn image_transform_reserves_graphics_stack_depth_before_mutation() {
    use pdf::parser::options::{Limits, ParseOptions};
    for (depth, allowed) in [(7, true), (8, false)] {
        let options = ParseOptions {
            limits: Limits {
                max_depth: 8,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = Document::new_with_options(options).unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        let source = format!(
            "{}{name_prefix}{name} Do {}",
            "q ".repeat(depth),
            "Q ".repeat(depth),
            name_prefix = "/"
        );
        doc.replace_page_contents(0, source.into_bytes()).unwrap();
        assert!(doc.get_page(0).unwrap().display_list().is_ok());
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let revision = doc.revision();
        let result =
            doc.transform_page_image_draw(0, &snapshot, depth, [1.0, 0.0, 0.0, 1.0, 10.0, 20.0]);
        if allowed {
            result.unwrap();
            assert!(doc.get_page(0).unwrap().display_list().is_ok());
            let reopened = Document::from_bytes_with_options(
                doc.save_to_bytes().unwrap(),
                ParseOptions {
                    mode: ParseMode::Strict,
                    ..options
                },
            )
            .unwrap();
            assert!(reopened.get_page(0).unwrap().display_list().is_ok());
        } else {
            assert!(matches!(
                result,
                Err(pdf::error::PdfError::ResourceLimit(_))
            ));
            assert_eq!(doc.revision(), revision);
            assert_eq!(
                doc.get_page(0)
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
fn image_transform_rejects_unbalanced_or_operand_bearing_q_q() {
    use pdf::{page::content_program::ContentProgram, parser::options::ParseOptions};
    for (source, index) in [
        ("Q /Im Do", 1),
        ("q /Im Do", 1),
        ("1 q /Im Do Q", 1),
        ("q /Im Do 1 Q", 1),
    ] {
        let program =
            ContentProgram::from_decoded_bytes(source.as_bytes().to_vec(), ParseOptions::default())
                .unwrap();
        assert!(
            program
                .with_transformed_image_operation(
                    index,
                    [1.0, 0.0, 0.0, 1.0, 0.0, 0.0],
                    ParseOptions::default()
                )
                .is_err()
        );
    }
}

#[test]
fn one_image_translation_is_isolated_and_preserves_shared_page() {
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
    let original = doc
        .get_page(0)
        .unwrap()
        .extract_images_paint_order()
        .unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    assert!(
        doc.transform_page_image_draw(0, &snapshot, 1, [1.0, 0.0, 0.0, 1.0, f32::NAN, 0.0])
            .is_err()
    );
    assert!(
        doc.transform_page_image_draw(0, &snapshot, 0, [1.0, 0.0, 0.0, 1.0, 10.0, 20.0])
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    doc.transform_page_image_draw(0, &snapshot, 1, [1.0, 0.0, 0.0, 1.0, 10.0, 20.0])
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let program = document.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(program.operations().len(), snapshot.operations().len() + 3);
        assert!(program.source_bytes().starts_with(b"% keep\nq "));
        let images = document
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].matrix.e(), original[0].matrix.e() + 10.0);
        assert_eq!(images[0].matrix.f(), original[0].matrix.f() - 20.0);
        assert_eq!(images[1].matrix.e(), original[1].matrix.e());
        assert_eq!(images[1].matrix.f(), original[1].matrix.f());
        assert_eq!(images[0].image.rgba_buffer().unwrap(), [255, 0, 0, 255]);
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
