use pdf::{
    document::{
        PdfDocument,
        resource_manager::ResourceKind,
        save_options::{SaveOptions, StreamCompression},
    },
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::{page_builder::PageBuilder, page_object::PageObject},
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;

fn parse(source: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}

#[test]
fn truncated_coons_registration_is_atomic_and_repairable() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let dict = parse("<< /ShadingType 6 /ColorSpace /DeviceGray /BitsPerFlag 8 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 3 0 3 0 1] >>").into_dict().unwrap();
    // Complete flag-0 record: flag + twelve coordinate pairs + four colors.
    let mut valid = vec![0; 29];
    valid[25..].fill(255);
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            dict.clone(),
            valid[..28].to_vec(),
        )))
        .unwrap();
    let revision = doc.revision();
    let before = doc.save_to_bytes().unwrap();
    assert!(
        doc.resources()
            .register_existing(0, ResourceKind::Shading, id)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    doc.update_object(id, PdfObject::PdfStream(PdfStream::new(dict, valid)))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Shading, id)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} sh").into_bytes())
        .unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    assert!(list.diagnostics().is_empty());
    let [PageObject::Shading(object)] = list.objects() else {
        panic!("expected repaired Coons shading")
    };
    assert_eq!(
        object
            .shading
            .color_at_point(0.0, 0.0)
            .unwrap()
            .unwrap()
            .r(),
        255
    );
}

#[test]
fn incomplete_lattice_registration_is_atomic_and_repairable() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let dict = parse("<< /ShadingType 5 /ColorSpace /DeviceGray /VerticesPerRow 2 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 40 0 20 0 1] >>").into_dict().unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            dict.clone(),
            vec![0, 0, 0, 255, 0, 255, 0, 255, 0],
        )))
        .unwrap();
    let revision = doc.revision();
    let before = doc.save_to_bytes().unwrap();
    assert!(
        doc.resources()
            .register_existing(0, ResourceKind::Shading, id)
            .is_err()
    );
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    doc.update_object(
        id,
        PdfObject::PdfStream(PdfStream::new(
            dict,
            vec![0, 0, 0, 255, 0, 255, 0, 255, 0, 255, 255, 255],
        )),
    )
    .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Shading, id)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} sh").into_bytes())
        .unwrap();
    assert!(
        doc.get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .diagnostics()
            .is_empty()
    );
}

#[test]
fn rgb_mesh_three_component_records_survive_save_and_import() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 20.0))
        .unwrap();
    let dict = parse("<< /ShadingType 4 /ColorSpace /DeviceRGB /BitsPerFlag 8 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 40 0 20 0 1 0 1 0 1] >>").into_dict().unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            dict,
            vec![
                0, 0, 0, 255, 0, 0, 0, 255, 0, 0, 255, 0, 0, 0, 255, 0, 0, 255,
            ],
        )))
        .unwrap();
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Shading, id)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} sh").into_bytes())
        .unwrap();
    let verify = |target: &PdfDocument<Cursor<Vec<u8>>>| {
        let list = target.get_page(0).unwrap().display_list().unwrap();
        assert!(list.diagnostics().is_empty());
        let [PageObject::Shading(object)] = list.objects() else {
            panic!("expected RGB mesh")
        };
        for (point, expected) in [
            ([0.0, 0.0], [255, 0, 0]),
            ([40.0, 0.0], [0, 255, 0]),
            ([0.0, 20.0], [0, 0, 255]),
            ([10.0, 5.0], [128, 64, 64]),
        ] {
            let color = object
                .shading
                .color_at_point(point[0], point[1])
                .unwrap()
                .unwrap();
            assert_eq!([color.r(), color.g(), color.b()], expected);
        }
        assert!(object.shading.color_at_point(30.0, 15.0).unwrap().is_none());
    };
    verify(&doc);
    let imported = PdfDocument::new().unwrap();
    imported.import_page_from(&doc, 0, 0).unwrap();
    verify(&imported);
    for target in [&doc, &imported] {
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let bytes = target
                .save_to_bytes_with_options(SaveOptions {
                    compression,
                    ..Default::default()
                })
                .unwrap();
            verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
        }
    }
}

#[test]
fn invalid_mesh_registration_preserves_revision_and_saved_bytes() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let source = "<< /ShadingType 4 /ColorSpace /DeviceGray /BitsPerFlag 8 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 40 0 20 0 1] >>";
    for data in [vec![0, 0, 0], vec![1, 0, 0, 0], vec![0, 0, 0, 0]] {
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                parse(source).into_dict().unwrap(),
                data,
            )))
            .unwrap();
        let revision = doc.revision();
        let before = doc.save_to_bytes().unwrap();
        assert!(
            doc.resources()
                .register_existing(0, ResourceKind::Shading, id)
                .is_err()
        );
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        // Repairing the same retained object permits registration and painting.
        doc.update_object(
            id,
            PdfObject::PdfStream(PdfStream::new(
                parse(source).into_dict().unwrap(),
                vec![0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 0],
            )),
        )
        .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Shading, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} sh").into_bytes())
            .unwrap();
        assert!(
            doc.get_page(0)
                .unwrap()
                .display_list()
                .unwrap()
                .diagnostics()
                .is_empty()
        );
    }
}

#[test]
fn type4_type5_and_type6_resources_import_and_both_stream_strategies() {
    for (nonlinear, kind) in [
        (false, 4),
        (true, 4),
        (false, 5),
        (true, 5),
        (false, 6),
        (true, 6),
    ] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 20.0))
            .unwrap();
        let function = if nonlinear {
            "/Function << /FunctionType 2 /Domain [0 1] /N 2 >>"
        } else {
            ""
        };
        let geometry = match kind {
            5 => "/ShadingType 5 /VerticesPerRow 2",
            6 => "/ShadingType 6 /BitsPerFlag 8",
            _ => "/ShadingType 4 /BitsPerFlag 8",
        };
        let data = if kind == 6 {
            let mut bytes = vec![0];
            for point in [
                [0, 0],
                [0, 85],
                [0, 170],
                [0, 255],
                [85, 255],
                [170, 255],
                [255, 255],
                [255, 170],
                [255, 85],
                [255, 0],
                [170, 0],
                [85, 0],
            ] {
                bytes.extend(point);
            }
            bytes.extend([0, 0, 255, 255]);
            bytes
        } else if kind == 5 {
            vec![0, 0, 0, 255, 0, 255, 0, 255, 0, 255, 255, 255]
        } else {
            vec![0, 0, 0, 0, 0, 255, 0, 255, 0, 0, 255, 0, 1, 255, 255, 255]
        };
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(
            parse(&format!("<< {geometry} /ColorSpace /DeviceGray /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 40 0 20 0 1] {function} >>")).into_dict().unwrap(),
            data,
        ))).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Shading, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} sh").into_bytes())
            .unwrap();
        let verify = |target: &PdfDocument<Cursor<Vec<u8>>>| {
            let list = target.get_page(0).unwrap().display_list().unwrap();
            assert!(list.diagnostics().is_empty());
            let [PageObject::Shading(object)] = list.objects() else {
                panic!("expected mesh shading")
            };
            assert!(object.source().is_some());
            for (point, direct, squared) in [
                ([10.0, 5.0], 64, 16),
                ([30.0, 15.0], 191, 143),
                ([20.0, 10.0], 128, 64),
            ] {
                let color = object
                    .shading
                    .color_at_point(point[0], point[1])
                    .unwrap()
                    .unwrap();
                let expected = if nonlinear { squared } else { direct };
                assert_eq!([color.r(), color.g(), color.b()], [expected; 3]);
            }
            assert!(object.shading.color_at_point(41.0, 10.0).unwrap().is_none());
            if kind == 6 {
                // Verify the certified-corner path in original/imported
                // resources and after both strict save/reopen strategies.
                for (point, expected) in [([0.0, 0.0], 0), ([0.0, 20.0], 0),
                    ([40.0, 0.0], 255), ([40.0, 20.0], 255)] {
                    let color = object.shading.color_at_point(point[0], point[1])
                        .unwrap().unwrap();
                    assert_eq!([color.r(), color.g(), color.b()], [expected; 3]);
                }
                for (point, direct, squared) in [([0.0, 10.0], 0, 0),
                    ([40.0, 10.0], 255, 255), ([10.0, 0.0], 64, 16),
                    ([30.0, 20.0], 191, 143)] {
                    let color = object.shading.color_at_point(point[0], point[1])
                        .unwrap().unwrap();
                    let expected = if nonlinear { squared } else { direct };
                    assert_eq!([color.r(), color.g(), color.b()], [expected; 3]);
                }
            }
        };
        verify(&doc);
        let imported = PdfDocument::new().unwrap();
        imported.import_page_from(&doc, 0, 0).unwrap();
        verify(&imported);
        for target in [&doc, &imported] {
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let bytes = target
                    .save_to_bytes_with_options(SaveOptions {
                        compression,
                        ..Default::default()
                    })
                    .unwrap();
                verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
            }
        }
    }
}

#[test]
fn type1_shading_functions_resources_and_import_survive_strict_save() {
    for sampled in [false, true] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(40.0, 20.0))
            .unwrap();
        let (dict, data) = if sampled {
            (
                "<< /FunctionType 0 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] /Size [2 2] /BitsPerSample 8 >>",
                vec![0, 0, 0, 255, 0, 128, 0, 255, 128, 255, 255, 255],
            )
        } else {
            (
                "<< /FunctionType 4 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] >>",
                b"{ 2 copy add 2 div }".to_vec(),
            )
        };
        let function = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                parse(dict).into_dict().unwrap(),
                data,
            )))
            .unwrap();
        let id = doc.try_insert_object(parse(&format!(
            "<< /ShadingType 1 /ColorSpace /DeviceRGB /Matrix [40 0 0 20 0 0] /Function {} {} R >>",
            function.obj_num(), function.gen_num()
        ))).unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Shading, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} sh").into_bytes())
            .unwrap();
        let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            assert!(list.diagnostics().is_empty());
            let [PageObject::Shading(object)] = list.objects() else {
                panic!()
            };
            assert!(object.source().is_some());
            let color = object.shading.color_at_point(10.0, 15.0).unwrap().unwrap();
            assert_eq!([color.r(), color.g(), color.b()], [64, 191, 128]);
            assert!(object.shading.color_at_point(41.0, 10.0).unwrap().is_none());
        };
        verify(&doc);
        let imported = PdfDocument::new().unwrap();
        imported.import_page_from(&doc, 0, 0).unwrap();
        verify(&imported);
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            for source in [&doc, &imported] {
                let bytes = source
                    .save_to_bytes_with_options(SaveOptions {
                        compression,
                        ..Default::default()
                    })
                    .unwrap();
                verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
            }
        }
    }
}

#[test]
fn form_shading_scope_sources_and_state_survive_both_save_strategies() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(200.0, 120.0))
        .unwrap();
    let id = doc.try_insert_object(parse(
        "<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 80 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [1] /C1 [1] /N 1 >> >>"
    )).unwrap();
    let shading_name = doc
        .resources()
        .register_existing(0, ResourceKind::Shading, id)
        .unwrap();
    let dict = parse(&format!(
        "<< /Type /XObject /Subtype /Form /BBox [0 0 80 80] /Resources << /Shading << /{shading_name} << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 80 0] /Function << /FunctionType 2 /Domain [0 1] /N 2 >> >> >> >> >>"
    )).into_dict().unwrap();
    let form_id = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, format!("/{shading_name} sh").into_bytes()).unwrap(),
        ))
        .unwrap();
    let form_name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, form_id)
        .unwrap();
    doc.replace_page_contents(0, format!(
        "/{shading_name} sh q 1 0 0 1 10 20 cm /{form_name} Do Q q 1 0 0 1 110 20 cm /{form_name} Do Q /{shading_name} sh"
    ).into_bytes()).unwrap();
    let verify = |doc: &PdfDocument<Cursor<Vec<u8>>>| {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        assert!(list.diagnostics().is_empty());
        assert_eq!(list.objects().len(), 4);
        let objects: Vec<_> = list
            .objects()
            .iter()
            .map(|object| {
                let PageObject::Shading(shading) = object else {
                    panic!("expected shading")
                };
                shading
            })
            .collect();
        for index in [0, 3] {
            assert_eq!(objects[index].shading.color_at(0.5).unwrap().r(), 255);
            assert!(objects[index].source().is_some());
            assert!(objects[index].nested_source().is_none());
        }
        for (index, x) in [(1, 10.0), (2, 110.0)] {
            let object = objects[index];
            assert_eq!(object.shading.color_at(0.5).unwrap().r(), 64);
            assert_eq!(object.matrix.e(), x);
            assert!(object.source().is_none());
            let source = object.nested_source().unwrap();
            assert_eq!(source.invocations.len(), 1);
            assert_eq!(source.invocations[0].form_id, Some(form_id));
            assert_eq!(source.operation_index, 0);
            assert!(!object.graphic_state.clip_path.elements.is_empty());
        }
        assert_ne!(
            objects[1].nested_source().unwrap().invocations[0].operation_index,
            objects[2].nested_source().unwrap().invocations[0].operation_index
        );
    };
    verify(&doc);
    let imported = PdfDocument::new().unwrap();
    imported.import_page_from(&doc, 0, 0).unwrap();
    let verify_import = |target: &PdfDocument<Cursor<Vec<u8>>>| {
        let list = target.get_page(0).unwrap().display_list().unwrap();
        assert!(list.diagnostics().is_empty());
        assert_eq!(list.objects().len(), 4);
        for (index, expected) in [(0, 255), (1, 64), (2, 64), (3, 255)] {
            let PageObject::Shading(object) = &list.objects()[index] else {
                panic!()
            };
            assert_eq!(object.shading.color_at(0.5).unwrap().r(), expected);
            assert_eq!(object.nested_source().is_some(), index == 1 || index == 2);
        }
    };
    verify_import(&imported);
    for compression in [
        StreamCompression::Preserve,
        StreamCompression::FlateUnencoded,
    ] {
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                compression,
                ..Default::default()
            })
            .unwrap();
        verify(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
        let bytes = imported
            .save_to_bytes_with_options(SaveOptions {
                compression,
                ..Default::default()
            })
            .unwrap();
        verify_import(&PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap());
    }
}
