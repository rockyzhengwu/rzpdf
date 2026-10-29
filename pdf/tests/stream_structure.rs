use pdf::{
    document::owned::Document,
    objects::{
        PdfObject, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_reference::PdfReference,
        pdf_stream::PdfStream,
    },
    parser::syntax::ParseMode,
};

#[test]
fn direct_stream_in_container_is_rejected_before_destination_changes() {
    for container in 0..3 {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let stream = PdfObject::PdfStream(PdfStream::new(PdfDict::default(), b"data".to_vec()));
        let object = match container {
            0 => {
                let mut array = PdfArray::default();
                array.add_obj(stream);
                PdfObject::PdfArray(array)
            }
            1 => {
                let mut dict = PdfDict::default();
                dict.insert("Nested".into(), stream);
                PdfObject::PdfDict(dict)
            }
            _ => {
                let mut dict = PdfDict::default();
                dict.insert("Nested".into(), stream);
                PdfObject::PdfStream(PdfStream::new(dict, Vec::new()))
            }
        };
        doc.try_insert_object(object).unwrap();
        let revision = doc.revision();
        let mut destination = b"original".to_vec();
        assert!(doc.save_to_writer(&mut destination).is_err());
        assert_eq!(destination, b"original");
        assert_eq!(doc.revision(), revision);
    }
}

#[test]
fn indirect_stream_reference_in_container_survives_strict_save() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let stream_id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            b"data".to_vec(),
        )))
        .unwrap();
    let mut dict = PdfDict::default();
    dict.insert(
        "Nested".into(),
        PdfObject::PdfReference(PdfReference::from_id(stream_id)),
    );
    let id = doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
    let reopened =
        Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert!(
        reopened
            .resolve_object(id)
            .unwrap()
            .as_dict()
            .unwrap()
            .get("Nested")
            .unwrap()
            .as_reference()
            .is_some()
    );
    assert_eq!(
        reopened
            .resolve_object(stream_id)
            .unwrap()
            .as_stream()
            .unwrap()
            .raw_data()
            .unwrap(),
        b"data"
    );
}
