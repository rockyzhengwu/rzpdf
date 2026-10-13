use pdf::{
    document::{PdfDocument, acroform::FormValue, links::LinkTarget, save_options::PdfVersion},
    parser::syntax::ParseMode,
};

#[test]
fn reportlab_navigation_links_and_form_can_be_read_and_filled() {
    let bytes = include_bytes!("fixtures/v04/reportlab.pdf").to_vec();
    let doc = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    let bookmarks = doc.flat_bookmarks().unwrap();
    assert_eq!(bookmarks.len(), 2);
    assert_eq!(bookmarks[0].title, "External first");
    assert_eq!(bookmarks[1].page_index, 1);
    let links = doc.page_links(0).unwrap();
    assert_eq!(links.len(), 2);
    assert!(
        links
            .iter()
            .any(|link| matches!(&link.target, LinkTarget::Page(1)))
    );
    assert!(
        links.iter().any(
            |link| matches!(&link.target, LinkTarget::Uri(uri) if uri == "https://example.org/")
        )
    );
    let fields = doc.form_fields().unwrap();
    assert_eq!(fields.len(), 2);
    assert!(
        fields.iter().any(|field| field.name == "external_text"
            && field.value == FormValue::Text("External".into()))
    );
    assert!(fields.iter().any(|field| field.name == "external_check" && field.value == FormValue::Checkbox(true)));
    let revision = doc.commit_revision();
    assert!(
        doc.set_form_value("external_text", FormValue::Text("x".repeat(101)))
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    doc.set_form_value("external_text", FormValue::Text("Updated".into()))
        .unwrap();
    doc.set_form_value("external_check", FormValue::Checkbox(false))
        .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert!(
        reopened
            .form_fields()
            .unwrap()
            .iter()
            .any(|field| field.name == "external_text"
                && field.value == FormValue::Text("Updated".into()))
    );
    assert!(
        reopened.form_fields().unwrap().iter().any(
            |field| field.name == "external_check" && field.value == FormValue::Checkbox(false)
        )
    );
}

#[test]
fn interaction_writes_respect_document_version() {
    let doc = PdfDocument::new_with_version(PdfVersion::V1_0).unwrap();
    doc.add_page(0).unwrap();
    let revision = doc.commit_revision();
    assert!(doc.add_attachment("a.txt", vec![]).is_err());
    assert!(doc.set_named_destination("first", 0).is_err());
    assert!(
        doc.add_square_annotation(0, [10.0, 10.0, 40.0, 40.0], [1.0, 0.0, 0.0])
            .is_err()
    );
    assert!(
        doc.add_text_field(0, [10.0, 10.0, 100.0, 40.0], "name", "value")
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
}

#[test]
fn qpdf_embedded_file_can_be_read_and_updated() {
    let bytes = include_bytes!("fixtures/v04/qpdf-attachment.pdf").to_vec();
    let doc = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(doc.attachment_names().unwrap(), vec!["attachment.txt"]);
    let attachment = doc.attachment_data("attachment.txt").unwrap().unwrap();
    assert_eq!(
        attachment.as_slice(),
        include_bytes!("fixtures/v04/attachment.txt")
    );
    doc.add_attachment("second.txt", b"second".to_vec())
        .unwrap();
    assert!(doc.remove_attachment("attachment.txt").unwrap());
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(reopened.attachment_names().unwrap(), vec!["second.txt"]);
    assert_eq!(
        reopened.attachment_data("second.txt").unwrap(),
        Some(b"second".to_vec())
    );
}

#[test]
fn pypdf_square_and_named_destination_are_readable() {
    let bytes = include_bytes!("fixtures/v04/pypdf-square.pdf").to_vec();
    let doc = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(
        doc.named_destinations().unwrap(),
        vec![("external-target".into(), 1)]
    );
    let squares = doc.page_square_annotations(0).unwrap();
    assert_eq!(squares.len(), 1);
    assert_eq!(squares[0].rect, [300.0, 480.0, 450.0, 540.0]);
    assert_eq!(squares[0].color, [0.0, 0.0, 1.0]);
}

#[test]
fn page_reorder_keeps_interaction_targets_bound_to_page_objects() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    doc.add_flat_bookmark("second", 1).unwrap();
    doc.set_named_destination("second", 1).unwrap();
    doc.add_page_link(0, [10.0, 10.0, 50.0, 30.0], 1).unwrap();
    doc.reorder_pages(&[1, 0]).unwrap();
    assert_eq!(doc.flat_bookmarks().unwrap()[0].page_index, 0);
    assert_eq!(
        doc.named_destinations().unwrap(),
        vec![("second".into(), 0)]
    );
    assert!(
        doc.page_links(1)
            .unwrap()
            .iter()
            .any(|link| matches!(&link.target, LinkTarget::Page(0)))
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(reopened.flat_bookmarks().unwrap()[0].page_index, 0);
    assert_eq!(
        reopened.named_destinations().unwrap(),
        vec![("second".into(), 0)]
    );
    assert!(
        reopened
            .page_links(1)
            .unwrap()
            .iter()
            .any(|link| matches!(&link.target, LinkTarget::Page(0)))
    );
}
