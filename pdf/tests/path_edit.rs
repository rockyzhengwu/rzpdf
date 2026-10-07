use pdf::{document::owned::Document, parser::syntax::ParseMode};

#[test]
fn path_removal_preserves_geometry_clipping_and_shared_page() {
    use pdf::page::page_object::PageObject;
    for paint in ["S", "s", "f", "F", "f*", "B", "B*", "b", "b*"] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(
            0,
            format!("% keep\n0 0 10 10 re W {paint} 20 20 5 5 re f\n").into_bytes(),
        )
        .unwrap();
        doc.copy_page(0, 1).unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let before = doc.get_page(0).unwrap().display_list().unwrap();
        let PageObject::Path(neighbor) = &before.objects()[1] else {
            panic!("path")
        };
        let clip_count = neighbor.graphic_state().clip_path.elements().len();
        let revision = doc.revision();
        assert!(doc.remove_page_path_draw(0, &snapshot, 0).is_err());
        assert_eq!(doc.revision(), revision);
        doc.remove_page_path_draw(0, &snapshot, 2).unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let mut page = document.get_page(0).unwrap();
            let program = page.content_program().unwrap();
            assert_eq!(
                program
                    .operations()
                    .iter()
                    .map(|op| op.operator.name())
                    .collect::<Vec<_>>(),
                ["re", "W", "n", "re", "f"]
            );
            let list = page.display_list().unwrap();
            assert_eq!(list.objects().len(), 1);
            let PageObject::Path(path) = &list.objects()[0] else {
                panic!("path")
            };
            assert_eq!(path.graphic_state().clip_path.elements().len(), clip_count);
            assert!(list.diagnostics().is_empty());
            assert!(program.source_bytes().starts_with(b"% keep\n"));
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

#[test]
fn path_sources_track_interleaved_geometry_and_reset_after_discard() {
    use pdf::page::page_object::PageObject;
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    // q/Q do not save the current path. n discards it, unlike a state change.
    doc.replace_page_contents(0, b"0 0 m q 1 0 0 rg 10 0 l Q\n".to_vec())
        .unwrap();
    doc.append_page_contents(0, b"% join\n10 10 l h B 0 0 1 1 re n 2 2 3 3 re s".to_vec())
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let mut page = document.get_page(0).unwrap();
        let program = page.content_program().unwrap();
        let list = page.display_list().unwrap();
        let paths: Vec<_> = list
            .objects()
            .iter()
            .filter_map(|object| {
                if let PageObject::Path(path) = object {
                    Some(path)
                } else {
                    None
                }
            })
            .collect();
        assert_eq!(paths.len(), 2);
        for (path, geometry, painting) in
            [(paths[0], vec![0, 3, 5, 6], 7), (paths[1], vec![10], 11)]
        {
            let source = path.source().unwrap();
            assert_eq!(source.geometry_operation_indices, geometry);
            assert_eq!(source.painting_operation_index, painting);
            assert_eq!(
                source.painting_byte_offset,
                program.operations()[painting].operator.byte_offset()
            );
        }
    }
    let mut page = doc.get_page(0).unwrap();
    let snapshot = page.content_program().unwrap();
    let list = page.display_list().unwrap();
    let PageObject::Path(path) = &list.objects()[1] else {
        panic!("expected path")
    };
    doc.replace_page_path_numbers(
        0,
        &snapshot,
        path.source().unwrap().geometry_operation_indices[0],
        &[5., 6., 7., 8.],
    )
    .unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .operations()[10]
            .operator
            .get_as_f32(0)
            .unwrap(),
        5.
    );
}

#[test]
fn all_supported_path_geometry_operations_retain_operator_and_paint() {
    for (operator, count) in [("m", 2), ("l", 2), ("c", 6), ("v", 4), ("y", 4), ("re", 4)] {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let numbers = (1..=count)
            .map(|value| value.to_string())
            .collect::<Vec<_>>()
            .join(" ");
        doc.replace_page_contents(0, format!("0 0 m {numbers} {operator} S\n").into_bytes())
            .unwrap();
        let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
        let values: Vec<_> = (0..count).map(|index| 10.0 + index as f32).collect();
        doc.replace_page_path_numbers(0, &snapshot, 1, &values)
            .unwrap();
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let program = reopened.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(program.operations().len(), 3);
        assert_eq!(program.operations()[1].operator.name(), operator);
        assert_eq!(program.operations()[2].operator.name(), "S");
        for (index, value) in values.into_iter().enumerate() {
            assert_eq!(
                program.operations()[1].operator.get_as_f32(index).unwrap(),
                value
            );
        }
        assert!(
            reopened
                .get_page(0)
                .unwrap()
                .display_list()
                .unwrap()
                .diagnostics()
                .is_empty()
        );
    }
}

#[test]
fn path_geometry_edit_preserves_other_operations_and_shared_page() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"% before\nq 1 2 3 4 re f Q\n% after\n".to_vec())
        .unwrap();
    doc.copy_page(0, 1).unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let revision = doc.revision();
    for (operation, values) in [
        (0, vec![1.0, 2.0]),
        (1, vec![1.0]),
        (1, vec![f32::NAN, 2.0, 3.0, 4.0]),
        (1, vec![1.0, 2.0, f32::INFINITY, 4.0]),
    ] {
        assert!(
            doc.replace_page_path_numbers(0, &snapshot, operation, &values)
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
    }
    assert!(
        doc.replace_page_path_numbers(1, &snapshot, 1, &[5.0, 6.0, 7.0, 8.0])
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    doc.replace_page_path_numbers(0, &snapshot, 1, &[5.0, 6.0, -7.0, 8.0])
        .unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [&doc, &reopened] {
        let edited = document.get_page(0).unwrap().content_program().unwrap();
        assert_eq!(edited.operations().len(), 4);
        for (index, expected) in [5.0, 6.0, -7.0, 8.0].into_iter().enumerate() {
            assert_eq!(
                edited.operations()[1].operator.get_as_f32(index).unwrap(),
                expected
            );
        }
        assert!(edited.source_bytes().starts_with(b"% before\nq "));
        assert!(edited.source_bytes().ends_with(b" f Q\n% after\n"));
        assert_eq!(
            document
                .get_page(1)
                .unwrap()
                .content_program()
                .unwrap()
                .source_bytes(),
            snapshot.source_bytes()
        );
        assert!(
            document
                .get_page(0)
                .unwrap()
                .display_list()
                .unwrap()
                .diagnostics()
                .is_empty()
        );
    }
}
