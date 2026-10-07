use pdf::{
    document::PdfDocument,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_stream::PdfStream},
    parser::syntax::ParseMode,
};

#[test]
fn decoded_replacement_clears_encoding_and_survives_save() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let mut dict = PdfDict::default();
    for key in ["Filter", "DecodeParms", "DL"] {
        dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new("old".into())))
            .unwrap();
    }
    dict.try_insert(
        "Custom".into(),
        PdfObject::PdfName(PdfName::new("keep".into())),
    )
    .unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            dict,
            b"encoded".to_vec(),
        )))
        .unwrap();
    doc.replace_stream_decoded_data(id, b"decoded replacement".to_vec())
        .unwrap();
    assert_eq!(
        doc.resolve_object(id)
            .unwrap()
            .as_stream()
            .unwrap()
            .data_state(),
        pdf::objects::pdf_stream::StreamDataState::Decoded
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let object = reopened.resolve_object(id).unwrap();
    let stream = object.as_stream().unwrap();
    assert_eq!(
        stream.data_state(),
        pdf::objects::pdf_stream::StreamDataState::RawEncoded
    );
    assert_eq!(stream.raw_data(), b"decoded replacement");
    for key in ["Filter", "DecodeParms", "DL"] {
        assert!(stream.dict().get(key).is_none());
    }
    assert!(stream.dict().get("Custom").is_some());
    assert_eq!(
        stream
            .dict()
            .get("Length")
            .unwrap()
            .as_number()
            .unwrap()
            .as_u64_checked()
            .unwrap(),
        19
    );
}

#[test]
fn external_stream_rejection_preserves_original_object() {
    let doc = PdfDocument::new().unwrap();
    let mut dict = PdfDict::default();
    dict.try_insert(
        "F".into(),
        PdfObject::PdfName(PdfName::new("external".into())),
    )
    .unwrap();
    let original = PdfObject::PdfStream(PdfStream::new(dict, b"original".to_vec()));
    let id = doc.try_insert_object(original.clone()).unwrap();
    assert!(
        doc.replace_stream_decoded_data(id, b"replacement".to_vec())
            .is_err()
    );
    assert_eq!(doc.resolve_object(id).unwrap(), original);
}

#[test]
fn external_page_content_is_not_interpreted_as_inline_content() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.replace_page_contents(0, b"q Q".to_vec()).unwrap();
    let snapshot = doc.get_page(0).unwrap().content_program().unwrap();
    let id = snapshot.stream_object_id(0).unwrap();
    let mut dict = PdfDict::default();
    dict.insert(
        "F".into(),
        PdfObject::PdfName(PdfName::new("external".into())),
    );
    doc.update_object(
        id,
        PdfObject::PdfStream(PdfStream::new(dict, b"q Q".to_vec())),
    )
    .unwrap();
    let revision = doc.revision();
    assert!(doc.get_page(0).unwrap().content_program().is_err());
    assert!(doc.get_page(0).unwrap().display_list().is_err());
    assert_eq!(doc.revision(), revision);
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert!(reopened.get_page(0).unwrap().content_program().is_err());
    assert!(reopened.get_page(0).unwrap().display_list().is_err());
    assert_eq!(
        reopened
            .resolve_object(id)
            .unwrap()
            .as_stream()
            .unwrap()
            .raw_data(),
        b"q Q"
    );
}

#[test]
fn replacement_limits_accept_boundary_and_reject_without_mutation() {
    use pdf::{
        error::PdfError,
        parser::options::{Limits, ParseOptions},
    };
    for (raw, decoded) in [(3, 8), (8, 3)] {
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_raw_stream_bytes: raw,
                max_decoded_bytes: decoded,
                ..Limits::default()
            },
            ..ParseOptions::default()
        })
        .unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                Vec::new(),
            )))
            .unwrap();
        doc.replace_stream_decoded_data(id, b"abc".to_vec())
            .unwrap();
        let before = doc.resolve_object(id).unwrap();
        assert!(matches!(
            doc.replace_stream_decoded_data(id, b"abcd".to_vec()),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(doc.resolve_object(id).unwrap(), before);
        doc.replace_stream_decoded_data(id, Vec::new()).unwrap();
        assert!(
            doc.resolve_object(id)
                .unwrap()
                .as_stream()
                .unwrap()
                .raw_data()
                .is_empty()
        );
    }
}

#[test]
fn non_stream_target_is_rejected_without_mutation() {
    let doc = PdfDocument::new().unwrap();
    let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    assert!(doc.replace_stream_decoded_data(id, Vec::new()).is_err());
    assert_eq!(doc.resolve_object(id).unwrap(), PdfObject::PdfNull);
}

#[test]
fn flate_policy_preserves_external_streams() {
    use pdf::document::save_options::{SaveOptions, StreamCompression};
    for key in ["F", "FFilter", "FDecodeParms"] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let mut dict = PdfDict::default();
        dict.try_insert(
            key.into(),
            PdfObject::PdfName(PdfName::new("external".into())),
        )
        .unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                dict,
                b"inline bytes".to_vec(),
            )))
            .unwrap();
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                compression: StreamCompression::FlateUnencoded,
                ..SaveOptions::default()
            })
            .unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        let object = reopened.resolve_object(id).unwrap();
        let stream = object.as_stream().unwrap();
        assert_eq!(stream.raw_data(), b"inline bytes");
        assert!(stream.dict().get(key).is_some());
        assert!(stream.dict().get("Filter").is_none());
        assert!(doc.replace_stream_decoded_data(id, Vec::new()).is_err());
    }
}

#[test]
fn replacement_can_be_recompressed_without_mutating_document() {
    use pdf::document::save_options::{SaveOptions, StreamCompression};
    use std::io::Read;
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            Vec::new(),
        )))
        .unwrap();
    let data = b"q 1 0 0 rg 0 0 10 10 re f Q\n";
    doc.replace_stream_decoded_data(id, data.to_vec()).unwrap();
    let before = doc.resolve_object(id).unwrap();
    let bytes = doc
        .save_to_bytes_with_options(SaveOptions {
            compression: StreamCompression::FlateUnencoded,
            ..SaveOptions::default()
        })
        .unwrap();
    assert_eq!(doc.resolve_object(id).unwrap(), before);
    let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    let object = reopened.resolve_object(id).unwrap();
    let stream = object.as_stream().unwrap();
    assert_eq!(
        stream
            .dict()
            .get("Filter")
            .unwrap()
            .as_name()
            .unwrap()
            .bytes(),
        b"FlateDecode"
    );
    let mut decoded = Vec::new();
    flate2::read::ZlibDecoder::new(stream.raw_data())
        .read_to_end(&mut decoded)
        .unwrap();
    assert_eq!(decoded, data);
    reopened
        .replace_stream_decoded_data(id, b"q Q\n".to_vec())
        .unwrap();
    let final_doc =
        PdfDocument::from_bytes_with_mode(reopened.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
    let final_object = final_doc.resolve_object(id).unwrap();
    assert_eq!(final_object.as_stream().unwrap().raw_data(), b"q Q\n");
    assert!(
        final_object
            .as_stream()
            .unwrap()
            .dict()
            .get("Filter")
            .is_none()
    );
}

#[test]
fn flate_save_classifies_latest_indirect_filter_and_null_parameters() {
    use pdf::{
        document::save_options::{SaveOptions, StreamCompression},
        objects::pdf_reference::PdfReference,
    };
    use std::io::Read;
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let filter_id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let params_id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let params_chain = doc
        .try_insert_object(PdfObject::PdfReference(PdfReference::from_id(params_id)))
        .unwrap();
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Filter".into(),
        PdfObject::PdfReference(PdfReference::from_id(filter_id)),
    )
    .unwrap();
    dict.try_insert(
        "DecodeParms".into(),
        PdfObject::PdfReference(PdfReference::from_id(params_chain)),
    )
    .unwrap();
    let original = PdfObject::PdfStream(PdfStream::new(dict, b"0102>".to_vec()));
    let stream_id = doc.try_insert_object(original.clone()).unwrap();
    let options = SaveOptions {
        compression: StreamCompression::FlateUnencoded,
        ..SaveOptions::default()
    };
    let reopened = PdfDocument::from_bytes_with_mode(
        doc.save_to_bytes_with_options(options).unwrap(),
        ParseMode::Strict,
    )
    .unwrap();
    let object = reopened.resolve_object(stream_id).unwrap();
    let stream = object.as_stream().unwrap();
    assert_eq!(
        stream
            .dict()
            .get("Filter")
            .unwrap()
            .as_name()
            .unwrap()
            .bytes(),
        b"FlateDecode"
    );
    assert!(stream.dict().get("DecodeParms").is_none());
    let mut decoded = Vec::new();
    flate2::read::ZlibDecoder::new(stream.raw_data())
        .read_to_end(&mut decoded)
        .unwrap();
    assert_eq!(decoded, b"0102>");
    assert_eq!(doc.resolve_object(stream_id).unwrap(), original);
    doc.update_object(
        filter_id,
        PdfObject::PdfName(PdfName::new("ASCIIHexDecode".into())),
    )
    .unwrap();
    let reopened = PdfDocument::from_bytes_with_mode(
        doc.save_to_bytes_with_options(options).unwrap(),
        ParseMode::Strict,
    )
    .unwrap();
    let object = reopened.resolve_object(stream_id).unwrap();
    let stream = object.as_stream().unwrap();
    assert_eq!(stream.raw_data(), b"0102>");
    assert_eq!(
        stream
            .dict()
            .get("Filter")
            .unwrap()
            .as_reference()
            .unwrap()
            .id(),
        filter_id
    );
    assert_eq!(doc.resolve_object(stream_id).unwrap(), original);
    doc.update_object(filter_id, PdfObject::PdfNull).unwrap();
    doc.update_object(params_id, PdfObject::PdfDict(PdfDict::default()))
        .unwrap();
    let mut writer = b"unchanged".to_vec();
    assert!(
        doc.save_to_writer_with_options(&mut writer, options)
            .is_err()
    );
    assert_eq!(writer, b"unchanged");
    doc.update_object(
        filter_id,
        PdfObject::PdfReference(PdfReference::from_id(filter_id)),
    )
    .unwrap();
    assert!(matches!(
        doc.save_to_writer_with_options(&mut writer, options),
        Err(pdf::error::PdfError::ResourceLimit(_))
    ));
    assert_eq!(writer, b"unchanged");
}

#[test]
fn page_content_byte_limits_reject_before_editing() {
    use pdf::{
        page::page_builder::PageBuilder,
        parser::options::{Limits, ParseOptions},
    };
    for (raw, decoded) in [(4, 8), (8, 4)] {
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_raw_stream_bytes: raw,
                max_decoded_bytes: decoded,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"q Q\n".to_vec()).unwrap();
        let before = doc.stamp();
        assert!(doc.replace_page_contents(0, b"q Q\n ".to_vec()).is_err());
        assert!(doc.append_page_contents(0, b"q Q\n ".to_vec()).is_err());
        assert!(
            doc.add_page_with_builder(1, PageBuilder::new().content(b"q Q\n ".to_vec()))
                .is_err()
        );
        assert!(doc.is_stamp_current(&before));
        assert_eq!(doc.try_page_count().unwrap(), 1);
        assert!(doc.get_page(0).unwrap().display_list().is_ok());
    }
}
