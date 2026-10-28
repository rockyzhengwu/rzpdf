use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        metadata::InfoTextField,
        save_options::{SaveLayout, SaveOptions, StreamCompression},
    },
    error::PdfError,
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_reference::PdfReference},
    parser::{
        parser::parse_xref,
        syntax::{ParseMode, SyntaxParser},
    },
};
use std::io::Cursor;

const SOURCE: &[u8] = include_bytes!("fixtures/linearized/two-pages.pdf");
const LAYOUTS: [SaveLayout; 3] = [
    SaveLayout::Classic,
    SaveLayout::XrefStream,
    SaveLayout::XrefStreamWithObjectStreams,
];

fn open(bytes: &[u8]) -> Document {
    Document::from_bytes_with_mode(bytes.to_vec(), ParseMode::Strict).unwrap()
}

fn table(bytes: &[u8]) -> pdf::document::cross_ref_table::CrossRefTable {
    let mut parser = SyntaxParser::with_mode(
        StreamReader::try_new(Cursor::new(bytes)).unwrap(),
        ParseMode::Strict,
    );
    parser.read_header().unwrap();
    parse_xref(&mut parser).unwrap()
}

#[test]
fn edited_linearized_source_rewrites_all_layouts_without_stale_structures() {
    check_rewrite(SOURCE);
    check_rewrite(include_bytes!("fixtures/linearized/two-pages-objects.pdf"));
    check_rewrite(include_bytes!("fixtures/linearized/two-pages-overflow.pdf"));
}

fn check_rewrite(source: &[u8]) {
    let doc = open(source);
    let original_table = table(source);
    let first = original_table
        .objects()
        .values()
        .filter(|info| {
            matches!(
                info.state(),
                pdf::document::cross_ref_table::ObjectState::Normal
            )
        })
        .min_by_key(|info| info.offset())
        .unwrap();
    let parameters = first.object_id();
    let object = doc.resolve_object(parameters).unwrap();
    let hints = object
        .as_dict()
        .unwrap()
        .get("H")
        .unwrap()
        .as_array()
        .unwrap();
    let mut removed = vec![parameters];
    for index in (0..hints.len()).step_by(2) {
        let offset = hints
            .get(index)
            .unwrap()
            .as_number()
            .unwrap()
            .as_u64_checked()
            .unwrap();
        removed.push(
            original_table
                .objects()
                .values()
                .find(|info| info.offset() == offset)
                .unwrap()
                .object_id(),
        );
    }
    assert!(
        doc.resolve_object(parameters)
            .unwrap()
            .as_dict()
            .unwrap()
            .contains_key("Linearized")
    );
    doc.set_info_text(InfoTextField::Title, Some("edited linearized input"))
        .unwrap();
    doc.replace_page_contents(1, b"0 1 0 rg 10 10 80 60 re f".to_vec())
        .unwrap();
    let stamp = doc.stamp();
    let expected_pages: Vec<_> = (0..2)
        .map(|i| {
            format!(
                "{:?}",
                doc.get_page(i).unwrap().display_list().unwrap().objects()
            )
        })
        .collect();
    for layout in LAYOUTS {
        for compression in [
            StreamCompression::Preserve,
            StreamCompression::FlateUnencoded,
        ] {
            let options = SaveOptions {
                compression,
                ..Default::default()
            };
            let output = doc.save_to_bytes_with_layout(options, layout).unwrap();
            assert!(
                !output
                    .windows(b"/Linearized".len())
                    .any(|w| w == b"/Linearized")
            );
            let reopened = open(&output);
            assert_eq!(reopened.try_page_count().unwrap(), 2);
            for (i, expected) in expected_pages.iter().enumerate() {
                assert_eq!(
                    &format!(
                        "{:?}",
                        reopened
                            .get_page(i as u32)
                            .unwrap()
                            .display_list()
                            .unwrap()
                            .objects()
                    ),
                    expected
                );
            }
            assert_eq!(
                reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
                Some("edited linearized input")
            );
            for &id in &removed {
                assert!(reopened.resolve_object(id).is_err());
            }
            // Every retained original object keeps its logical ID and value;
            // edits and physical output-only XRef containers are separate.
            for info in original_table.objects().values() {
                let id = info.object_id();
                if matches!(
                    info.state(),
                    pdf::document::cross_ref_table::ObjectState::Free
                ) || removed.contains(&id)
                {
                    continue;
                }
                let before = doc.resolve_object(id).unwrap();
                if before.as_stream().is_some() {
                    continue;
                }
                let after = reopened.resolve_object(id).unwrap();
                assert_eq!(before, after, "{id:?}");
            }
            assert!(doc.is_stamp_current(&stamp));
            assert!(doc.resolve_object(parameters).is_ok());
            for &id in &removed {
                assert!(doc.resolve_object(id).is_ok());
            }
            assert_eq!(
                output,
                doc.save_to_bytes_with_layout(options, layout).unwrap()
            );
            let budget = OperationBudget::new(16 * 1024 * 1024);
            let budgeted = doc
                .save_with_layout_and_budget(options, layout, &budget)
                .unwrap();
            assert_eq!(budgeted.as_slice(), output);
        }
    }
    assert!(doc.save_incremental_to_bytes().is_err());
}

#[test]
fn referenced_or_modified_structures_refuse_before_host_write() {
    for id in [ObjectId::new(4, 0), ObjectId::new(6, 0)] {
        for modify in [false, true] {
            let doc = open(SOURCE);
            if modify {
                let mut dict = PdfDict::default();
                dict.try_insert(
                    "Custom".into(),
                    PdfObject::PdfBool(pdf::objects::pdf_bool::PdfBool::new(b"true")),
                )
                .unwrap();
                doc.update_object(id, PdfObject::PdfDict(dict)).unwrap();
            } else {
                doc.try_insert_object(PdfObject::PdfReference(PdfReference::new(
                    id.obj_num(),
                    id.gen_num(),
                )))
                .unwrap();
            }
            let stamp = doc.stamp();
            for layout in LAYOUTS {
                let mut target = b"keep".to_vec();
                assert!(matches!(
                    doc.save_to_writer_with_layout(&mut target, Default::default(), layout),
                    Err(PdfError::UnsupportedFeature(_))
                ));
                assert_eq!(target, b"keep");
                assert!(doc.is_stamp_current(&stamp));
            }
        }
    }
}

#[test]
fn unrecognized_linearized_marker_refuses_all_full_layouts() {
    let doc = Document::new().unwrap();
    doc.add_page(0).unwrap();
    let mut dict = PdfDict::default();
    dict.try_insert(
        "Linearized".into(),
        PdfObject::PdfBool(pdf::objects::pdf_bool::PdfBool::new(b"true")),
    )
    .unwrap();
    doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
    for layout in LAYOUTS {
        assert!(matches!(
            doc.save_to_bytes_with_layout(Default::default(), layout),
            Err(PdfError::UnsupportedFeature(_))
        ));
    }
}

#[test]
fn malformed_or_unknown_source_structures_are_not_silently_discarded() {
    for (old, new) in [
        (b"529 116".as_slice(), b"529 0  ".as_slice()),
        (b"529 116".as_slice(), b"999 116".as_slice()),
        (b"529 116".as_slice(), b"529 1  ".as_slice()),
        (b"529 116".as_slice(), b"529 999".as_slice()),
        (b"/T 1129".as_slice(), b"/Z 1129".as_slice()),
    ] {
        let mut source = SOURCE.to_vec();
        let start = source.windows(old.len()).position(|w| w == old).unwrap();
        source[start..start + old.len()].copy_from_slice(new);
        let doc = open(&source);
        for layout in LAYOUTS {
            assert!(matches!(
                doc.save_to_bytes_with_layout(Default::default(), layout),
                Err(PdfError::UnsupportedFeature(_))
            ));
        }
    }
}

#[test]
fn default_full_save_and_output_budget_keep_the_source_usable() {
    let doc = open(SOURCE);
    let stamp = doc.stamp();
    let output = doc.save_to_bytes().unwrap();
    assert_eq!(
        output,
        doc.save_to_bytes_with_layout(Default::default(), SaveLayout::Classic)
            .unwrap()
    );
    let mut target = b"keep".to_vec();
    assert!(matches!(
        doc.save_to_writer_with_layout(
            &mut target,
            SaveOptions {
                max_output_bytes: 32,
                ..Default::default()
            },
            SaveLayout::Classic
        ),
        Err(PdfError::ResourceLimit(_))
    ));
    assert_eq!(target, b"keep");
    let budget = OperationBudget::new(1);
    assert!(matches!(
        doc.save_with_layout_and_budget(Default::default(), SaveLayout::Classic, &budget),
        Err(PdfError::ResourceLimit(_))
    ));
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(output, doc.save_to_bytes().unwrap());
}

#[test]
fn r6_linearized_input_preserves_protection_for_user_and_owner() {
    use pdf::{parser::options::ParseOptions, security::Authentication};
    let source = include_bytes!("fixtures/linearized/two-pages-r6.pdf");
    let options = ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    };
    for (password, authentication) in [
        ("synthetic-user", Authentication::User),
        ("synthetic-owner", Authentication::Owner),
    ] {
        let doc =
            Document::from_bytes_with_password_and_options(source.to_vec(), password, options)
                .unwrap();
        assert_eq!(
            doc.security_report().unwrap().authentication,
            authentication
        );
        doc.set_info_text(
            InfoTextField::Title,
            Some("edited encrypted linearized input"),
        )
        .unwrap();
        doc.replace_page_contents(1, b"0 1 0 rg 10 10 80 60 re f".to_vec())
            .unwrap();
        let expected = format!(
            "{:?}",
            doc.get_page(1).unwrap().display_list().unwrap().objects()
        );
        let stamp = doc.stamp();
        for layout in LAYOUTS {
            let bytes = doc
                .save_to_bytes_with_layout(Default::default(), layout)
                .unwrap();
            assert!(!bytes.windows(11).any(|w| w == b"/Linearized"));
            assert!(Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).is_err());
            for secret in ["synthetic-user", "synthetic-owner"] {
                let reopened =
                    Document::from_bytes_with_password_and_options(bytes.clone(), secret, options)
                        .unwrap();
                assert_eq!(reopened.security_report().unwrap().revision, 6);
                assert_eq!(
                    reopened.security_report().unwrap().permissions,
                    doc.security_report().unwrap().permissions
                );
                assert_eq!(
                    reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
                    Some("edited encrypted linearized input")
                );
                assert_eq!(
                    format!(
                        "{:?}",
                        reopened
                            .get_page(1)
                            .unwrap()
                            .display_list()
                            .unwrap()
                            .objects()
                    ),
                    expected
                );
            }
            assert!(doc.is_stamp_current(&stamp));
        }
        assert!(doc.save_incremental_to_bytes().is_err());
    }
}

#[test]
fn cms_signed_linearized_prefix_remains_read_only_with_explicit_history_boundary() {
    let source = include_bytes!("fixtures/linearized/two-pages-signed.pdf");
    assert!(source.starts_with(SOURCE));
    let doc = open(source);
    assert!(
        matches!(doc.verify_signatures(), Err(PdfError::UnsupportedFeature(message)) if message.contains("linearized signature history"))
    );
    let stamp = doc.stamp();
    for layout in LAYOUTS {
        let mut target = b"keep".to_vec();
        assert!(matches!(
            doc.save_to_writer_with_layout(&mut target, Default::default(), layout),
            Err(PdfError::UnsupportedFeature(_))
        ));
        assert_eq!(target, b"keep");
    }
    assert!(doc.is_stamp_current(&stamp));
    let commit = doc.commit_revision();
    assert!(doc.replace_page_contents(0, b"0 0 0 rg".to_vec()).is_err());
    assert!(
        doc.update_object(ObjectId::new(12, 0), PdfObject::PdfNull)
            .is_err()
    );
    assert!(doc.prepare_signature(Default::default()).is_err());
    assert!(doc.save_incremental_to_bytes().is_err());
    // Rejected high-level transactions may invalidate stamps on rollback;
    // they must not commit changes or damage the signed source.
    assert_eq!(doc.commit_revision(), commit);
    assert_eq!(
        doc.get_page(0)
            .unwrap()
            .display_list()
            .unwrap()
            .objects()
            .len(),
        1
    );
}
