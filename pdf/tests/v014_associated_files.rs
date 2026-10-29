use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        associated_files::{AssociatedFileDefinition, AssociatedFileRelationship as Relation},
        save_options::{PdfVersion, SaveLayout, SaveOptions, StreamCompression},
    },
    error::PdfError,
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream,
        pdf_string::PdfString,
    },
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};
fn pool() -> OperationBudget {
    OperationBudget::new(128 * 1024 * 1024)
}
fn doc() -> Document {
    let d = Document::new_with_version(PdfVersion::V2_0).unwrap();
    d.add_page(0).unwrap();
    d.add_page(1).unwrap();
    d
}
fn definition() -> AssociatedFileDefinition {
    let mut d =
        AssociatedFileDefinition::new("源数据.bin", "application/octet-stream", Relation::Data);
    d.description = Some("二进制 source".into());
    d
}
fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn name(s: &str) -> PdfObject {
    PdfObject::PdfName(PdfName::new(s.into()))
}
fn text(s: &str) -> PdfObject {
    PdfObject::PdfString(PdfString::new(s.as_bytes().to_vec(), false))
}
fn array(items: &[PdfObject]) -> PdfObject {
    let mut a = PdfArray::default();
    for item in items {
        a.try_add_obj(item.clone()).unwrap();
    }
    PdfObject::PdfArray(a)
}
fn owner_id(d: &Document, page: Option<u32>) -> ObjectId {
    match page {
        None => {
            let bytes = d.save_to_bytes().unwrap();
            let reader =
                pdf::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(bytes)).unwrap();
            let mut parser =
                pdf::parser::syntax::SyntaxParser::with_mode(reader, ParseMode::Strict);
            parser.read_header().unwrap();
            pdf::parser::parser::parse_xref(&mut parser)
                .unwrap()
                .trailer()
                .get("Root")
                .unwrap()
                .as_reference()
                .unwrap()
                .id()
        }
        Some(index) => d
            .associated_file_owner(Some(index), &pool())
            .unwrap()
            .page()
            .unwrap()
            .object_id(),
    }
}
// Fixture-only raw construction. Public AF binding is a separate phase-B task.
fn set_af(d: &Document, page: Option<u32>, value: PdfObject) {
    let id = owner_id(d, page);
    let mut dict = d.resolve_object(id).unwrap().into_dict().unwrap();
    dict.try_insert("AF".into(), value).unwrap();
    d.update_object(id, PdfObject::PdfDict(dict)).unwrap();
}
fn read_data(d: &Document, page: Option<u32>, b: &OperationBudget) -> Vec<u8> {
    let owner = d.associated_file_owner(page, b).unwrap();
    let list = d.associated_files(&owner, b).unwrap();
    d.associated_file_data(&list, 0, b)
        .unwrap()
        .as_slice()
        .to_vec()
}

#[test]
fn create_unicode_five_relationships_and_three_layouts() {
    let d = doc();
    let b = pool();
    let mut specs = Vec::new();
    for relation in [
        Relation::Source,
        Relation::Data,
        Relation::Alternative,
        Relation::Supplement,
        Relation::Unspecified,
    ] {
        let mut def = definition();
        def.relationship = relation;
        let h = d.create_associated_file(&def, b"\0\xffhello", &b).unwrap();
        let spec = d.resolve_handle(&h).unwrap();
        let spec = spec.as_dict().unwrap();
        assert_eq!(
            spec.get("AFRelationship")
                .unwrap()
                .as_name()
                .unwrap()
                .name(),
            relation.name()
        );
        assert_eq!(
            spec.get("F")
                .unwrap()
                .as_string()
                .unwrap()
                .text_content()
                .unwrap(),
            "attachment.bin"
        );
        let ef = spec.get("EF").unwrap().as_dict().unwrap();
        assert_eq!(ef.get("F"), ef.get("UF"));
        specs.push(reference(h.object_id()));
    }
    assert!(d.attachment_names().unwrap().is_empty());
    set_af(&d, None, array(&specs));
    set_af(&d, Some(0), array(&specs[..1]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let list = d.associated_files(&owner, &b).unwrap();
    assert_eq!(list.files().len(), 5);
    assert_eq!(list.files()[0].name(), "源数据.bin");
    assert_eq!(list.files()[0].description(), Some("二进制 source"));
    assert_eq!(
        list.files()[0].mime_type(),
        Some("application/octet-stream")
    );
    assert_eq!(list.files()[0].size(), None);
    assert_eq!(list.files()[0].relationship(), "Source");
    assert_eq!(
        d.associated_file_data(&list, 0, &b).unwrap().as_slice(),
        b"\0\xffhello"
    );
    drop(list);
    assert_eq!(b.live_bytes(), 0);
    for layout in [
        SaveLayout::Classic,
        SaveLayout::XrefStream,
        SaveLayout::XrefStreamWithObjectStreams,
    ] {
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let out = d
                .save_to_bytes_with_layout(
                    SaveOptions {
                        compression,
                        ..Default::default()
                    },
                    layout,
                )
                .unwrap();
            let r = Document::from_bytes_with_mode(out, ParseMode::Strict).unwrap();
            assert_eq!(read_data(&r, None, &b), b"\0\xffhello");
            assert_eq!(read_data(&r, Some(0), &b), b"\0\xffhello");
            let owner = r.associated_file_owner(Some(1), &b).unwrap();
            assert!(r.associated_files(&owner, &b).unwrap().files().is_empty());
        }
    }
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn direct_filespec_indirect_array_unknown_relation_and_external_file() {
    let d = doc();
    let b = pool();
    let h = d
        .create_associated_file(&definition(), b"payload", &b)
        .unwrap();
    let mut spec = d.resolve_handle(&h).unwrap().into_dict().unwrap();
    spec.try_insert("AFRelationship".into(), name("CustomRelationship"))
        .unwrap();
    let a = d
        .try_insert_object(array(&[PdfObject::PdfDict(spec)]))
        .unwrap();
    set_af(&d, None, reference(a));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let list = d.associated_files(&owner, &b).unwrap();
    assert_eq!(list.files()[0].relationship(), "CustomRelationship");
    assert!(list.files()[0].filespec().is_none());
    assert_eq!(
        d.associated_file_data(&list, 0, &b).unwrap().as_slice(),
        b"payload"
    );
    drop(list);
    let mut spec = PdfDict::default();
    spec.try_insert("F".into(), text("/definitely/not/opened/file.bin"))
        .unwrap();
    spec.try_insert("AFRelationship".into(), name("Unspecified"))
        .unwrap();
    set_af(&d, None, array(&[PdfObject::PdfDict(spec)]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let list = d.associated_files(&owner, &b).unwrap();
    assert!(!list.files()[0].is_embedded());
    assert!(matches!(
        d.associated_file_data(&list, 0, &b),
        Err(PdfError::UnsupportedFeature(_))
    ));
    assert!(d.associated_file_data(&list, 1, &b).is_err());
    drop(list);
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn foreign_and_stale_targets_and_snapshots_refuse() {
    let d = doc();
    let b = pool();
    let foreign = doc();
    let target = d.associated_file_owner(Some(0), &b).unwrap();
    let list = d.associated_files(&target, &b).unwrap();
    assert!(foreign.associated_files(&target, &b).is_err());
    assert!(foreign.associated_file_data(&list, 0, &b).is_err());
    let stamp = d.stamp();
    d.create_associated_file(&definition(), b"", &b).unwrap();
    assert!(!d.is_stamp_current(&stamp));
    assert!(d.associated_files(&target, &b).is_err());
    assert!(d.associated_file_data(&list, 0, &b).is_err());
    assert!(d.associated_file_owner(Some(2), &b).is_err());
    let target = d.associated_file_owner(None, &b).unwrap();
    assert!(d.associated_files(&target, &b).unwrap().files().is_empty());
    drop(list);
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn malformed_graphs_fail_without_editing() {
    let d = doc();
    let b = pool();
    let h = d.create_associated_file(&definition(), b"abc", &b).unwrap();
    for value in [
        name("NotAnArray"),
        array(&[PdfObject::PdfNull]),
        array(&[reference(h.object_id()), reference(h.object_id())]),
        array(&[reference(ObjectId::new(99999, 0))]),
    ] {
        set_af(&d, None, value);
        let owner = d.associated_file_owner(None, &b).unwrap();
        let stamp = d.stamp();
        assert!(d.associated_files(&owner, &b).is_err());
        assert!(d.is_stamp_current(&stamp));
        assert_eq!(b.live_bytes(), 0);
    }
    let cycle = d.try_insert_object(PdfObject::PdfNull).unwrap();
    d.update_object(cycle, reference(cycle)).unwrap();
    set_af(&d, None, reference(cycle));
    let owner = d.associated_file_owner(None, &b).unwrap();
    assert!(d.associated_files(&owner, &b).is_err());
    assert_eq!(b.live_bytes(), 0);
    set_af(&d, None, PdfObject::PdfNull);
    let owner = d.associated_file_owner(None, &b).unwrap();
    assert!(d.associated_files(&owner, &b).unwrap().files().is_empty());
}

#[test]
fn list_never_decodes_payload_and_data_retains_quota() {
    let d = doc();
    let b = pool();
    let h = d.create_associated_file(&definition(), b"abc", &b).unwrap();
    let spec = d.resolve_handle(&h).unwrap().into_dict().unwrap();
    let id = spec
        .get("EF")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("F")
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let stream = d.resolve_object(id).unwrap().into_stream().unwrap();
    let (mut dict, raw) = stream.into_raw_parts().unwrap();
    dict.try_insert("Filter".into(), name("NotARealFilter"))
        .unwrap();
    d.update_object(id, PdfObject::PdfStream(PdfStream::new(dict, raw)))
        .unwrap();
    set_af(&d, None, array(&[reference(h.object_id())]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let list = d.associated_files(&owner, &b).unwrap();
    assert!(list.files()[0].is_embedded());
    assert!(d.associated_file_data(&list, 0, &b).is_err());
    drop(list);
    assert_eq!(b.live_bytes(), 0);
    let h = d
        .create_associated_file(&definition(), b"123456789", &b)
        .unwrap();
    set_af(&d, None, array(&[reference(h.object_id())]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let list = d.associated_files(&owner, &b).unwrap();
    let payload = d.associated_file_data(&list, 0, &b).unwrap();
    drop(list);
    assert!(b.live_bytes() >= 9);
    assert_eq!(payload.as_slice(), b"123456789");
    drop(payload);
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn mismatched_ef_mime_type_and_required_fields_refuse() {
    let d = doc();
    let b = pool();
    let a = d.create_associated_file(&definition(), b"A", &b).unwrap();
    let z = d.create_associated_file(&definition(), b"Z", &b).unwrap();
    let original = d.resolve_handle(&a).unwrap().into_dict().unwrap();
    let other = d.resolve_handle(&z).unwrap().into_dict().unwrap();
    let other_stream = other
        .get("EF")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("F")
        .unwrap();
    for kind in 0..5 {
        let mut spec = original.clone();
        match kind {
            0 => {
                spec.remove("Type");
            }
            1 => {
                spec.remove("AFRelationship");
            }
            2 => {
                spec.remove("UF");
            }
            3 => {
                let mut ef = spec.get("EF").unwrap().as_dict().unwrap().clone();
                ef.try_insert("UF".into(), other_stream.clone()).unwrap();
                spec.try_insert("EF".into(), PdfObject::PdfDict(ef))
                    .unwrap();
            }
            _ => {
                spec.try_insert("Type".into(), name("Page")).unwrap();
            }
        }
        set_af(&d, None, array(&[PdfObject::PdfDict(spec)]));
        let owner = d.associated_file_owner(None, &b).unwrap();
        assert!(d.associated_files(&owner, &b).is_err(), "kind {kind}");
        assert_eq!(b.live_bytes(), 0);
    }
    let id = original
        .get("EF")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("F")
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let (mut dict, raw) = d
        .resolve_object(id)
        .unwrap()
        .into_stream()
        .unwrap()
        .into_raw_parts()
        .unwrap();
    dict.remove("Subtype");
    d.update_object(id, PdfObject::PdfStream(PdfStream::new(dict, raw)))
        .unwrap();
    set_af(&d, None, array(&[reference(a.object_id())]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    assert!(d.associated_files(&owner, &b).is_err());
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn legacy_attachment_clone_isolated_and_name_removal_does_not_unlink_af() {
    let d = doc();
    let b = pool();
    d.add_attachment("original.bin", b"\0\xffold".to_vec())
        .unwrap();
    let old = d.attachment_filespec("original.bin").unwrap().unwrap();
    let mut spec = d.resolve_handle(&old).unwrap().into_dict().unwrap();
    spec.try_insert("Private".into(), text("opaque")).unwrap();
    d.update_handle(&old, PdfObject::PdfDict(spec)).unwrap();
    let before = d.resolve_handle(&old).unwrap();
    let new = d.clone_associated_file(&old, &definition(), &b).unwrap();
    assert_ne!(old, new);
    assert_eq!(d.resolve_handle(&old).unwrap(), before);
    assert_eq!(
        d.attachment_data("original.bin").unwrap().unwrap(),
        b"\0\xffold"
    );
    let created = d.resolve_handle(&new).unwrap();
    assert_eq!(
        created.as_dict().unwrap().get("Private"),
        Some(&text("opaque"))
    );
    set_af(&d, None, array(&[reference(new.object_id())]));
    d.remove_attachment("original.bin").unwrap();
    assert_eq!(read_data(&d, None, &b), b"\0\xffold");
    assert_eq!(d.resolve_handle(&old).unwrap(), before);
    assert_eq!(b.live_bytes(), 0);
    let foreign = doc();
    assert!(
        foreign
            .clone_associated_file(&old, &definition(), &b)
            .is_err()
    );
}

#[test]
fn invalid_definitions_and_low_version_do_not_publish() {
    let d = doc();
    let b = pool();
    for mime in [
        "plain",
        "text/",
        "/plain",
        "text/plain; charset=utf-8",
        "text/pl#ain",
        "text/*",
        " text/plain",
        "text/pl=ain",
        "text/plain/extra",
    ] {
        let mut def = definition();
        def.mime_type = mime.into();
        let before = d.save_to_bytes().unwrap();
        let stamp = d.stamp();
        assert!(
            d.create_associated_file(&def, b"abc", &b).is_err(),
            "{mime}"
        );
        assert!(d.is_stamp_current(&stamp));
        assert_eq!(d.save_to_bytes().unwrap(), before);
    }
    for filename in ["", "bad\0name", "bad\nname"] {
        let mut def = definition();
        def.name = filename.into();
        assert!(d.create_associated_file(&def, b"", &b).is_err());
    }
    let old = Document::new_with_version(PdfVersion::V1_7).unwrap();
    assert!(old.create_associated_file(&definition(), b"", &b).is_err());
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn exact_create_and_list_budget_thresholds_release_and_retry() {
    let measure = doc();
    let probe = pool();
    measure
        .create_associated_file(&definition(), b"abc", &probe)
        .unwrap();
    let required = probe.peak_bytes();
    assert_eq!(probe.live_bytes(), 0);
    for maximum in [required - 1, required] {
        let d = doc();
        let b = OperationBudget::new(maximum);
        let before = d.save_to_bytes().unwrap();
        let result = d.create_associated_file(&definition(), b"abc", &b);
        assert_eq!(
            result.is_ok(),
            maximum == required,
            "maximum={maximum}, required={required}, result={result:?}"
        );
        if result.is_err() {
            assert_eq!(d.save_to_bytes().unwrap(), before);
        }
        assert_eq!(b.live_bytes(), 0);
    }
    let d = doc();
    let b = pool();
    let h = d.create_associated_file(&definition(), b"abc", &b).unwrap();
    set_af(&d, None, array(&[reference(h.object_id())]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let probe = pool();
    let snapshot = d.associated_files(&owner, &probe).unwrap();
    let required = probe.peak_bytes();
    assert!(probe.live_bytes() > 0);
    drop(snapshot);
    assert_eq!(probe.live_bytes(), 0);
    let exact = OperationBudget::new(required);
    let neighbor = exact.reserve(1).unwrap();
    assert!(d.associated_files(&owner, &exact).is_err());
    assert_eq!(exact.live_bytes(), 1);
    drop(neighbor);
    let snapshot = d.associated_files(&owner, &exact).unwrap();
    drop(snapshot);
    assert_eq!(exact.live_bytes(), 0);
    let blocked = OperationBudget::new(0);
    assert!(
        d.create_associated_file(&definition(), b"abc", &blocked)
            .is_err()
    );
    assert_eq!(blocked.live_bytes(), 0);
}

#[test]
fn parameters_and_payload_limits_are_checked() {
    let d = doc();
    let b = pool();
    let h = d.create_associated_file(&definition(), b"abc", &b).unwrap();
    let spec = d.resolve_handle(&h).unwrap();
    let id = spec
        .as_dict()
        .unwrap()
        .get("EF")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("F")
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let (mut dict, raw) = d
        .resolve_object(id)
        .unwrap()
        .into_stream()
        .unwrap()
        .into_raw_parts()
        .unwrap();
    let mut params = PdfDict::default();
    params
        .try_insert("Size".into(), PdfObject::PdfNumber(PdfNumber::new(4.0)))
        .unwrap();
    params
        .try_insert("ModDate".into(), text("D:20200101000000Z"))
        .unwrap();
    dict.try_insert("Params".into(), PdfObject::PdfDict(params))
        .unwrap();
    d.update_object(id, PdfObject::PdfStream(PdfStream::new(dict, raw)))
        .unwrap();
    set_af(&d, None, array(&[reference(h.object_id())]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let list = d.associated_files(&owner, &b).unwrap();
    assert_eq!(list.files()[0].size(), Some(4));
    assert_eq!(
        list.files()[0].modification_date(),
        Some("D:20200101000000Z")
    );
    assert!(d.associated_file_data(&list, 0, &b).is_err());
    assert!(d.clone_associated_file(&h, &definition(), &b).is_err());
    drop(list);
    assert_eq!(b.live_bytes(), 0);
    let limited = Document::new_with_version_and_options(
        PdfVersion::V2_0,
        ParseOptions {
            limits: Limits {
                max_raw_stream_bytes: 2,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    assert!(matches!(
        limited.create_associated_file(&definition(), b"abc", &b),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn signature_and_r6_permissions_are_not_bypassed() {
    use pdf::security::{EncryptionOptions, SaveSecurity};
    let d = doc();
    let b = pool();
    let mut sig = PdfDict::default();
    sig.try_insert("Type".into(), name("Sig")).unwrap();
    d.try_insert_object(PdfObject::PdfDict(sig)).unwrap();
    let signed =
        Document::from_bytes_with_mode(d.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let stamp = signed.stamp();
    assert!(
        signed
            .create_associated_file(&definition(), b"no", &b)
            .is_err()
    );
    assert!(signed.is_stamp_current(&stamp));
    let d = doc();
    let mut encryption = EncryptionOptions::new("user", "owner").unwrap();
    encryption.permissions.modify = false;
    let bytes = d
        .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&encryption))
        .unwrap();
    let user = Document::from_bytes_with_password(bytes.clone(), "user").unwrap();
    let stamp = user.stamp();
    assert!(
        user.create_associated_file(&definition(), b"no", &b)
            .is_err()
    );
    assert!(user.is_stamp_current(&stamp));
    let owner = Document::from_bytes_with_password(bytes, "owner").unwrap();
    owner
        .create_associated_file(&definition(), b"yes", &b)
        .unwrap();
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn exact_entry_and_utf16_text_limits_have_adjacent_refusals() {
    let d = doc();
    let b = pool();
    let mut def = definition();
    def.name = "a".repeat(32767);
    let h = d.create_associated_file(&def, b"", &b).unwrap();
    set_af(&d, None, array(&[reference(h.object_id())]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let list = d.associated_files(&owner, &b).unwrap();
    assert_eq!(list.files()[0].name(), def.name);
    drop(list);
    def.name.push('a');
    assert!(matches!(
        d.create_associated_file(&def, b"", &b),
        Err(PdfError::ResourceLimit(_))
    ));
    let h = d.create_associated_file(&definition(), b"", &b).unwrap();
    let small = d.resolve_handle(&h).unwrap();
    set_af(&d, None, array(&vec![small.clone(); 1024]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let list = d.associated_files(&owner, &b).unwrap();
    assert_eq!(list.files().len(), 1024);
    drop(list);
    set_af(&d, None, array(&vec![small; 1025]));
    let owner = d.associated_file_owner(None, &b).unwrap();
    match d.associated_files(&owner, &b) {
        Err(PdfError::ResourceLimit(message)) => assert_eq!(message, "AF entries"),
        other => panic!("expected the AF entry-count boundary, got {other:?}"),
    }
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn generated_token_limits_roll_back_after_stream_insertion() {
    // EmbeddedFile and MIME fit, but the generated AFRelationship key does not.
    // This exercises rollback after the first object has actually been staged.
    let d = Document::new_with_version_and_options(
        PdfVersion::V2_0,
        ParseOptions {
            limits: Limits {
                max_token_bytes: 12,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    let b = pool();
    let before = d.save_to_bytes().unwrap();
    let def = AssociatedFileDefinition::new("a", "text/plain", Relation::Source);
    assert!(matches!(
        d.create_associated_file(&def, b"abc", &b),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(d.save_to_bytes().unwrap(), before);
    assert_eq!(b.live_bytes(), 0);
    // The same pool remains valid for an operation on a document with normal limits.
    doc().create_associated_file(&def, b"abc", &b).unwrap();
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn binding_unbinding_are_local_and_exact_noops_preserve_stamps() {
    let d = doc();
    let b = pool();
    let file = d
        .create_associated_file(&definition(), b"shared", &b)
        .unwrap();
    for page in [None, Some(0), Some(1)] {
        let owner = d.associated_file_owner(page, &b).unwrap();
        assert!(d.bind_associated_file(&owner, &file, &b).unwrap());
        assert!(d.bind_associated_file(&owner, &file, &b).is_err()); // stale
        let owner = d.associated_file_owner(page, &b).unwrap();
        let before = d.save_to_bytes().unwrap();
        let stamp = d.stamp();
        assert!(!d.bind_associated_file(&owner, &file, &b).unwrap());
        assert!(d.is_stamp_current(&stamp));
        assert_eq!(before, d.save_to_bytes().unwrap());
    }
    let original = d.resolve_handle(&file).unwrap();
    let owner = d.associated_file_owner(Some(0), &b).unwrap();
    assert!(d.unbind_associated_file(&owner, &file, &b).unwrap());
    let owner = d.associated_file_owner(Some(0), &b).unwrap();
    let stamp = d.stamp();
    assert!(!d.unbind_associated_file(&owner, &file, &b).unwrap());
    assert!(d.is_stamp_current(&stamp));
    assert!(
        !d.resolve_handle(owner.page().unwrap())
            .unwrap()
            .as_dict()
            .unwrap()
            .contains_key("AF")
    );
    assert_eq!(read_data(&d, None, &b), b"shared");
    assert_eq!(read_data(&d, Some(1), &b), b"shared");
    assert_eq!(d.resolve_handle(&file).unwrap(), original);
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn indirect_array_aliases_and_private_owner_fields_are_not_changed() {
    let d = doc();
    let b = pool();
    let file = d.create_associated_file(&definition(), b"old", &b).unwrap();
    let added = d.create_associated_file(&definition(), b"new", &b).unwrap();
    let original = array(&[reference(file.object_id())]);
    let shared = d.try_insert_object(original.clone()).unwrap();
    for page in [None, Some(0), Some(1)] {
        set_af(&d, page, reference(shared));
    }
    let id = owner_id(&d, Some(0));
    let mut page = d.resolve_object(id).unwrap().into_dict().unwrap();
    page.try_insert("Private".into(), text("keep")).unwrap();
    d.update_object(id, PdfObject::PdfDict(page)).unwrap();
    let owner = d.associated_file_owner(Some(0), &b).unwrap();
    d.bind_associated_file(&owner, &added, &b).unwrap();
    assert_eq!(d.resolve_object(shared).unwrap(), original);
    let page = d.resolve_object(id).unwrap();
    assert_eq!(page.as_dict().unwrap().get("Private"), Some(&text("keep")));
    let owner = d.associated_file_owner(Some(0), &b).unwrap();
    assert_eq!(d.associated_files(&owner, &b).unwrap().files().len(), 2);
    for page in [None, Some(1)] {
        let owner = d.associated_file_owner(page, &b).unwrap();
        assert_eq!(d.associated_files(&owner, &b).unwrap().files().len(), 1);
    }
    let owner = d.associated_file_owner(Some(0), &b).unwrap();
    d.unbind_associated_file(&owner, &file, &b).unwrap();
    assert_eq!(read_data(&d, Some(0), &b), b"new");
    assert_eq!(d.resolve_object(shared).unwrap(), original);
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn attachment_annotation_and_af_lifetimes_are_independent_and_legacy_selection_stays() {
    use pdf::document::annotations::{Annotation, AnnotationKind, AnnotationRemoval};
    let d = doc();
    let b = pool();
    d.add_attachment("named", b"F payload".to_vec()).unwrap();
    let old = d.attachment_filespec("named").unwrap().unwrap();
    let cloned = d.clone_associated_file(&old, &definition(), &b).unwrap();
    let annotation = d
        .add_annotation(
            0,
            &Annotation::new(
                AnnotationKind::FileAttachment {
                    file: cloned.clone(),
                    icon: "PushPin".into(),
                },
                [10., 10., 30., 30.],
            ),
        )
        .unwrap();
    let owner = d.associated_file_owner(None, &b).unwrap();
    d.bind_associated_file(&owner, &cloned, &b).unwrap();
    d.remove_attachment("named").unwrap();
    assert_eq!(d.annotation_file_data(&annotation).unwrap(), b"F payload");
    assert_eq!(read_data(&d, None, &b), b"F payload");
    let owner = d.associated_file_owner(None, &b).unwrap();
    d.unbind_associated_file(&owner, &cloned, &b).unwrap();
    assert_eq!(d.annotation_file_data(&annotation).unwrap(), b"F payload");
    let owner = d.associated_file_owner(None, &b).unwrap();
    d.bind_associated_file(&owner, &cloned, &b).unwrap();
    d.remove_annotation(0, &annotation, AnnotationRemoval::RejectRelated)
        .unwrap();
    assert_eq!(read_data(&d, None, &b), b"F payload");
    d.add_attachment("dual", b"F".to_vec()).unwrap();
    let dual = d.attachment_filespec("dual").unwrap().unwrap();
    let mut dict = PdfDict::default();
    dict.try_insert("Type".into(), name("EmbeddedFile"))
        .unwrap();
    let stream = d
        .try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(dict, b"UF".to_vec()).unwrap(),
        ))
        .unwrap();
    let mut spec = d.resolve_handle(&dual).unwrap().into_dict().unwrap();
    let mut ef = spec.get("EF").unwrap().as_dict().unwrap().clone();
    ef.try_insert("UF".into(), reference(stream)).unwrap();
    spec.try_insert("EF".into(), PdfObject::PdfDict(ef))
        .unwrap();
    d.update_handle(&dual, PdfObject::PdfDict(spec)).unwrap();
    let ann = d
        .add_annotation(
            0,
            &Annotation::new(
                AnnotationKind::FileAttachment {
                    file: dual.clone(),
                    icon: "PushPin".into(),
                },
                [10., 10., 30., 30.],
            ),
        )
        .unwrap();
    assert_eq!(d.attachment_data("dual").unwrap().unwrap(), b"F");
    assert_eq!(d.annotation_file_data(&ann).unwrap(), b"UF");
    assert!(d.clone_associated_file(&dual, &definition(), &b).is_err());
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn page_operations_keep_af_sharing_and_do_not_merge_catalog_names() {
    let d = doc();
    let b = pool();
    let file = d
        .create_associated_file(&definition(), b"page", &b)
        .unwrap();
    for page in [None, Some(0), Some(1)] {
        let owner = d.associated_file_owner(page, &b).unwrap();
        d.bind_associated_file(&owner, &file, &b).unwrap();
    }
    d.copy_page(0, 2).unwrap();
    assert_eq!(read_data(&d, Some(2), &b), b"page");
    d.reorder_pages(&[2, 1, 0]).unwrap();
    d.delete_page(1).unwrap();
    assert_eq!(read_data(&d, None, &b), b"page");
    let target = Document::new_with_version(PdfVersion::V2_0).unwrap();
    target.import_pages_from(&d, &[1, 0], 0).unwrap();
    let owner = target.associated_file_owner(None, &b).unwrap();
    assert!(
        target
            .associated_files(&owner, &b)
            .unwrap()
            .files()
            .is_empty()
    );
    let mut ids = Vec::new();
    for page in [0, 1] {
        let owner = target.associated_file_owner(Some(page), &b).unwrap();
        let list = target.associated_files(&owner, &b).unwrap();
        ids.push(list.files()[0].filespec().unwrap().clone());
        assert_eq!(
            target
                .associated_file_data(&list, 0, &b)
                .unwrap()
                .as_slice(),
            b"page"
        );
    }
    assert_eq!(ids[0], ids[1]);
    assert_ne!(ids[0].document_identity(), file.document_identity());
    assert!(target.attachment_names().unwrap().is_empty());
    let owner = target.associated_file_owner(Some(0), &b).unwrap();
    target.unbind_associated_file(&owner, &ids[0], &b).unwrap();
    assert_eq!(read_data(&target, Some(1), &b), b"page");
    let extracted = d.extract_pages(&[0]).unwrap();
    let owner = extracted.associated_file_owner(Some(0), &b).unwrap();
    let list = extracted.associated_files(&owner, &b).unwrap();
    assert_eq!(
        extracted
            .associated_file_data(&list, 0, &b)
            .unwrap()
            .as_slice(),
        b"page"
    );
    drop(list);
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn unknown_page_backreferences_refuse_atomically_and_can_retry() {
    let d = doc();
    let b = pool();
    let file = d
        .create_associated_file(&definition(), b"data", &b)
        .unwrap();
    let mut spec = d.resolve_handle(&file).unwrap().into_dict().unwrap();
    spec.try_insert("PrivateOwner".into(), reference(owner_id(&d, Some(1))))
        .unwrap();
    d.update_handle(&file, PdfObject::PdfDict(spec)).unwrap();
    for page in [None, Some(0)] {
        let owner = d.associated_file_owner(page, &b).unwrap();
        d.bind_associated_file(&owner, &file, &b).unwrap();
    }
    let before = d.save_to_bytes().unwrap();
    assert!(d.delete_page(1).is_err());
    assert_eq!(d.save_to_bytes().unwrap(), before);
    let target = Document::new_with_version(PdfVersion::V2_0).unwrap();
    let before = target.save_to_bytes().unwrap();
    assert!(target.import_pages_from(&d, &[0], 0).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
    target.import_pages_from(&d, &[0, 1], 0).unwrap();
    assert_eq!(read_data(&target, Some(0), &b), b"data");
}

#[test]
fn bind_budget_exact_threshold_and_foreign_refusals_release() {
    // Measure/retry the same graph, not independently seeded HashMap builders.
    // The outer savepoint is test setup, outside the measured API operation.
    fn trial(
        d: &Document,
        file: &pdf::document::identity::ObjectHandle,
        b: &OperationBudget,
    ) -> pdf::error::PdfResult<bool> {
        let mut result = None;
        let rolled: pdf::error::PdfResult<()> = d.edit_transaction(|doc| {
            let owner = doc.associated_file_owner(None, b)?;
            result = Some(doc.bind_associated_file(&owner, file, b));
            Err(PdfError::DocumentError("test rollback".into()))
        });
        assert!(rolled.is_err());
        result.unwrap()
    }
    let d = doc();
    let file = d
        .create_associated_file(&definition(), b"data", &pool())
        .unwrap();
    let before = d.save_to_bytes().unwrap();
    trial(&d, &file, &pool()).unwrap();
    let probe = pool();
    trial(&d, &file, &probe).unwrap();
    let required = probe.peak_bytes();
    assert_eq!(probe.live_bytes(), 0);
    for maximum in [required - 1, required] {
        let b = OperationBudget::new(maximum);
        let result = trial(&d, &file, &b);
        assert_eq!(
            result.is_ok(),
            maximum == required,
            "maximum={maximum}, required={required}, result={result:?}"
        );
        assert_eq!(before, d.save_to_bytes().unwrap());
        assert_eq!(b.live_bytes(), 0);
    }
    let foreign = doc();
    let b = pool();
    let owner = foreign.associated_file_owner(None, &b).unwrap();
    assert!(foreign.bind_associated_file(&owner, &file, &b).is_err());
    assert!(foreign.unbind_associated_file(&owner, &file, &b).is_err());
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn binding_rejects_unprepared_legacy_unknown_relationship_and_signed_sources() {
    let d = doc();
    let b = pool();
    d.add_attachment("legacy", b"data".to_vec()).unwrap();
    let legacy = d.attachment_filespec("legacy").unwrap().unwrap();
    let owner = d.associated_file_owner(None, &b).unwrap();
    let before = d.save_to_bytes().unwrap();
    assert!(d.bind_associated_file(&owner, &legacy, &b).is_err());
    assert_eq!(before, d.save_to_bytes().unwrap());
    let file = d
        .create_associated_file(&definition(), b"data", &b)
        .unwrap();
    let mut spec = d.resolve_handle(&file).unwrap().into_dict().unwrap();
    spec.try_insert("AFRelationship".into(), name("Schema"))
        .unwrap();
    d.update_handle(&file, PdfObject::PdfDict(spec)).unwrap();
    let owner = d.associated_file_owner(None, &b).unwrap();
    assert!(d.bind_associated_file(&owner, &file, &b).is_err());
    let mut sig = PdfDict::default();
    sig.try_insert("Type".into(), name("Sig")).unwrap();
    d.try_insert_object(PdfObject::PdfDict(sig)).unwrap();
    let signed =
        Document::from_bytes_with_mode(d.save_to_bytes().unwrap(), ParseMode::Strict).unwrap();
    let owner = signed.associated_file_owner(None, &b).unwrap();
    let file = signed.object_handle(file.object_id()).unwrap();
    let stamp = signed.stamp();
    assert!(signed.bind_associated_file(&owner, &file, &b).is_err());
    assert!(signed.unbind_associated_file(&owner, &file, &b).is_err());
    assert!(signed.is_stamp_current(&stamp));
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn full_array_allows_noop_and_unlink_then_rebind_but_rejects_one_more() {
    let d = doc();
    let b = pool();
    let file = d.create_associated_file(&definition(), b"x", &b).unwrap();
    let next = d.create_associated_file(&definition(), b"y", &b).unwrap();
    let raw = d.resolve_handle(&file).unwrap();
    let mut entries = vec![raw; 1023];
    entries.push(reference(file.object_id()));
    set_af(&d, None, array(&entries));
    let owner = d.associated_file_owner(None, &b).unwrap();
    let stamp = d.stamp();
    assert!(!d.bind_associated_file(&owner, &file, &b).unwrap());
    assert!(d.is_stamp_current(&stamp));
    let before = d.save_to_bytes().unwrap();
    assert!(matches!(
        d.bind_associated_file(&owner, &next, &b),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(d.save_to_bytes().unwrap(), before);
    d.unbind_associated_file(&owner, &file, &b).unwrap();
    let owner = d.associated_file_owner(None, &b).unwrap();
    d.bind_associated_file(&owner, &next, &b).unwrap();
    let owner = d.associated_file_owner(None, &b).unwrap();
    let list = d.associated_files(&owner, &b).unwrap();
    assert_eq!(list.files().len(), 1024);
    assert_eq!(list.files()[1023].filespec(), Some(&next));
    drop(list);
    assert_eq!(b.live_bytes(), 0);
}

#[test]
fn bind_rejects_extended_specs_and_params_without_moddate() {
    let d = doc();
    let b = pool();
    let file = d.create_associated_file(&definition(), b"x", &b).unwrap();
    let original = d.resolve_handle(&file).unwrap().into_dict().unwrap();
    for key in ["FS", "RF", "EP", "CI", "ID", "V", "DOS", "Mac", "Unix"] {
        let mut spec = original.clone();
        spec.try_insert(key.into(), PdfObject::PdfNull).unwrap();
        d.update_handle(&file, PdfObject::PdfDict(spec)).unwrap();
        let owner = d.associated_file_owner(None, &b).unwrap();
        let before = d.save_to_bytes().unwrap();
        assert!(d.bind_associated_file(&owner, &file, &b).is_err());
        assert_eq!(d.save_to_bytes().unwrap(), before);
    }
    d.update_handle(&file, PdfObject::PdfDict(original.clone()))
        .unwrap();
    let id = original
        .get("EF")
        .unwrap()
        .as_dict()
        .unwrap()
        .get("F")
        .unwrap()
        .as_reference()
        .unwrap()
        .id();
    let (mut dict, data) = d
        .resolve_object(id)
        .unwrap()
        .into_stream()
        .unwrap()
        .into_raw_parts()
        .unwrap();
    dict.try_insert("Params".into(), PdfObject::PdfDict(PdfDict::default()))
        .unwrap();
    d.update_object(id, PdfObject::PdfStream(PdfStream::new(dict, data)))
        .unwrap();
    let owner = d.associated_file_owner(None, &b).unwrap();
    assert!(d.bind_associated_file(&owner, &file, &b).is_err());
    assert!(d.clone_associated_file(&file, &definition(), &b).is_err());
    assert_eq!(b.live_bytes(), 0);
}
