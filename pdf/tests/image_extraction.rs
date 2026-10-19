use pdf::{document::owned::Document, parser::syntax::ParseMode};

#[test]
fn image_report_matches_only_same_page_document_and_revision() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let mut page = doc.get_page(0).unwrap();
    let program = page.content_program().unwrap();
    let report = page.extract_images_paint_order_report().unwrap();
    assert!(report.matches_program(&program));
    assert!(!report.matches_program(&doc.get_page(1).unwrap().content_program().unwrap()));
    let offline = pdf::page::content_program::ContentProgram::from_decoded_bytes(
        program.source_bytes().to_vec(),
        Default::default(),
    )
    .unwrap();
    assert!(!report.matches_program(&offline));
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert!(!report.matches_program(&reopened.get_page(0).unwrap().content_program().unwrap()));
    doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
    assert!(!doc.is_stamp_current(report.source_stamp()));
    let fresh = doc.get_page(0).unwrap().content_program().unwrap();
    assert!(!report.matches_program(&fresh));
    // Two old snapshots still match each other; currentness is a separate check.
    assert!(report.matches_program(&program));
}

#[test]
fn image_draw_sources_match_content_program_across_stream_boundaries() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    // Split the Do token itself; offsets belong to decoded concatenated Contents.
    doc.replace_page_contents(0, format!("% prefix\nq /{name} D").into_bytes())
        .unwrap();
    doc.append_page_contents(0, format!("o\n% between\n/{name} Do Q\n").into_bytes())
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let mut page = document.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let images = page.extract_images_paint_order().unwrap();
        assert_eq!(images.len(), 2);
        for (image, index) in images.iter().zip([1, 2]) {
            let source = image.source().unwrap();
            assert_eq!(source.operation_index, index);
            assert_eq!(
                Some(source.operator_byte_offset),
                program.operations()[index].operator.byte_offset()
            );
            assert_eq!(
                &program.source_bytes()[source.operator_byte_offset as usize..][..2],
                b"Do"
            );
        }
        let manual = pdf::page::image_object::ImageObject::new(
            images[0].matrix.clone(),
            images[0].image.clone(),
            images[0].graphic_state.clone(),
        );
        assert_eq!(manual.source(), None);
    }
}

#[test]
fn unknown_operations_are_counted_in_image_draw_source_indices() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(0, format!("RzpdfUnknown q /{name} Do Q").into_bytes())
        .unwrap();
    let mut page = doc.get_page(0).unwrap();
    let program = page.content_program().unwrap();
    let report = page.extract_images_paint_order_report().unwrap();
    assert_eq!(report.diagnostics.len(), 1);
    let source = report.images[0].source().unwrap();
    assert_eq!(source.operation_index, 2);
    assert_eq!(
        Some(source.operator_byte_offset),
        program.operations()[2].operator.byte_offset()
    );
}

#[test]
fn image_extraction_report_retains_compatibility_diagnostics() {
    use pdf::{page::diagnostic::DiagnosticKind, parser::options::ParseOptions};
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"RzpdfUnknown\n".to_vec())
        .unwrap();
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_images_paint_order_report()
        .unwrap();
    assert!(report.images.is_empty());
    assert_eq!(report.diagnostics.len(), 1);
    assert_eq!(report.diagnostics[0].kind, DiagnosticKind::UnknownOperator);
    assert_eq!(report.diagnostics[0].operator, "RzpdfUnknown");
    assert!(report.diagnostics[0].byte_offset.is_some());
    let strict = Document::from_bytes_with_options(
        doc.save_to_bytes().unwrap(),
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )
    .unwrap();
    assert!(
        strict
            .get_page(0)
            .unwrap()
            .extract_images_paint_order_report()
            .is_err()
    );
}

#[test]
fn image_draw_sample_budget_counts_soft_masks_and_repeated_draws() {
    use pdf::{
        error::PdfError,
        page::content_builder::ContentBuilder,
        parser::options::{Limits, ParseOptions},
    };
    for (budget, draws, success) in [(256, 2, true), (255, 2, false), (256, 3, false)] {
        let doc = Document::new_with_options(ParseOptions {
            limits: Limits {
                max_decoded_bytes: budget,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .embed_rgba_image(0, 32, 1, vec![128; 32 * 4])
            .unwrap();
        // Each draw retains 96 normalized RGB bytes plus 32 soft-mask bytes.
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .extract_decoded_image(&name)
                .unwrap()
                .data()
                .len(),
            96
        );
        let mut builder = ContentBuilder::new();
        for index in 0..draws {
            builder
                .draw_image(&name, [32.0, 0.0, 0.0, 1.0, index as f32 * 40.0, 10.0])
                .unwrap();
        }
        doc.replace_page_content_builder(0, &builder).unwrap();
        let revision = doc.revision();
        let display = doc.get_page(0).unwrap().display_list();
        if success {
            assert!(display.is_ok());
        } else {
            assert!(matches!(display, Err(PdfError::ResourceLimit(_))));
        }
        let result = doc.get_page(0).unwrap().extract_images_paint_order();
        if success {
            assert_eq!(result.unwrap().len(), draws);
        } else {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        }
        assert_eq!(doc.revision(), revision);
    }
}

#[test]
fn image_draw_extraction_preserves_repetition_positions_and_unused_resources() {
    use pdf::page::content_builder::ContentBuilder;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgba_image(0, 1, 1, vec![10, 20, 30, 128])
        .unwrap();
    doc.resources()
        .embed_rgba_image(0, 1, 1, vec![0, 0, 0, 255])
        .unwrap();
    let mut builder = ContentBuilder::new();
    builder
        .draw_image(&name, [10.0, 0.0, 0.0, 20.0, 48.0, 100.0])
        .unwrap()
        .draw_image(&name, [30.0, 0.0, 0.0, 40.0, 148.0, 200.0])
        .unwrap();
    doc.replace_page_content_builder(0, &builder).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let images = document
            .get_page(0)
            .unwrap()
            .extract_images_paint_order()
            .unwrap();
        assert_eq!(images.len(), 2);
        assert_eq!(images[0].image.name(), name);
        assert_eq!(images[1].image.name(), name);
        assert_eq!(images[0].matrix.a(), 10.0);
        assert_eq!(images[0].matrix.e(), 48.0);
        assert_eq!(images[1].matrix.a(), 30.0);
        assert_eq!(images[1].matrix.e(), 148.0);
        assert_eq!(images[0].image.rgba_buffer().unwrap(), [10, 20, 30, 128]);
    }
}

#[test]
fn image_resource_discovery_is_sorted_and_keeps_unused_images() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let mut expected = Vec::new();
    for value in 0..12 {
        expected.push(
            doc.resources()
                .embed_rgba_image(0, 1, 1, vec![value, 0, 0, 255])
                .unwrap(),
        );
    }
    expected.sort_unstable();
    assert_eq!(
        doc.get_page(0).unwrap().image_resource_names().unwrap(),
        expected
    );
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .image_resource_names()
            .unwrap(),
        expected
    );
}

#[test]
fn unknown_encoding_is_preserved_for_raw_extraction_but_cannot_decode() {
    use pdf::{
        document::resource_manager::ResourceKind,
        io::stream_reader::StreamReader,
        objects::{PdfObject, pdf_stream::PdfStream},
        parser::syntax::SyntaxParser,
    };
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let mut parser = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
        b"<< /Type /XObject /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Filter /RzpdfUnknown /DecodeParms << /Private 7 >> >>".to_vec(),
    )).unwrap());
    let dict = parser.get_object().unwrap().into_dict().unwrap();
    let bytes = vec![1, 2, 3, 0, 255];
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, bytes.clone())))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Image, id)
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let page = document.get_page(0).unwrap();
        assert_eq!(
            page.image_resource_names().unwrap().as_slice(),
            std::slice::from_ref(&name)
        );
        let source = page.extract_image_source(&name).unwrap();
        assert_eq!(source.stream.raw_data(), bytes);
        assert_eq!(
            source
                .stream
                .dict()
                .get("Filter")
                .unwrap()
                .as_name()
                .unwrap()
                .name(),
            "RzpdfUnknown"
        );
        assert_eq!(
            source
                .stream
                .dict()
                .get("DecodeParms")
                .unwrap()
                .as_dict()
                .unwrap()
                .get("Private")
                .unwrap()
                .as_number()
                .unwrap()
                .as_u32_checked()
                .unwrap(),
            7
        );
        assert!(page.extract_decoded_image(&name).is_err());
    }
}

#[test]
fn named_image_source_and_rgba_survive_save_without_painting() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .image_resource_names()
            .unwrap()
            .is_empty()
    );
    let name = doc
        .resources()
        .embed_rgba_image(0, 1, 1, vec![10, 20, 30, 128])
        .unwrap();
    let source = doc
        .get_page(0)
        .unwrap()
        .extract_image_source(&name)
        .unwrap();
    assert_eq!(
        (source.width, source.height, source.bits_per_component),
        (1, 1, Some(8))
    );
    assert_eq!(source.stream.raw_data(), &[10, 20, 30]);
    assert!(source.stream.dict().get("SMask").is_some());
    assert!(source.stream.dict().get("Filter").is_none());
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let page = document.get_page(0).unwrap();
        assert_eq!(
            page.image_resource_names().unwrap().as_slice(),
            std::slice::from_ref(&name)
        );
        let image = page.extract_decoded_image(&name).unwrap();
        assert_eq!(image.pixel_dimensions(), (1, 1));
        assert_eq!(image.data(), &[10, 20, 30]);
        assert_eq!(image.rgba_buffer().unwrap(), [10, 20, 30, 128]);
        assert!(page.extract_image_source("Missing").is_err());
    }
}
