use pdf::{document::owned::Document, geom::rectangle::Rectangle, parser::syntax::ParseMode};

#[test]
fn display_affine_transform_matches_requested_rotation_scale_and_shear() {
    use pdf::geom::{matrix::Matrix, point::Point};
    for rotation in [0, 90, 180, 270] {
        for transform in [
            [0., 1., -1., 0., 100., 50.],
            [2., 0., 0., 0.5, -10., 20.],
            [-1., 0., 0.5, 1., 80., -5.],
        ] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_rotation(0, rotation).unwrap();
            let name = doc
                .resources()
                .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
                .unwrap();
            doc.replace_page_contents(
                0,
                format!("q 2 1 0 4 10 20 cm /{name} Do /{name} Do Q").into_bytes(),
            )
            .unwrap();
            let before = doc
                .get_page(0)
                .unwrap()
                .extract_images_paint_order()
                .unwrap();
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            doc.transform_page_image_draw_display(0, &snapshot, 2, transform)
                .unwrap();
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            let [a, b, c, d, e, f] = transform;
            let desired = Matrix::new(a, b, c, d, e, f);
            for document in [&doc, &reopened] {
                let after = document
                    .get_page(0)
                    .unwrap()
                    .extract_images_paint_order()
                    .unwrap();
                for (x, y) in [(0., 0.), (1., 0.), (0., 1.), (1., 1.)] {
                    let old = before[0].matrix.transform(&Point::new(x, y));
                    let expected = desired.transform(&old);
                    let actual = after[0].matrix.transform(&Point::new(x, y));
                    assert!((actual.x() - expected.x()).abs() < 0.001);
                    assert!((actual.y() - expected.y()).abs() < 0.001);
                    let old_neighbor = before[1].matrix.transform(&Point::new(x, y));
                    let neighbor = after[1].matrix.transform(&Point::new(x, y));
                    assert_eq!(neighbor.x(), old_neighbor.x());
                    assert_eq!(neighbor.y(), old_neighbor.y());
                }
            }
        }
    }
}

#[test]
fn display_translation_handles_page_rotation_crop_and_affine_ctm() {
    for rotation in [0, 90, 180, 270] {
        for ctm in [
            "2 0 0 4 10 20",
            "0 2 -4 0 10 20",
            "2 1 0 4 10 20",
            "-2 0 0 4 10 20",
        ] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_rotation(0, rotation).unwrap();
            doc.set_page_crop_box(0, Some(Rectangle::new(5., 10., 200., 300.)))
                .unwrap();
            let name = doc
                .resources()
                .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
                .unwrap();
            doc.replace_page_contents(
                0,
                format!("q {ctm} cm /{name} Do /{name} Do Q").into_bytes(),
            )
            .unwrap();
            doc.copy_page(0, 1).unwrap();
            let before = doc
                .get_page(0)
                .unwrap()
                .extract_images_paint_order()
                .unwrap();
            let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
            let operation = before[0].source().unwrap().operation_index;
            doc.translate_page_image_draw_display(0, &snapshot, operation, 12., -8.)
                .unwrap();
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for document in [&doc, &reopened] {
                let images = document
                    .get_page(0)
                    .unwrap()
                    .extract_images_paint_order()
                    .unwrap();
                assert!((images[0].matrix.e() - before[0].matrix.e() - 12.).abs() < 0.0001);
                assert!((images[0].matrix.f() - before[0].matrix.f() + 8.).abs() < 0.0001);
                for (new, old) in [
                    (&images[0].matrix, &before[0].matrix),
                    (&images[1].matrix, &before[1].matrix),
                ] {
                    assert_eq!(
                        [new.a(), new.b(), new.c(), new.d()],
                        [old.a(), old.b(), old.c(), old.d()]
                    );
                }
                assert_eq!(images[1].matrix.e(), before[1].matrix.e());
                assert_eq!(images[1].matrix.f(), before[1].matrix.f());
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
}

#[test]
fn display_translation_rejects_invalid_targets_and_singular_transforms_without_mutation() {
    for ctm in ["0 0 0 0 0 0", "1 2 2 4 0 0", "1e-40 0 0 1e-40 0 0"] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        // PDF numbers have no exponent syntax.
        let ctm = if ctm.starts_with("1e") {
            "0.0000000000000000000000000000000000000001 0 0 0.0000000000000000000000000000000000000001 0 0"
        } else {
            ctm
        };
        doc.replace_page_contents(0, format!("q {ctm} cm /{name} Do Q").into_bytes())
            .unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let revision = doc.revision();
        for (index, dx, dy) in [(2, 1., 1.), (2, f32::NAN, 0.), (1, 1., 0.), (99, 1., 0.)] {
            assert!(
                doc.translate_page_image_draw_display(0, &snapshot, index, dx, dy)
                    .is_err()
            );
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
