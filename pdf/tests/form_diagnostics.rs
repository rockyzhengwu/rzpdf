use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_stream::PdfStream},
    parser::{
        options::{Limits, ParseOptions},
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;
type Doc = PdfDocument<Cursor<Vec<u8>>>;

fn form(doc: &Doc, resources: &str, data: &[u8]) -> ObjectId {
    let dict = SyntaxParser::new(
        StreamReader::try_new(Cursor::new(
            format!("<< /Subtype /Form /BBox [0 0 10 10] /Resources {resources} >>").into_bytes(),
        ))
        .unwrap(),
    )
    .get_object()
    .unwrap()
    .into_dict()
    .unwrap();
    doc.try_insert_object(PdfObject::PdfStream(
        PdfStream::from_decoded_data(dict, data.to_vec()).unwrap(),
    ))
    .unwrap()
}
fn fixture() -> (Vec<u8>, ObjectId, ObjectId) {
    let doc = Doc::new().unwrap();
    doc.add_page(0).unwrap();
    let leaf = form(&doc, "<< >>", b"badop 0 0 1 1 re f");
    let parent = form(
        &doc,
        &format!("<< /XObject << /L {} 0 R >> >>", leaf.obj_num()),
        b"/L Do /L Do",
    );
    let name = doc
        .resources()
        .register_existing(0, ResourceKind::Form, parent)
        .unwrap();
    doc.replace_page_contents(0, format!("/{name} Do /{name} Do pageop").into_bytes())
        .unwrap();
    (doc.save_to_bytes().unwrap(), parent, leaf)
}
#[test]
fn repeated_nested_form_warnings_keep_leaf_offsets_and_each_call_site() {
    let (bytes, parent, leaf) = fixture();
    let doc = Doc::from_bytes_with_mode(bytes.clone(), ParseMode::Compatible).unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    assert_eq!(list.objects().len(), 4);
    let diagnostics = list.diagnostics();
    assert_eq!(diagnostics.len(), 5);
    for (i, d) in diagnostics[..4].iter().enumerate() {
        assert_eq!(d.operator, "badop");
        let source = d.source.as_ref().unwrap();
        assert_eq!(source.operator_byte_offset, d.byte_offset);
        assert_eq!(source.operation_index, 0);
        assert_eq!(source.invocations.len(), 2);
        assert_eq!(source.invocations[0].form_id, Some(parent));
        assert_eq!(source.invocations[0].operation_index, i / 2);
        assert_eq!(source.invocations[1].form_id, Some(leaf));
        assert_eq!(source.invocations[1].operation_index, i % 2);
    }
    assert_eq!(diagnostics[4].operator, "pageop");
    assert!(diagnostics[4].source.is_none());
    let report = doc
        .get_page(0)
        .unwrap()
        .extract_characters_paint_order_report()
        .unwrap();
    assert_eq!(report.diagnostics, diagnostics);
    let strict = Doc::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert!(matches!(
        strict.get_page(0).unwrap().display_list(),
        Err(PdfError::UnsupportedFeature(_))
    ));
}
#[test]
fn nested_form_diagnostic_limits_are_shared_across_all_invocations() {
    let (bytes, _, _) = fixture();
    let doc = Doc::from_bytes_with_mode(bytes.clone(), ParseMode::Compatible).unwrap();
    let list = doc.get_page(0).unwrap().display_list().unwrap();
    let size = list
        .diagnostics()
        .iter()
        .map(|d| {
            d.operator.len()
                + d.message.len()
                + d.source.as_ref().map_or(0, |s| {
                    s.invocations.len()
                        * std::mem::size_of::<pdf::page::source_provenance::FormInvocation>()
                })
        })
        .sum::<usize>();
    for (count, byte_limit, allowed) in [(5, size, true), (4, size, false), (5, size - 1, false)] {
        let doc = Doc::from_bytes_with_options(
            bytes.clone(),
            ParseOptions {
                mode: ParseMode::Compatible,
                limits: Limits {
                    max_content_diagnostics: count,
                    max_content_diagnostic_bytes: byte_limit,
                    ..Default::default()
                },
                ..Default::default()
            },
        )
        .unwrap();
        let result = doc.get_page(0).unwrap().display_list();
        if allowed {
            assert!(result.is_ok());
        } else {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        }
    }
}
