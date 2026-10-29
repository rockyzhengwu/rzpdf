use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        annotations::{Annotation, AnnotationKind, TextAlignment, UnicodeFreeTextFont},
        embedded_font::TrueTypeEmbeddingOptions,
    },
    parser::syntax::ParseMode,
};
const FONT: &[u8] = include_bytes!("fixtures/v03/NotoSansSC-static-subset.ttf");
fn font() -> UnicodeFreeTextFont<'static> {
    UnicodeFreeTextFont {
        bytes: FONT,
        embedding: TrueTypeEmbeddingOptions {
            postscript_name: "NotoSansSC-Regular".into(),
            cap_height: 733.,
            stem_v: 80.,
            serif: false,
        },
        omit_layout_and_auxiliary: true,
    }
}
fn annotation(text: &str) -> Annotation {
    let mut a = Annotation::new(
        AnnotationKind::FreeText {
            font_size: 16.,
            alignment: TextAlignment::Left,
        },
        [20., 20., 300., 100.],
    );
    a.contents = text.into();
    a.color = [0., 0., 0.];
    a
}
fn base() -> Document {
    let d = Document::new().unwrap();
    d.add_page(0).unwrap();
    d.add_page(1).unwrap();
    d
}
#[test]
fn explicit_font_unicode_create_save_reopen_update_without_asset() {
    let d = base();
    let a = annotation("你好，世界！Hello PDF\n你好");
    let h = d.add_unicode_freetext(0, &a, font()).unwrap();
    assert_eq!(
        d.page_annotations(0).unwrap()[0]
            .annotation
            .as_ref()
            .unwrap()
            .contents,
        a.contents
    );
    let binding = d.unicode_freetext_binding(&h).unwrap().unwrap();
    assert!(binding.repertoire.contains('你'));
    let bytes = d.save_to_bytes().unwrap();
    let next = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    let handle = next.page_annotations(0).unwrap()[0].handle.clone().unwrap();
    let mut update = annotation("Hello\n世界");
    update.kind = AnnotationKind::FreeText {
        font_size: 14.,
        alignment: TextAlignment::Right,
    };
    update.opacity = 0.6;
    next.update_unicode_freetext(0, &handle, &update, None)
        .unwrap();
    assert!(next.update_annotation(0, &handle, &update).is_err());
    let third =
        Document::from_bytes_with_mode(next.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let record = third.page_annotations(0).unwrap().remove(0);
    assert_eq!(
        record.annotation.as_ref().unwrap().contents,
        update.contents
    );
    assert!(
        third
            .unicode_freetext_binding(record.handle.as_ref().unwrap())
            .unwrap()
            .is_some()
    );
}
#[test]
fn fresh_font_expands_repertoire_and_keeps_other_page_unchanged() {
    let d = base();
    let a = annotation("你好");
    let h = d.add_unicode_freetext(0, &a, font()).unwrap();
    let other = d.add_unicode_freetext(1, &a, font()).unwrap();
    let other_before = d.resolve_handle(&other).unwrap();
    let update = annotation("新增字符");
    let commit = d.commit_revision();
    assert!(d.update_unicode_freetext(0, &h, &update, None).is_err());
    assert_eq!(d.commit_revision(), commit);
    d.update_unicode_freetext(0, &h, &update, Some(font()))
        .unwrap();
    assert_eq!(d.resolve_handle(&other).unwrap(), other_before);
}
#[test]
fn failures_publish_no_annotation_and_operation_budget_is_honored() {
    let d = base();
    for a in [
        annotation("不存在的字"),
        annotation("你好\n你好\n你好\n你好"),
        annotation("مرحبا"),
    ] {
        let commit = d.commit_revision();
        assert!(d.add_unicode_freetext(0, &a, font()).is_err());
        assert_eq!(d.commit_revision(), commit);
        assert!(d.page_annotations(0).unwrap().is_empty());
    }
    let mut tiny = annotation("你好，世界！Hello PDF");
    tiny.rect = [0., 0., 10., 10.];
    assert!(d.add_unicode_freetext(0, &tiny, font()).is_err());
    assert!(
        d.add_unicode_freetext_with_budget(
            0,
            &annotation("你好"),
            font(),
            &OperationBudget::new(1)
        )
        .is_err()
    );
    assert!(d.page_annotations(0).unwrap().is_empty());
    let mut bad = font();
    bad.bytes = b"invalid";
    assert!(d.add_unicode_freetext(0, &annotation("你好"), bad).is_err());
    d.add_unicode_freetext(0, &annotation("你好"), font())
        .unwrap();
}

#[test]
fn copied_page_shares_immutable_font_and_updates_only_target_appearance() {
    let d = base();
    let a = annotation("你好，世界！Hello PDF");
    let original = d.add_unicode_freetext(0, &a, font()).unwrap();
    d.copy_page(0, 1).unwrap();
    let copied = d.page_annotations(1).unwrap()[0].handle.clone().unwrap();
    let original_before = d.resolve_handle(&original).unwrap();
    let binding = d.unicode_freetext_binding(&original).unwrap().unwrap();
    let font_before = d.resolve_handle(&binding.font).unwrap();
    let copy_binding = d.unicode_freetext_binding(&copied).unwrap().unwrap();
    assert_eq!(binding.font.object_id(), copy_binding.font.object_id());
    d.update_unicode_freetext(1, &copied, &annotation("Hello"), None)
        .unwrap();
    assert_eq!(d.resolve_handle(&original).unwrap(), original_before);
    assert_eq!(d.resolve_handle(&binding.font).unwrap(), font_before);
    d.update_unicode_freetext(1, &copied, &annotation("新增"), Some(font()))
        .unwrap();
    assert_eq!(d.resolve_handle(&original).unwrap(), original_before);
    assert_eq!(d.resolve_handle(&binding.font).unwrap(), font_before);
}

#[test]
fn alignments_and_updates_preserve_rotated_cropped_user_unit_geometry() {
    use pdf::geom::rectangle::Rectangle;
    for angle in [0, 90, 180, 270] {
        let d = base();
        d.set_page_rotation(0, angle).unwrap();
        d.set_page_crop_box(0, Some(Rectangle::new(10., 10., 500., 500.)))
            .unwrap();
        d.set_page_user_unit(0, Some(1.5)).unwrap();
        let before = format!("{:?}", d.get_page(0).unwrap().geometry().unwrap());
        let mut a = annotation("你好\nHello PDF");
        let h = d.add_unicode_freetext(0, &a, font()).unwrap();
        for alignment in [
            TextAlignment::Left,
            TextAlignment::Center,
            TextAlignment::Right,
        ] {
            a.kind = AnnotationKind::FreeText {
                font_size: 15.,
                alignment,
            };
            d.update_unicode_freetext(0, &h, &a, None).unwrap();
            assert_eq!(
                format!("{:?}", d.get_page(0).unwrap().geometry().unwrap()),
                before
            );
            let reopened =
                Document::from_bytes_with_mode(d.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert_eq!(
                format!("{:?}", reopened.get_page(0).unwrap().geometry().unwrap()),
                before
            );
            assert_eq!(
                reopened.page_annotations(0).unwrap()[0]
                    .annotation
                    .as_ref()
                    .unwrap(),
                &a
            );
        }
    }
}

#[test]
fn annotation_permission_is_distinct_from_content_permission_and_signed_sources_refuse() {
    use pdf::{
        document::save_options::{PdfVersion, SaveLayout, SaveOptions},
        parser::options::ParseOptions,
        security::{EncryptionOptions, SaveSecurity},
    };
    for allowed in [true, false] {
        let d = base();
        let mut options = EncryptionOptions::new("user", "owner").unwrap();
        options.permissions.annotate = allowed;
        options.permissions.modify = !allowed;
        let bytes = d
            .save_to_bytes_with_layout_and_security(
                SaveOptions {
                    version: Some(PdfVersion::V2_0),
                    ..Default::default()
                },
                SaveLayout::Classic,
                SaveSecurity::Set(&options),
            )
            .unwrap();
        let protected = Document::from_bytes_with_password_and_options(
            bytes,
            "user",
            ParseOptions {
                mode: ParseMode::Strict,
                ..Default::default()
            },
        )
        .unwrap();
        let commit = protected.commit_revision();
        let result = protected.add_unicode_freetext(0, &annotation("你好"), font());
        assert_eq!(result.is_ok(), allowed, "{result:?}");
        if allowed {
            assert!(protected.replace_page_contents(0, b"q Q".to_vec()).is_err());
            let mut broken = font();
            broken.bytes = b"invalid";
            assert!(
                protected
                    .add_unicode_freetext(0, &annotation("你好"), broken)
                    .is_err()
            );
            assert!(protected.replace_page_contents(0, b"q Q".to_vec()).is_err());
        }
        if !allowed {
            assert_eq!(protected.commit_revision(), commit);
            assert!(protected.page_annotations(0).unwrap().is_empty());
        } else {
            let reopened = Document::from_bytes_with_password_and_options(
                protected.save_to_bytes().unwrap(),
                "owner",
                ParseOptions {
                    mode: ParseMode::Strict,
                    ..Default::default()
                },
            )
            .unwrap();
            assert!(
                reopened.page_annotations(0).unwrap()[0]
                    .annotation
                    .as_ref()
                    .unwrap()
                    .contents
                    .contains('你')
            );
        }
    }
    let signed = Document::from_bytes_with_mode(
        include_bytes!("fixtures/linearized/two-pages-signed.pdf").to_vec(),
        ParseMode::Strict,
    )
    .unwrap();
    assert!(
        signed
            .add_unicode_freetext(0, &annotation("你好"), font())
            .is_err()
    );
}

#[test]
fn reopened_binding_rechecks_actual_embedding_rights_instead_of_trusting_marker() {
    use pdf::{
        font::truetype::{reader::TrueTypeReader, sfnt::Sfnt},
        objects::PdfObject,
        parser::options::Limits,
    };
    let d = base();
    let h = d
        .add_unicode_freetext(0, &annotation("你好"), font())
        .unwrap();
    let binding = d.unicode_freetext_binding(&h).unwrap().unwrap();
    let f = d.resolve_handle(&binding.font).unwrap();
    let cid = f
        .as_dict()
        .unwrap()
        .get("DescendantFonts")
        .unwrap()
        .as_array()
        .unwrap()
        .get(0)
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let cid = d.resolve_object(cid).unwrap();
    let descriptor = cid
        .as_dict()
        .unwrap()
        .get("FontDescriptor")
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let descriptor = d.resolve_object(descriptor).unwrap();
    let file = descriptor
        .as_dict()
        .unwrap()
        .get("FontFile2")
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let object = d.resolve_object(file).unwrap();
    let mut data = object.as_stream().unwrap().raw_data().unwrap().to_vec();
    let sfnt =
        Sfnt::try_new_with_limits(&mut TrueTypeReader::new(&data), Limits::default()).unwrap();
    let offset = sfnt.get_table("OS/2").unwrap().offset() as usize + 8;
    data[offset..offset + 2].copy_from_slice(&2u16.to_be_bytes()); // Restricted embedding.
    d.replace_stream_decoded_data(file, data).unwrap();
    let next =
        Document::from_bytes_with_mode(d.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let handle = next.page_annotations(0).unwrap()[0].handle.clone().unwrap();
    let before = next.resolve_handle(&handle).unwrap();
    let commit = next.commit_revision();
    assert!(matches!(
        next.update_unicode_freetext(0, &handle, &annotation("你好"), None),
        Err(pdf::error::PdfError::UnsupportedFeature(message)) if message.contains("fsType prohibits embedding")
    ));
    assert_eq!(next.commit_revision(), commit);
    assert_eq!(next.resolve_handle(&handle).unwrap(), before);
    assert!(matches!(before, PdfObject::PdfDict(_)));
}
