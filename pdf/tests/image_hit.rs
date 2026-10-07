use pdf::{document::owned::Document, geom::matrix::Matrix, parser::syntax::ParseMode};

#[test]
fn image_geometry_hits_select_last_draw_in_display_space_and_can_edit() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(
        0,
        format!("q 10 0 0 10 20 30 cm /{name} Do /{name} Do Q").into_bytes(),
    )
    .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let mut page = document.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let display = page.display_list().unwrap();
        assert!(display.matches_program(&program));
        let pdf::page::page_object::PageObject::Image(image) = &display.objects()[0] else {
            panic!("image")
        };
        let center = image
            .matrix
            .transform(&pdf::geom::point::Point::new(0.5, 0.5));
        let hits = display
            .image_geometry_hits(center.x(), center.y())
            .collect::<pdf::error::PdfResult<Vec<_>>>()
            .unwrap();
        assert_eq!(
            hits.iter().map(|(index, _)| *index).collect::<Vec<_>>(),
            [1, 0]
        );
        assert_eq!(hits[0].1.source().unwrap().operation_index, 3);
        assert_eq!(display.image_geometry_hits(-10., -10.).count(), 0);
    }
    let mut page = doc.get_page(0).unwrap();
    let display = page.display_list().unwrap();
    let snapshot = page.content_program().unwrap();
    let pdf::page::page_object::PageObject::Image(image) = &display.objects()[0] else {
        panic!("image")
    };
    let center = image
        .matrix
        .transform(&pdf::geom::point::Point::new(0.5, 0.5));
    let (_, selected) = display
        .image_geometry_hits(center.x(), center.y())
        .next()
        .unwrap()
        .unwrap();
    doc.transform_page_image_draw(
        0,
        &snapshot,
        selected.source().unwrap().operation_index,
        [1., 0., 0., 1., 1., 0.],
    )
    .unwrap();
    assert!(!doc.is_stamp_current(display.source_stamp().unwrap()));
}

#[test]
fn image_geometry_handles_rotation_shear_reflection_and_numeric_edges() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
        .unwrap();
    let mut image = doc
        .get_page(0)
        .unwrap()
        .extract_images_paint_order()
        .unwrap()
        .remove(0);
    for matrix in [
        Matrix::new(0., 10., -20., 0., 30., 40.),
        Matrix::new(10., 0., 5., 10., 0., 0.),
        Matrix::new(-10., 0., 0., 10., 20., 0.),
        Matrix::new(1e30, 0., 0., 1e30, 0., 0.),
        Matrix::new(1e-30, 0., 0., 1e-30, 0., 0.),
    ] {
        image.matrix = matrix;
        for (u, v, expected) in [
            (0.5, 0.5, true),
            (0., 0., true),
            (1., 1., true),
            (-0.5, 0.5, false),
            (0.5, 1.5, false),
        ] {
            let point = image.matrix.transform(&pdf::geom::point::Point::new(u, v));
            assert_eq!(
                image
                    .geometry_contains_display_point(point.x(), point.y())
                    .unwrap(),
                expected
            );
        }
    }
    image.matrix = Matrix::new(1., 2., 2., 4., 0., 0.);
    assert!(!image.geometry_contains_display_point(0., 0.).unwrap());
    assert!(image.geometry_contains_display_point(f32::NAN, 0.).is_err());
    image.matrix = Matrix::new(f32::INFINITY, 0., 0., 1., 0., 0.);
    assert!(image.geometry_contains_display_point(0., 0.).is_err());
}
