use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_name::PdfName},
    parser::syntax::ParseMode,
};

fn font() -> PdfObject {
    let mut dict = PdfDict::default();
    for (key, value) in [
        ("Type", "Font"),
        ("Subtype", "Type1"),
        ("BaseFont", "Helvetica"),
    ] {
        dict.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))
            .unwrap();
    }
    PdfObject::PdfDict(dict)
}

#[test]
fn packed_soft_masks_preserve_alpha_after_save_and_strict_reopen() {
    use pdf::objects::{pdf_array::PdfArray, pdf_number::PdfNumber, pdf_stream::PdfStream};
    use pdf::page::{content_builder::ContentBuilder, page_object::PageObject};
    for (bits, bytes, expected) in [
        (1, vec![0x5f, 0xbf], vec![255, 0, 255, 0, 255, 0]),
        (2, vec![0x1f, 0xe7], vec![255, 170, 0, 0, 85, 170]),
        (
            4,
            vec![0x08, 0xff, 0xf8, 0x0f],
            vec![255, 119, 0, 0, 119, 255],
        ),
        (
            16,
            vec![0, 0, 0x80, 0, 0xff, 0xff, 0xff, 0xff, 0x80, 0, 0, 0],
            vec![255, 127, 0, 0, 127, 255],
        ),
    ] {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let mut mask = PdfDict::default();
        for (key, value) in [("Subtype", "Image"), ("ColorSpace", "DeviceGray")] {
            mask.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))
                .unwrap();
        }
        for (key, value) in [("Width", 3), ("Height", 2), ("BitsPerComponent", bits)] {
            mask.try_insert(
                key.into(),
                PdfObject::PdfNumber(PdfNumber::new(value as f32)),
            )
            .unwrap();
        }
        let mut decode = PdfArray::default();
        decode
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(1.0)))
            .unwrap();
        decode
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(0.0)))
            .unwrap();
        mask.try_insert("Decode".into(), PdfObject::PdfArray(decode))
            .unwrap();
        let mask_id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(mask, bytes.clone())))
            .unwrap();
        let name = doc
            .resources()
            .embed_rgb_image(0, 3, 2, vec![40; 18])
            .unwrap();
        let resources = doc.get_page(0).unwrap().resource().unwrap();
        let image_id = resources
            .get("XObject")
            .unwrap()
            .as_dict()
            .unwrap()
            .get(&name)
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let image = doc.resolve_object(image_id).unwrap();
        let image = image.as_stream().unwrap();
        let mut dict = image.dict().clone();
        dict.try_insert(
            "SMask".into(),
            PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(mask_id)),
        )
        .unwrap();
        doc.update_object(
            image_id,
            PdfObject::PdfStream(PdfStream::new(dict, image.raw_data().to_vec())),
        )
        .unwrap();
        let mut content = ContentBuilder::new();
        content
            .draw_image(&name, [30.0, 0.0, 0.0, 20.0, 0.0, 0.0])
            .unwrap();
        doc.replace_page_content_builder(0, &content).unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [doc, reopened] {
            let list = document.get_page(0).unwrap().display_list().unwrap();
            let PageObject::Image(image) = &list.objects()[0] else {
                panic!("expected image")
            };
            let rgba = image.image.rgba_buffer().unwrap();
            assert_eq!(
                rgba.chunks_exact(4).map(|p| p[3]).collect::<Vec<_>>(),
                expected
            );
            assert!(rgba.chunks_exact(4).all(|p| p[..3] == [40, 40, 40]));
            let mask = document.resolve_object(mask_id).unwrap();
            assert_eq!(mask.as_stream().unwrap().raw_data(), bytes);
        }
    }
}

#[test]
fn rgba_embedding_splits_straight_pixels_and_preserves_smask_on_save() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let name = doc
        .resources()
        .embed_rgba_image(0, 2, 1, vec![255, 0, 0, 0, 0, 128, 255, 128])
        .unwrap();
    let resources = doc.get_page(0).unwrap().resource().unwrap();
    let id = resources
        .get("XObject")
        .unwrap()
        .as_dict()
        .unwrap()
        .get(&name)
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let mut content = pdf::page::content_builder::ContentBuilder::new();
    content
        .draw_image(&name, [100.0, 0.0, 0.0, 50.0, 0.0, 0.0])
        .unwrap();
    doc.replace_page_content_builder(0, &content).unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for document in [doc, reopened] {
        assert!(document.get_page(0).unwrap().display_list().is_ok());
        let image = document.resolve_object(id).unwrap();
        let image = image.as_stream().unwrap();
        assert_eq!(image.raw_data(), &[255, 0, 0, 0, 128, 255]);
        let mask_id = image
            .dict()
            .get("SMask")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let mask = document.resolve_object(mask_id).unwrap();
        let mask = mask.as_stream().unwrap();
        assert_eq!(mask.raw_data(), &[0, 128]);
        assert_eq!(
            mask.dict()
                .get("ColorSpace")
                .unwrap()
                .as_name()
                .unwrap()
                .name(),
            "DeviceGray"
        );
    }
}

#[test]
fn rgba_second_object_failure_rolls_back_mask_and_page_resources() {
    use pdf::{
        error::PdfError,
        parser::options::{Limits, ParseOptions},
    };
    let doc = PdfDocument::new_with_options(ParseOptions {
        limits: Limits {
            max_objects: 5,
            ..Default::default()
        },
        ..Default::default()
    })
    .unwrap();
    doc.add_page(0).unwrap();
    let before = doc.get_page(0).unwrap().resource().unwrap();
    assert!(matches!(
        doc.resources().embed_rgba_image(0, 1, 1, vec![1, 2, 3, 4]),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), before);
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        5
    );
}

#[test]
fn rgba_invalid_inputs_preserve_resources_and_object_number() {
    use pdf::error::PdfError;
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let before = doc.get_page(0).unwrap().resource().unwrap();
    for (width, height, pixels) in [
        (0, 1, vec![]),
        (1, 0, vec![]),
        (1, 1, vec![1, 2, 3]),
        (1, 1, vec![0; 5]),
    ] {
        assert!(matches!(
            doc.resources().embed_rgba_image(0, width, height, pixels),
            Err(PdfError::ObjectError(_))
        ));
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), before);
    }
    assert!(
        doc.resources()
            .embed_rgba_image(1, 1, 1, vec![0; 4])
            .is_err()
    );
    assert!(matches!(
        doc.resources()
            .embed_rgba_image(0, u32::MAX, u32::MAX, vec![]),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        5
    );
}

#[test]
fn rgba_exact_pixel_byte_limits_and_low_version_rejection() {
    use pdf::{
        error::PdfError,
        parser::options::{Limits, ParseOptions},
    };
    for (raw, decoded, pixels, accepted) in [
        (4, 4, 1, true),
        (3, 4, 1, false),
        (4, 3, 1, false),
        (4, 4, 0, false),
    ] {
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_raw_stream_bytes: raw,
                max_decoded_bytes: decoded,
                max_image_pixels: pixels,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        let result = doc
            .resources()
            .embed_rgba_image(0, 1, 1, vec![20, 40, 60, 255]);
        if accepted {
            assert_eq!(result.unwrap(), "Im1");
        } else {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
            assert_eq!(
                doc.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
                5
            );
        }
    }
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let mut bytes = doc.save_to_bytes().unwrap();
    bytes[7] = b'3';
    let old = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert!(matches!(
        old.resources().embed_rgba_image(0, 1, 1, vec![0; 4]),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert_eq!(
        old.try_insert_object(PdfObject::PdfNull).unwrap().obj_num(),
        5
    );
}

#[test]
fn registration_reuses_ids_avoids_names_and_is_page_local() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    doc.add_page(1).unwrap();
    let first = doc.try_insert_object(font()).unwrap();
    let second = doc.try_insert_object(font()).unwrap();
    assert_eq!(
        doc.resources()
            .register_existing(0, ResourceKind::Font, first)
            .unwrap(),
        "F1"
    );
    assert_eq!(
        doc.resources()
            .register_existing(0, ResourceKind::Font, first)
            .unwrap(),
        "F1"
    );
    assert_eq!(
        doc.resources()
            .register_existing(0, ResourceKind::Font, second)
            .unwrap(),
        "F2"
    );
    assert!(
        doc.get_page(1)
            .unwrap()
            .resource()
            .unwrap()
            .get("Font")
            .is_none()
    );
    assert_eq!(
        doc.resources()
            .register_existing(1, ResourceKind::Font, first)
            .unwrap(),
        "F1"
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    for index in [0, 1] {
        let resources = reopened.get_page(index).unwrap().resource().unwrap();
        let entries = resources.get("Font").unwrap().as_dict().unwrap();
        assert_eq!(
            entries.get("F1").unwrap().as_reference().unwrap().id(),
            first
        );
    }
}

#[test]
fn invalid_resource_registration_leaves_page_unchanged() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let null = doc.try_insert_object(PdfObject::PdfNull).unwrap();
    let before = doc.get_page(0).unwrap().resource().unwrap();
    for (kind, id) in [
        (ResourceKind::Font, null),
        (ResourceKind::Image, null),
        (ResourceKind::Font, ObjectId::new(999, 0)),
    ] {
        assert!(doc.resources().register_existing(0, kind, id).is_err());
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), before);
    }
}

#[test]
fn shared_indirect_resource_category_is_copied_before_registration() {
    use pdf::{objects::pdf_reference::PdfReference, page::page_builder::PageBuilder};
    let doc = PdfDocument::new().unwrap();
    let first = doc.try_insert_object(font()).unwrap();
    let second = doc.try_insert_object(font()).unwrap();
    let mut fonts = PdfDict::default();
    fonts
        .try_insert(
            "F1".into(),
            PdfObject::PdfReference(PdfReference::from_id(first)),
        )
        .unwrap();
    let shared = doc
        .try_insert_object(PdfObject::PdfDict(fonts.clone()))
        .unwrap();
    let mut resources = PdfDict::default();
    resources
        .try_insert(
            "Font".into(),
            PdfObject::PdfReference(PdfReference::from_id(shared)),
        )
        .unwrap();
    for index in [0, 1] {
        doc.add_page_with_builder(index, PageBuilder::new().resources(resources.clone()))
            .unwrap();
    }
    assert_eq!(
        doc.resources()
            .register_existing(0, ResourceKind::Font, second)
            .unwrap(),
        "F2"
    );
    assert_eq!(
        doc.resolve_object(shared).unwrap(),
        PdfObject::PdfDict(fonts)
    );
    let untouched = doc.get_page(1).unwrap().resource().unwrap();
    assert_eq!(untouched, resources);
    let edited = doc.get_page(0).unwrap().resource().unwrap();
    assert_eq!(
        edited
            .get("Font")
            .unwrap()
            .as_dict()
            .unwrap()
            .get("F2")
            .unwrap()
            .as_reference()
            .unwrap()
            .id(),
        second
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(reopened.get_page(1).unwrap().resource().unwrap(), resources);
    assert!(
        reopened
            .get_page(0)
            .unwrap()
            .resource()
            .unwrap()
            .get("Font")
            .unwrap()
            .as_dict()
            .unwrap()
            .get("F2")
            .is_some()
    );
}

#[test]
fn image_registration_resolves_subtype_and_preserves_stream_on_roundtrip() {
    use pdf::objects::{pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    let subtype = doc
        .try_insert_object(PdfObject::PdfName(PdfName::new("Image".into())))
        .unwrap();
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Subtype".into(),
        PdfObject::PdfReference(PdfReference::from_id(subtype)),
    )
    .unwrap();
    dict.try_insert(
        "Type".into(),
        PdfObject::PdfName(PdfName::new("XObject".into())),
    )
    .unwrap();
    dict.try_insert(
        "ColorSpace".into(),
        PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
    )
    .unwrap();
    for (name, value) in [("Width", 1.0), ("Height", 1.0), ("BitsPerComponent", 8.0)] {
        dict.try_insert(name.into(), PdfObject::PdfNumber(PdfNumber::new(value)))
            .unwrap();
    }
    let image = doc
        .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![255, 0, 0])))
        .unwrap();
    assert_eq!(
        doc.resources()
            .register_existing(0, ResourceKind::Image, image)
            .unwrap(),
        "Im1"
    );
    assert_eq!(
        doc.resources()
            .register_existing(0, ResourceKind::Image, image)
            .unwrap(),
        "Im1"
    );
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let resources = reopened.get_page(0).unwrap().resource().unwrap();
    assert_eq!(
        resources
            .get("XObject")
            .unwrap()
            .as_dict()
            .unwrap()
            .get("Im1")
            .unwrap()
            .as_reference()
            .unwrap()
            .id(),
        image
    );
    assert_eq!(
        reopened
            .resolve_object(image)
            .unwrap()
            .as_stream()
            .unwrap()
            .raw_data(),
        &[255, 0, 0]
    );
}

#[test]
fn embedded_rgb_image_can_be_read_back_and_interpreted() {
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    assert_eq!(
        doc.resources()
            .embed_rgb_image(0, 2, 1, vec![255, 0, 0, 0, 255, 0])
            .unwrap(),
        "Im1"
    );
    let mut content = pdf::page::content_builder::ContentBuilder::new();
    content
        .draw_image("Im1", [100.0, 0.0, 0.0, 50.0, 0.0, 0.0])
        .unwrap();
    doc.replace_page_content_builder(0, &content).unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let mut page = reopened.get_page(0).unwrap();
    let image = page.get_xobject("Im1").unwrap();
    assert_eq!(
        image.as_stream().unwrap().raw_data(),
        &[255, 0, 0, 0, 255, 0]
    );
    assert!(page.display_list().is_ok());
}

#[test]
fn failed_image_embedding_restores_object_allocation() {
    use pdf::page::page_builder::PageBuilder;
    let doc = PdfDocument::new().unwrap();
    let mut resources = PdfDict::default();
    resources
        .try_insert("XObject".into(), PdfObject::PdfNull)
        .unwrap();
    doc.add_page_with_builder(0, PageBuilder::new().resources(resources.clone()))
        .unwrap();
    for (width, height, bytes) in [(0, 1, vec![]), (1, 1, vec![0]), (1, 1, vec![0, 0, 0])] {
        assert!(
            doc.resources()
                .embed_rgb_image(0, width, height, bytes)
                .is_err()
        );
        assert_eq!(doc.get_page(0).unwrap().resource().unwrap(), resources);
    }
    assert_eq!(
        doc.try_insert_object(PdfObject::PdfNull).unwrap(),
        ObjectId::new(5, 0)
    );
}

#[test]
fn standard_fonts_create_text_and_reuse_existing_page_resources() {
    use pdf::{document::resource_manager::StandardFont, page::content_builder::ContentBuilder};
    let doc = PdfDocument::new().unwrap();
    doc.add_page(0).unwrap();
    for (font, expected) in [
        (StandardFont::Helvetica, "F1"),
        (StandardFont::TimesRoman, "F2"),
        (StandardFont::Courier, "F3"),
    ] {
        assert_eq!(
            doc.resources().register_standard_font(0, font).unwrap(),
            expected
        );
        assert_eq!(
            doc.resources().register_standard_font(0, font).unwrap(),
            expected
        );
    }
    let mut builder = ContentBuilder::new();
    builder.begin_text();
    builder.try_set_font("F1", 12.0).unwrap();
    builder.move_text(20.0, 50.0);
    builder.show_encoded_text(b"Hello PDF").unwrap();
    builder.end_text();
    doc.replace_page_content_builder(0, &builder).unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    assert_eq!(
        reopened
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap(),
        "F1"
    );
    assert!(reopened.get_page(0).unwrap().display_list().is_ok());
}

#[test]
fn standard_font_encoding_preserves_unicode_or_returns_an_error() {
    use pdf::{document::resource_manager::StandardFont, font::encoding::FontEncoding};
    let font = StandardFont::Helvetica;
    assert_eq!(
        font.encode_text("Café € — Œ").unwrap(),
        vec![67, 97, 102, 233, 32, 128, 32, 151, 32, 140]
    );
    for text in ["中文", "🙂", "line\nfeed", "\0", "\u{a0}", "\u{ad}"] {
        assert!(font.encode_text(text).is_err());
    }
    for code in 0..=255 {
        if let Some(character) = FontEncoding::WinAnsi
            .unicode_from_charcode(code)
            .and_then(char::from_u32)
        {
            if !character.is_control() {
                let encoded = font.encode_text(&character.to_string()).unwrap();
                assert_eq!(encoded.len(), 1);
                assert_eq!(
                    FontEncoding::WinAnsi.unicode_from_charcode(encoded[0]),
                    Some(character as u32)
                );
            }
        }
    }
}

#[test]
fn standard_font_encoding_limits_count_characters_not_utf8_bytes() {
    use pdf::{document::resource_manager::StandardFont, error::PdfError, parser::options::Limits};
    let font = StandardFont::Courier;
    let limits = Limits {
        max_text_chars: 2,
        max_decoded_bytes: 2,
        ..Default::default()
    };
    assert_eq!(
        font.encode_text_with_limits("é€", limits).unwrap(),
        vec![233, 128]
    );
    for restricted in [
        Limits {
            max_text_chars: 1,
            ..limits
        },
        Limits {
            max_decoded_bytes: 1,
            ..limits
        },
    ] {
        assert!(matches!(
            font.encode_text_with_limits("é€", restricted),
            Err(PdfError::ResourceLimit(_))
        ));
    }
    let zero = Limits {
        max_text_chars: 0,
        max_decoded_bytes: 0,
        ..limits
    };
    assert!(font.encode_text_with_limits("", zero).unwrap().is_empty());
    assert!(matches!(
        font.encode_text_with_limits("A", zero),
        Err(PdfError::ResourceLimit(_))
    ));
}
