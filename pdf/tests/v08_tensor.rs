use pdf::{
    color::shading::ShadingGeometry,
    document::{
        PdfDocument,
        resource_manager::ResourceKind,
        save_options::{SaveOptions, StreamCompression},
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::{page_builder::PageBuilder, page_object::PageObject},
    parser::syntax::{ParseMode, SyntaxParser},
};
use std::io::Cursor;
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn definition(function: bool, moved: bool) -> PdfObject {
    let mut points = [
        [0u8, 0],
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
        [85, 85],
        [85, 170],
        [170, 170],
        [170, 85],
    ];
    if moved {
        points[12][0] += 64;
    }
    let mut data = vec![0];
    for point in points {
        data.extend(point);
    }
    data.extend([0, 0, 255, 255]);
    let f = if function {
        "/Function << /FunctionType 2 /Domain [0 1] /N 2 >>"
    } else {
        ""
    };
    let dict=parse(&format!("<< /ShadingType 7 /ColorSpace /DeviceGray /BitsPerFlag 8 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 255 0 255 0 1] {f} >>")).into_dict().unwrap();
    PdfObject::PdfStream(PdfStream::from_decoded_data(dict, data).unwrap())
}
#[test]
fn tensor_shading_preserves_interior_and_parameter_before_function() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    for moved in [false, true] {
        for function in [false, true] {
            let id = doc.try_insert_object(definition(function, moved)).unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Shading, id)
                .unwrap();
            doc.replace_page_contents(0, format!("/{name} sh").into_bytes())
                .unwrap();
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            let PageObject::Shading(object) = &list.objects()[0] else {
                panic!()
            };
            let shading = &object.shading;
            assert!(matches!(shading.geometry(), ShadingGeometry::TensorMesh));
            let point = if moved {
                [136.5, 127.5]
            } else {
                [127.5, 127.5]
            };
            assert_eq!(
                shading
                    .color_at_point(point[0], point[1])
                    .unwrap()
                    .unwrap()
                    .r(),
                if function { 64 } else { 128 }
            );
            assert!(shading.color_at_point(-1., 128.).unwrap().is_none());
        }
    }
}
#[test]
fn tensor_registration_import_and_both_stream_strategies_strictly_reopen() {
    for function in [false, true] {
        let source = PdfDocument::new().unwrap();
        source
            .add_page_with_builder(0, PageBuilder::new().size(40., 40.))
            .unwrap();
        let id = source
            .try_insert_object(definition(function, true))
            .unwrap();
        let name = source
            .resources()
            .register_existing(0, ResourceKind::Shading, id)
            .unwrap();
        source
            .replace_page_contents(0, format!("0.1 0 0 0.1 2 2 cm /{name} sh").into_bytes())
            .unwrap();
        let target = PdfDocument::new().unwrap();
        target
            .add_page_with_builder(0, PageBuilder::new().size(40., 40.))
            .unwrap();
        let handle = target
            .import_resource_from(
                &source,
                &source.object_handle(id).unwrap(),
                ResourceKind::Shading,
            )
            .unwrap();
        let name = target
            .resources()
            .register_handle(0, ResourceKind::Shading, &handle)
            .unwrap();
        target
            .replace_page_contents(0, format!("0.1 0 0 0.1 2 2 cm /{name} sh").into_bytes())
            .unwrap();
        for doc in [&source, &target] {
            for compression in [
                StreamCompression::Preserve,
                StreamCompression::FlateUnencoded,
            ] {
                let bytes = doc
                    .save_to_bytes_with_options(SaveOptions {
                        compression,
                        ..SaveOptions::default()
                    })
                    .unwrap();
                let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
                let list = reopened.get_page(0).unwrap().display_list().unwrap();
                let PageObject::Shading(shading) = &list.objects()[0] else {
                    panic!()
                };
                assert_eq!(
                    shading
                        .shading
                        .color_at_point(136.5, 127.5)
                        .unwrap()
                        .unwrap()
                        .r(),
                    if function { 64 } else { 128 }
                );
            }
        }
    }
}
#[test]
fn truncated_tensor_registration_leaves_revision_and_page_unchanged() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let mut object = definition(false, false);
    let stream = object.as_stream().unwrap();
    let dict = stream.dict().clone();
    let mut data = stream.raw_data().to_vec();
    data.pop();
    object = PdfObject::PdfStream(PdfStream::from_decoded_data(dict, data).unwrap());
    let id = doc.try_insert_object(object).unwrap();
    let revision = doc.revision();
    let saved = doc.save_to_bytes().unwrap();
    assert!(matches!(
        doc.resources()
            .register_existing(0, ResourceKind::Shading, id),
        Err(PdfError::ColorError(_))
    ));
    assert_eq!(doc.revision(), revision);
    assert_eq!(doc.save_to_bytes().unwrap(), saved);
    doc.update_object(id, definition(false, false)).unwrap();
    assert!(
        doc.resources()
            .register_existing(0, ResourceKind::Shading, id)
            .is_ok()
    );
}
