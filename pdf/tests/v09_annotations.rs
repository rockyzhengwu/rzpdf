use pdf::{
    device::trace::TraceDevice,
    document::{
        PdfDocument,
        annotations::{Annotation, AnnotationKind as Kind, AnnotationRemoval, TextAlignment},
        destination::DestinationView,
    },
    io::stream_reader::StreamReader,
    objects::PdfObject,
    page::annotations::{AnnotationRenderIntent, AnnotationRenderOptions},
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;

fn parse(text: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn annotation(kind: Kind) -> Annotation {
    Annotation::new(kind, [10., 20., 150., 100.])
}

#[test]
fn every_markup_kind_crud_roundtrips_and_preserves_contents_and_unknown_keys() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
    doc.add_attachment("sample.txt", b"attachment".to_vec())
        .unwrap();
    let fs = doc.attachment_filespec("sample.txt").unwrap().unwrap();
    let quad = vec![[20., 80., 130., 80., 20., 60., 130., 60.]];
    let kinds = vec![
        Kind::Square,
        Kind::Circle,
        Kind::Text {
            icon: "Note".into(),
            open: false,
        },
        Kind::Popup { open: false },
        Kind::FreeText {
            font_size: 12.,
            alignment: TextAlignment::Center,
        },
        Kind::Line {
            points: [20., 30., 130., 90.],
        },
        Kind::Polygon {
            vertices: vec![[20., 30.], [130., 30.], [70., 90.]],
        },
        Kind::PolyLine {
            vertices: vec![[20., 30.], [70., 90.], [130., 30.]],
        },
        Kind::Highlight {
            quads: quad.clone(),
        },
        Kind::Underline {
            quads: quad.clone(),
        },
        Kind::Squiggly {
            quads: quad.clone(),
        },
        Kind::StrikeOut { quads: quad },
        Kind::Ink {
            strokes: vec![
                vec![[20., 30.], [70., 90.], [130., 30.]],
                vec![[20., 90.], [130., 40.]],
            ],
        },
        Kind::Stamp {
            name: "Approved".into(),
        },
        Kind::FileAttachment {
            file: fs,
            icon: "PushPin".into(),
        },
    ];
    let mut expected = Vec::new();
    for kind in kinds {
        let mut a = annotation(kind);
        a.contents = "first".into();
        a.opacity = 0.7;
        let handle = doc.add_annotation(0, &a).unwrap();
        let mut dict = doc.resolve_handle(&handle).unwrap().into_dict().unwrap();
        dict.try_insert("Custom".into(), parse("(preserve)"))
            .unwrap();
        if let Some(ap) = dict.get("AP") {
            let mut ap = ap.as_dict().unwrap().clone();
            ap.try_insert("D".into(), parse("null")).unwrap();
            dict.try_insert("AP".into(), PdfObject::PdfDict(ap))
                .unwrap();
        }
        doc.update_handle(&handle, PdfObject::PdfDict(dict))
            .unwrap();
        a.contents = if matches!(a.kind, Kind::FreeText { .. }) {
            "edited\ntext".into()
        } else {
            "批注内容".into()
        };
        doc.update_annotation(0, &handle, &a).unwrap();
        let dict = doc.resolve_handle(&handle).unwrap().into_dict().unwrap();
        assert!(dict.get("Custom").is_some());
        if let Some(ap) = dict.get("AP") {
            assert!(ap.as_dict().unwrap().get("D").is_some());
        }
        expected.push(a);
    }
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .content_program()
            .unwrap()
            .source_bytes(),
        b"q Q"
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let actual = reopened.page_annotations(0).unwrap();
    assert_eq!(actual.len(), 15);
    for (record, expected) in actual.iter().zip(expected) {
        let value = record.annotation.as_ref().unwrap();
        assert_eq!(value.kind.subtype(), expected.kind.subtype());
        assert_eq!(value.contents, expected.contents);
        assert_eq!(value.opacity, 0.7);
        if record.subtype == "FileAttachment" {
            assert_eq!(
                reopened
                    .annotation_file_data(record.handle.as_ref().unwrap())
                    .unwrap(),
                b"attachment"
            );
        }
    }
    assert_eq!(
        reopened
            .get_page(0)
            .unwrap()
            .display_list_with_annotations(Default::default())
            .unwrap()
            .appearance_count(),
        14
    );
    for record in actual {
        reopened
            .remove_annotation(0, &record.handle.unwrap(), AnnotationRemoval::RejectRelated)
            .unwrap();
    }
    assert!(reopened.page_annotations(0).unwrap().is_empty());
    assert_eq!(
        reopened.attachment_data("sample.txt").unwrap(),
        Some(b"attachment".to_vec())
    );
}

#[test]
fn popup_reply_cycles_cascade_and_locked_descendants_are_atomic() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let note = annotation(Kind::Text {
        icon: "Note".into(),
        open: false,
    });
    let parent = doc.add_annotation(0, &note).unwrap();
    let reply = doc.add_annotation(0, &note).unwrap();
    let nested = doc.add_annotation(0, &note).unwrap();
    let popup = doc
        .add_annotation(0, &annotation(Kind::Popup { open: true }))
        .unwrap();
    doc.set_annotation_popup(&parent, Some(&popup)).unwrap();
    doc.set_annotation_reply(&reply, Some(&parent)).unwrap();
    doc.set_annotation_reply(&nested, Some(&reply)).unwrap();
    let revision = doc.commit_revision();
    assert!(doc.set_annotation_reply(&parent, Some(&nested)).is_err());
    assert_eq!(doc.commit_revision(), revision);
    assert!(
        doc.remove_annotation(0, &parent, AnnotationRemoval::RejectRelated)
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    let mut dict = doc.resolve_handle(&nested).unwrap().into_dict().unwrap();
    dict.try_insert("F".into(), parse("128")).unwrap();
    doc.update_handle(&nested, PdfObject::PdfDict(dict.clone()))
        .unwrap();
    let revision = doc.commit_revision();
    assert!(
        doc.remove_annotation(0, &parent, AnnotationRemoval::Cascade)
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    assert_eq!(doc.page_annotations(0).unwrap().len(), 4);
    dict.try_insert("F".into(), parse("4")).unwrap();
    doc.update_handle(&nested, PdfObject::PdfDict(dict))
        .unwrap();
    let other = doc.add_annotation(1, &note).unwrap();
    let revision = doc.commit_revision();
    assert!(doc.set_annotation_reply(&other, Some(&parent)).is_err());
    assert_eq!(doc.commit_revision(), revision);
    doc.remove_annotation(0, &parent, AnnotationRemoval::Cascade)
        .unwrap();
    assert!(doc.page_annotations(0).unwrap().is_empty());
    assert_eq!(doc.page_annotations(1).unwrap().len(), 1);
}

#[test]
fn popup_reassignment_and_removal_keep_reciprocal_links() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let a = doc.add_annotation(0, &annotation(Kind::Square)).unwrap();
    let b = doc.add_annotation(0, &annotation(Kind::Circle)).unwrap();
    let p = doc
        .add_annotation(0, &annotation(Kind::Popup { open: false }))
        .unwrap();
    doc.set_annotation_popup(&a, Some(&p)).unwrap();
    let revision = doc.commit_revision();
    assert!(doc.set_annotation_popup(&b, Some(&p)).is_err());
    assert_eq!(doc.commit_revision(), revision);
    doc.set_annotation_popup(&a, None).unwrap();
    doc.set_annotation_popup(&b, Some(&p)).unwrap();
    doc.remove_annotation(0, &p, AnnotationRemoval::RejectRelated)
        .unwrap();
    assert!(
        doc.page_annotations(0)
            .unwrap()
            .iter()
            .all(|r| r.popup.is_none())
    );
}

#[test]
fn invalid_geometry_unicode_layout_and_appearance_limits_roll_back() {
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_raw_stream_bytes: 256,
            max_path_segments: 30,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    let revision = doc.commit_revision();
    for kind in [
        Kind::Line {
            points: [0., 0., 10., 10.],
        },
        Kind::Polygon {
            vertices: vec![[20., 30.]],
        },
        Kind::Ink { strokes: vec![] },
        Kind::Squiggly {
            quads: vec![[10., 90., 150., 90., 10., 30., 150., 30.]],
        },
    ] {
        assert!(doc.add_annotation(0, &annotation(kind)).is_err());
        assert_eq!(doc.commit_revision(), revision);
    }
    let mut a = annotation(Kind::FreeText {
        font_size: 12.,
        alignment: TextAlignment::Left,
    });
    a.contents = "中文".into();
    assert!(doc.add_annotation(0, &a).is_err());
    assert_eq!(doc.commit_revision(), revision);
    a.contents = "x".repeat(200);
    assert!(doc.add_annotation(0, &a).is_err());
    assert_eq!(doc.commit_revision(), revision);
    doc.add_annotation(0, &annotation(Kind::Square)).unwrap();
}

#[test]
fn contents_and_annotation_provenance_and_visibility_are_separate() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"0 0 10 10 re f".to_vec())
        .unwrap();
    for flags in [0, 2, 4, 32, 36] {
        let mut a = annotation(Kind::Square);
        a.flags = flags;
        doc.add_annotation(0, &a).unwrap();
    }
    doc.add_checkbox_field(0, [160., 20., 190., 50.], "check", true)
        .unwrap();
    let revision = doc.commit_revision();
    let mut page = doc.get_page(0).unwrap();
    let original = page.display_list().unwrap();
    let program = page.content_program().unwrap();
    let screen = page
        .display_list_with_annotations(Default::default())
        .unwrap();
    assert_eq!(screen.appearance_count(), 3);
    assert!(screen.contents().matches_program(&program));
    let print = page
        .display_list_with_annotations(AnnotationRenderOptions {
            intent: AnnotationRenderIntent::Print,
            include_widgets: false,
            view_zoom: 1.,
        })
        .unwrap();
    assert_eq!(print.appearance_count(), 2);
    assert_eq!(original.objects().len(), screen.contents().objects().len());
    assert_eq!(doc.commit_revision(), revision);
    assert_eq!(
        page.content_program().unwrap().source_bytes(),
        b"0 0 10 10 re f"
    );
    let mut trace = TraceDevice::new();
    screen.replay(&mut trace).unwrap();
    assert!(trace.objects().len() > original.objects().len());
    for object in &trace.objects()[original.objects().len()..] {
        use pdf::page::page_object::PageObject;
        match object {
            PageObject::Path(v) => {
                assert!(v.source().is_none());
                assert!(v.nested_source().is_none());
            }
            PageObject::Text(v) => {
                assert!(v.source().is_none());
                assert!(v.nested_source().is_none());
            }
            PageObject::Image(v) => {
                assert!(v.source().is_none());
                assert!(v.nested_source().is_none());
            }
            PageObject::Shading(v) => {
                assert!(v.source().is_none());
                assert!(v.nested_source().is_none());
            }
            _ => {}
        }
    }
}

#[test]
fn malformed_state_matrix_and_visible_missing_appearance_fail_before_device_use() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let handle = doc.add_annotation(0, &annotation(Kind::Square)).unwrap();
    let original = doc.resolve_handle(&handle).unwrap().into_dict().unwrap();
    for ap in ["<< /N << /On null >> >>", "<< >>"] {
        let mut dict = original.clone();
        dict.try_insert("AP".into(), parse(ap)).unwrap();
        doc.update_handle(&handle, PdfObject::PdfDict(dict))
            .unwrap();
        let mut trace = TraceDevice::new();
        assert!(
            doc.get_page(0)
                .unwrap()
                .display_with_annotations(&mut trace, Default::default())
                .is_err()
        );
        assert!(trace.objects().is_empty());
    }
    let mut dict = original;
    dict.remove("AP");
    dict.try_insert("F".into(), parse("2")).unwrap();
    doc.update_handle(&handle, PdfObject::PdfDict(dict))
        .unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_annotations(Default::default())
            .unwrap()
            .appearance_count(),
        0
    );
}

#[test]
fn foreign_handles_wrong_pages_and_unknown_annotations_are_preserved() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let a = annotation(Kind::Square);
    let h = doc.add_annotation(0, &a).unwrap();
    let foreign = PdfDocument::new().unwrap();
    foreign.add_page(0).unwrap();
    let other = foreign.add_annotation(0, &a).unwrap();
    let revision = doc.commit_revision();
    assert!(doc.update_annotation(0, &other, &a).is_err());
    assert!(doc.update_annotation(1, &h, &a).is_err());
    assert_eq!(doc.commit_revision(), revision);
    let mut dict = doc.resolve_handle(&h).unwrap().into_dict().unwrap();
    dict.try_insert("Subtype".into(), parse("/UnknownPrivate"))
        .unwrap();
    doc.update_handle(&h, PdfObject::PdfDict(dict)).unwrap();
    let records = doc.page_annotations(0).unwrap();
    assert!(records[0].annotation.is_none());
    let revision = doc.commit_revision();
    assert!(
        doc.remove_annotation(0, &h, AnnotationRemoval::Cascade)
            .is_err()
    );
    assert_eq!(doc.commit_revision(), revision);
    let page = doc.destination(0, DestinationView::Fit).unwrap().page;
    assert!(doc.resolve_handle(&page).unwrap().as_dict().is_some());
}

#[test]
fn annotation_work_limits_are_shared_across_appearances_and_retry_is_clean() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    for _ in 0..3 {
        doc.add_annotation(0, &annotation(Kind::Square)).unwrap();
    }
    let bytes = doc.save_to_bytes().unwrap();
    let restricted = PdfDocument::from_bytes_with_options(
        bytes,
        ParseOptions {
            mode: ParseMode::Strict,
            limits: Limits {
                max_content_operations: 35,
                ..Default::default()
            },
        },
    )
    .unwrap();
    let revision = restricted.commit_revision();
    let mut trace = TraceDevice::new();
    assert!(
        restricted
            .get_page(0)
            .unwrap()
            .display_with_annotations(&mut trace, Default::default())
            .is_err()
    );
    assert!(trace.objects().is_empty());
    assert_eq!(restricted.commit_revision(), revision);
    let handle = restricted.page_annotations(0).unwrap()[2]
        .handle
        .clone()
        .unwrap();
    restricted
        .remove_annotation(0, &handle, AnnotationRemoval::RejectRelated)
        .unwrap();
    restricted
        .get_page(0)
        .unwrap()
        .display_with_annotations(&mut trace, Default::default())
        .unwrap();
}

#[test]
fn invisible_only_suppresses_unknown_types_and_no_view_does_not_suppress_print() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let mut a = annotation(Kind::Square);
    a.flags = 1;
    let handle = doc.add_annotation(0, &a).unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_annotations(Default::default())
            .unwrap()
            .appearance_count(),
        1
    );
    let mut dict = doc.resolve_handle(&handle).unwrap().into_dict().unwrap();
    dict.try_insert("Subtype".into(), parse("/CustomPrivate"))
        .unwrap();
    doc.update_handle(&handle, PdfObject::PdfDict(dict))
        .unwrap();
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .display_list_with_annotations(Default::default())
            .unwrap()
            .appearance_count(),
        0
    );
}

#[test]
fn all_annotation_mutations_enforce_r6_annotation_permissions() {
    use pdf::{
        document::Document,
        security::{EncryptionOptions, SaveSecurity},
    };
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let a = annotation(Kind::Square);
    doc.add_annotation(0, &a).unwrap();
    doc.add_annotation(0, &annotation(Kind::Popup { open: false }))
        .unwrap();
    let mut encryption = EncryptionOptions::new("user", "owner").unwrap();
    encryption.permissions.annotate = false;
    let bytes = doc
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&encryption))
        .unwrap();
    let user = Document::from_bytes_with_password_and_options(
        bytes,
        "user",
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )
    .unwrap();
    let records = user.page_annotations(0).unwrap();
    let h = records[0].handle.as_ref().unwrap();
    let p = records[1].handle.as_ref().unwrap();
    let revision = user.commit_revision();
    assert!(user.add_annotation(0, &a).is_err());
    assert!(user.update_annotation(0, h, &a).is_err());
    assert!(
        user.remove_annotation(0, h, AnnotationRemoval::Cascade)
            .is_err()
    );
    assert!(user.set_annotation_popup(h, Some(p)).is_err());
    assert!(user.set_annotation_reply(h, None).is_err());
    assert_eq!(user.commit_revision(), revision);
    assert_eq!(
        user.get_page(0)
            .unwrap()
            .display_list_with_annotations(Default::default())
            .unwrap()
            .appearance_count(),
        1
    );
}

#[test]
fn annotation_versions_are_guarded_on_create_update_reply_and_low_level_save() {
    use pdf::document::save_options::PdfVersion;
    let doc = PdfDocument::new_with_version(PdfVersion::V1_4).unwrap();
    doc.add_page(0).unwrap();
    let a = annotation(Kind::Square);
    let h = doc.add_annotation(0, &a).unwrap();
    let other = doc.add_annotation(0, &a).unwrap();
    let revision = doc.commit_revision();
    assert!(
        doc.add_annotation(
            0,
            &annotation(Kind::Polygon {
                vertices: vec![[20., 30.], [130., 30.], [70., 90.]]
            })
        )
        .is_err()
    );
    assert!(doc.set_annotation_reply(&h, Some(&other)).is_err());
    let mut changed = a.clone();
    changed.flags = 256;
    assert!(doc.update_annotation(0, &h, &changed).is_err());
    changed.flags = 512;
    assert!(doc.update_annotation(0, &h, &changed).is_err());
    assert_eq!(doc.commit_revision(), revision);
    let mut dict = doc.resolve_handle(&h).unwrap().into_dict().unwrap();
    dict.try_insert("F".into(), parse("512")).unwrap();
    doc.update_handle(&h, PdfObject::PdfDict(dict.clone()))
        .unwrap();
    let revision = doc.commit_revision();
    assert!(doc.save_to_bytes().is_err());
    assert_eq!(doc.commit_revision(), revision);
    dict.try_insert("F".into(), parse("4")).unwrap();
    doc.update_handle(&h, PdfObject::PdfDict(dict)).unwrap();
    assert!(doc.save_to_bytes().is_ok());
    let doc = PdfDocument::new_with_version(PdfVersion::V1_5).unwrap();
    doc.add_page(0).unwrap();
    let first = doc.add_annotation(0, &a).unwrap();
    let second = doc.add_annotation(0, &a).unwrap();
    doc.set_annotation_reply(&second, Some(&first)).unwrap();
    let mut dict = doc.resolve_handle(&second).unwrap().into_dict().unwrap();
    assert!(dict.get("IRT").is_some());
    assert!(dict.get("RT").is_none());
    assert!(doc.save_to_bytes().is_ok());
    dict.try_insert("RT".into(), parse("/R")).unwrap();
    doc.update_handle(&second, PdfObject::PdfDict(dict))
        .unwrap();
    assert!(doc.save_to_bytes().is_err());
}
