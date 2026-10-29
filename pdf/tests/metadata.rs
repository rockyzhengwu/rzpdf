use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict, pdf_string::PdfString},
    parser::syntax::ParseMode,
};

fn info(title: &str) -> PdfDict {
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Title".into(),
        PdfObject::PdfString(PdfString::from_text(title).unwrap()),
    )
    .unwrap();
    dict.try_insert("Custom".into(), PdfObject::PdfNull)
        .unwrap();
    dict
}

fn assert_info(value: Option<PdfDict>, title: &str) {
    let dict = value.unwrap();
    assert_eq!(
        dict.get("Title")
            .unwrap()
            .as_string()
            .unwrap()
            .text_content()
            .unwrap(),
        title
    );
    assert_eq!(dict.get("Custom"), Some(&PdfObject::PdfNull));
}

#[test]
fn info_unicode_roundtrips_and_unlinks_conservatively() {
    let doc = PdfDocument::new().unwrap();
    assert!(doc.info_dict().unwrap().is_none());
    doc.set_info_dict(info("中文🙂")).unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_info(reopened.info_dict().unwrap(), "中文🙂");
    reopened.remove_info_dict().unwrap();
    assert!(reopened.info_dict().unwrap().is_none());
    assert!(
        PdfDocument::from_bytes_with_mode(reopened.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap()
            .info_dict()
            .unwrap()
            .is_none()
    );
}

#[test]
fn info_trailer_and_object_allocation_rollback_together() {
    let doc = PdfDocument::new().unwrap();
    doc.set_info_dict(info("original")).unwrap();
    let result: PdfResult<()> = doc.edit_transaction(|doc| {
        doc.set_info_dict(info("changed"))?;
        doc.remove_info_dict()?;
        Err(PdfError::DocumentError("injected failure".into()))
    });
    assert!(result.is_err());
    assert_eq!(doc.info_dict().unwrap(), Some(info("original")));
    let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        let _: PdfResult<()> = doc.edit_transaction(|doc| {
            doc.set_info_dict(info("panic"))?;
            panic!("injected unwind");
        });
    }));
    assert!(panic.is_err());
    assert_eq!(doc.info_dict().unwrap(), Some(info("original")));
    assert_info(
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap()
            .info_dict()
            .unwrap(),
        "original",
    );
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        4
    );
}

#[test]
fn text_field_edits_preserve_custom_entries_and_old_objects() {
    use pdf::{document::metadata::InfoTextField, objects::object_id::ObjectId};
    let doc = PdfDocument::new().unwrap();
    doc.set_info_dict(info("original")).unwrap();
    let old_id = ObjectId::new(3, 0);
    let old = doc.resolve_object(old_id).unwrap();
    for field in [
        InfoTextField::Title,
        InfoTextField::Author,
        InfoTextField::Subject,
        InfoTextField::Keywords,
        InfoTextField::Creator,
        InfoTextField::Producer,
    ] {
        doc.set_info_text(field, Some("中文🙂")).unwrap();
        assert_eq!(doc.info_text(field).unwrap().as_deref(), Some("中文🙂"));
    }
    assert_eq!(doc.resolve_object(old_id).unwrap(), old);
    assert_eq!(
        doc.info_dict().unwrap().unwrap().get("Custom"),
        Some(&PdfObject::PdfNull)
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("中文🙂")
    );
    reopened.set_info_text(InfoTextField::Title, None).unwrap();
    assert!(reopened.info_text(InfoTextField::Title).unwrap().is_none());
    assert_eq!(
        reopened
            .info_text(InfoTextField::Author)
            .unwrap()
            .as_deref(),
        Some("中文🙂")
    );
}

#[test]
fn text_field_limits_and_wrong_types_fail_without_changes() {
    use pdf::{
        document::metadata::InfoTextField,
        parser::options::{Limits, ParseOptions},
    };
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 2,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    assert!(matches!(
        doc.set_info_text(InfoTextField::Title, Some("title")),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(doc.info_dict().unwrap().is_none());
    doc.set_info_text(InfoTextField::Title, None).unwrap();
    let doc = PdfDocument::new().unwrap();
    let mut dict = info("original");
    dict.try_insert("Author".into(), PdfObject::PdfDict(PdfDict::default()))
        .unwrap();
    doc.set_info_dict(dict.clone()).unwrap();
    assert!(matches!(
        doc.info_text(InfoTextField::Author),
        Err(PdfError::DocumentError(_))
    ));
    assert_eq!(doc.info_dict().unwrap(), Some(dict));
}

#[test]
fn effective_catalog_version_controls_utf8_info_and_observes_edits() {
    use pdf::{
        document::{metadata::InfoTextField, save_options::PdfVersion},
        objects::{object_id::ObjectId, pdf_name::PdfName},
    };
    let doc = PdfDocument::new().unwrap();
    assert_eq!(doc.effective_version().unwrap(), PdfVersion::V1_7);
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Title".into(),
        PdfObject::PdfString(PdfString::new(
            [vec![0xef, 0xbb, 0xbf], "中文🙂".as_bytes().to_vec()].concat(),
            false,
        )),
    )
    .unwrap();
    doc.set_info_dict(dict).unwrap();
    assert!(matches!(
        doc.info_text(InfoTextField::Title),
        Err(PdfError::UnsupportedFeature(_))
    ));
    let root_id = ObjectId::new(2, 0);
    let mut root = doc.resolve_object(root_id).unwrap().into_dict().unwrap();
    root.try_insert(
        "Version".into(),
        PdfObject::PdfName(PdfName::new("2.0".into())),
    )
    .unwrap();
    doc.update_object(root_id, PdfObject::PdfDict(root.clone()))
        .unwrap();
    assert_eq!(doc.version(), "1.7");
    assert_eq!(doc.effective_version().unwrap(), PdfVersion::V2_0);
    assert_eq!(
        doc.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("中文🙂")
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V2_0);
    assert_eq!(
        reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("中文🙂")
    );
    for (value, expected) in [
        (
            PdfObject::PdfName(PdfName::new("1.4".into())),
            Some(PdfVersion::V1_7),
        ),
        (PdfObject::PdfNull, Some(PdfVersion::V1_7)),
        (PdfObject::PdfName(PdfName::new("9.9".into())), None),
        (PdfObject::PdfDict(PdfDict::default()), None),
    ] {
        root.try_insert("Version".into(), value).unwrap();
        doc.update_object(root_id, PdfObject::PdfDict(root.clone()))
            .unwrap();
        match expected {
            Some(version) => assert_eq!(doc.effective_version().unwrap(), version),
            None => assert!(doc.effective_version().is_err()),
        }
    }
}

#[test]
fn metadata_packet_roundtrips_compressed_and_rolls_back_with_info() {
    use pdf::document::{
        metadata::InfoTextField,
        save_options::{SaveOptions, StreamCompression},
    };
    let packet = "<x:xmpmeta xmlns:x='adobe:ns:meta/'>中文🙂</x:xmpmeta>"
        .as_bytes()
        .to_vec();
    let doc = PdfDocument::new().unwrap();
    assert!(doc.metadata_packet().unwrap().is_none());
    doc.set_info_text(InfoTextField::Title, Some("Info title"))
        .unwrap();
    doc.set_metadata_packet(Some(packet.clone())).unwrap();
    let bytes = doc
        .save_to_bytes_with_options(SaveOptions {
            compression: StreamCompression::FlateUnencoded,
            ..Default::default()
        })
        .unwrap();
    let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(reopened.metadata_packet().unwrap(), Some(packet.clone()));
    assert_eq!(
        reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("Info title")
    );
    let result: PdfResult<()> = reopened.edit_transaction(|doc| {
        doc.set_metadata_packet(Some(b"<changed/>".to_vec()))?;
        doc.set_info_text(InfoTextField::Title, Some("changed"))?;
        Err(PdfError::DocumentError("injected metadata failure".into()))
    });
    assert!(result.is_err());
    assert_eq!(reopened.metadata_packet().unwrap(), Some(packet));
    assert_eq!(
        reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("Info title")
    );
    reopened.set_metadata_packet(None).unwrap();
    assert!(
        PdfDocument::from_bytes_with_mode(reopened.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap()
            .metadata_packet()
            .unwrap()
            .is_none()
    );
}

#[test]
fn metadata_packet_byte_limit_is_checked_before_document_edits() {
    use pdf::parser::options::{Limits, ParseOptions};
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_raw_stream_bytes: 4,
            max_decoded_bytes: 4,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.set_metadata_packet(Some(b"<a/>".to_vec())).unwrap();
    assert!(matches!(
        doc.set_metadata_packet(Some(b"<bad/>".to_vec())),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(doc.metadata_packet().unwrap(), Some(b"<a/>".to_vec()));
}

#[test]
fn metadata_replacement_preserves_shared_old_stream() {
    use pdf::objects::{object_id::ObjectId, pdf_reference::PdfReference};
    let doc = PdfDocument::new().unwrap();
    doc.set_metadata_packet(Some(b"<old/>".to_vec())).unwrap();
    let old_id = ObjectId::new(3, 0);
    let original = doc.resolve_object(old_id).unwrap();
    let mut shared = PdfDict::default();
    shared
        .try_insert(
            "SharedMetadata".into(),
            PdfObject::PdfReference(PdfReference::from_id(old_id)),
        )
        .unwrap();
    let shared_id = doc
        .try_insert_object(PdfObject::PdfDict(shared.clone()))
        .unwrap();
    doc.set_metadata_packet(Some(b"<new/>".to_vec())).unwrap();
    assert_eq!(doc.resolve_object(old_id).unwrap(), original);
    assert_eq!(
        doc.resolve_object(shared_id).unwrap(),
        PdfObject::PdfDict(shared)
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened.metadata_packet().unwrap(),
        Some(b"<new/>".to_vec())
    );
    assert_eq!(
        reopened
            .read_stream_data(
                &reopened
                    .resolve_object(old_id)
                    .unwrap()
                    .as_stream()
                    .unwrap()
            )
            .unwrap()
            .as_ref(),
        b"<old/>"
    );
    reopened.set_metadata_packet(None).unwrap();
    assert_eq!(
        reopened
            .read_stream_data(
                &reopened
                    .resolve_object(old_id)
                    .unwrap()
                    .as_stream()
                    .unwrap()
            )
            .unwrap()
            .as_ref(),
        b"<old/>"
    );
}

#[test]
fn invalid_metadata_stream_types_return_errors() {
    use pdf::objects::{object_id::ObjectId, pdf_name::PdfName, pdf_stream::PdfStream};
    let doc = PdfDocument::new().unwrap();
    doc.set_metadata_packet(Some(b"<ok/>".to_vec())).unwrap();
    let metadata_id = ObjectId::new(3, 0);
    let mut valid = PdfDict::default();
    valid
        .try_insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new("Metadata".into())),
        )
        .unwrap();
    valid
        .try_insert(
            "Subtype".into(),
            PdfObject::PdfName(PdfName::new("XML".into())),
        )
        .unwrap();
    for key in ["Type", "Subtype"] {
        for replacement in [
            None,
            Some(PdfObject::PdfNull),
            Some(PdfObject::PdfName(PdfName::new("Wrong".into()))),
        ] {
            let mut dict = valid.clone();
            dict.remove(key);
            if let Some(value) = replacement {
                dict.try_insert(key.into(), value).unwrap();
            }
            doc.update_object(
                metadata_id,
                PdfObject::PdfStream(PdfStream::new(dict, b"<ok/>".to_vec())),
            )
            .unwrap();
            assert!(matches!(
                doc.metadata_packet(),
                Err(PdfError::DocumentError(_))
            ));
        }
    }
    doc.update_object(metadata_id, PdfObject::PdfDict(valid))
        .unwrap();
    assert!(matches!(
        doc.metadata_packet(),
        Err(PdfError::DocumentError(_))
    ));
}

#[test]
fn metadata_version_and_object_limits_reject_without_allocating_ids() {
    use pdf::{
        objects::object_id::ObjectId,
        parser::options::{Limits, ParseOptions},
    };
    let doc = PdfDocument::new().unwrap();
    let mut bytes = doc.save_to_bytes().unwrap();
    bytes[7] = b'3'; // Same-length Header replacement keeps all xref offsets valid.
    let old = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert!(matches!(
        old.set_metadata_packet(Some(b"<a/>".to_vec())),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert!(old.metadata_packet().unwrap().is_none());
    assert_eq!(
        old.try_insert_object(PdfObject::PdfNull).unwrap(),
        ObjectId::new(3, 0)
    );
    let limited = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 2,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    assert!(matches!(
        limited.set_metadata_packet(Some(b"<a/>".to_vec())),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(limited.metadata_packet().unwrap().is_none());
    limited.set_metadata_packet(None).unwrap();
}

#[test]
fn paired_info_metadata_update_is_atomic_on_second_step_failure() {
    use pdf::{
        objects::object_id::ObjectId,
        parser::options::{Limits, ParseOptions},
    };
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_raw_stream_bytes: 4,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.set_info_and_metadata(Some(info("original")), Some(b"<a/>".to_vec()))
        .unwrap();
    assert!(matches!(
        doc.set_info_and_metadata(Some(info("changed")), Some(b"<long/>".to_vec())),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_info(doc.info_dict().unwrap(), "original");
    assert_eq!(doc.metadata_packet().unwrap(), Some(b"<a/>".to_vec()));
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap(),
        ObjectId::new(5, 0)
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_info(reopened.info_dict().unwrap(), "original");
    assert_eq!(reopened.metadata_packet().unwrap(), Some(b"<a/>".to_vec()));
    reopened.set_info_and_metadata(None, None).unwrap();
    assert!(reopened.info_dict().unwrap().is_none());
    assert!(reopened.metadata_packet().unwrap().is_none());
}

#[test]
fn paired_metadata_allocation_failure_restores_previous_info() {
    use pdf::parser::options::{Limits, ParseOptions};
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 4,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.set_info_dict(info("original")).unwrap();
    // Replacing Info uses the final allowed object; Metadata allocation then fails.
    assert!(matches!(
        doc.set_info_and_metadata(Some(info("changed")), Some(b"<a/>".to_vec())),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_info(doc.info_dict().unwrap(), "original");
    assert!(doc.metadata_packet().unwrap().is_none());
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        4
    );
}
