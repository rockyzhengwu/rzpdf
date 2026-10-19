use pdf::{
    document::{
        Document,
        destination::DestinationView,
        metadata::InfoTextField,
        save_options::{
            IncrementalSaveLayout, PdfVersion, SaveLayout, SaveOptions, StreamCompression,
        },
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_reference::PdfReference, pdf_stream::PdfStream, pdf_string::PdfString,
    },
    parser::{
        options::{Limits, ParseOptions},
        parser::parse_xref,
        syntax::{ParseMode, SyntaxParser},
    },
    security::{EncryptionOptions, SaveSecurity},
};
fn reference(n: u32) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(ObjectId::new(n, 0)))
}
fn string(s: &str) -> PdfObject {
    PdfObject::PdfString(PdfString::new(s.as_bytes().to_vec(), false))
}
fn open(bytes: Vec<u8>) -> Document {
    Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap()
}
fn object(doc: &Document, n: u32) -> PdfObject {
    doc.resolve_object(ObjectId::new(n, 0)).unwrap()
}
fn trailer(bytes: &[u8]) -> PdfDict {
    let mut p = SyntaxParser::with_mode(
        StreamReader::try_new(std::io::Cursor::new(bytes)).unwrap(),
        ParseMode::Strict,
    );
    p.read_header().unwrap();
    parse_xref(&mut p).unwrap().trailer().clone()
}
fn fixture(info_extra: &str, trailer_extra: &str) -> Vec<u8> {
    let xml = b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n  <!-- raw packet -->\n</x:xmpmeta>\n";
    let payload = b"associated payload\x00\xff";
    let stream = |extra: &str, bytes: &[u8]| {
        let mut out = format!("<< /Length {} {extra} >>\nstream\n", bytes.len()).into_bytes();
        out.extend_from_slice(bytes);
        out.extend_from_slice(b"\nendstream");
        out
    };
    let objects = vec![
        b"<< /Type /Catalog /Pages 2 0 R /Private 6 0 R /Alias 6 0 R /AF [7 0 R] /Metadata 9 0 R /Names << /EmbeddedFiles << /Names [(note.bin) 7 0 R] >> >> >>".to_vec(),
        b"<< /Type /Pages /Kids [3 0 R 4 0 R] /Count 2 >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 320 240] /Resources << >> /Contents 5 0 R /Private 6 0 R /Alias 6 0 R /AF [7 0 R] /Metadata 9 0 R >>".to_vec(),
        b"<< /Type /Page /Parent 2 0 R /MediaBox [0 0 320 240] /Resources << >> /Contents 5 0 R /Private 6 0 R /AF [7 0 R] /Metadata 9 0 R >>".to_vec(),
        stream("", b"0 0 1 rg 20 20 60 40 re f"),
        b"<< /Label <FEFF4F60597D> /Self 6 0 R /PrivateArray [null true 1.2500 /PrivateName] >>".to_vec(),
        b"<< /Type /Filespec /F (note.bin) /UF <FEFF006E006F00740065002E00620069006E> /AFRelationship /Data /Desc (associated file) /EF << /F 8 0 R >> /Private 6 0 R >>".to_vec(),
        stream("/Type /EmbeddedFile /Subtype /application#2Foctet-stream", payload),
        stream("/Type /Metadata /Subtype /XML", xml),
        format!("<< /Title (before) /Private 6 0 R {info_extra} >>").into_bytes(),
        b"(unreachable business object)".to_vec(),
    ];
    let mut bytes = b"%PDF-2.0\n%\xe2\xe3\xcf\xd3\n".to_vec();
    let mut offsets = vec![];
    for (i, value) in objects.iter().enumerate() {
        offsets.push(bytes.len());
        bytes.extend_from_slice(format!("{} 0 obj\n", i + 1).as_bytes());
        bytes.extend_from_slice(value);
        bytes.extend_from_slice(b"\nendobj\n");
    }
    let pos = bytes.len();
    bytes.extend_from_slice(b"xref\n0 12\n0000000000 65535 f \n");
    for offset in offsets {
        bytes.extend_from_slice(format!("{offset:010} 00000 n \n").as_bytes());
    }
    bytes.extend_from_slice(format!("trailer\n<< /Size 12 /Root 1 0 R /Info 10 0 R /Private 6 0 R {trailer_extra} >>\nstartxref\n{pos}\n%%EOF\n").as_bytes());
    bytes
}
const LAYOUTS: [SaveLayout; 3] = [
    SaveLayout::Classic,
    SaveLayout::XrefStream,
    SaveLayout::XrefStreamWithObjectStreams,
];
fn check(doc: &Document) {
    assert_eq!(doc.try_page_count().unwrap(), 2);
    assert_eq!(
        object(doc, 6).as_dict().unwrap().get("Self"),
        Some(&reference(6))
    );
    assert_eq!(
        object(doc, 7)
            .as_dict()
            .unwrap()
            .get("AFRelationship")
            .unwrap()
            .as_name()
            .unwrap()
            .name(),
        "Data"
    );
    assert_eq!(
        doc.attachment_data("note.bin").unwrap().unwrap(),
        b"associated payload\0\xff"
    );
    assert_eq!(
        doc.metadata_packet().unwrap().unwrap(),
        b"<x:xmpmeta xmlns:x=\"adobe:ns:meta/\">\n  <!-- raw packet -->\n</x:xmpmeta>\n"
    );
    assert_eq!(object(doc, 11), string("unreachable business object"));
}

#[test]
fn all_full_layouts_preserve_unknown_graph_trailer_af_xmp_and_unreachable_objects() {
    let original = fixture("", "");
    let source = open(original.clone());
    for layout in LAYOUTS {
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let stamp = source.stamp();
            let bytes = source
                .save_to_bytes_with_layout(
                    SaveOptions {
                        compression,
                        ..Default::default()
                    },
                    layout,
                )
                .unwrap();
            assert_eq!(trailer(&bytes).get("Private"), Some(&reference(6)));
            let reopened = open(bytes);
            check(&reopened);
            for n in [1, 2, 3, 4, 6, 7, 10, 11] {
                assert_eq!(object(&reopened, n), object(&source, n));
            }
            assert_eq!(source.stamp(), stamp);
        }
    }
}

#[test]
fn both_incremental_layouts_preserve_unknown_roots_and_original_prefix() {
    for full in LAYOUTS {
        let original = open(fixture("", ""))
            .save_to_bytes_with_layout(Default::default(), full)
            .unwrap();
        for layout in [
            IncrementalSaveLayout::Classic,
            IncrementalSaveLayout::XrefStream,
        ] {
            let doc = open(original.clone());
            let before: Vec<_> = (1..=10).map(|n| object(&doc, n)).collect();
            doc.update_object(ObjectId::new(11, 0), string("updated private object"))
                .unwrap();
            doc.set_info_text(InfoTextField::Title, Some("changed"))
                .unwrap();
            let bytes = doc
                .save_incremental_to_bytes_with_layout(Default::default(), layout)
                .unwrap();
            assert!(bytes.starts_with(&original));
            assert_eq!(trailer(&bytes).get("Private"), Some(&reference(6)));
            let next = open(bytes);
            for (i, value) in before.iter().enumerate() {
                assert_eq!(&object(&next, (i + 1) as u32), value);
            }
            assert_eq!(
                next.info_dict().unwrap().unwrap().get("Private"),
                Some(&reference(6))
            );
            assert_eq!(object(&next, 11), string("updated private object"));
        }
    }
}

#[test]
fn r6_full_and_incremental_preserve_af_unknown_values_and_raw_metadata() {
    let source = open(fixture("", ""));
    let settings = EncryptionOptions::new("user", "owner").unwrap();
    for layout in LAYOUTS {
        let bytes = source
            .save_to_bytes_with_layout_and_security(
                Default::default(),
                layout,
                SaveSecurity::Set(&settings),
            )
            .unwrap();
        let doc = Document::from_bytes_with_password_and_options(
            bytes.clone(),
            "user",
            ParseOptions {
                mode: ParseMode::Strict,
                ..Default::default()
            },
        )
        .unwrap();
        check(&doc);
        doc.set_info_text(InfoTextField::Title, Some("private changed"))
            .unwrap();
        let output = doc
            .save_incremental_to_bytes_with_layout(
                Default::default(),
                IncrementalSaveLayout::XrefStream,
            )
            .unwrap();
        assert!(output.starts_with(&bytes));
        let next = Document::from_bytes_with_password_and_options(
            output,
            "owner",
            ParseOptions {
                mode: ParseMode::Strict,
                ..Default::default()
            },
        )
        .unwrap();
        check(&next);
        assert_eq!(
            next.info_dict().unwrap().unwrap().get("Private"),
            Some(&reference(6))
        );
    }
}

fn page_dict(doc: &Document, index: u32) -> (ObjectId, PdfDict) {
    let id = doc
        .destination(index, DestinationView::Fit)
        .unwrap()
        .page
        .object_id();
    (id, doc.resolve_object(id).unwrap().into_dict().unwrap())
}
fn target() -> Document {
    let d = Document::new_with_version(PdfVersion::V2_0).unwrap();
    d.add_page(0).unwrap();
    d
}

#[test]
fn import_remaps_shared_cycles_af_and_page_metadata_without_merging_catalog_metadata() {
    let source = open(fixture("", ""));
    let (id, mut page) = page_dict(&source, 0);
    page.try_insert("Peer".into(), reference(4)).unwrap();
    source.update_object(id, PdfObject::PdfDict(page)).unwrap();
    let before = source.save_to_bytes().unwrap();
    let target = target();
    target
        .set_info_text(InfoTextField::Title, Some("destination"))
        .unwrap();
    target
        .set_metadata_packet(Some(b"target metadata".to_vec()))
        .unwrap();
    target.import_pages_from(&source, &[1, 0], 0).unwrap();
    let (_, first) = page_dict(&target, 0);
    let (_, second) = page_dict(&target, 1);
    assert_eq!(
        second.get("Peer").unwrap().as_object_id(),
        Some(page_dict(&target, 0).0)
    );
    assert_eq!(first.get("Private"), second.get("Private"));
    assert_eq!(second.get("Private"), second.get("Alias"));
    let shared = second.get("Private").unwrap().as_object_id().unwrap();
    let value = target.resolve_object(shared).unwrap();
    assert_eq!(
        value.as_dict().unwrap().get("Self").unwrap().as_object_id(),
        Some(shared)
    );
    assert_eq!(first.get("AF"), second.get("AF"));
    let spec = target
        .resolve_object(
            second
                .get("AF")
                .unwrap()
                .as_array()
                .unwrap()
                .get(0)
                .unwrap()
                .as_object_id()
                .unwrap(),
        )
        .unwrap();
    assert_eq!(
        spec.as_dict().unwrap().get("AFRelationship"),
        object(&source, 7).as_dict().unwrap().get("AFRelationship")
    );
    let packet = target
        .resolve_object(second.get("Metadata").unwrap().as_object_id().unwrap())
        .unwrap();
    assert_eq!(
        packet.as_stream().unwrap().raw_data(),
        object(&source, 9).as_stream().unwrap().raw_data()
    );
    assert_eq!(
        target.metadata_packet().unwrap().unwrap(),
        b"target metadata"
    );
    assert_eq!(
        target.info_text(InfoTextField::Title).unwrap().as_deref(),
        Some("destination")
    );
    assert!(target.attachment_names().unwrap().is_empty());
    assert_eq!(source.save_to_bytes().unwrap(), before);
    for layout in LAYOUTS {
        assert_eq!(
            open(
                target
                    .save_to_bytes_with_layout(Default::default(), layout)
                    .unwrap()
            )
            .try_page_count()
            .unwrap(),
            3
        );
    }
}

#[test]
fn same_document_copy_shares_ordinary_extensions_and_delete_keeps_shared_af_payloads() {
    let doc = open(fixture("", ""));
    doc.copy_page(0, 2).unwrap();
    let (_, original) = page_dict(&doc, 0);
    let (_, copy) = page_dict(&doc, 2);
    for key in ["Private", "Alias", "AF", "Metadata"] {
        assert_eq!(copy.get(key), original.get(key));
    }
    doc.delete_page(0).unwrap();
    assert_eq!(doc.try_page_count().unwrap(), 2);
    assert_eq!(
        object(&doc, 8).as_stream().unwrap().raw_data(),
        b"associated payload\0\xff"
    );
    for layout in LAYOUTS {
        check(&open(
            doc.save_to_bytes_with_layout(Default::default(), layout)
                .unwrap(),
        ));
    }
}

#[test]
fn info_and_private_trailer_back_references_block_deletion_and_roll_back_then_retry() {
    for source in [
        fixture("/Owner 3 0 R", ""),
        fixture("", "/PrivateOwner 3 0 R"),
    ] {
        let doc = open(source.clone());
        let before = doc.save_to_bytes().unwrap();
        let commit = doc.commit_revision();
        assert!(matches!(
            doc.delete_page(0),
            Err(PdfError::UnsupportedFeature(_))
        ));
        assert_eq!(doc.commit_revision(), commit);
        assert_eq!(doc.save_to_bytes().unwrap(), before);
        assert_eq!(doc.try_page_count().unwrap(), 2);
        assert_eq!(
            doc.save_incremental_to_bytes_with_layout(
                Default::default(),
                IncrementalSaveLayout::XrefStream
            )
            .unwrap(),
            source
        );
        // The unreferenced second page is a valid retry on the same document.
        doc.delete_page(1).unwrap();
        assert_eq!(doc.try_page_count().unwrap(), 1);
    }
}

#[test]
fn private_af_back_references_are_not_retargeted_as_if_they_were_navigation() {
    let doc = open(fixture("", ""));
    let mut spec = object(&doc, 7).into_dict().unwrap();
    spec.try_insert("Owner".into(), reference(3)).unwrap();
    doc.update_object(ObjectId::new(7, 0), PdfObject::PdfDict(spec))
        .unwrap();
    let before = doc.save_to_bytes().unwrap();
    let commit = doc.commit_revision();
    assert!(
        doc.delete_page_with_policy(
            0,
            pdf::document::page_transfer::PageDeletionPolicy::Retarget { page_index: 1 }
        )
        .is_err()
    );
    assert_eq!(doc.save_to_bytes().unwrap(), before);
    assert_eq!(doc.commit_revision(), commit);
}

#[test]
fn unselected_owner_in_unknown_graph_rolls_back_import_and_allocator_then_retries() {
    let source = open(fixture("", ""));
    let mut shared = object(&source, 6).into_dict().unwrap();
    shared.try_insert("Owner".into(), reference(4)).unwrap();
    source
        .update_object(ObjectId::new(6, 0), PdfObject::PdfDict(shared))
        .unwrap();
    let target = target();
    let before = target.save_to_bytes().unwrap();
    let commit = target.commit_revision();
    assert!(target.import_pages_from(&source, &[0], 0).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
    assert_eq!(target.commit_revision(), commit);
    let mut shared = object(&source, 6).into_dict().unwrap();
    shared.remove("Owner");
    source
        .update_object(ObjectId::new(6, 0), PdfObject::PdfDict(shared))
        .unwrap();
    target.import_pages_from(&source, &[0], 0).unwrap();
    let control = super_target(&source);
    assert_eq!(
        target.save_to_bytes().unwrap(),
        control.save_to_bytes().unwrap()
    );
}
fn super_target(source: &Document) -> Document {
    let d = target();
    d.import_pages_from(source, &[0], 0).unwrap();
    d
}

#[test]
fn structural_container_in_private_page_graph_cannot_be_imported_as_opaque_payload() {
    for (kind, indirect) in [("XRef", false), ("ObjStm", false), ("ObjStm", true)] {
        let source = open(fixture("", ""));
        let mut dict = PdfDict::default();
        let name = PdfObject::PdfName(PdfName::new(kind.into()));
        let kind = if indirect {
            PdfObject::PdfReference(PdfReference::from_id(
                source.try_insert_object(name).unwrap(),
            ))
        } else {
            name
        };
        dict.try_insert("Type".into(), kind).unwrap();
        dict.try_insert(
            "N".into(),
            PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(0.0)),
        )
        .unwrap();
        dict.try_insert(
            "First".into(),
            PdfObject::PdfNumber(pdf::objects::pdf_number::PdfNumber::new(0.0)),
        )
        .unwrap();
        let container = source
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![])))
            .unwrap();
        let (id, mut page) = page_dict(&source, 0);
        page.try_insert(
            "PrivateContainer".into(),
            PdfObject::PdfReference(PdfReference::from_id(container)),
        )
        .unwrap();
        source.update_object(id, PdfObject::PdfDict(page)).unwrap();
        let target = target();
        let before = target.save_to_bytes().unwrap();
        let commit = target.commit_revision();
        assert!(matches!(
            target.import_pages_from(&source, &[0], 0),
            Err(PdfError::UnsupportedFeature(_))
        ));
        assert_eq!(target.save_to_bytes().unwrap(), before);
        assert_eq!(target.commit_revision(), commit);
        // Delete must also refuse reachable opaque container semantics.
        let before = source.save_to_bytes().unwrap();
        assert!(source.delete_page(1).is_err());
        assert_eq!(source.save_to_bytes().unwrap(), before);
    }
}

#[test]
fn imported_private_payload_limits_fail_atomically() {
    let source = open(fixture("", ""));
    let target = Document::new_with_version_and_options(
        PdfVersion::V2_0,
        ParseOptions {
            limits: Limits {
                max_decoded_bytes: 64,
                ..Default::default()
            },
            ..Default::default()
        },
    )
    .unwrap();
    target.add_page(0).unwrap();
    let before = target.save_to_bytes().unwrap();
    let commit = target.commit_revision();
    assert!(matches!(
        target.import_pages_from(&source, &[0], 0),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(target.commit_revision(), commit);
    assert_eq!(target.save_to_bytes().unwrap(), before);
    assert_eq!(source.try_page_count().unwrap(), 2);
}

#[test]
fn structure_and_ocg_graphs_preserve_on_save_but_do_not_gain_transfer_permission() {
    use pdf::document::structure::StructureElementDefinition;
    let doc = open(fixture("", ""));
    doc.add_structure_element(None, 0, &StructureElementDefinition::new("Document"))
        .unwrap();
    let group = doc
        .try_insert_object({
            let mut dict = PdfDict::default();
            dict.try_insert(
                "Type".into(),
                PdfObject::PdfName(PdfName::new("OCG".into())),
            )
            .unwrap();
            dict.try_insert("Name".into(), string("visible group"))
                .unwrap();
            dict.try_insert("Private".into(), reference(6)).unwrap();
            PdfObject::PdfDict(dict)
        })
        .unwrap();
    let mut parser = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
        format!("<< /OCGs [{0} 0 R] /D << /BaseState /ON /Order [{0} 0 R] /Private (configuration data) >> >>", group.obj_num()).into_bytes()
    )).unwrap());
    let properties = parser.get_object().unwrap();
    let mut root = object(&doc, 1).into_dict().unwrap();
    root.try_insert("OCProperties".into(), properties).unwrap();
    doc.update_object(ObjectId::new(1, 0), PdfObject::PdfDict(root))
        .unwrap();
    let source = doc.save_to_bytes().unwrap();
    let end = trailer(&source).get("Size").unwrap().as_u32().unwrap();
    let expected: Vec<_> = (1..end)
        .filter_map(|n| doc.resolve_object(ObjectId::new(n, 0)).ok().map(|v| (n, v)))
        .collect();
    for layout in LAYOUTS {
        let saved = doc
            .save_to_bytes_with_layout(Default::default(), layout)
            .unwrap();
        let saved = open(saved);
        assert!(saved.structure_tree().unwrap().is_some());
        assert_eq!(
            saved.optional_content().unwrap().unwrap().groups[0].name,
            "visible group"
        );
        for (n, value) in &expected {
            assert_eq!(&object(&saved, *n), value);
        }
    }
    for layout in [
        IncrementalSaveLayout::Classic,
        IncrementalSaveLayout::XrefStream,
    ] {
        let saved = open(source.clone());
        saved
            .set_info_text(InfoTextField::Title, Some("metadata-only edit"))
            .unwrap();
        let saved = open(
            saved
                .save_incremental_to_bytes_with_layout(Default::default(), layout)
                .unwrap(),
        );
        for (n, value) in &expected {
            assert_eq!(&object(&saved, *n), value);
        }
    }
    let target = target();
    let before = target.save_to_bytes().unwrap();
    assert!(target.import_pages_from(&doc, &[0, 1], 0).is_err());
    assert_eq!(target.save_to_bytes().unwrap(), before);
    assert_eq!(doc.save_to_bytes().unwrap(), source);
}

#[test]
fn attachment_unlink_and_catalog_xmp_replacement_do_not_rewrite_page_af_or_shared_packets() {
    let original = fixture("", "");
    let doc = open(original.clone());
    doc.remove_attachment("note.bin").unwrap();
    doc.set_metadata_packet(Some(b"new catalog packet".to_vec()))
        .unwrap();
    let mut outputs = Vec::new();
    for layout in LAYOUTS {
        outputs.push(
            doc.save_to_bytes_with_layout(Default::default(), layout)
                .unwrap(),
        );
    }
    for layout in [
        IncrementalSaveLayout::Classic,
        IncrementalSaveLayout::XrefStream,
    ] {
        let bytes = doc
            .save_incremental_to_bytes_with_layout(Default::default(), layout)
            .unwrap();
        assert!(bytes.starts_with(&original));
        outputs.push(bytes);
    }
    for bytes in outputs {
        let doc = open(bytes);
        assert!(doc.attachment_names().unwrap().is_empty());
        assert_eq!(
            doc.metadata_packet().unwrap().unwrap(),
            b"new catalog packet"
        );
        let root = object(&doc, 1);
        let page = object(&doc, 3);
        assert_eq!(
            root.as_dict().unwrap().get("AF"),
            page.as_dict().unwrap().get("AF")
        );
        assert_eq!(page.as_dict().unwrap().get("Metadata"), Some(&reference(9)));
        assert_eq!(
            object(&doc, 9).as_stream().unwrap().raw_data(),
            open(fixture("", "")).metadata_packet().unwrap().unwrap()
        );
        assert_eq!(
            object(&doc, 8).as_stream().unwrap().raw_data(),
            b"associated payload\0\xff"
        );
    }
}
