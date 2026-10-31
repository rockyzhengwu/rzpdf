use pdf::{
    document::{owned::Document, resource_manager::ResourceKind},
    error::PdfError,
    parser::options::{Limits, ParseOptions},
};

#[test]
fn graphics_stack_clipping_copies_are_bounded_and_released_on_restore() {
    for (limit, suffix, success) in [
        (9, "q q Q Q", false),
        (10, "q q Q Q", true),
        (5, "q Q q Q q Q", true),
    ] {
        let doc = Document::new_with_options(ParseOptions {
            limits: Limits {
                max_path_segments: limit,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, format!("0 0 10 10 re W n {suffix}").into_bytes())
            .unwrap();
        let revision = doc.revision();
        let result = doc.get_page(0).unwrap().display_list();
        if success {
            assert!(result.unwrap().objects().is_empty());
        } else {
            assert!(
                matches!(result,Err(PdfError::ResourceLimit(ref message)) if message == "graphics stack clipping segments")
            );
        }
        assert_eq!(doc.revision(), revision);
    }
}

#[test]
fn exhausted_clipping_budget_is_checked_before_next_image_decoder() {
    use pdf::objects::{PdfObject, pdf_name::PdfName, pdf_stream::PdfStream};
    for limit in [9, 10] {
        let doc = Document::new_with_options(ParseOptions {
            limits: Limits {
                max_path_segments: limit,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        let first = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        let source = doc
            .get_page(0)
            .unwrap()
            .extract_image_source(&first)
            .unwrap();
        let mut dict = source.stream.dict().clone();
        dict.try_insert(
            "Filter".into(),
            PdfObject::PdfName(PdfName::new("RzpdfUnknownCodec".into())),
        )
        .unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(dict, vec![255, 0, 0])))
            .unwrap();
        let second = doc
            .resources()
            .register_existing(0, ResourceKind::Image, id)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("0 0 10 10 re W n /{first} Do /{second} Do").into_bytes(),
        )
        .unwrap();
        let revision = doc.revision();
        let error = doc.get_page(0).unwrap().display_list().unwrap_err();
        if limit == 9 {
            assert!(
                matches!(error,PdfError::ResourceLimit(ref message) if message == "display retained path segments")
            );
        } else {
            // At the exact clipping boundary the unsupported decoder is reached.
            assert!(
                matches!(error, PdfError::FilterError(ref message) if message == "unsupported filter: RzpdfUnknownCodec")
            );
        }
        assert_eq!(doc.revision(), revision);
    }
}

#[test]
fn display_counts_repeated_clipping_copies_not_only_source_geometry() {
    for (limit, draws, success) in [(5, 1, true), (9, 2, false), (10, 2, true)] {
        let doc = Document::new_with_options(ParseOptions {
            limits: Limits {
                max_path_segments: limit,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        doc.add_page(0).unwrap();
        let image = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        let mut source = "0 0 10 10 re W n".to_string();
        for _ in 0..draws {
            source.push_str(&format!(" /{image} Do"));
        }
        doc.replace_page_contents(0, source.into_bytes()).unwrap();
        let revision = doc.revision();
        let result = doc.get_page(0).unwrap().display_list();
        if success {
            assert_eq!(result.unwrap().objects().len(), draws);
        } else {
            assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
        }
        assert_eq!(doc.revision(), revision);
    }
}

#[test]
fn display_retained_unicode_and_image_bytes_share_a_budget_in_either_order() {
    for image_first in [false, true] {
        for (characters, success) in [(31, true), (32, true), (33, false)] {
            let doc = Document::new_with_options(ParseOptions {
                limits: Limits {
                    max_decoded_bytes: 128,
                    max_display_list_bytes: 128,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
            doc.add_page(0).unwrap();
            // Use a tiny custom CMap, not built-in AFM data whose own parsing
            // budget would fail before this retained-payload boundary is reached.
            use pdf::{
                io::stream_reader::StreamReader,
                objects::{
                    PdfObject, pdf_dict::PdfDict, pdf_reference::PdfReference,
                    pdf_stream::PdfStream,
                },
                parser::syntax::SyntaxParser,
            };
            let mut parser = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
                b"<< /Type /Font /Subtype /Type0 /BaseFont /BudgetTest /DescendantFonts [<< /Subtype /CIDFontType2 /BaseFont /BudgetTest /DW 500 >>] >>".to_vec()
            )).unwrap());
            let mut dict = parser.get_object().unwrap().into_dict().unwrap();
            for (name,data) in [
                ("Encoding",b"1 begincodespacerange <00> <FF> endcodespacerange 1 begincidchar <41> 1 endcidchar".as_slice()),
                ("ToUnicode",b"1 beginbfchar <41> <0041> endbfchar".as_slice()),
            ] {
                let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::new(PdfDict::default(),data.to_vec()))).unwrap();
                dict.try_insert(name.into(),PdfObject::PdfReference(PdfReference::from_id(id))).unwrap();
            }
            let id = doc.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
            let font = doc
                .resources()
                .register_existing(0, ResourceKind::Font, id)
                .unwrap();
            // 96 normalized RGB sample bytes, plus one Unicode byte per ASCII char.
            let image = doc
                .resources()
                .embed_rgb_image(0, 32, 1, vec![128; 96])
                .unwrap();
            let text = format!("BT /{font} 12 Tf ({}) Tj ET", "A".repeat(characters));
            let draw = format!("/{image} Do");
            let source = if image_first {
                format!("{draw} {text}")
            } else {
                format!("{text} {draw}")
            };
            doc.replace_page_contents(0, source.into_bytes()).unwrap();
            let revision = doc.revision();
            let result = doc.get_page(0).unwrap().display_list();
            if success {
                assert_eq!(result.unwrap().objects().len(), 2);
            } else {
                assert!(matches!(
                    result,
                    Err(PdfError::ResourceLimitExceeded {
                        reason: "display_list_bytes",
                        ..
                    })
                ));
            }
            assert_eq!(doc.revision(), revision);
        }
    }
}
