//! Version-scoped regression: compose public APIs instead of testing them alone.
use pdf::{
    document::{
        PdfDocument,
        resource_manager::StandardFont,
        save_options::{SaveOptions, StreamCompression},
    },
    geom::rectangle::Rectangle,
    page::{content_builder::ContentBuilder, page_builder::PageBuilder},
    parser::syntax::ParseMode,
};

#[test]
fn v01_independent_mupdf_sample_extracts_text_positions_and_image() {
    let bytes = include_bytes!("fixtures/v01/mupdf.pdf").to_vec();
    let source = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(source.try_page_count().unwrap(), 2);
    for index in 0..2 {
        let mut page = source.get_page(index).unwrap();
        let report = page.extract_characters_paint_order_report().unwrap();
        assert!(report.diagnostics.is_empty());
        let expected = format!("External producer - page {}", index + 1);
        assert_eq!(
            report
                .characters
                .iter()
                .map(|ch| ch.unicode.as_deref().unwrap())
                .collect::<String>(),
            expected
        );
        let first = &report.characters[0];
        let expected_position = if index == 0 { (20., 40.) } else { (120., 20.) };
        assert!((first.position.x() - expected_position.0).abs() < 0.001);
        assert!((first.position.y() - expected_position.1).abs() < 0.001);
        assert_eq!(first.font_size, 16.);
        let names = page.image_resource_names().unwrap();
        if index == 0 {
            assert_eq!(names, ["Im0"]);
            let raw = page.extract_image_source("Im0").unwrap();
            assert_eq!(
                (raw.width, raw.height, raw.bits_per_component),
                (2, 1, Some(8))
            );
            assert_eq!(
                raw.stream
                    .dict()
                    .get("Filter")
                    .unwrap()
                    .as_name()
                    .unwrap()
                    .name(),
                "FlateDecode"
            );
            assert!(!raw.stream.raw_data().unwrap().is_empty());
            assert_eq!(
                page.extract_decoded_image("Im0")
                    .unwrap()
                    .rgba_buffer()
                    .unwrap(),
                [255, 0, 0, 255, 0, 0, 255, 255]
            );
            let draws = page.extract_images_paint_order_report().unwrap();
            assert!(draws.diagnostics.is_empty());
            assert_eq!(draws.images.len(), 1);
        } else {
            assert!(names.is_empty());
        }
    }
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let saved = source
            .save_to_bytes_with_options(SaveOptions {
                compression,
                ..Default::default()
            })
            .unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(saved, ParseMode::Strict).unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 2);
        assert_eq!(
            reopened
                .get_page(0)
                .unwrap()
                .extract_decoded_image("Im0")
                .unwrap()
                .rgba_buffer()
                .unwrap(),
            [255, 0, 0, 255, 0, 0, 255, 255]
        );
    }
}

#[test]
fn v01_page_workflow_preserves_source_and_survives_both_save_modes() {
    let source = PdfDocument::new().unwrap();
    for index in 0..2 {
        source
            .add_page_with_builder(index, PageBuilder::new().size(200., 120.))
            .unwrap();
        let font = source
            .resources()
            .register_standard_font(index, StandardFont::Helvetica)
            .unwrap();
        let image = source
            .resources()
            .embed_rgb_image(index, 1, 1, vec![255, 0, 0])
            .unwrap();
        let mut content = ContentBuilder::new();
        content.begin_text();
        content.try_set_font(&font, 12.).unwrap();
        content.move_text(20., 80.);
        content
            .show_encoded_text(
                &StandardFont::Helvetica
                    .encode_text(&format!("Page{index}"))
                    .unwrap(),
            )
            .unwrap();
        content.end_text();
        content
            .draw_image(&image, [10., 0., 0., 10., 20., 20.])
            .unwrap();
        content.rectangle(40., 20., 10., 10.).fill();
        source
            .replace_page_content_builder(index, &content)
            .unwrap();
    }
    let source_stamp = source.stamp();
    let source_bytes = source.save_to_bytes().unwrap();
    let target = PdfDocument::new().unwrap();
    target.merge_document(&source).unwrap();
    target.copy_page(0, 2).unwrap();
    target.set_page_rotation(2, 90).unwrap();
    target
        .set_page_crop_box(2, Some(Rectangle::new(10., 10., 190., 110.)))
        .unwrap();
    target.reorder_pages(&[2, 1, 0]).unwrap();
    target.delete_page(2).unwrap();
    target.add_page(2).unwrap();
    target.delete_page(2).unwrap();

    let before = target.save_to_bytes().unwrap();
    assert!(target.reorder_pages(&[0, 0]).is_err());
    assert!(target.import_pages_from(&source, &[0, 99], 0).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
    let selected = target.extract_pages(&[1, 0]).unwrap();
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let mut output = Vec::new();
        selected
            .save_to_writer_with_options(
                &mut output,
                SaveOptions {
                    compression,
                    ..Default::default()
                },
            )
            .unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(output, ParseMode::Strict).unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 2);
        for (index, expected) in [(0, "Page1"), (1, "Page0")] {
            let mut page = reopened.get_page(index).unwrap();
            let text = page
                .extract_characters_paint_order()
                .unwrap()
                .into_iter()
                .map(|ch| ch.unicode.unwrap())
                .collect::<String>();
            assert_eq!(text, expected);
            assert_eq!(page.extract_images_paint_order().unwrap().len(), 1);
            let display = page.display_list().unwrap();
            let expected_size = if index == 0 {
                (200., 120.)
            } else {
                (100., 180.)
            };
            assert_eq!((display.page_width(), display.page_height()), expected_size);
        }
    }
    assert!(source.is_stamp_current(&source_stamp));
    assert_eq!(source.save_to_bytes().unwrap(), source_bytes);
}
