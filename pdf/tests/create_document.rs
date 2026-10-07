use pdf::{
    document::PdfDocument,
    error::PdfError,
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};

#[test]
fn constructed_clipped_path_survives_save_and_content_interpretation() {
    use pdf::page::content_builder::ContentBuilder;
    let document = PdfDocument::new().unwrap();
    document.add_page(0).unwrap();
    let mut builder = ContentBuilder::new();
    builder
        .save_state()
        .rectangle(0.0, 0.0, 100.0, 100.0)
        .clip()
        .end_path()
        .set_fill_rgb(1.0, 0.0, 0.0)
        .move_to(0.0, 0.0)
        .line_to(50.0, 0.0)
        .curve_to([50.0, 25.0], [25.0, 50.0], [0.0, 50.0])
        .close_path()
        .fill()
        .restore_state();
    document.replace_page_content_builder(0, &builder).unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(document.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    assert!(reopened.get_page(0).unwrap().display_list().is_ok());
}

#[test]
fn new_document_saves_and_edits_without_an_input_source() {
    let document = PdfDocument::new().unwrap();
    assert_eq!(document.version(), "1.7");
    assert_eq!(document.try_page_count().unwrap(), 0);
    let empty = document.save_to_bytes().unwrap();
    let reopened = PdfDocument::from_bytes_with_mode(empty, ParseMode::Strict).unwrap();
    assert_eq!(reopened.try_page_count().unwrap(), 0);
    document.add_page(0).unwrap();
    document.add_page(1).unwrap();
    document
        .replace_page_contents(0, b"0 0 m 10 10 l S".to_vec())
        .unwrap();
    document
        .append_page_contents(1, b"1 2 3 4 re f".to_vec())
        .unwrap();
    assert_eq!(document.try_page_count().unwrap(), 2);
    let reopened =
        PdfDocument::from_bytes_with_mode(document.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    assert_eq!(reopened.try_page_count().unwrap(), 2);
    document.delete_page(0).unwrap();
    assert_eq!(document.try_page_count().unwrap(), 1);
    let reopened =
        PdfDocument::from_bytes_with_mode(document.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    assert_eq!(reopened.try_page_count().unwrap(), 1);
}

#[test]
fn new_document_enforces_object_budget_and_rolls_back_failed_page_creation() {
    for max_objects in [0, 1] {
        assert!(matches!(
            PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_objects,
                    ..Default::default()
                },
                ..Default::default()
            }),
            Err(PdfError::ResourceLimit(_))
        ));
    }
    let document = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 3,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    for _ in 0..2 {
        assert!(matches!(
            document.add_page(0),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(document.try_page_count().unwrap(), 0);
    }
    assert_eq!(
        document
            .try_insert_object(pdf::objects::PdfObject::PdfNull)
            .unwrap()
            .obj_num(),
        3
    );
}
