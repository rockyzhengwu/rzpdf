use super::syntax::ParseMode;

#[derive(Debug, Clone, Copy)]
pub struct Limits {
    pub max_objects: usize,
    pub max_depth: usize,
    pub max_decoded_bytes: usize,
    /// Shared live-byte ceiling for allocations explicitly attached to one
    /// OperationBudget. Not automatically enforced by legacy entry points.
    pub max_operation_bytes: usize,
    pub max_image_pixels: u64,
    /// Maximum pixels in a rasterized page (separate from embedded images).
    pub max_render_pixels: u64,
    /// Maximum bytes for one RGBA page surface, excluding backend overhead.
    pub max_render_bytes: usize,
    pub max_raw_stream_bytes: usize,
    pub max_cached_stream_bytes: usize,
    pub max_content_operations: usize,
    /// Maximum retained content diagnostics per page.
    pub max_content_diagnostics: usize,
    /// Maximum cumulative operator/message bytes retained in page diagnostics.
    pub max_content_diagnostic_bytes: usize,
    /// Maximum cumulative clipping intersections in one graphics state.
    pub max_clip_paths: usize,
    /// Maximum expanded character events in one page interpretation.
    pub max_text_chars: usize,
    /// Maximum path segments constructed during one page interpretation.
    pub max_path_segments: usize,
    pub max_token_bytes: usize,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            max_objects: 1_000_000,
            max_depth: 128,
            max_decoded_bytes: 256 * 1024 * 1024,
            max_operation_bytes: 512 * 1024 * 1024,
            max_image_pixels: 100_000_000,
            max_render_pixels: 100_000_000,
            max_render_bytes: 400_000_000,
            max_raw_stream_bytes: 256 * 1024 * 1024,
            max_cached_stream_bytes: 512 * 1024 * 1024,
            max_content_operations: 1_000_000,
            max_content_diagnostics: 1024,
            max_content_diagnostic_bytes: 16 * 1024 * 1024,
            max_clip_paths: 1024,
            max_text_chars: 1_000_000,
            max_path_segments: 1_000_000,
            max_token_bytes: 16 * 1024 * 1024,
        }
    }
}

impl Limits {
    /// Create once at an operation boundary, then pass/clone the same handle
    /// through its components. Repeated calls create independent pools.
    pub fn operation_budget(&self) -> crate::budget::OperationBudget {
        crate::budget::OperationBudget::new(self.max_operation_bytes)
    }
}

#[derive(Debug, Clone, Copy)]
pub struct ParseOptions {
    pub mode: ParseMode,
    pub limits: Limits,
}

impl Default for ParseOptions {
    fn default() -> Self {
        Self {
            mode: ParseMode::Compatible,
            limits: Limits::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::PdfDocument,
        error::PdfError,
        io::stream_reader::StreamReader,
        objects::{pdf_dict::PdfDict, pdf_number::PdfNumber, pdf_stream::PdfStream, PdfObject},
        parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;

    #[test]
    fn operation_limit_is_independent_and_shared_by_handle() {
        let limits = Limits {
            max_operation_bytes: 5,
            max_decoded_bytes: 100,
            ..Default::default()
        };
        let budget = limits.operation_budget();
        let shared = budget.clone();
        let first = crate::budget::BudgetedBytes::copy_from_slice(&budget, b"abc").unwrap();
        assert!(crate::budget::BudgetedBytes::copy_from_slice(&shared, b"def").is_err());
        assert_eq!(budget.live_bytes(), first.capacity());
        drop(first);
        assert_eq!(shared.live_bytes(), 0);
        assert_eq!(limits.max_decoded_bytes, 100);
        let zero = Limits {
            max_operation_bytes: 0,
            ..limits
        }
        .operation_budget();
        assert!(zero.reserve(1).is_err());
        assert!(zero.reserve(0).is_ok());
    }

    #[test]
    fn nesting_limit_is_enforced_and_depth_recovers_after_error() {
        let options = ParseOptions {
            limits: Limits {
                max_depth: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let reader = StreamReader::try_new(Cursor::new(b"[[[null]]] null ".to_vec())).unwrap();
        let mut syntax = SyntaxParser::with_options(reader, options);
        assert!(matches!(
            syntax.get_object(),
            Err(PdfError::ResourceLimit(_))
        ));
        syntax.set_pos(11).unwrap();
        assert_eq!(syntax.get_object().unwrap(), PdfObject::PdfNull);
    }

    #[test]
    fn document_options_limit_objects_and_stream_bytes() {
        let bytes = std::fs::read("../tests/path-test.pdf").unwrap();
        let options = ParseOptions {
            limits: Limits {
                max_objects: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        assert!(matches!(
            PdfDocument::from_reader_with_options(Cursor::new(bytes.clone()), options),
            Err(PdfError::ResourceLimit(_))
        ));
        let options = ParseOptions {
            limits: Limits {
                max_decoded_bytes: 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = PdfDocument::from_reader_with_options(Cursor::new(bytes), options).unwrap();
        let stream = PdfStream::new(PdfDict::default(), b"12345".to_vec());
        assert!(matches!(
            stream.decode_data(doc.ctx()),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn content_diagnostic_byte_budget_counts_both_strings() {
        let source = b"unknownA unknownB";
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        doc.replace_page_contents(index, source.to_vec()).unwrap();
        let list = doc.get_page(index).unwrap().display_list().unwrap();
        let bytes: usize = list
            .diagnostics()
            .iter()
            .map(|d| d.operator.len() + d.message.len())
            .sum();
        for (limit, allowed) in [(bytes, true), (bytes - 1, false)] {
            let options = ParseOptions {
                limits: Limits {
                    max_content_diagnostic_bytes: limit,
                    ..Default::default()
                },
                ..Default::default()
            };
            let doc = PdfDocument::open_with_options("../tests/path-test.pdf", options).unwrap();
            let index = doc.try_page_count().unwrap() as u32;
            doc.add_page(index).unwrap();
            doc.replace_page_contents(index, source.to_vec()).unwrap();
            let result = doc.get_page(index).unwrap().display_list();
            if allowed {
                assert!(result.is_ok());
            } else {
                assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
            }
        }
    }

    #[test]
    fn compatible_diagnostic_count_has_exact_limit() {
        let options = ParseOptions {
            limits: Limits {
                max_content_diagnostics: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = PdfDocument::open_with_options("../tests/path-test.pdf", options).unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        doc.replace_page_contents(index, b"unknownA unknownB".to_vec())
            .unwrap();
        assert_eq!(
            doc.get_page(index)
                .unwrap()
                .display_list()
                .unwrap()
                .diagnostics()
                .len(),
            2
        );
        doc.replace_page_contents(index, b"unknownA unknownB unknownC".to_vec())
            .unwrap();
        assert!(matches!(
            doc.get_page(index).unwrap().display_list(),
            Err(PdfError::ResourceLimit(_))
        ));
        doc.replace_page_contents(index, b"q Q".to_vec()).unwrap();
        assert!(doc
            .get_page(index)
            .unwrap()
            .display_list()
            .unwrap()
            .diagnostics()
            .is_empty());
    }

    #[test]
    fn colorspace_switch_resets_only_the_corresponding_color() {
        use crate::page::page_object::PageObject;
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for (source, fill, stroke) in [
            (
                "1 0 0 rg 0.5 G /DeviceRGB cs 0 0 10 10 re B",
                vec![0.0, 0.0, 0.0],
                vec![0.5],
            ),
            (
                "1 0 0 RG 0.5 g /DeviceCMYK CS 0 0 10 10 re B",
                vec![0.5],
                vec![0.0, 0.0, 0.0, 1.0],
            ),
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            let list = doc.get_page(index).unwrap().display_list().unwrap();
            let PageObject::Path(path) = &list.objects()[0] else {
                panic!("expected path")
            };
            let state = &path.graphic_state().color_state;
            assert_eq!(state.fill_color_value.values(), fill);
            assert_eq!(state.stroke_color_value.values(), stroke);
        }
    }

    #[test]
    fn dynamic_color_counts_fail_without_waiting_for_paint() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for source in [
            "sc",
            "0 1 SC",
            "/DeviceRGB cs 0 1 sc",
            "/DeviceRGB CS 0 1 0 1 SCN",
            "/DeviceCMYK cs 0 0 0 scn",
            "/DeviceGray CS /bad SC",
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(
                matches!(
                    doc.get_page(index).unwrap().display_list(),
                    Err(PdfError::PageOperatorError(_))
                ),
                "{source}"
            );
        }
        for source in [
            "0 sc 1 SC",
            "/DeviceRGB cs 0 1 0 scn",
            "/DeviceRGB CS 1 0 1 SC",
            "/DeviceCMYK cs 0 0 0 1 sc",
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(
                doc.get_page(index).unwrap().display_list().is_ok(),
                "{source}"
            );
        }
    }

    #[test]
    fn fixed_operand_counts_reject_missing_and_extra_parameters() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for source in [
            "1 q Q",
            "1 2 w",
            "0 m",
            "0 0 1 l",
            "0 0 1 1 2 re",
            "1 0 0 1 0 cm",
            "1 BT ET",
            "BT 1 T* ET",
            "1 2 3 4 rg",
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(
                matches!(
                    doc.get_page(index).unwrap().display_list(),
                    Err(PdfError::PageOperatorError(_))
                ),
                "{source}"
            );
        }
        doc.replace_page_contents(index, b"q 1 w 0 0 m 1 1 l S Q BT T* ET".to_vec())
            .unwrap();
        assert!(doc.get_page(index).unwrap().display_list().is_ok());
    }

    #[test]
    fn content_stream_count_checks_empty_streams_before_decoding() {
        use crate::{
            document::cross_ref_table::CrossRefTable, objects::pdf_array::PdfArray, page::PdfPage,
            parser::parser::PdfParser, pdf_context::PDFContext,
        };
        let options = ParseOptions {
            limits: Limits {
                max_objects: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let reader = StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap();
        let ctx = PDFContext::try_new(
            PdfParser::new(SyntaxParser::with_options(reader, options)),
            CrossRefTable::new(std::collections::HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        for (count, allowed) in [(0, true), (2, true), (3, false)] {
            let mut contents = PdfArray::default();
            for _ in 0..count {
                contents.add_obj(PdfObject::PdfStream(PdfStream::new(
                    PdfDict::default(),
                    Vec::new(),
                )));
            }
            let mut page = PdfDict::default();
            page.insert("Contents".into(), PdfObject::PdfArray(contents));
            let mut page = PdfPage::new(PdfObject::PdfDict(page), &ctx);
            let result = page.display_list();
            if allowed {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(matches!(result, Err(PdfError::ResourceLimit(_))));
            }
        }
    }

    #[test]
    fn xobject_subtype_references_and_unsupported_errors() {
        use crate::{
            objects::{pdf_name::PdfName, pdf_reference::PdfReference},
            page::PdfPage,
        };
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        for subtype in ["Form", "PS"] {
            let subtype_id = doc.insert_object(PdfObject::PdfName(PdfName::new(subtype.into())));
            let mut dict = PdfDict::default();
            if subtype == "Form" {
                let mut bounds = crate::objects::pdf_array::PdfArray::default();
                for value in [0., 0., 10., 10.] {
                    bounds.add_obj(PdfObject::PdfNumber(
                        crate::objects::pdf_number::PdfNumber::new(value),
                    ));
                }
                dict.insert("BBox".into(), PdfObject::PdfArray(bounds));
            }
            dict.insert(
                "Subtype".into(),
                PdfObject::PdfReference(PdfReference::from_id(subtype_id)),
            );
            let mut xobjects = PdfDict::default();
            xobjects.insert(
                "X".into(),
                PdfObject::PdfStream(PdfStream::new(dict, vec![])),
            );
            let mut resources = PdfDict::default();
            resources.insert("XObject".into(), PdfObject::PdfDict(xobjects));
            let mut page = PdfDict::default();
            page.insert(
                "Type".into(),
                PdfObject::PdfName(PdfName::new("Page".into())),
            );
            page.insert("Resources".into(), PdfObject::PdfDict(resources));
            let contents_id = doc.insert_object(PdfObject::PdfStream(PdfStream::new(
                PdfDict::default(),
                b"/X Do".to_vec(),
            )));
            page.insert(
                "Contents".into(),
                PdfObject::PdfReference(PdfReference::from_id(contents_id)),
            );
            let mut page = PdfPage::new(PdfObject::PdfDict(page), doc.ctx());
            let result = page.display_list();
            if subtype == "Form" {
                assert!(result.is_ok(), "{result:?}");
                continue;
            }
            assert!(
                matches!(result, Err(PdfError::UnsupportedFeature(_))),
                "{result:?}"
            );
        }
    }

    #[test]
    fn image_requires_stream_and_checks_explicit_subtype() {
        use crate::{objects::pdf_name::PdfName, page::pdf_image::PdfImage};
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        for object in [PdfObject::PdfNull, PdfObject::PdfDict(PdfDict::default())] {
            let error = PdfImage::try_new("image".into(), &object, doc.ctx()).unwrap_err();
            assert!(error.to_string().contains("must be a stream"));
        }
        let mut dict = PdfDict::default();
        dict.insert("Width".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        dict.insert("Height".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        dict.insert(
            "ColorSpace".into(),
            PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
        );
        for subtype in [
            PdfObject::PdfNull,
            PdfObject::PdfName(PdfName::new("Form".into())),
        ] {
            dict.insert("Subtype".into(), subtype);
            let image = PdfObject::PdfStream(PdfStream::new(dict.clone(), vec![1, 2, 3]));
            assert!(matches!(
                PdfImage::try_new("image".into(), &image, doc.ctx()),
                Err(PdfError::ObjectError(_))
            ));
        }
        dict.insert(
            "Subtype".into(),
            PdfObject::PdfName(PdfName::new("Image".into())),
        );
        let image = PdfObject::PdfStream(PdfStream::new(dict, vec![1, 2, 3]));
        assert!(PdfImage::try_new("image".into(), &image, doc.ctx()).is_ok());
    }

    #[test]
    fn image_bit_depth_validates_packed_sample_lengths() {
        use crate::{objects::pdf_name::PdfName, page::pdf_image::PdfImage};
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let mut dict = PdfDict::default();
        dict.insert("Width".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        dict.insert("Height".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        dict.insert(
            "ColorSpace".into(),
            PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
        );
        for bits in [
            PdfObject::PdfNull,
            PdfObject::PdfNumber(PdfNumber::new(-1.0)),
            PdfObject::PdfNumber(PdfNumber::new(0.0)),
            PdfObject::PdfNumber(PdfNumber::new(3.0)),
            PdfObject::PdfNumber(PdfNumber::new(8.5)),
        ] {
            dict.insert("BitsPerComponent".into(), bits);
            let image = PdfObject::PdfStream(PdfStream::new(dict.clone(), vec![1, 2, 3]));
            assert!(matches!(
                PdfImage::try_new("image".into(), &image, doc.ctx()),
                Err(PdfError::ObjectError(_))
            ));
        }
        for (bits, bytes) in [
            (1.0, vec![0]),
            (2.0, vec![0]),
            (4.0, vec![0, 0]),
            (16.0, vec![0; 6]),
        ] {
            dict.insert(
                "BitsPerComponent".into(),
                PdfObject::PdfNumber(PdfNumber::new(bits)),
            );
            let image = PdfObject::PdfStream(PdfStream::new(dict.clone(), bytes));
            assert!(PdfImage::try_new("image".into(), &image, doc.ctx()).is_ok());
        }
        dict.insert(
            "BitsPerComponent".into(),
            PdfObject::PdfNumber(PdfNumber::new(8.0)),
        );
        let image = PdfObject::PdfStream(PdfStream::new(dict, vec![1, 2, 3]));
        assert!(PdfImage::try_new("image".into(), &image, doc.ctx()).is_ok());
    }

    #[test]
    fn image_masks_and_decode_are_not_silently_ignored() {
        use crate::{
            objects::{pdf_bool::PdfBool, pdf_name::PdfName},
            page::pdf_image::PdfImage,
        };
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let mut base = PdfDict::default();
        base.insert("Width".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        base.insert("Height".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        base.insert(
            "ColorSpace".into(),
            PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
        );
        for key in ["Mask", "SMask", "Decode", "ImageMask"] {
            let mut dict = base.clone();
            dict.insert(key.into(), PdfObject::PdfBool(PdfBool::new(b"true")));
            let image = PdfObject::PdfStream(PdfStream::new(dict, vec![1, 2, 3]));
            let result = PdfImage::try_new("image".into(), &image, doc.ctx());
            if matches!(key, "SMask" | "Decode") {
                assert!(matches!(result, Err(PdfError::ObjectError(_))));
            } else {
                assert!(matches!(result, Err(PdfError::UnsupportedFeature(_))));
            }
        }
        let mut dict = base.clone();
        dict.insert(
            "ImageMask".into(),
            PdfObject::PdfNumber(PdfNumber::new(1.0)),
        );
        assert!(matches!(
            PdfImage::try_new(
                "image".into(),
                &PdfObject::PdfStream(PdfStream::new(dict, vec![])),
                doc.ctx()
            ),
            Err(PdfError::ObjectError(_))
        ));
        for key in ["Mask", "SMask", "Decode"] {
            base.insert(key.into(), PdfObject::PdfNull);
        }
        base.insert(
            "ImageMask".into(),
            PdfObject::PdfBool(PdfBool::new(b"false")),
        );
        let image = PdfObject::PdfStream(PdfStream::new(base, vec![1, 2, 3]));
        assert_eq!(
            PdfImage::try_new("image".into(), &image, doc.ctx())
                .unwrap()
                .rgba_buffer()
                .unwrap(),
            [1, 2, 3, 255]
        );
    }

    #[test]
    fn image_rgba_budget_is_checked_before_filter_decoding() {
        use crate::{objects::pdf_name::PdfName, page::pdf_image::PdfImage};
        let options = ParseOptions {
            limits: Limits {
                max_decoded_bytes: 3,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = PdfDocument::open_with_options("../tests/path-test.pdf", options).unwrap();
        let mut dict = PdfDict::default();
        dict.insert("Width".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        dict.insert("Height".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        dict.insert(
            "ColorSpace".into(),
            PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
        );
        dict.insert(
            "Filter".into(),
            PdfObject::PdfName(PdfName::new("UnsupportedFilter".into())),
        );
        let image = PdfObject::PdfStream(PdfStream::new(dict, vec![]));
        assert!(matches!(
            PdfImage::try_new("image".into(), &image, doc.ctx()),
            Err(PdfError::ResourceLimit(_))
        ));
        let options = ParseOptions {
            limits: Limits {
                max_decoded_bytes: 4,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = PdfDocument::open_with_options("../tests/path-test.pdf", options).unwrap();
        let mut dict = PdfDict::default();
        dict.insert("Width".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        dict.insert("Height".into(), PdfObject::PdfNumber(PdfNumber::new(1.0)));
        dict.insert(
            "ColorSpace".into(),
            PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
        );
        let image = PdfObject::PdfStream(PdfStream::new(dict, vec![1, 2, 3]));
        assert_eq!(
            PdfImage::try_new("image".into(), &image, doc.ctx())
                .unwrap()
                .rgba_buffer()
                .unwrap(),
            [1, 2, 3, 255]
        );
    }

    #[test]
    fn oversized_image_is_rejected_before_decoding() {
        let options = ParseOptions {
            limits: Limits {
                max_image_pixels: 16,
                ..Default::default()
            },
            ..Default::default()
        };
        let bytes = std::fs::read("../tests/path-test.pdf").unwrap();
        let doc = PdfDocument::from_reader_with_options(Cursor::new(bytes), options).unwrap();
        let mut dict = PdfDict::default();
        dict.insert("Width".into(), PdfObject::PdfNumber(PdfNumber::new(5.0)));
        dict.insert("Height".into(), PdfObject::PdfNumber(PdfNumber::new(5.0)));
        let image = PdfObject::PdfStream(PdfStream::new(dict, Vec::new()));
        assert!(matches!(
            crate::page::pdf_image::PdfImage::try_new("image".into(), &image, doc.ctx()),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn object_stream_uses_custom_count_limit() {
        let mut dict = PdfDict::default();
        dict.insert("First".into(), PdfObject::PdfNumber(PdfNumber::new(0.0)));
        dict.insert("N".into(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
        let options = ParseOptions {
            limits: Limits {
                max_objects: 1,
                ..Default::default()
            },
            ..Default::default()
        };
        let mut stream = crate::objects::object_streams::ObjectStreams::try_new_with_options(
            &dict,
            Vec::new(),
            options,
        )
        .unwrap();
        assert!(matches!(
            stream.parse_objects(),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn page_content_aggregate_and_recursive_globals_are_limited() {
        use crate::objects::{pdf_name::PdfName, pdf_reference::PdfReference};
        let bytes = std::fs::read("../tests/path-test.pdf").unwrap();
        let options = ParseOptions {
            limits: Limits {
                max_decoded_bytes: 4,
                max_depth: 8,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = PdfDocument::from_bytes_with_options(bytes, options).unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        doc.replace_page_contents(index, b"q Q".to_vec()).unwrap();
        doc.append_page_contents(index, b"q Q".to_vec()).unwrap();
        assert!(matches!(
            doc.get_page(index).unwrap().display_list(),
            Err(PdfError::ResourceLimit(_))
        ));
        let id = doc.insert_object(PdfObject::PdfNull);
        let mut params = PdfDict::default();
        params.insert(
            "JBIG2Globals".into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        );
        let mut dict = PdfDict::default();
        dict.insert(
            "Filter".into(),
            PdfObject::PdfName(PdfName::new("JBIG2Decode".into())),
        );
        dict.insert("DecodeParms".into(), PdfObject::PdfDict(params));
        let stream = PdfStream::new(dict, Vec::new());
        doc.update_object(id, PdfObject::PdfStream(stream.clone()))
            .unwrap();
        for _ in 0..2 {
            assert!(matches!(
                stream.decode_data(doc.ctx()),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert!(PdfStream::new(PdfDict::default(), b"ok".to_vec())
            .decode_data(doc.ctx())
            .is_ok());
    }

    #[test]
    fn raw_stream_limit_and_retained_cache_budget_are_enforced() {
        let options = ParseOptions {
            limits: Limits {
                max_raw_stream_bytes: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        for source in [
            b"1 0 obj << /Length 3 >> stream\nabc\nendstream\nendobj".as_slice(),
            b"1 0 obj << >> stream\nabc\nendstream\nendobj".as_slice(),
        ] {
            let reader = StreamReader::try_new(Cursor::new(source.to_vec())).unwrap();
            let mut syntax = SyntaxParser::with_options(reader, options);
            assert!(matches!(
                syntax.get_indirect_object(),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let first = doc.insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            b"abc".to_vec(),
        )));
        let second = doc.insert_object(PdfObject::PdfStream(PdfStream::new(
            PdfDict::default(),
            b"def".to_vec(),
        )));
        let bytes = doc.save_to_bytes().unwrap();
        let options = ParseOptions {
            limits: Limits {
                max_cached_stream_bytes: 5,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = PdfDocument::from_bytes_with_options(bytes, options).unwrap();
        assert!(doc.resolve_object(first).is_ok());
        for _ in 0..2 {
            assert!(matches!(
                doc.resolve_object(second),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert!(doc.resolve_object(first).is_ok());
    }

    #[test]
    fn graphics_stack_underflow_depth_and_operation_count_return_errors() {
        let options = ParseOptions {
            limits: Limits {
                max_depth: 8,
                max_content_operations: 20,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = PdfDocument::open_with_options("../tests/path-test.pdf", options).unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        doc.replace_page_contents(index, b"Q".to_vec()).unwrap();
        assert!(matches!(
            doc.get_page(index).unwrap().display_list(),
            Err(PdfError::PageOperatorError(_))
        ));
        doc.replace_page_contents(index, b"q ".repeat(9)).unwrap();
        assert!(matches!(
            doc.get_page(index).unwrap().display_list(),
            Err(PdfError::ResourceLimit(_))
        ));
        let mut boundary = b"q ".repeat(8);
        boundary.extend_from_slice(&b"Q ".repeat(8));
        doc.replace_page_contents(index, boundary).unwrap();
        assert!(doc.get_page(index).unwrap().display_list().is_ok());
        doc.replace_page_contents(index, b"q Q ".repeat(11))
            .unwrap();
        assert!(matches!(
            doc.get_page(index).unwrap().display_list(),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn clipping_limit_honors_exact_boundary_and_graphics_restore() {
        let options = ParseOptions {
            limits: Limits {
                max_clip_paths: 2,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = PdfDocument::open_with_options("../tests/path-test.pdf", options).unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for source in ["W n W n W n", "W* n W* n W* n"] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(matches!(
                doc.get_page(index).unwrap().display_list(),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        for source in ["W n W n 0 0 10 10 re S", "q W n W n Q W n 0 0 10 10 re S"] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(doc.get_page(index).unwrap().display_list().is_ok());
        }
    }

    #[test]
    fn graphics_scalar_ranges_and_dash_cycles_are_checked() {
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for source in [
            "-1 i",
            "101 i",
            "[300000000000000000000000000000000000000 300000000000000000000000000000000000000] 0 d",
            "[200000000000000000000000000000000000000] 0 d",
            "[0 0] 0 d",
            "[1] -1 d",
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(
                matches!(
                    doc.get_page(index).unwrap().display_list(),
                    Err(PdfError::PageOperatorError(_))
                ),
                "{source}"
            );
        }
        for source in ["0 i", "100 i", "[] 0 d", "[0 1] 0 d", "[1 2 3] 4 d"] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(
                doc.get_page(index).unwrap().display_list().is_ok(),
                "{source}"
            );
        }
    }

    #[test]
    fn path_segment_budget_counts_expansion_and_discarded_paths() {
        let options = ParseOptions {
            limits: Limits {
                max_path_segments: 5,
                ..Default::default()
            },
            ..Default::default()
        };
        let doc = PdfDocument::open_with_options("../tests/path-test.pdf", options).unwrap();
        let index = doc.try_page_count().unwrap() as u32;
        doc.add_page(index).unwrap();
        for source in [
            "0 0 1 1 re 2 2 m",
            "0 0 1 1 re n 2 2 m",
            "0 0 m 1 1 l 2 2 l 3 3 l 4 4 l h",
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(matches!(
                doc.get_page(index).unwrap().display_list(),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        for source in [
            "0 0 1 1 re S",
            "0 0 m 1 1 l 2 2 l 3 3 l h h S",
            "0 0 m 1 1 2 2 3 3 c 4 4 5 5 v 6 6 7 7 y h S",
        ] {
            doc.replace_page_contents(index, source.as_bytes().to_vec())
                .unwrap();
            assert!(doc.get_page(index).unwrap().display_list().is_ok());
        }
    }

    #[test]
    fn token_and_container_limits_reject_excessive_input() {
        for source in [
            b"/abcdefgh ".as_slice(),
            b"(abcdefgh) ".as_slice(),
            b"<0102030405> ".as_slice(),
        ] {
            let options = ParseOptions {
                limits: Limits {
                    max_token_bytes: 4,
                    ..Default::default()
                },
                ..Default::default()
            };
            let reader = StreamReader::try_new(Cursor::new(source.to_vec())).unwrap();
            let mut syntax = SyntaxParser::with_options(reader, options);
            assert!(matches!(
                syntax.get_object(),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        for source in [
            b"[null null] ".as_slice(),
            b"<< /A null /B null >> ".as_slice(),
        ] {
            let options = ParseOptions {
                limits: Limits {
                    max_objects: 1,
                    ..Default::default()
                },
                ..Default::default()
            };
            let reader = StreamReader::try_new(Cursor::new(source.to_vec())).unwrap();
            let mut syntax = SyntaxParser::with_options(reader, options);
            assert!(matches!(
                syntax.get_object(),
                Err(PdfError::ResourceLimit(_))
            ));
        }
    }
}
