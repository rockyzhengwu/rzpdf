use pdf::{
    document::PdfDocument,
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
fn group(
    doc: &PdfDocument<Cursor<Vec<u8>>>,
    extra: &str,
    content: &str,
) -> pdf::objects::object_id::ObjectId {
    doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(parse(&format!("<< /Subtype /Form /BBox [0 0 20 20] /Group << /S /Transparency /I true /CS /DeviceRGB {extra} >> /Resources << >> >>")).into_dict().unwrap(),content.as_bytes().to_vec()).unwrap())).unwrap()
}
#[test]
fn malformed_mask_variants_fail_without_mutating_document_and_valid_retry_works() {
    let doc = PdfDocument::new().unwrap();
    let gid = group(&doc, "", "0 0 20 20 re f");
    let g = gid.obj_num();
    for extra in [
        "/S /Bad".to_string(),
        "/G null".into(),
        "/BC [0 0]".into(),
        "/TR /Bad".into(),
        "/TR << /FunctionType 2 /Domain [0 1] /C0 [0 0] /C1 [1 1] /N 1 >>".into(),
    ] {
        let resources = parse(&format!(
            "<< /ExtGState << /M << /SMask << /S /Alpha /G {g} 0 R {extra} >> >> >> >>"
        ))
        .into_dict()
        .unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page_with_builder(
            index,
            PageBuilder::new()
                .size(20., 20.)
                .resources(resources)
                .content(b"/M gs 0 0 20 20 re f".to_vec()),
        )
        .unwrap();
        let revision = doc.revision();
        let bytes = doc.save_to_bytes().unwrap();
        assert!(
            doc.get_page(index).unwrap().display_list().is_err(),
            "{extra}"
        );
        assert_eq!(doc.revision(), revision);
        assert_eq!(doc.save_to_bytes().unwrap(), bytes);
        doc.replace_page_contents(index, b"0 0 20 20 re f".to_vec())
            .unwrap();
        assert!(doc.get_page(index).unwrap().display_list().is_ok());
    }
}
#[test]
fn recursive_mask_is_rejected_and_knockout_mask_keeps_explicit_boundaries() {
    let doc = PdfDocument::new().unwrap();
    let gid = group(&doc, "/K true", "0 0 20 20 re f");
    let resources = parse(&format!(
        "<< /ExtGState << /M << /SMask << /S /Alpha /G {} 0 R >> >> >> >>",
        gid.obj_num()
    ))
    .into_dict()
    .unwrap();
    doc.add_page_with_builder(
        0,
        PageBuilder::new()
            .size(20., 20.)
            .resources(resources.clone())
            .content(b"/M gs 0 0 20 20 re f".to_vec()),
    )
    .unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for doc in [&doc, &reopened] {
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let PageObject::Path(p) = &list.objects()[0] else {
            panic!()
        };
        let mask = p.graphic_state().general_state.soft_mask.as_ref().unwrap();
        assert!(matches!(mask.list.objects()[0],PageObject::GroupBegin(ref g) if g.knockout));
    }
    let resources = format!(
        "<< /ExtGState << /M << /SMask << /S /Alpha /G {} 0 R >> >> >> >>",
        gid.obj_num()
    );
    let dict = parse(&format!("<< /Subtype /Form /BBox [0 0 20 20] /Group << /S /Transparency /I true /CS /DeviceRGB >> /Resources {resources} >>")).into_dict().unwrap();
    doc.update_object(
        gid,
        PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, b"/M gs 0 0 20 20 re f".to_vec()).unwrap(),
        ),
    )
    .unwrap();
    assert!(doc.get_page(0).unwrap().display_list().is_err());
    doc.replace_page_contents(0, b"0 0 20 20 re f".to_vec())
        .unwrap();
    assert!(doc.get_page(0).unwrap().display_list().is_ok());
}
