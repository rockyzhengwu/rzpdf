use pdf::{document::owned::Document, parser::syntax::ParseMode};

#[test]
fn remove_cross_stream_image_draw_preserves_path_neighbors_resources_and_shared_page() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    doc.replace_page_contents(0, format!("% head\n0 0 m /{name} D").into_bytes())
        .unwrap();
    doc.append_page_contents(0, format!("o\n10 10 l S /{name} Do\n% tail\n").into_bytes())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    assert!(doc.remove_page_image_draw(0, &snapshot, 0).is_err());
    assert_eq!(doc.revision(), revision);
    doc.remove_page_image_draw(0, &snapshot, 1).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let mut page = document.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        assert_eq!(
            program
                .operations()
                .iter()
                .map(|op| op.operator.name())
                .collect::<Vec<_>>(),
            ["m", "l", "S", "Do"]
        );
        assert_eq!(program.stream_ranges().len(), 2);
        assert!(program.source_bytes().starts_with(b"% head\n0 0 m "));
        assert!(program.source_bytes().ends_with(b"\n% tail\n"));
        assert_eq!(page.extract_images_paint_order().unwrap().len(), 1);
        assert_eq!(page.image_resource_names().unwrap(), [name.clone()]);
        assert!(page.display_list().unwrap().diagnostics().is_empty());
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

#[test]
fn offline_removal_handles_first_last_only_and_empty_streams() {
    use pdf::page::content_program::ContentProgram;
    let options = Default::default();
    for (bytes, index, remaining) in [
        (b"q Q".as_slice(), 0, vec!["Q"]),
        (b"q Q", 1, vec!["q"]),
        (b"q", 0, vec![]),
    ] {
        let program = ContentProgram::from_decoded_streams(
            vec![Vec::new(), bytes.to_vec(), Vec::new()],
            options,
        )
        .unwrap();
        let edited = program.without_operation(index, options).unwrap();
        assert_eq!(edited.stream_ranges().len(), 3);
        assert_eq!(
            edited
                .operations()
                .iter()
                .map(|op| op.operator.name())
                .collect::<Vec<_>>(),
            remaining
        );
        assert!(edited.source_stamp().is_none());
    }
}
