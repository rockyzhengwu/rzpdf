use pdf::{
    document::owned::Document,
    geom::{point::Point, rectangle::Rectangle},
    parser::syntax::ParseMode,
};

#[test]
fn geometry_query_does_not_interpret_content_and_refreshes_after_edit() {
    for rotation in [0, 90, 180, 270] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.set_page_crop_box(0, Some(Rectangle::new(10., 20., 90., 70.)))
            .unwrap();
        doc.set_page_rotation(0, rotation).unwrap();
        doc.replace_page_contents(0, b"/MissingImage Do".to_vec())
            .unwrap();
        let stamp = doc.stamp();
        let mut page = doc.get_page(0).unwrap();
        assert!(page.display_list().is_err());
        let geometry = page.geometry().unwrap();
        let dimensions = if rotation % 180 == 0 {
            (80., 50.)
        } else {
            (50., 80.)
        };
        assert_eq!(
            (geometry.display_width, geometry.display_height),
            dimensions
        );
        assert_eq!(geometry.rotation, rotation);
        assert_eq!(
            (
                geometry.visible_box.llx(),
                geometry.visible_box.lly(),
                geometry.visible_box.urx(),
                geometry.visible_box.ury()
            ),
            (10., 20., 90., 70.)
        );
        for (x, y) in [(10., 20.), (90., 20.), (10., 70.), (90., 70.)] {
            let actual = geometry.user_to_display.transform(&Point::new(x, y));
            assert_eq!((actual.x(), actual.y()), expected_point(rotation, x, y));
            let restored = geometry.display_to_user_point(&actual).unwrap();
            assert_eq!((restored.x(), restored.y()), (x, y));
        }
        assert!(
            geometry
                .display_to_user_point(&Point::new(f32::NAN, 0.))
                .is_err()
        );
        assert!(doc.is_stamp_current(&stamp));
        doc.rotate_page(0, 90).unwrap();
        assert_eq!(page.geometry().unwrap().rotation, (rotation + 90) % 360);
        // Returned geometry is a value snapshot, not a live view of the page.
        assert_eq!(geometry.rotation, rotation);
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let after = reopened.get_page(0).unwrap().geometry().unwrap();
        assert_eq!(after.rotation, (rotation + 90) % 360);
    }
}

fn expected_point(rotation: i32, x: f32, y: f32) -> (f32, f32) {
    match rotation {
        0 => (x - 10., 70. - y),
        90 => (y - 20., x - 10.),
        180 => (90. - x, y - 20.),
        270 => (70. - y, 90. - x),
        _ => unreachable!(),
    }
}

#[test]
fn relative_rotation_refreshes_retained_page_and_survives_strict_save() {
    for initial in [0, 90, 180, 270] {
        for delta in [-450, -90, 90, 450] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_crop_box(0, Some(Rectangle::new(10., 20., 90., 70.)))
                .unwrap();
            doc.set_page_rotation(0, initial).unwrap();
            let name = doc
                .resources()
                .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
                .unwrap();
            let content = format!("q 20 0 0 10 30 50 cm /{name} Do Q");
            doc.replace_page_contents(0, content.as_bytes().to_vec())
                .unwrap();
            doc.copy_page(0, 1).unwrap();
            let mut retained = doc.get_page(0).unwrap();
            let original_program = retained.content_program().unwrap();
            let content_id = original_program.stream_object_id(0);
            let before = retained.display_list().unwrap();
            let old_stamp = before.source_stamp().unwrap().clone();
            doc.rotate_page(0, delta).unwrap();
            assert!(!doc.is_stamp_current(&old_stamp));
            let rotation = (initial + delta).rem_euclid(360);
            let fresh = retained.display_list().unwrap();
            let dimensions = if rotation % 180 == 0 {
                (80., 50.)
            } else {
                (50., 80.)
            };
            assert_eq!((fresh.page_width(), fresh.page_height()), dimensions);
            assert!(doc.is_stamp_current(fresh.source_stamp().unwrap()));
            let stamp = doc.stamp();
            let reopened =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert!(doc.is_stamp_current(&stamp));
            for document in [&doc, &reopened] {
                for (index, angle) in [(0, rotation), (1, initial)] {
                    let mut page = document.get_page(index).unwrap();
                    let program = page.content_program().unwrap();
                    assert_eq!(program.source_bytes(), content.as_bytes());
                    assert_eq!(program.stream_object_id(0), content_id);
                    let list = page.display_list().unwrap();
                    let dims = if angle % 180 == 0 {
                        (80., 50.)
                    } else {
                        (50., 80.)
                    };
                    assert_eq!((list.page_width(), list.page_height()), dims);
                    let images = page.extract_images_paint_order().unwrap();
                    assert_eq!(images.len(), 1);
                    for (x, y) in [(0., 0.), (1., 0.), (0., 1.), (1., 1.)] {
                        let expected = expected_point(angle, 30. + 20. * x, 60. - 10. * y);
                        let actual = images[0].matrix.transform(&Point::new(x, y));
                        assert!((actual.x() - expected.0).abs() < 0.0001);
                        assert!((actual.y() - expected.1).abs() < 0.0001);
                    }
                }
            }
        }
    }
}
