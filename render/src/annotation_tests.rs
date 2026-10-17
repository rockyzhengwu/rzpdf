use super::SkiaRender;
use pdf::{
    document::{
        PdfDocument,
        annotations::{Annotation, AnnotationKind},
        destination::DestinationView,
    },
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference, pdf_stream::PdfStream},
    page::{annotations::AnnotationRenderOptions, page_builder::PageBuilder},
    parser::syntax::SyntaxParser,
};
use std::io::Cursor;
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn pixels(renderer: &mut SkiaRender, w: usize, h: usize) -> Vec<u8> {
    let info = skia_safe::ImageInfo::new(
        (w as i32, h as i32),
        skia_safe::ColorType::RGBA8888,
        skia_safe::AlphaType::Unpremul,
        None,
    );
    let mut bytes = vec![0; w * h * 4];
    assert!(
        renderer
            .surface
            .read_pixels(&info, &mut bytes, w * 4, (0, 0))
    );
    bytes
}
fn bounds(bytes: &[u8], w: usize, color: [u8; 3]) -> (usize, usize, usize, usize) {
    let pts: Vec<_> = bytes
        .chunks_exact(4)
        .enumerate()
        .filter(|(_, p)| p[..3] == color)
        .map(|(i, _)| (i % w, i / w))
        .collect();
    assert!(!pts.is_empty());
    (
        pts.iter().map(|p| p.0).min().unwrap(),
        pts.iter().map(|p| p.1).min().unwrap(),
        pts.iter().map(|p| p.0).max().unwrap(),
        pts.iter().map(|p| p.1).max().unwrap(),
    )
}

#[test]
fn annotation_no_rotate_keeps_spec_anchor_with_nonzero_bbox_and_ap_matrix() {
    for (rotation, (left, top)) in [
        (0, (105, 55)),
        (90, (65, 105)),
        (180, (95, 65)),
        (270, (55, 95)),
    ] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(240., 160.))
            .unwrap();
        let handle = doc.destination(0, DestinationView::Fit).unwrap().page;
        let mut page = doc.resolve_handle(&handle).unwrap().into_dict().unwrap();
        page.try_insert("CropBox".into(), parse("[10 20 210 140]"))
            .unwrap();
        page.try_insert("Rotate".into(), parse(&rotation.to_string()))
            .unwrap();
        doc.update_handle(&handle, PdfObject::PdfDict(page))
            .unwrap();
        let mut a = Annotation::new(AnnotationKind::Square, [115., 45., 160., 85.]);
        a.flags = 20;
        let annotation = doc.add_annotation(0, &a).unwrap();
        let stream=PdfStream::from_decoded_data(parse("<< /Type /XObject /Subtype /Form /BBox [-10 -20 30 40] /Matrix [0 1 -1 0 4 5] /Resources << >> >>").into_dict().unwrap(),b"0 0 1 rg -10 -20 20 60 re f 1 0 0 rg 10 -20 20 60 re f".to_vec()).unwrap();
        let id = doc.try_insert_object(PdfObject::PdfStream(stream)).unwrap();
        let mut dict = doc
            .resolve_handle(&annotation)
            .unwrap()
            .into_dict()
            .unwrap();
        let mut ap = PdfDict::default();
        ap.try_insert(
            "N".into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        )
        .unwrap();
        dict.try_insert("AP".into(), PdfObject::PdfDict(ap))
            .unwrap();
        doc.update_handle(&annotation, PdfObject::PdfDict(dict))
            .unwrap();
        let mut page = doc.get_page(0).unwrap();
        let geometry = page.geometry().unwrap();
        let mut renderer = SkiaRender::new(72., 72.).unwrap();
        page.display_with_annotations(&mut renderer, Default::default())
            .unwrap();
        let width = geometry.display_width as usize;
        let bytes = pixels(&mut renderer, width, geometry.display_height as usize);
        assert_eq!(
            bounds(&bytes, width, [255, 0, 0]),
            (left, top, left + 44, top + 19),
            "rotation {rotation}"
        );
        assert_eq!(
            bounds(&bytes, width, [0, 0, 255]),
            (left, top + 20, left + 44, top + 39),
            "rotation {rotation}"
        );
    }
}

#[test]
fn annotation_no_zoom_compensates_view_magnification_not_device_density() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().size(100., 100.))
        .unwrap();
    let mut a = Annotation::new(AnnotationKind::Square, [10., 40., 30., 60.]);
    a.color = [1., 0., 0.];
    a.interior_color = Some([1., 0., 0.]);
    a.flags = 12;
    let handle = doc.add_annotation(0, &a).unwrap();
    let stream = PdfStream::from_decoded_data(
        parse("<< /Type /XObject /Subtype /Form /BBox [0 0 20 20] /Resources << >> >>")
            .into_dict()
            .unwrap(),
        b"1 0 0 rg 0 0 20 20 re f".to_vec(),
    )
    .unwrap();
    let id = doc.try_insert_object(PdfObject::PdfStream(stream)).unwrap();
    let mut dict = doc.resolve_handle(&handle).unwrap().into_dict().unwrap();
    let mut ap = PdfDict::default();
    ap.try_insert(
        "N".into(),
        PdfObject::PdfReference(PdfReference::from_id(id)),
    )
    .unwrap();
    dict.try_insert("AP".into(), PdfObject::PdfDict(ap))
        .unwrap();
    doc.update_handle(&handle, PdfObject::PdfDict(dict))
        .unwrap();
    for zoom in [1., 2.] {
        let mut renderer = SkiaRender::new(72. * zoom, 72. * zoom).unwrap();
        doc.get_page(0)
            .unwrap()
            .display_with_annotations(
                &mut renderer,
                AnnotationRenderOptions {
                    view_zoom: zoom,
                    ..Default::default()
                },
            )
            .unwrap();
        let size = (100. * zoom) as usize;
        let bytes = pixels(&mut renderer, size, size);
        let (x, y, r, b) = bounds(&bytes, size, [255, 0, 0]);
        assert_eq!((r - x + 1, b - y + 1), (20, 20));
        assert_eq!((x, y), ((10. * zoom) as usize, (40. * zoom) as usize));
    }
}
