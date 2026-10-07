use pdf::{
    document::{
        owned::Document,
        save_options::{PdfVersion, SaveOptions},
    },
    error::PdfError,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_number::PdfNumber, pdf_reference::PdfReference},
    page::page_builder::PageBuilder,
    parser::syntax::ParseMode,
};

#[test]
fn shading_versions_reject_before_output_and_accept_explicit_upgrade() {
    for kind in 1..=7 {
        for indirect in [false, true] {
            let doc = Document::new_with_version(PdfVersion::V1_2).unwrap();
            doc.add_page(0).unwrap();
            let mut value = PdfObject::PdfNumber(PdfNumber::new(kind as f32));
            if indirect {
                value = PdfObject::PdfReference(PdfReference::from_id(
                    doc.try_insert_object(value).unwrap(),
                ));
            }
            let mut dict = PdfDict::default();
            dict.insert("ShadingType".into(), value);
            // Version detection is independent of model/rendering validation.
            let object = if kind >= 4 {
                PdfObject::PdfStream(pdf::objects::pdf_stream::PdfStream::new(dict, vec![]))
            } else {
                PdfObject::PdfDict(dict)
            };
            let id = doc.try_insert_object(object).unwrap();
            let revision = doc.revision();
            let mut output = b"original".to_vec();
            assert!(
                matches!(doc.save_to_writer_with_options(&mut output, SaveOptions::default()),
                Err(PdfError::UnsupportedFeature(message)) if message.contains("shading"))
            );
            assert_eq!(output, b"original");
            assert_eq!(doc.revision(), revision);
            let bytes = doc
                .save_to_bytes_with_options(SaveOptions {
                    version: Some(PdfVersion::V1_3),
                    ..Default::default()
                })
                .unwrap();
            let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V1_3);
            assert!(reopened.resolve_object(id).is_ok());
        }
    }
}

#[test]
fn shading_version_detection_does_not_round_unknown_values() {
    for lexeme in [b"2.00000000001".as_slice(), b"2.0", b"0", b"8", b"-1"] {
        let doc = Document::new_with_version(PdfVersion::V1_2).unwrap();
        doc.add_page(0).unwrap();
        let mut dict = PdfDict::default();
        dict.insert(
            "ShadingType".into(),
            PdfObject::PdfNumber(PdfNumber::new_from_lexeme(lexeme).unwrap()),
        );
        doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
        assert!(doc.save_to_bytes().is_ok());
    }
}

#[test]
fn function_types_require_their_introduction_version_before_writer_output() {
    for (kind, old, minimum) in [
        (0, PdfVersion::V1_1, PdfVersion::V1_2),
        (2, PdfVersion::V1_2, PdfVersion::V1_3),
        (3, PdfVersion::V1_2, PdfVersion::V1_3),
        (4, PdfVersion::V1_2, PdfVersion::V1_3),
    ] {
        for indirect in [false, true] {
            let doc = Document::new_with_version(old).unwrap();
            doc.add_page(0).unwrap();
            let value = PdfObject::PdfNumber(PdfNumber::new(kind as f32));
            let value = if indirect {
                PdfObject::PdfReference(PdfReference::from_id(
                    doc.try_insert_object(value).unwrap(),
                ))
            } else {
                value
            };
            // Retained objects exercise feature detection, not function validity
            // or execution; those have separate constructor/evaluation tests.
            let mut dict = PdfDict::default();
            dict.insert("FunctionType".into(), value);
            let object = if matches!(kind, 0 | 4) {
                PdfObject::PdfStream(pdf::objects::pdf_stream::PdfStream::new(dict, Vec::new()))
            } else {
                PdfObject::PdfDict(dict)
            };
            let id = doc.try_insert_object(object).unwrap();
            let revision = doc.revision();
            let mut destination = b"original".to_vec();
            assert!(matches!(
                doc.save_to_writer_with_options(&mut destination, SaveOptions::default()),
                Err(PdfError::UnsupportedFeature(_))
            ));
            assert_eq!(destination, b"original");
            assert_eq!(doc.revision(), revision);
            let bytes = doc
                .save_to_bytes_with_options(SaveOptions {
                    version: Some(minimum),
                    ..Default::default()
                })
                .unwrap();
            let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            assert_eq!(reopened.effective_version().unwrap(), minimum);
            assert!(reopened.resolve_object(id).is_ok());
            assert_eq!(doc.revision(), revision);
        }
    }
}

#[test]
fn function_preflight_uses_effective_catalog_version_without_rewriting_header() {
    use pdf::objects::{object_id::ObjectId, pdf_name::PdfName};
    let doc = Document::new_with_version(PdfVersion::V1_2).unwrap();
    doc.add_page(0).unwrap();
    let mut function = PdfDict::default();
    function.insert(
        "FunctionType".into(),
        PdfObject::PdfNumber(PdfNumber::new(3.0)),
    );
    doc.try_insert_object(PdfObject::PdfDict(function)).unwrap();
    assert!(matches!(
        doc.save_to_bytes(),
        Err(PdfError::UnsupportedFeature(_))
    ));
    // New documents create the catalog at 2 0; this fixture does not assume
    // arbitrary parsed documents have the same catalog address.
    let root_id = ObjectId::new(2, 0);
    let mut root = doc.resolve_object(root_id).unwrap().into_dict().unwrap();
    let version_id = doc
        .try_insert_object(PdfObject::PdfName(PdfName::new("1.4".into())))
        .unwrap();
    root.insert(
        "Version".into(),
        PdfObject::PdfReference(PdfReference::from_id(version_id)),
    );
    doc.update_object(root_id, PdfObject::PdfDict(root))
        .unwrap();
    let revision = doc.revision();
    let bytes = doc.save_to_bytes().unwrap();
    assert!(bytes.starts_with(b"%PDF-1.2"));
    assert_eq!(doc.revision(), revision);
    let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V1_4);
    assert!(reopened.save_to_bytes().is_ok());
}

#[test]
fn function_preflight_does_not_round_or_validate_unknown_types() {
    for value in [
        PdfObject::PdfNumber(PdfNumber::new_from_lexeme(b"2.00000000001").unwrap()),
        PdfObject::PdfNumber(PdfNumber::new_from_lexeme(b"2.0").unwrap()),
        PdfObject::PdfNumber(PdfNumber::new_from_lexeme(b"18446744073709551616").unwrap()),
        PdfObject::PdfNumber(PdfNumber::new(-2.0)),
        PdfObject::PdfNumber(PdfNumber::new(1.0)),
        PdfObject::PdfNull,
    ] {
        for indirect in [false, true] {
            let doc = Document::new_with_version(PdfVersion::V1_1).unwrap();
            doc.add_page(0).unwrap();
            let value = if indirect {
                PdfObject::PdfReference(PdfReference::from_id(
                    doc.try_insert_object(value.clone()).unwrap(),
                ))
            } else {
                value.clone()
            };
            let mut dict = PdfDict::default();
            dict.insert("FunctionType".into(), value);
            let id = doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
            let revision = doc.revision();
            let bytes = doc.save_to_bytes().unwrap();
            assert_eq!(doc.revision(), revision);
            let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V1_1);
            assert!(reopened.resolve_object(id).is_ok());
        }
    }
}

#[test]
fn named_and_image_color_spaces_require_their_introduction_versions() {
    use pdf::objects::{pdf_array::PdfArray, pdf_name::PdfName};
    for (family, old, minimum) in [
        ("Pattern", PdfVersion::V1_1, PdfVersion::V1_2),
        ("Separation", PdfVersion::V1_1, PdfVersion::V1_2),
        ("ICCBased", PdfVersion::V1_2, PdfVersion::V1_3),
        ("DeviceN", PdfVersion::V1_2, PdfVersion::V1_3),
    ] {
        for resource_table in [false, true] {
            for indirect in [false, true] {
                let doc = Document::new_with_version(old).unwrap();
                doc.add_page(0).unwrap();
                let name = PdfObject::PdfName(PdfName::new(family.into()));
                let mut array = PdfArray::default();
                array.add_obj(name);
                // Version fixtures only; these deliberately do not claim valid
                // profile/tint-transform parameters or rendering support.
                let space = if family == "Pattern" {
                    PdfObject::PdfName(PdfName::new(family.into()))
                } else {
                    PdfObject::PdfArray(array)
                };
                let space = if indirect {
                    PdfObject::PdfReference(PdfReference::from_id(
                        doc.try_insert_object(space).unwrap(),
                    ))
                } else {
                    space
                };
                let value = if resource_table {
                    let mut resources = PdfDict::default();
                    resources.insert("CS1".into(), space);
                    PdfObject::PdfDict(resources)
                } else {
                    space
                };
                let value = if indirect {
                    PdfObject::PdfReference(PdfReference::from_id(
                        doc.try_insert_object(value).unwrap(),
                    ))
                } else {
                    value
                };
                let mut dict = PdfDict::default();
                dict.insert("ColorSpace".into(), value);
                doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
                let revision = doc.revision();
                let mut output = b"unchanged".to_vec();
                assert!(matches!(
                    doc.save_to_writer_with_options(&mut output, SaveOptions::default()),
                    Err(PdfError::UnsupportedFeature(_))
                ));
                assert_eq!(output, b"unchanged");
                assert_eq!(doc.revision(), revision);
                let bytes = doc
                    .save_to_bytes_with_options(SaveOptions {
                        version: Some(minimum),
                        ..Default::default()
                    })
                    .unwrap();
                let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
                assert_eq!(reopened.effective_version().unwrap(), minimum);
            }
        }
    }
}

#[test]
fn indexed_color_base_versions_and_cycles_are_checked_before_output() {
    use pdf::objects::{pdf_array::PdfArray, pdf_name::PdfName};
    for family in ["ICCBased", "Separation", "DeviceN"] {
        let doc = Document::new_with_version(PdfVersion::V1_2).unwrap();
        doc.add_page(0).unwrap();
        let mut base = PdfArray::default();
        base.add_obj(PdfObject::PdfName(PdfName::new(family.into())));
        let base_id = doc.try_insert_object(PdfObject::PdfArray(base)).unwrap();
        let mut indexed = PdfArray::default();
        indexed.add_obj(PdfObject::PdfName(PdfName::new("Indexed".into())));
        indexed.add_obj(PdfObject::PdfReference(PdfReference::from_id(base_id)));
        let mut dict = PdfDict::default();
        dict.insert("ColorSpace".into(), PdfObject::PdfArray(indexed));
        doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
        assert!(matches!(
            doc.save_to_bytes(),
            Err(PdfError::UnsupportedFeature(_))
        ));
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                version: Some(PdfVersion::V1_3),
                ..Default::default()
            })
            .unwrap();
        assert!(Document::from_bytes_with_mode(bytes, ParseMode::Strict).is_ok());
    }
    let doc = Document::new_with_version(PdfVersion::V1_2).unwrap();
    doc.add_page(0).unwrap();
    let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let mut array = PdfArray::default();
    array.add_obj(PdfObject::PdfName(PdfName::new("Indexed".into())));
    array.add_obj(PdfObject::PdfReference(PdfReference::from_id(id)));
    doc.update_object(id, PdfObject::PdfArray(array)).unwrap();
    let mut dict = PdfDict::default();
    dict.insert(
        "ColorSpace".into(),
        PdfObject::PdfReference(PdfReference::from_id(id)),
    );
    doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
    let revision = doc.revision();
    let mut output = b"original".to_vec();
    assert!(matches!(
        doc.save_to_writer_with_options(&mut output, SaveOptions::default()),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(output, b"original");
    assert_eq!(doc.revision(), revision);
    doc.update_object(id, PdfObject::PdfName(PdfName::new("DeviceRGB".into())))
        .unwrap();
    assert!(doc.save_to_bytes().is_ok());
}

#[test]
fn external_stream_settings_require_1_2_even_without_newer_filter() {
    use pdf::objects::{pdf_name::PdfName, pdf_stream::PdfStream};
    for key in ["F", "FFilter", "FDecodeParms"] {
        let doc = Document::new_with_version(PdfVersion::V1_1).unwrap();
        doc.add_page(0).unwrap();
        let mut dict = PdfDict::default();
        let value = if key == "FDecodeParms" {
            PdfObject::PdfDict(PdfDict::default())
        } else {
            PdfObject::PdfName(PdfName::new("ASCIIHexDecode".into()))
        };
        dict.insert(key.into(), value);
        doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, Vec::new())))
            .unwrap();
        assert!(matches!(
            doc.save_to_bytes(),
            Err(PdfError::UnsupportedFeature(_))
        ));
        assert!(
            doc.save_to_bytes_with_options(SaveOptions {
                version: Some(PdfVersion::V1_2),
                ..Default::default()
            })
            .is_ok()
        );
    }
}

#[test]
fn external_filter_versions_are_checked_without_reading_external_payload() {
    use pdf::objects::{
        pdf_array::PdfArray, pdf_name::PdfName, pdf_stream::PdfStream, pdf_string::PdfString,
    };
    for (filter, old, minimum) in [
        ("FlateDecode", PdfVersion::V1_1, PdfVersion::V1_2),
        ("JBIG2Decode", PdfVersion::V1_3, PdfVersion::V1_4),
        ("JPXDecode", PdfVersion::V1_4, PdfVersion::V1_5),
    ] {
        for indirect in [false, true] {
            let doc = Document::new_with_version(old).unwrap();
            doc.add_page(0).unwrap();
            let value = PdfObject::PdfName(PdfName::new(filter.into()));
            let value = if indirect {
                let id = doc.try_insert_object(value).unwrap();
                let mut array = PdfArray::default();
                array.add_obj(PdfObject::PdfReference(PdfReference::from_id(id)));
                let id = doc.try_insert_object(PdfObject::PdfArray(array)).unwrap();
                PdfObject::PdfReference(PdfReference::from_id(id))
            } else {
                value
            };
            let mut dict = PdfDict::default();
            dict.insert("FFilter".into(), value);
            dict.insert(
                "F".into(),
                PdfObject::PdfString(PdfString::new(
                    b"nonexistent-external-payload".to_vec(),
                    false,
                )),
            );
            let id = doc
                .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                    dict,
                    b"placeholder".to_vec(),
                )))
                .unwrap();
            let revision = doc.revision();
            let mut destination = b"original".to_vec();
            assert!(matches!(
                doc.save_to_writer_with_options(&mut destination, SaveOptions::default()),
                Err(PdfError::UnsupportedFeature(_))
            ));
            assert_eq!(destination, b"original");
            assert_eq!(doc.revision(), revision);
            let bytes = doc
                .save_to_bytes_with_options(SaveOptions {
                    version: Some(minimum),
                    ..Default::default()
                })
                .unwrap();
            let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            let object = reopened.resolve_object(id).unwrap();
            let stream = object.as_stream().unwrap();
            assert_eq!(stream.raw_data(), b"placeholder");
            assert!(stream.dict().get("FFilter").is_some());
            assert!(stream.dict().get("F").is_some());
        }
    }
}

#[test]
fn preserved_structural_stream_types_require_1_5() {
    use pdf::objects::{pdf_name::PdfName, pdf_stream::PdfStream};
    // These are unused retained streams, not a replacement for validating an
    // active object-stream/xref chain. The writer emits a fresh classic table.
    for stream_type in ["ObjStm", "XRef"] {
        for indirect in [false, true] {
            let doc = Document::new_with_version(PdfVersion::V1_4).unwrap();
            doc.add_page(0).unwrap();
            let value = PdfObject::PdfName(PdfName::new(stream_type.into()));
            let value = if indirect {
                PdfObject::PdfReference(PdfReference::from_id(
                    doc.try_insert_object(value).unwrap(),
                ))
            } else {
                value
            };
            let mut dict = PdfDict::default();
            dict.insert("Type".into(), value);
            if stream_type == "ObjStm" {
                for key in ["N", "First"] {
                    dict.insert(key.into(), PdfObject::PdfNumber(PdfNumber::new(0.0)));
                }
            } else {
                let mut widths = pdf::objects::pdf_array::PdfArray::default();
                for _ in 0..3 {
                    widths.add_obj(PdfObject::PdfNumber(PdfNumber::new(1.0)));
                }
                dict.insert("W".into(), PdfObject::PdfArray(widths));
                dict.insert("Size".into(), PdfObject::PdfNumber(PdfNumber::new(0.0)));
            }
            let id = doc
                .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, Vec::new())))
                .unwrap();
            let revision = doc.revision();
            let mut destination = b"original".to_vec();
            assert!(matches!(
                doc.save_to_writer_with_options(&mut destination, SaveOptions::default()),
                Err(PdfError::UnsupportedFeature(_))
            ));
            assert_eq!(destination, b"original");
            assert_eq!(doc.revision(), revision);
            let bytes = doc
                .save_to_bytes_with_options(SaveOptions {
                    version: Some(PdfVersion::V1_5),
                    ..Default::default()
                })
                .unwrap();
            let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
            assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V1_5);
            assert!(
                reopened
                    .resolve_object(id)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .raw_data()
                    .is_empty()
            );
            assert_eq!(reopened.page_count(), 1);
        }
    }
    let doc = Document::new_with_version(PdfVersion::V1_4).unwrap();
    doc.add_page(0).unwrap();
    for stream_type in ["ObjStm", "XRef"] {
        let mut ordinary = PdfDict::default();
        ordinary.insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new(stream_type.into())),
        );
        doc.try_insert_object(PdfObject::PdfDict(ordinary)).unwrap();
    }
    assert!(doc.save_to_bytes().is_ok());
}

#[test]
fn sixteen_bit_image_requires_1_5_and_preserves_samples() {
    use pdf::objects::{pdf_name::PdfName, pdf_stream::PdfStream};
    for indirect in [false, true] {
        let doc = Document::new_with_version(PdfVersion::V1_4).unwrap();
        doc.add_page(0).unwrap();
        let bits = PdfObject::PdfNumber(PdfNumber::new(16.0));
        let bits = if indirect {
            PdfObject::PdfReference(PdfReference::from_id(doc.try_insert_object(bits).unwrap()))
        } else {
            bits
        };
        let mut image = PdfDict::default();
        image.insert(
            "Subtype".into(),
            PdfObject::PdfName(PdfName::new("Image".into())),
        );
        image.insert("BitsPerComponent".into(), bits);
        image.insert(
            "ColorSpace".into(),
            PdfObject::PdfName(PdfName::new("DeviceGray".into())),
        );
        image.insert("Width".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        image.insert("Height".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(image, vec![0x80, 0])))
            .unwrap();
        let image_name = doc
            .resources()
            .register_existing(0, pdf::document::resource_manager::ResourceKind::Image, id)
            .unwrap();
        let revision = doc.revision();
        let mut destination = b"original".to_vec();
        assert!(matches!(
            doc.save_to_writer_with_options(&mut destination, SaveOptions::default()),
            Err(PdfError::UnsupportedFeature(_))
        ));
        assert_eq!(destination, b"original");
        assert_eq!(doc.revision(), revision);
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                version: Some(PdfVersion::V1_5),
                ..Default::default()
            })
            .unwrap();
        let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V1_5);
        assert_eq!(
            reopened
                .resolve_object(id)
                .unwrap()
                .as_stream()
                .unwrap()
                .raw_data(),
            [0x80, 0]
        );
        assert_eq!(
            reopened
                .get_page(0)
                .unwrap()
                .extract_decoded_image(&image_name)
                .unwrap()
                .rgba_buffer()
                .unwrap(),
            [128, 128, 128, 255]
        );
    }
    // Same-named custom fields do not establish an image feature.
    let doc = Document::new_with_version(PdfVersion::V1_4).unwrap();
    doc.add_page(0).unwrap();
    let mut custom = PdfDict::default();
    custom.insert(
        "BitsPerComponent".into(),
        PdfObject::PdfNumber(PdfNumber::new(16.0)),
    );
    doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(custom, vec![0, 0])))
        .unwrap();
    assert!(doc.save_to_bytes().is_ok());
}

#[test]
fn low_level_metadata_stream_requires_explicit_version_upgrade() {
    use pdf::objects::{pdf_name::PdfName, pdf_stream::PdfStream};
    for indirect in [false, true] {
        let doc = Document::new_with_version(PdfVersion::V1_3).unwrap();
        doc.add_page(0).unwrap();
        // The high-level metadata setter already checks 1.4. Imported or
        // low-level streams must independently satisfy the save boundary.
        assert!(doc.set_metadata_packet(Some(b"<x/>".to_vec())).is_err());
        let type_name = PdfObject::PdfName(PdfName::new("Metadata".into()));
        let type_name = if indirect {
            PdfObject::PdfReference(PdfReference::from_id(
                doc.try_insert_object(type_name).unwrap(),
            ))
        } else {
            type_name
        };
        let mut dict = PdfDict::default();
        dict.insert("Type".into(), type_name);
        dict.insert(
            "Subtype".into(),
            PdfObject::PdfName(PdfName::new("XML".into())),
        );
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, b"<x/>".to_vec())))
            .unwrap();
        let revision = doc.revision();
        let mut destination = b"original".to_vec();
        assert!(matches!(
            doc.save_to_writer_with_options(&mut destination, SaveOptions::default()),
            Err(PdfError::UnsupportedFeature(_))
        ));
        assert_eq!(destination, b"original");
        assert_eq!(doc.revision(), revision);
        let saved = doc
            .save_to_bytes_with_options(SaveOptions {
                version: Some(PdfVersion::V1_4),
                ..Default::default()
            })
            .unwrap();
        let reopened = Document::from_bytes_with_mode(saved, ParseMode::Strict).unwrap();
        assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V1_4);
        assert_eq!(
            reopened
                .resolve_object(id)
                .unwrap()
                .as_stream()
                .unwrap()
                .raw_data(),
            b"<x/>"
        );
    }
    let doc = Document::new_with_version(PdfVersion::V1_3).unwrap();
    doc.add_page(0).unwrap();
    let mut ordinary = PdfDict::default();
    ordinary.insert(
        "Type".into(),
        PdfObject::PdfName(PdfName::new("Metadata".into())),
    );
    doc.try_insert_object(PdfObject::PdfDict(ordinary)).unwrap();
    assert!(doc.save_to_bytes().is_ok());
}

#[test]
fn transparency_version_requires_explicit_upgrade_and_preserves_destination() {
    for indirect in [false, true] {
        let doc = Document::new_with_version(PdfVersion::V1_3).unwrap();
        let mut state = PdfDict::default();
        state.insert("ca".into(), PdfObject::PdfNumber(PdfNumber::new(0.5)));
        let state = if indirect {
            PdfObject::PdfReference(PdfReference::from_id(
                doc.try_insert_object(PdfObject::PdfDict(state)).unwrap(),
            ))
        } else {
            PdfObject::PdfDict(state)
        };
        let mut states = PdfDict::default();
        states.insert("G".into(), state);
        let states = if indirect {
            PdfObject::PdfReference(PdfReference::from_id(
                doc.try_insert_object(PdfObject::PdfDict(states)).unwrap(),
            ))
        } else {
            PdfObject::PdfDict(states)
        };
        let mut resources = PdfDict::default();
        resources.insert("ExtGState".into(), states);
        doc.add_page_with_builder(
            0,
            PageBuilder::new()
                .resources(resources)
                .content(b"/G gs 0 0 10 10 re f".to_vec()),
        )
        .unwrap();
        let revision = doc.revision();
        assert!(matches!(
            doc.save_to_bytes(),
            Err(PdfError::UnsupportedFeature(_))
        ));
        let mut destination = b"existing destination".to_vec();
        assert!(
            doc.save_to_writer_with_options(&mut destination, SaveOptions::default())
                .is_err()
        );
        assert_eq!(destination, b"existing destination");
        assert_eq!(doc.revision(), revision);
        let bytes = doc
            .save_to_bytes_with_options(SaveOptions {
                version: Some(PdfVersion::V1_4),
                ..Default::default()
            })
            .unwrap();
        let reopened = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V1_4);
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
fn image_soft_mask_requires_transparency_version_but_plain_rgb_does_not() {
    use pdf::objects::{pdf_name::PdfName, pdf_stream::PdfStream};
    let doc = Document::new_with_version(PdfVersion::V1_3).unwrap();
    doc.add_page(0).unwrap();
    assert!(
        doc.resources()
            .embed_rgba_image(0, 1, 1, vec![255, 0, 0, 128])
            .is_err()
    );
    // The high-level builder already gates this feature. Exercise low-level
    // inserted/imported objects, which the save boundary must independently check.
    let mut image = PdfDict::default();
    image.insert(
        "Subtype".into(),
        PdfObject::PdfName(PdfName::new("Image".into())),
    );
    for key in ["Width", "Height"] {
        image.insert(key.into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
    }
    image.insert(
        "BitsPerComponent".into(),
        PdfObject::PdfNumber(PdfNumber::new(8.0)),
    );
    image.insert(
        "ColorSpace".into(),
        PdfObject::PdfName(PdfName::new("DeviceGray".into())),
    );
    let mask = doc
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(image.clone(), vec![128]).unwrap(),
        ))
        .unwrap();
    image.insert(
        "ColorSpace".into(),
        PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
    );
    image.insert(
        "SMask".into(),
        PdfObject::PdfReference(PdfReference::from_id(mask)),
    );
    doc.try_insert_object(PdfObject::PdfStream(
        PdfStream::from_decoded_data(image, vec![255, 0, 0]).unwrap(),
    ))
    .unwrap();
    assert!(matches!(
        doc.save_to_bytes(),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert!(
        doc.save_to_bytes_with_options(SaveOptions {
            version: Some(PdfVersion::V1_4),
            ..Default::default()
        })
        .is_ok()
    );
    let plain = Document::new_with_version(PdfVersion::V1_3).unwrap();
    plain.add_page(0).unwrap();
    plain
        .resources()
        .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
        .unwrap();
    let mut custom = PdfDict::default();
    custom.insert("ca".into(), PdfObject::PdfNumber(PdfNumber::new(0.5)));
    plain.try_insert_object(PdfObject::PdfDict(custom)).unwrap();
    assert!(plain.save_to_bytes().is_ok());
    for states in [PdfObject::PdfNull, {
        let mut states = PdfDict::default();
        states.insert("Unused".into(), PdfObject::PdfNull);
        PdfObject::PdfDict(states)
    }] {
        let doc = Document::new_with_version(PdfVersion::V1_3).unwrap();
        let mut resources = PdfDict::default();
        resources.insert("ExtGState".into(), states);
        doc.add_page_with_builder(0, PageBuilder::new().resources(resources))
            .unwrap();
        assert!(doc.save_to_bytes().is_ok());
    }
}

#[test]
fn typed_states_and_form_groups_or_resources_require_transparency_version() {
    use pdf::{
        io::stream_reader::StreamReader, objects::pdf_stream::PdfStream,
        parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;
    for (source, stream) in [
        ("<< /Type /ExtGState /BM /Normal >>", false),
        ("<< /Type /Group /S /Transparency >>", false),
        (
            "<< /Subtype /Form /BBox [0 0 10 10] /Group << /S /Transparency >> >>",
            true,
        ),
        (
            "<< /Subtype /Form /BBox [0 0 10 10] /Resources << /ExtGState << /G << /CA 0.5 >> >> >> >>",
            true,
        ),
    ] {
        let doc = Document::new_with_version(PdfVersion::V1_3).unwrap();
        doc.add_page(0).unwrap();
        let dict = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        let object = if stream {
            PdfObject::PdfStream(PdfStream::from_decoded_data(dict, Vec::new()).unwrap())
        } else {
            PdfObject::PdfDict(dict)
        };
        doc.try_insert_object(object).unwrap();
        assert!(
            matches!(doc.save_to_bytes(), Err(PdfError::UnsupportedFeature(_))),
            "{source}"
        );
        let saved = doc
            .save_to_bytes_with_options(SaveOptions {
                version: Some(PdfVersion::V1_4),
                ..Default::default()
            })
            .unwrap();
        assert!(Document::from_bytes_with_mode(saved, ParseMode::Strict).is_ok());
    }
}

#[test]
fn preserved_encoded_filters_require_their_minimum_versions() {
    use pdf::objects::{pdf_array::PdfArray, pdf_name::PdfName, pdf_stream::PdfStream};
    for (filter, old, minimum) in [
        ("FlateDecode", PdfVersion::V1_1, PdfVersion::V1_2),
        ("Fl", PdfVersion::V1_1, PdfVersion::V1_2),
        ("JBIG2Decode", PdfVersion::V1_3, PdfVersion::V1_4),
        ("JPXDecode", PdfVersion::V1_4, PdfVersion::V1_5),
        ("Crypt", PdfVersion::V1_4, PdfVersion::V1_5),
    ] {
        for indirect_array in [false, true] {
            let doc = Document::new_with_version(old).unwrap();
            doc.add_page(0).unwrap();
            let value = PdfObject::PdfName(PdfName::new(filter.into()));
            let value = if indirect_array {
                let name = doc.try_insert_object(value).unwrap();
                let mut array = PdfArray::default();
                array
                    .try_add_obj(PdfObject::PdfReference(PdfReference::from_id(name)))
                    .unwrap();
                let array = doc.try_insert_object(PdfObject::PdfArray(array)).unwrap();
                PdfObject::PdfReference(PdfReference::from_id(array))
            } else {
                value
            };
            let mut dict = PdfDict::default();
            dict.insert("Filter".into(), value);
            let id = doc
                .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![1, 2, 3])))
                .unwrap();
            let revision = doc.revision();
            assert!(
                matches!(doc.save_to_bytes(), Err(PdfError::UnsupportedFeature(_))),
                "{filter}"
            );
            assert_eq!(doc.revision(), revision);
            let saved = doc
                .save_to_bytes_with_options(SaveOptions {
                    version: Some(minimum),
                    ..Default::default()
                })
                .unwrap();
            let reopened = Document::from_bytes_with_mode(saved, ParseMode::Strict).unwrap();
            assert_eq!(reopened.effective_version().unwrap(), minimum);
            assert_eq!(
                reopened
                    .resolve_object(id)
                    .unwrap()
                    .as_stream()
                    .unwrap()
                    .raw_data(),
                &[1, 2, 3]
            );
        }
    }
}

#[test]
fn real_flate_bytes_remain_encoded_after_version_upgrade() {
    use pdf::objects::{pdf_name::PdfName, pdf_stream::PdfStream};
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(b"q Q\n").unwrap();
    let encoded = encoder.finish().unwrap();
    let doc = Document::new_with_version(PdfVersion::V1_1).unwrap();
    doc.add_page(0).unwrap();
    let mut dict = PdfDict::default();
    dict.insert(
        "Filter".into(),
        PdfObject::PdfName(PdfName::new("FlateDecode".into())),
    );
    let id = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, encoded.clone())))
        .unwrap();
    assert!(doc.save_to_bytes().is_err());
    let reopened = Document::from_bytes_with_mode(
        doc.save_to_bytes_with_options(SaveOptions {
            version: Some(PdfVersion::V1_2),
            ..Default::default()
        })
        .unwrap(),
        ParseMode::Strict,
    )
    .unwrap();
    let object = reopened.resolve_object(id).unwrap();
    let stream = object.as_stream().unwrap();
    assert_eq!(stream.raw_data(), encoded);
    assert_eq!(
        stream
            .dict()
            .get("Filter")
            .unwrap()
            .as_name()
            .unwrap()
            .name(),
        "FlateDecode"
    );
    let mut decoded = Vec::new();
    std::io::Read::read_to_end(
        &mut flate2::read::ZlibDecoder::new(stream.raw_data()),
        &mut decoded,
    )
    .unwrap();
    assert_eq!(decoded, b"q Q\n");
    let plain = Document::new_with_version(PdfVersion::V1_1).unwrap();
    plain.add_page(0).unwrap();
    let mut metadata = PdfDict::default();
    metadata.insert(
        "Filter".into(),
        PdfObject::PdfName(PdfName::new("JPXDecode".into())),
    );
    plain
        .try_insert_object(PdfObject::PdfDict(metadata))
        .unwrap();
    assert!(
        plain.save_to_bytes().is_ok(),
        "non-stream Filter fields are not encoding declarations"
    );
}
