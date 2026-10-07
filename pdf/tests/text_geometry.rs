use pdf::{
    document::{PdfDocument, resource_manager::StandardFont, save_options::PdfVersion},
    geom::point::Point,
    parser::syntax::ParseMode,
};

#[test]
fn character_em_transform_matches_explicit_affine_text_and_graphics_equations() {
    for rotation in [0, 90, 180, 270] {
        for tm in [
            [0., 1., -1., 0., 20., 40.],
            [1., 0.25, 0.5, 1., 20., 40.],
            [-1., 0., 0., 1., 20., 40.],
        ] {
            let doc = PdfDocument::new_with_version(PdfVersion::V1_6).unwrap();
            doc.add_page(0).unwrap();
            doc.set_page_rotation(0, rotation).unwrap();
            doc.set_page_user_unit(0, Some(1.5)).unwrap();
            let font = doc
                .resources()
                .register_standard_font(0, StandardFont::Helvetica)
                .unwrap();
            doc.replace_page_contents(0, format!("q 1 0.25 -0.5 1 100 100 cm BT /{font} 12 Tf -75 Tz 3 Ts {} {} {} {} {} {} Tm (A) Tj ET Q", tm[0], tm[1], tm[2], tm[3], tm[4], tm[5]).into_bytes()).unwrap();
            let reopened =
                PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            for doc in [&doc, &reopened] {
                let mut page = doc.get_page(0).unwrap();
                let geometry = page.geometry().unwrap();
                let chars = page.extract_characters_paint_order().unwrap();
                assert_eq!(chars.len(), 1);
                let character = &chars[0];
                for (x, y) in [(0., 0.), (1., 0.5), (-0.25, 1.)] {
                    // Independent scalar equations: Tf/Tz, Ts, Tm, then cm.
                    let local_x = x * -9.;
                    let local_y = y * 12. + 3.;
                    let text_x = tm[0] * local_x + tm[2] * local_y + tm[4];
                    let text_y = tm[1] * local_x + tm[3] * local_y + tm[5];
                    let user =
                        Point::new(text_x - 0.5 * text_y + 100., 0.25 * text_x + text_y + 100.);
                    let expected = geometry.user_to_display.checked_transform(&user).unwrap();
                    let actual = character
                        .em_to_display_matrix()
                        .unwrap()
                        .checked_transform(&Point::new(x, y))
                        .unwrap();
                    assert!(
                        (actual.x() - expected.x()).abs() < 0.001
                            && (actual.y() - expected.y()).abs() < 0.001,
                        "rotation={rotation}, tm={tm:?}, point=({x},{y})"
                    );
                    let restored = character.display_to_em_point(&expected).unwrap();
                    assert!((restored.x() - x).abs() < 0.001 && (restored.y() - y).abs() < 0.001);
                }
                for point in [Point::new(f32::NAN, 0.), Point::new(0., f32::INFINITY)] {
                    assert!(character.display_to_em_point(&point).is_err());
                }
            }
        }
    }
}

#[test]
fn character_em_coordinates_include_advancing_origins_rotation_and_user_unit() {
    for rotation in [0, 90, 180, 270] {
        let doc = PdfDocument::new_with_version(PdfVersion::V1_6).unwrap();
        doc.add_page(0).unwrap();
        doc.set_page_rotation(0, rotation).unwrap();
        doc.set_page_user_unit(0, Some(1.5)).unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{font} 10 Tf 50 Tz 1 0 0 1 20 40 Tm (AB) Tj ET").into_bytes(),
        )
        .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [&doc, &reopened] {
            let mut page = doc.get_page(0).unwrap();
            let geometry = page.geometry().unwrap();
            let characters = page.extract_characters_paint_order().unwrap();
            assert_eq!(characters.len(), 2);
            assert_ne!(
                (characters[0].position.x(), characters[0].position.y()),
                (characters[1].position.x(), characters[1].position.y())
            );
            for character in &characters {
                let matrix = character.em_to_display_matrix().unwrap();
                let origin = matrix.checked_transform(&Point::new(0., 0.)).unwrap();
                assert_eq!(
                    (origin.x(), origin.y()),
                    (character.position.x(), character.position.y())
                );
                let user_origin = geometry.display_to_user_point(&character.position).unwrap();
                let expected = geometry
                    .user_to_display
                    .checked_transform(&Point::new(user_origin.x() + 5., user_origin.y() + 10.))
                    .unwrap();
                let display = matrix.checked_transform(&Point::new(1., 1.)).unwrap();
                assert!((display.x() - expected.x()).abs() < 0.001);
                assert!((display.y() - expected.y()).abs() < 0.001);
                let local = character.display_to_em_point(&display).unwrap();
                assert!((local.x() - 1.).abs() < 0.001 && (local.y() - 1.).abs() < 0.001);
            }
            let mut invalid = characters[0].clone();
            invalid.horizontal_scale = 0.;
            assert!(invalid.display_to_em_point(&invalid.position).is_err());
            invalid.font_size = f32::NAN;
            assert!(invalid.em_to_display_matrix().is_err());
        }
    }
}
