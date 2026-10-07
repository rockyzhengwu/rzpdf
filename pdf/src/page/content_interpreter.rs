use std::io::{Read, Seek};

use crate::{
    color::{
        colorspace::ColorSpace, device_cmyk::DeviceCmyk, device_gray::DeviceGray,
        device_rgb::DeviceRgb, value::ColorValue,
    },
    device::{Device, trace::TraceDevice},
    error::{PdfError, PdfResult},
    font::{WritingMode, pdf_font::Font},
    geom::{matrix::Matrix, point::Point},
    objects::PdfObject,
    page::{
        PdfPage,
        all_state::AllState,
        content_parser::ContentParser,
        display_list::DisplayList,
        graph_state::{LineCap, LineJoin},
        graphic_state::FillType,
        image_object::{ImageDrawSource, ImageObject},
        operator::Operator,
        path_object::{PathDrawSource, PathObject},
        pdf_image::PdfImage,
        text_object::{CharItem, TextDrawSource, TextObject},
    },
    path::pdf_path::{Bezier, PdfPath, Segment, SubPath},
    pdf_context::PDFContext,
};

#[derive(Debug)]
pub struct Interpreter<'a, R: Seek + Read> {
    page: &'a PdfPage<'a, R>,
    ctx: &'a PDFContext<R>,
    parser: ContentParser,
    state_stack: Vec<AllState>,
    stack_clip_segments: usize,
    diagnostics: Vec<crate::page::diagnostic::ContentDiagnostic>,
    diagnostic_bytes: usize,
    current_state: AllState,
    current_path: PdfPath,
    current_path_point: Point,
    path_clip_type: FillType,
    path_start: Point,
    in_text_object: bool,
    text_chars: usize,
    path_segments: usize,
    operation_index: usize,
    operation_byte_offset: Option<u64>,
    path_source_indices: Vec<usize>,
    resource_scope: Option<crate::objects::pdf_dict::PdfDict>,
    execution_budget: super::execution_budget::ExecutionBudget,
    form_depth: usize,
    active_forms: Vec<crate::objects::object_id::ObjectId>,
    invocation_path: Vec<super::source_provenance::FormInvocation>,
}

#[cfg(test)]
mod resource_scope_tests {
    use super::*;
    #[test]
    fn direct_inner_form_is_located_through_parent_resource_scope() {
        use crate::{
            document::{PdfDocument, resource_manager::ResourceKind},
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            page::page_object::PageObject,
            parser::syntax::SyntaxParser,
        };
        use std::io::Cursor;
        let dictionary = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap()
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let inner = PdfObject::PdfStream(
            PdfStream::from_decoded_data(
                dictionary("<< /Subtype /Form /BBox [0 0 10 10] /Resources << >> >>"),
                b"0 0 2 2 re f".to_vec(),
            )
            .unwrap(),
        );
        let mut entries = PdfDict::default();
        entries.insert("Inner".into(), inner);
        let mut resources = PdfDict::default();
        resources.insert("XObject".into(), PdfObject::PdfDict(entries));
        let mut outer = dictionary("<< /Type /XObject /Subtype /Form /BBox [0 0 10 10] >>");
        outer.insert("Resources".into(), PdfObject::PdfDict(resources));
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(outer, b"/Inner Do".to_vec()).unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
            .unwrap();
        let page = doc.get_page(0).unwrap();
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        let PageObject::Path(path) = &list.objects()[0] else {
            panic!("missing direct-form path")
        };
        let source = path.nested_source().unwrap();
        assert_eq!(source.invocations.len(), 2);
        assert_eq!(source.invocations[0].form_id, Some(id));
        assert_eq!(source.invocations[1].form_id, None);
        let root = page.content_program().unwrap();
        assert_eq!(
            page.nested_content_program(&root, source)
                .unwrap()
                .source_bytes(),
            b"0 0 2 2 re f"
        );
        let mut forged = source.clone();
        forged.invocations[1].form_id = Some(id);
        assert!(page.nested_content_program(&root, &forged).is_err());
    }
    #[test]
    fn actual_nine_form_chain_checks_configured_depth_and_accumulates_geometry() {
        use crate::{
            document::{PdfDocument, resource_manager::ResourceKind},
            io::stream_reader::StreamReader,
            objects::pdf_stream::PdfStream,
            page::page_object::PageObject,
            parser::{
                options::{Limits, ParseOptions},
                syntax::{ParseMode, SyntaxParser},
            },
        };
        use std::io::Cursor;
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let mut next: Option<crate::objects::object_id::ObjectId> = None;
        for _ in 0..9 {
            let scope = next.map_or_else(
                || "<< >>".to_string(),
                |id| format!("<< /XObject << /Child {} 0 R >> >>", id.obj_num()),
            );
            let dictionary = format!(
                "<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] /Matrix [1 0 0 1 1 2] /Resources {scope} >>"
            );
            let mut parser = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(dictionary.into_bytes())).unwrap(),
            );
            let dictionary = parser.get_object().unwrap().into_dict().unwrap();
            let content = if next.is_some() {
                b"/Child Do".to_vec()
            } else {
                b"0 0 2 2 re f".to_vec()
            };
            next = Some(
                doc.try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(dictionary, content).unwrap(),
                ))
                .unwrap(),
            );
        }
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, next.unwrap())
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
            .unwrap();
        let bytes = doc.save_to_bytes().unwrap();
        let reopened = PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).unwrap();
        for document in [&doc, &reopened] {
            let list = document.get_page(0).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 1);
            let PageObject::Path(path) = &list.objects()[0] else {
                panic!("missing leaf path")
            };
            assert_eq!(path.graphic_state().clip_path.elements.len(), 9);
            let origin = path
                .matrix()
                .checked_transform(&Point::new(0., 0.))
                .unwrap();
            assert_eq!(
                (origin.x(), origin.y()),
                (
                    9.,
                    document
                        .get_page(0)
                        .unwrap()
                        .geometry()
                        .unwrap()
                        .display_height
                        - 18.
                )
            );
            assert!(path.source().is_none());
            let nested = path.nested_source().unwrap();
            assert_eq!(nested.invocations.len(), 9);
            assert_eq!(nested.operation_index, 1);
            assert_eq!(nested.geometry_operation_indices, vec![0]);
            assert!(nested.invocations.iter().all(|call| call.form_id.is_some()
                && call.operation_index == 0
                && call.operator_byte_offset.is_some()));
            let page = document.get_page(0).unwrap();
            let root = page.content_program().unwrap();
            let leaf = page.nested_content_program(&root, nested).unwrap();
            let prepared = page.prepare_form_chain(&root, nested).unwrap();
            assert_eq!(prepared.len(), 9);
            for (index, form) in prepared.iter().enumerate() {
                assert_eq!(
                    form.dictionary
                        .get("Subtype")
                        .and_then(PdfObject::as_name)
                        .unwrap()
                        .name(),
                    "Form"
                );
                assert_eq!(form.resource_scope.get("XObject").is_some(), index < 8);
                assert_eq!(
                    form.program.operations().len(),
                    if index < 8 { 1 } else { 2 }
                );
            }
            assert_eq!(leaf.source_bytes(), b"0 0 2 2 re f");
            assert_eq!(
                leaf.operations()[nested.operation_index].operator.name(),
                "f"
            );
            assert!(leaf.source_stamp().is_none());
            let mut invalid = nested.clone();
            invalid.geometry_operation_indices = vec![1];
            assert!(page.nested_content_program(&root, &invalid).is_err());
            let mut invalid = nested.clone();
            invalid.geometry_operation_indices = vec![0, 0];
            assert!(page.nested_content_program(&root, &invalid).is_err());
            let mut invalid = nested.clone();
            invalid.invocations[0].form_id = None;
            assert!(page.nested_content_program(&root, &invalid).is_err());
            let mut invalid = nested.clone();
            invalid.invocations[0].operator_byte_offset = None;
            assert!(page.nested_content_program(&root, &invalid).is_err());
            let mut invalid = nested.clone();
            invalid.operation_index = usize::MAX;
            assert!(page.nested_content_program(&root, &invalid).is_err());
            let mut invalid = nested.clone();
            invalid.operator_byte_offset = None;
            assert!(page.nested_content_program(&root, &invalid).is_err());
            let mut invalid = nested.clone();
            invalid.invocations[4].operation_index = usize::MAX;
            assert!(page.nested_content_program(&root, &invalid).is_err());
            assert!(page.nested_content_program(&leaf, nested).is_err());
            let other = if std::ptr::eq(document, &doc) {
                &reopened
            } else {
                &doc
            };
            assert!(
                page.nested_content_program(
                    &other.get_page(0).unwrap().content_program().unwrap(),
                    nested
                )
                .is_err()
            );
        }
        for (maximum, allowed) in [(8, false), (9, true)] {
            let document = PdfDocument::from_bytes_with_options(
                bytes.clone(),
                ParseOptions {
                    limits: Limits {
                        max_depth: maximum,
                        ..Limits::default()
                    },
                    ..ParseOptions::default()
                },
            )
            .unwrap();
            let result = document.get_page(0).unwrap().display_list();
            if allowed {
                assert!(result.is_ok(), "{result:?}");
            } else {
                assert!(
                    matches!(result,Err(PdfError::ResourceLimit(ref message)) if message=="Form invocation depth"),
                    "{result:?}"
                );
            }
        }
    }
    #[test]
    fn form_execution_isolates_path_color_clip_and_bounds_repeated_work() {
        use crate::{
            document::{PdfDocument, resource_manager::ResourceKind},
            io::stream_reader::StreamReader,
            objects::pdf_stream::PdfStream,
            page::page_object::PageObject,
            parser::{
                options::{Limits, ParseOptions},
                syntax::{ParseMode, SyntaxParser},
            },
        };
        use std::io::Cursor;
        let dict = |text: &str| {
            SyntaxParser::new(StreamReader::try_new(Cursor::new(text.as_bytes().to_vec())).unwrap())
                .get_object()
                .unwrap()
                .into_dict()
                .unwrap()
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let form_dict = dict(
            "<< /Type /XObject /Subtype /Form /BBox [0 0 10 20] /Matrix [1 0 0 1 3 4] /Resources << >> >>",
        );
        let body = b"1 0 0 rg 0 0 20 20 re f";
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(form_dict.clone(), body.to_vec()).unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(0, format!("0 1 0 rg 1 2 m /{name} Do 4 5 l S").into_bytes())
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &reopened] {
            let list = document.get_page(0).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 2);
            let PageObject::Path(child) = &list.objects()[0] else {
                panic!("missing child path")
            };
            let PageObject::Path(parent) = &list.objects()[1] else {
                panic!("missing parent path")
            };
            assert_eq!(
                child.graphic_state().color_state.fill_color_value.values(),
                [1., 0., 0.]
            );
            assert_eq!(
                parent.graphic_state().color_state.fill_color_value.values(),
                [0., 1., 0.]
            );
            assert_eq!(child.graphic_state().clip_path.elements.len(), 1);
            assert!(parent.graphic_state().clip_path.is_empty());
            assert_eq!(parent.pdf_path().subpaths()[0].segments().len(), 2);
            assert!(child.source().is_none());
            assert!(parent.source().is_some());
            let origin = child
                .matrix()
                .checked_transform(&Point::new(0., 0.))
                .unwrap();
            assert_eq!(
                (origin.x(), origin.y()),
                (
                    3.,
                    document
                        .get_page(0)
                        .unwrap()
                        .geometry()
                        .unwrap()
                        .display_height
                        - 4.
                )
            );
        }
        doc.replace_page_contents(0, format!("/{name} Do /{name} Do").into_bytes())
            .unwrap();
        // Repeated references after returning are legal, not a cycle.
        let repeated = doc.get_page(0).unwrap().display_list().unwrap();
        for (index, object) in repeated.objects().iter().enumerate() {
            let PageObject::Path(path) = object else {
                panic!("missing repeated path")
            };
            let nested = path.nested_source().unwrap();
            assert_eq!(nested.invocations.len(), 1);
            assert_eq!(nested.invocations[0].form_id, Some(id));
            assert_eq!(nested.invocations[0].operation_index, index);
            assert_eq!(nested.operation_index, 2);
            assert_eq!(nested.geometry_operation_indices, vec![1]);
            let page = doc.get_page(0).unwrap();
            let root = page.content_program().unwrap();
            assert!(page.nested_content_program(&root, nested).is_ok());
            let mut invalid = nested.clone();
            invalid.geometry_operation_indices = vec![0];
            assert!(page.nested_content_program(&root, &invalid).is_err());
        }
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .display_list()
                .unwrap()
                .objects()
                .len(),
            2
        );
        // Exercise the actual Form decode path using a nearly exhausted shared
        // work budget, without restricting unrelated document parser storage.
        let page = doc.get_page(0).unwrap();
        let parser = ContentParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(format!("/{name} Do /{name} Do").into_bytes()))
                .unwrap(),
        ));
        let mut interpreter = Interpreter::new(&page, doc.ctx(), parser);
        interpreter
            .execution_budget
            .decoded(
                doc.ctx().limits().max_decoded_bytes - body.len(),
                &doc.ctx().limits(),
            )
            .unwrap();
        let error = interpreter.run_to_display_list().unwrap_err();
        assert!(
            matches!(error,PdfError::ResourceLimit(ref message) if message=="content invocation decoded work")
        );
        // Hard implementation-depth guard also applies when configured limits
        // are permissive; it prevents unchecked native recursion.
        let parser = ContentParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(format!("/{name} Do").into_bytes())).unwrap(),
        ));
        let mut interpreter = Interpreter::new(&page, doc.ctx(), parser);
        interpreter.form_depth = 128;
        let error = interpreter.run_to_display_list().unwrap_err();
        assert!(
            matches!(error,PdfError::ResourceLimit(ref message) if message=="Form implementation depth (128)")
        );
        let bytes = doc.save_to_bytes().unwrap();
        let options = ParseOptions {
            limits: Limits {
                max_content_operations: 7,
                ..Limits::default()
            },
            ..ParseOptions::default()
        };
        let limited = PdfDocument::from_bytes_with_options(bytes.clone(), options).unwrap();
        assert!(matches!(
            limited.get_page(0).unwrap().display_list(),
            Err(PdfError::ResourceLimit(_))
        ));
        let path_limited = PdfDocument::from_bytes_with_options(
            bytes,
            ParseOptions {
                limits: Limits {
                    max_path_segments: 5,
                    ..Limits::default()
                },
                ..ParseOptions::default()
            },
        )
        .unwrap();
        let mut page = path_limited.get_page(0).unwrap();
        page.geometry().unwrap();
        let parser = ContentParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(format!("/{name} Do /{name} Do").into_bytes()))
                .unwrap(),
        ));
        let mut interpreter = Interpreter::new(&page, path_limited.ctx(), parser);
        // Keep retained-display quotas independent so this assertion exercises
        // constructed work rather than a duplicated clip payload quota.
        let mut device = TraceDevice::for_display_list(Limits::default());
        let error = interpreter.run(&mut device).unwrap_err();
        assert!(
            matches!(error,PdfError::ResourceLimit(ref message) if message=="content invocation path segments"),
            "{error:?}"
        );
        // Self-call through the page scope: omitted Form Resources uses legacy page fallback.
        let cycle = dict("<< /Type /XObject /Subtype /Form /BBox [0 0 10 20] >>");
        doc.update_object(
            id,
            PdfObject::PdfStream(
                PdfStream::from_decoded_data(cycle, format!("/{name} Do").into_bytes()).unwrap(),
            ),
        )
        .unwrap();
        assert!(matches!(
            doc.get_page(0).unwrap().display_list(),
            Err(PdfError::PageResourceError(_))
        ));
        // An unbalanced child's stack fails without being mistaken for the parent's Q.
        doc.update_object(
            id,
            PdfObject::PdfStream(PdfStream::from_decoded_data(form_dict, b"q".to_vec()).unwrap()),
        )
        .unwrap();
        assert!(matches!(
            doc.get_page(0).unwrap().display_list(),
            Err(PdfError::PageOperatorError(_))
        ));
        let font = doc
            .resources()
            .register_standard_font(
                0,
                crate::document::resource_manager::StandardFont::Helvetica,
            )
            .unwrap();
        let text_form = dict("<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>");
        doc.update_object(
            id,
            PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    text_form,
                    format!("BT /{font} 12 Tf (AB) Tj [(A) 0 (B)] TJ ET").into_bytes(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let list = doc.get_page(0).unwrap().display_list().unwrap();
        assert_eq!(list.objects().len(), 4);
        let count: usize = list
            .objects()
            .iter()
            .map(|o| match o {
                PageObject::Text(t) => t.items.len(),
                _ => 0,
            })
            .sum();
        assert_eq!(count, 8);
        let text_saved =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &text_saved] {
            let report = document
                .get_page(0)
                .unwrap()
                .extract_characters_paint_order_report()
                .unwrap();
            assert_eq!(report.characters.len(), 8);
            let program = document.get_page(0).unwrap().content_program().unwrap();
            assert!(report.matches_program(&program));
            let other = if std::ptr::eq(document, &doc) {
                &text_saved
            } else {
                &doc
            };
            assert!(
                !report.matches_program(&other.get_page(0).unwrap().content_program().unwrap())
            );
            for (index, character) in report.characters.iter().enumerate() {
                assert!(character.source.is_none());
                let nested = character.nested_source.as_ref().unwrap();
                assert_eq!(nested.invocations[0].form_id, Some(id));
                assert_eq!(nested.invocations[0].operation_index, index / 4);
                assert_eq!(nested.operation_index, 2 + (index % 4) / 2);
            }
        }
        for (index, object) in list.objects().iter().enumerate() {
            let PageObject::Text(text) = object else {
                panic!("missing text")
            };
            assert!(text.source().is_none());
            let nested = text.nested_source().unwrap();
            assert_eq!(nested.invocations[0].operation_index, index / 2);
            assert_eq!(nested.operation_index, 2 + index % 2);
        }
        let text_limited = PdfDocument::from_bytes_with_options(
            doc.save_to_bytes().unwrap(),
            ParseOptions {
                limits: Limits {
                    max_text_chars: 6,
                    ..Limits::default()
                },
                ..ParseOptions::default()
            },
        )
        .unwrap();
        assert!(matches!(
            text_limited.get_page(0).unwrap().display_list(),
            Err(PdfError::ResourceLimit(_))
        ));
        let image = doc
            .resources()
            .embed_rgb_image(0, 1, 1, vec![255, 0, 0])
            .unwrap();
        let image_dict = dict("<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>");
        doc.update_object(
            id,
            PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    image_dict,
                    format!("10 0 0 10 0 0 cm /{image} Do").into_bytes(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let image_saved =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&doc, &image_saved] {
            let report = document
                .get_page(0)
                .unwrap()
                .extract_images_paint_order_report()
                .unwrap();
            assert_eq!(report.images.len(), 2);
            for (index, image) in report.images.iter().enumerate() {
                assert!(image.source().is_none());
                assert_eq!(
                    image.nested_source().unwrap().invocations[0].operation_index,
                    index
                );
            }
            let list = document.get_page(0).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 2);
            for (index, object) in list.objects().iter().enumerate() {
                let PageObject::Image(image) = object else {
                    panic!("missing image")
                };
                assert!(image.source().is_none());
                let nested = image.nested_source().unwrap();
                assert_eq!(nested.invocations[0].form_id, Some(id));
                assert_eq!(nested.invocations[0].operation_index, index);
                assert_eq!(nested.operation_index, 1);
                assert!(nested.operator_byte_offset.is_some());
            }
        }
        let old_report = doc
            .get_page(0)
            .unwrap()
            .extract_images_paint_order_report()
            .unwrap();
        let old_program = doc.get_page(0).unwrap().content_program().unwrap();
        assert!(old_report.matches_program(&old_program));
        let location = old_report.images[0].nested_source().unwrap();
        let leaf = doc
            .get_page(0)
            .unwrap()
            .nested_content_program(&old_program, location)
            .unwrap();
        assert_eq!(
            leaf.operations()[location.operation_index].operator.name(),
            "Do"
        );
        assert!(
            !old_report
                .matches_program(&image_saved.get_page(0).unwrap().content_program().unwrap())
        );
        // Change only the Form stream, leaving the page Contents unchanged.
        let changed = dict("<< /Type /XObject /Subtype /Form /BBox [0 0 100 100] >>");
        doc.update_object(
            id,
            PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    changed,
                    format!("11 0 0 11 0 0 cm /{image} Do").into_bytes(),
                )
                .unwrap(),
            ),
        )
        .unwrap();
        let current_program = doc.get_page(0).unwrap().content_program().unwrap();
        assert!(!old_report.matches_program(&current_program));
        assert!(
            doc.get_page(0)
                .unwrap()
                .nested_content_program(&old_program, location)
                .is_err()
        );
        assert!(old_report.matches_program(&old_program)); // historical snapshots still pair
        doc.copy_page(0, 1).unwrap();
        let current_report = doc
            .get_page(0)
            .unwrap()
            .extract_images_paint_order_report()
            .unwrap();
        assert!(
            current_report.matches_program(&doc.get_page(0).unwrap().content_program().unwrap())
        );
        assert!(
            !current_report.matches_program(&doc.get_page(1).unwrap().content_program().unwrap())
        );
    }
    #[test]
    fn explicit_interpreter_scope_never_falls_back_to_page_names() {
        use crate::{
            document::PdfDocument, io::stream_reader::StreamReader, parser::syntax::SyntaxParser,
        };
        use std::io::Cursor;
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let page_font = doc
            .resources()
            .register_standard_font(
                0,
                crate::document::resource_manager::StandardFont::Helvetica,
            )
            .unwrap();
        let page = doc.get_page(0).unwrap();
        let parser = || {
            ContentParser::new(SyntaxParser::new(
                StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
            ))
        };
        let scope = SyntaxParser::new(StreamReader::try_new(Cursor::new(b"<< /Font << /F1 << /Marker /Local >> >> /ColorSpace << /C /DeviceRGB >> /ExtGState << /G << /LW 2 >> >> /XObject << /Null null >> >>".to_vec())).unwrap()).get_object().unwrap().into_dict().unwrap();
        let interpreter = Interpreter::new(&page, doc.ctx(), parser()).with_resource_scope(scope);
        let local = interpreter.named_resource("Font", "F1").unwrap();
        assert_eq!(
            local.get_attr("Marker").unwrap().as_name().unwrap().name(),
            "Local"
        );
        assert_eq!(
            interpreter
                .named_resource("ColorSpace", "C")
                .unwrap()
                .as_name()
                .unwrap()
                .name(),
            "DeviceRGB"
        );
        assert!(
            interpreter
                .named_resource("ExtGState", "G")
                .unwrap()
                .as_dict()
                .is_some()
        );
        assert!(interpreter.named_resource("Font", "Missing").is_err());
        assert!(interpreter.named_resource("XObject", "Null").is_err());
        let empty = Interpreter::new(&page, doc.ctx(), parser())
            .with_resource_scope(crate::objects::pdf_dict::PdfDict::default());
        assert!(
            Interpreter::new(&page, doc.ctx(), parser())
                .named_resource("Font", &page_font)
                .is_ok()
        );
        assert!(empty.named_resource("Font", &page_font).is_err());
    }
}

impl<'a, R: Seek + Read> Interpreter<'a, R> {
    pub fn new(page: &'a PdfPage<'a, R>, ctx: &'a PDFContext<R>, parser: ContentParser) -> Self {
        Interpreter {
            invocation_path: Vec::new(),
            form_depth: 0,
            active_forms: Vec::new(),
            execution_budget: super::execution_budget::ExecutionBudget::new(),
            resource_scope: None,
            page,
            ctx,
            parser,
            current_state: AllState::default(),
            state_stack: Vec::new(),
            stack_clip_segments: 0,
            diagnostics: Vec::new(),
            diagnostic_bytes: 0,
            current_path: PdfPath::default(),
            current_path_point: Point::default(),
            path_clip_type: FillType::default(),
            path_start: Point::default(),
            in_text_object: false,
            text_chars: 0,
            path_segments: 0,
            operation_index: 0,
            operation_byte_offset: None,
            path_source_indices: Vec::new(),
        }
    }

    pub fn run(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        self.run_unfiltered(&mut crate::device::painting::PaintingDevice(device))
    }

    fn run_unfiltered(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        device.start_page(self.page.page_width, self.page.page_height)?;
        self.run_content(device)?;
        device.end_page()?;
        Ok(())
    }

    fn run_content(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        let mut operations = 0usize;
        loop {
            if let Some(op) = self.parser.read_operator()? {
                self.execution_budget.operation(&self.ctx.limits())?;
                self.operation_index = operations;
                self.operation_byte_offset = op.byte_offset();
                operations += 1;
                self.invoke_oeration(op, device)?;
            } else {
                break;
            }
        }
        if self.in_text_object {
            return Err(PdfError::PageOperatorError(
                "content ends before matching ET".into(),
            ));
        }
        if !self.state_stack.is_empty() {
            return Err(PdfError::PageOperatorError(
                "content ends before matching Q".into(),
            ));
        }
        Ok(())
    }

    /// Replace the entire lookup scope for an isolated content invocation.
    /// Does not merge missing entries with page resources.
    pub fn with_resource_scope(mut self, scope: crate::objects::pdf_dict::PdfDict) -> Self {
        self.resource_scope = Some(scope);
        self
    }

    fn named_resource(&self, category: &str, name: &str) -> PdfResult<PdfObject> {
        let value = self.resource_entry(category, name)?;
        let value = self.ctx.resolve(&value)?;
        if matches!(value, PdfObject::PdfNull) {
            return Err(PdfError::PageResourceError(format!(
                "Resource {category}/{name} is null"
            )));
        }
        Ok(value)
    }

    fn resource_entry(&self, category: &str, name: &str) -> PdfResult<PdfObject> {
        let page_resources;
        let scope = if let Some(scope) = &self.resource_scope {
            scope
        } else {
            page_resources = self.page.resource()?;
            &page_resources
        };
        let category_value = scope.get(category).ok_or_else(|| {
            PdfError::PageResourceError(format!("Resource category {category} missing"))
        })?;
        let entries = self.ctx.resolve(category_value)?;
        let entries = entries.as_dict().ok_or_else(|| {
            PdfError::PageResourceError(format!(
                "Resource category {category} must be a dictionary"
            ))
        })?;
        let value = entries.get(name).ok_or_else(|| {
            PdfError::PageResourceError(format!("Resource {category}/{name} missing"))
        })?;
        Ok(value.clone())
    }

    pub fn run_to_display_list(&mut self) -> PdfResult<DisplayList> {
        let mut device = TraceDevice::for_display_list(self.ctx.limits());
        self.run_unfiltered(&mut device)?;
        let mut list = device.into_display_list();
        list.set_provenance(self.page.source_provenance());
        list.set_diagnostics(self.diagnostics.clone());
        Ok(list)
    }

    pub fn diagnostics(&self) -> &[crate::page::diagnostic::ContentDiagnostic] {
        &self.diagnostics
    }

    fn invoke_oeration(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        if let Some(expected) = crate::page::opcode::operand_count(op.name()) {
            if op.num_operands() != expected {
                return Err(PdfError::PageOperatorError(format!(
                    "{} requires {expected} operands, got {}",
                    op.name(),
                    op.num_operands()
                )));
            }
        }
        let added_segments = match op.name() {
            "m" | "l" | "c" | "v" | "y" => 1,
            "re" => 5,
            "h" | "s" | "b" | "b*" => usize::from(
                self.current_path
                    .subpaths()
                    .last()
                    .is_some_and(|path| !path.is_closed()),
            ),
            _ => 0,
        };
        crate::filter::check_output_size(
            self.path_segments,
            added_segments,
            self.ctx.limits().max_path_segments,
            "page path segments",
        )?;
        if matches!(op.name(), "l" | "c" | "v" | "y") && self.current_path.is_empty() {
            return Err(PdfError::PageOperatorError(format!(
                "{} requires a current path point",
                op.name()
            )));
        }
        if matches!(
            op.name(),
            "Td" | "TD" | "Tm" | "T*" | "Tj" | "TJ" | "'" | "\""
        ) && !self.in_text_object
        {
            return Err(PdfError::PageOperatorError(format!(
                "{} outside BT/ET",
                op.name()
            )));
        }
        self.execution_budget
            .path_segments(added_segments, &self.ctx.limits())?;
        self.path_segments += added_segments;
        if matches!(op.name(), "m" | "l" | "c" | "v" | "y" | "re" | "h") {
            if self.path_source_indices.len() >= self.ctx.limits().max_content_operations {
                return Err(PdfError::ResourceLimit("path source operations".into()));
            }
            self.path_source_indices
                .try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("path source allocation".into()))?;
            self.path_source_indices.push(self.operation_index);
        }
        // println!("{:?}", op);
        match op.name() {
            "q" => self.save_sate()?,
            "Q" => self.restore_state()?,
            "cm" => self.concat_matrix(op)?,
            "w" => self.handle_linewidth(op)?,
            "J" => self.handle_linecap(op)?,
            "j" => self.handle_linejoin(op)?,
            "M" => self.handle_miter_limit(op)?,

            "Do" => self.handle_do(op, device)?,
            "m" => self.move_to(op)?,
            "l" => self.line_to(op)?,
            "c" => self.curve_to_c(op)?,
            "v" => self.curve_to_v(op)?,
            "y" => self.curve_to_y(op)?,
            "h" => self.handle_close_path()?,
            "re" => self.handle_rect(op)?,
            "W" => self.handle_clip()?,
            "W*" => self.handle_eoclip()?,
            "n" => self.handle_end_path(device)?,
            "S" => self.handle_stroke_path(device)?,
            "s" => self.handle_close_stroke_path(device)?,
            "f" => self.handle_fill_path(device, FillType::Winding)?,
            "F" => self.handle_fill_path(device, FillType::Winding)?,
            "f*" => self.handle_fill_path(device, FillType::EvenOdd)?,
            "B" => self.handle_fill_and_stroke_path(device, FillType::Winding)?,
            "B*" => self.handle_fill_and_stroke_path(device, FillType::EvenOdd)?,
            "b" => {
                self.handle_close_path()?;
                self.handle_fill_and_stroke_path(device, FillType::Winding)?;
            }
            "b*" => {
                self.handle_close_path()?;
                self.handle_fill_and_stroke_path(device, FillType::EvenOdd)?;
            }
            "d" => self.handle_set_dash(op)?,
            "ri" => self.unsupported_operator(&op)?,
            "i" => self.handle_set_flat(op)?,
            "gs" => self.handle_set_extend_gs(op)?,

            "CS" => self.handle_set_colorspace_stroke(op)?,
            "cs" => self.handle_set_colorspace_fill(op)?,
            "SC" => self.handle_set_colorvalue_stroke(op)?,
            "SCN" => self.handle_set_colorvalue_ps_stroke(op)?,
            "sc" => self.handle_set_colorvalue_fill(op)?,
            "scn" => self.handle_set_colorvalue_ps_fill(op)?,
            "G" => self.handle_set_gray_stroke(op)?,
            "g" => self.handle_set_gray_fill(op)?,
            "RG" => self.handle_set_rgb_stroke(op)?,
            "rg" => self.handle_set_rgb_fill(op)?,
            "K" => self.handle_set_cmyk_stroke(op)?,
            "k" => self.handle_set_cmyk_fill(op)?,

            "BT" => {
                if self.in_text_object {
                    return Err(PdfError::PageOperatorError("nested BT text object".into()));
                }
                self.handle_begin_text()?;
                self.in_text_object = true;
            }
            "Tc" => self.handle_set_character_space(op)?,
            "Tw" => self.handle_setword_space(op)?,
            "Tz" => self.handle_set_text_horz_scale(op)?,
            "TL" => self.handle_set_text_leading(op)?,
            "Tf" => self.handle_set_font(op)?,
            "Tr" => self.handle_set_text_rendering(op)?,
            "Ts" => self.handle_set_text_rise(op)?,
            "Td" => self.handle_move_textpoint(op)?,
            "TD" => self.handle_move_textline(op)?,
            "Tm" => self.handle_set_text_matrix(op)?,
            "T*" => self.handle_move_to_nextline()?,
            "Tj" => self.handle_show_text(op, device)?,
            "'" => self.handle_show_text_nextline(op, device)?,
            "\"" => self.handle_show_text_nextline_space(op, device)?,
            "TJ" => self.handle_show_text_array(op, device)?,
            "ET" => {
                if !self.in_text_object {
                    return Err(PdfError::PageOperatorError("ET without matching BT".into()));
                }
                self.in_text_object = false;
            }

            _ => {
                self.unsupported_operator(&op)?;
            }
        }
        Ok(())
    }

    fn unsupported_operator(&mut self, op: &Operator) -> PdfResult<()> {
        use crate::page::diagnostic::{ContentDiagnostic, DiagnosticKind};
        crate::filter::check_output_size(
            self.diagnostics.len(),
            1,
            self.ctx.limits().max_content_diagnostics,
            "page content diagnostics",
        )?;
        let prefix = "unsupported content operator ";
        let suffix = format!(" at byte {:?}", op.byte_offset());
        let message_bytes = op
            .name()
            .len()
            .checked_add(prefix.len())
            .and_then(|n| n.checked_add(suffix.len()))
            .ok_or_else(|| PdfError::ResourceLimit("diagnostic size overflow".into()))?;
        let retained_bytes = message_bytes
            .checked_add(op.name().len())
            .ok_or_else(|| PdfError::ResourceLimit("diagnostic size overflow".into()))?;
        crate::filter::check_output_size(
            self.diagnostic_bytes,
            retained_bytes,
            self.ctx.limits().max_content_diagnostic_bytes,
            "page diagnostic bytes",
        )?;
        self.diagnostics
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("content diagnostic allocation".into()))?;
        let mut message = String::new();
        message
            .try_reserve_exact(message_bytes)
            .map_err(|_| PdfError::ResourceLimit("diagnostic message allocation".into()))?;
        message.push_str(prefix);
        message.push_str(op.name());
        message.push_str(&suffix);
        let mut operator = String::new();
        operator
            .try_reserve_exact(op.name().len())
            .map_err(|_| PdfError::ResourceLimit("diagnostic operator allocation".into()))?;
        operator.push_str(op.name());
        let strict_error = if self.ctx.options().mode == crate::parser::syntax::ParseMode::Strict {
            let mut copy = String::new();
            copy.try_reserve_exact(message.len())
                .map_err(|_| PdfError::ResourceLimit("diagnostic error allocation".into()))?;
            copy.push_str(&message);
            Some(PdfError::UnsupportedFeature(copy))
        } else {
            None
        };
        self.diagnostic_bytes += retained_bytes;
        self.diagnostics.push(ContentDiagnostic {
            kind: if crate::page::opcode::is_op(op.name().as_bytes()) {
                DiagnosticKind::UnsupportedOperator
            } else {
                DiagnosticKind::UnknownOperator
            },
            operator,
            byte_offset: op.byte_offset(),
            message,
        });
        if let Some(error) = strict_error {
            return Err(error);
        }
        Ok(())
    }

    fn handle_show_text_array(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        let params = op.get_as_array(0)?;
        for entry in params {
            match entry {
                PdfObject::PdfString(_) => {}
                PdfObject::PdfNumber(n) if n.value().is_finite() => {}
                _ => {
                    return Err(PdfError::PageOperatorError(
                        "TJ requires strings or finite numbers".into(),
                    ));
                }
            }
        }
        if self.current_state.graphic_state.text_state.font.is_none() {
            return Err(PdfError::PageOperatorError(
                "TJ requires a selected font".into(),
            ));
        }
        if let Some(font) = &self.current_state.graphic_state.text_state.font {
            let font_size = self.current_state.graphic_state.text_state.font_size;
            let char_space = self.current_state.graphic_state.text_state.char_space;
            let word_space = self.current_state.graphic_state.text_state.word_space;
            let horz_scale = self.current_state.text_horz_scale;
            device.preflight_draw(&self.current_state.graphic_state, None)?;
            let mut textobj = TextObject::new(self.current_state.graphic_state.clone());
            textobj.horizontal_scale = horz_scale;

            let wmd = font.writting_mode();
            let matrix = Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, self.current_state.text_rise)
                .checked_mul(&self.current_state.text_matrix)?
                .checked_mul(&self.current_state.ctm)?;
            let matrix = matrix.checked_mul(&self.page.page_matrix)?;

            textobj.matrix = matrix.clone();
            let ox = matrix.e();
            let oy = matrix.f();
            textobj.origin_x = ox;
            textobj.origin_y = oy;

            for (array_item_index, p) in params.into_iter().enumerate() {
                match p {
                    PdfObject::PdfString(s) => {
                        let content = s.bytes();
                        let chars = font.chars_limited(
                            content,
                            self.ctx
                                .limits()
                                .max_text_chars
                                .saturating_sub(self.text_chars)
                                .min(
                                    self.execution_budget
                                        .remaining_text_chars(&self.ctx.limits()),
                                ),
                        )?;
                        crate::filter::check_output_size(
                            self.text_chars,
                            chars.len(),
                            self.ctx.limits().max_text_chars,
                            "page text characters",
                        )?;
                        self.execution_budget
                            .text_chars(chars.len(), &self.ctx.limits())?;
                        self.text_chars += chars.len();
                        let mut code_offset = 0usize;
                        for char in chars.iter() {
                            let code_end = code_offset
                                .checked_add(usize::from(char.length()))
                                .filter(|end| *end <= content.len() && *end > code_offset)
                                .ok_or_else(|| {
                                    PdfError::ContentParseError(
                                        "text character code range mismatch".into(),
                                    )
                                })?;
                            let matrix =
                                Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, self.current_state.text_rise)
                                    .checked_mul(&self.current_state.text_matrix)?
                                    .checked_mul(&self.current_state.ctm)?;
                            let matrix = matrix.checked_mul(&self.page.page_matrix)?;

                            let pos = Point::new(matrix.e(), matrix.f());
                            let charitem =
                                CharItem::new(pos, char.to_owned(), font.unicode_mapping(char)?)
                                    .with_array_item_index(array_item_index)
                                    .with_decoded_code_range(code_offset..code_end);
                            code_offset = code_end;
                            textobj.add_item(charitem);
                            let mut displacement = (char.width() / 1000.0) * font_size + char_space;
                            if char.code() == 32 && char.length() == 1 {
                                displacement += word_space;
                            }
                            match font.writting_mode() {
                                WritingMode::Horizontal => {
                                    let translation = Matrix::new(
                                        1.0,
                                        0.0,
                                        0.0,
                                        1.0,
                                        displacement * horz_scale,
                                        0.0,
                                    );
                                    let m =
                                        translation.checked_mul(&self.current_state.text_matrix)?;
                                    self.current_state.text_matrix = m;
                                }
                                WritingMode::Vertical => {
                                    let translation =
                                        Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, displacement);
                                    let m =
                                        translation.checked_mul(&self.current_state.text_matrix)?;
                                    self.current_state.text_matrix = m;
                                }
                            }
                        }
                    }
                    PdfObject::PdfNumber(n) => {
                        let v = n.value();
                        let tj = v * 0.001 * self.current_state.graphic_state.text_state.font_size;
                        match wmd {
                            WritingMode::Horizontal => {
                                let tj = tj * self.current_state.text_horz_scale;
                                let rm = Matrix::new(1.0, 0.0, 0.0, 1.0, -tj, 0.0);
                                let text_matrix =
                                    rm.checked_mul(&self.current_state.text_matrix)?;
                                self.current_state.text_matrix = text_matrix;
                            }
                            WritingMode::Vertical => {
                                let rm = Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, -tj);
                                let text_matrix =
                                    rm.checked_mul(&self.current_state.text_matrix)?;
                                self.current_state.text_matrix = text_matrix;
                            }
                        }
                    }
                    _ => {
                        return Err(PdfError::PageOperatorError(
                            "invalid parameter type in TJ operator".to_string(),
                        ));
                    }
                }
            }
            let textobj = if self.form_depth == 0 {
                textobj.with_source(self.text_draw_source())
            } else {
                textobj.with_nested_source(self.nested_draw_source()?)
            };
            device.show_text(textobj)?;
        }

        Ok(())
    }
    fn handle_show_text_nextline_space(
        &mut self,
        op: Operator,
        device: &mut dyn Device,
    ) -> PdfResult<()> {
        let aw = op.get_as_f32(0)?;
        let ac = op.get_as_f32(1)?;
        self.current_state.graphic_state.text_state.word_space = aw;
        self.current_state.graphic_state.text_state.char_space = ac;
        let content = op.get_as_string(2)?;
        self.handle_move_to_nextline()?;
        self.do_show_text(content.bytes(), device)
    }

    fn handle_show_text_nextline(
        &mut self,
        op: Operator,
        device: &mut dyn Device,
    ) -> PdfResult<()> {
        self.handle_move_to_nextline()?;
        let content = op.get_as_string(0)?;
        self.do_show_text(content.bytes(), device)
    }
    fn do_show_text(&mut self, content: &[u8], device: &mut dyn Device) -> PdfResult<()> {
        let font_size = self.current_state.graphic_state.text_state.font_size;
        let char_space = self.current_state.graphic_state.text_state.char_space;
        let word_space = self.current_state.graphic_state.text_state.word_space;
        let horz_scale = self.current_state.text_horz_scale;
        device.preflight_draw(&self.current_state.graphic_state, None)?;
        let mut textobj = TextObject::new(self.current_state.graphic_state.clone());
        textobj.horizontal_scale = horz_scale;

        let matrix = Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, self.current_state.text_rise)
            .checked_mul(&self.current_state.text_matrix)?
            .checked_mul(&self.current_state.ctm)?;
        let matrix = matrix.checked_mul(&self.page.page_matrix)?;
        textobj.matrix = matrix.clone();
        let ox = matrix.e();
        let oy = matrix.f();
        textobj.origin_x = ox;
        textobj.origin_y = oy;
        if let Some(font) = &self.current_state.graphic_state.text_state.font {
            let chars = font.chars_limited(
                content,
                self.ctx
                    .limits()
                    .max_text_chars
                    .saturating_sub(self.text_chars)
                    .min(
                        self.execution_budget
                            .remaining_text_chars(&self.ctx.limits()),
                    ),
            )?;
            crate::filter::check_output_size(
                self.text_chars,
                chars.len(),
                self.ctx.limits().max_text_chars,
                "page text characters",
            )?;
            self.execution_budget
                .text_chars(chars.len(), &self.ctx.limits())?;
            self.text_chars += chars.len();
            let mut code_offset = 0usize;
            for char in chars.iter() {
                let code_end = code_offset
                    .checked_add(usize::from(char.length()))
                    .filter(|end| *end <= content.len() && *end > code_offset)
                    .ok_or_else(|| {
                        PdfError::ContentParseError("text character code range mismatch".into())
                    })?;
                let matrix = Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, self.current_state.text_rise)
                    .checked_mul(&self.current_state.text_matrix)?
                    .checked_mul(&self.current_state.ctm)?;
                let matrix = matrix.checked_mul(&self.page.page_matrix)?;

                let pos = Point::new(matrix.e(), matrix.f());
                let charitem = CharItem::new(pos, char.to_owned(), font.unicode_mapping(char)?)
                    .with_decoded_code_range(code_offset..code_end);
                code_offset = code_end;
                textobj.add_item(charitem);
                let mut displacement = (char.width() / 1000.0) * font_size + char_space;
                if char.code() == 32 && char.length() == 1 {
                    displacement += word_space;
                }
                match font.writting_mode() {
                    WritingMode::Horizontal => {
                        let translation =
                            Matrix::new(1.0, 0.0, 0.0, 1.0, displacement * horz_scale, 0.0);
                        let m = translation.checked_mul(&self.current_state.text_matrix)?;
                        self.current_state.text_matrix = m;
                    }
                    WritingMode::Vertical => {
                        let translation = Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, displacement);
                        let m = translation.checked_mul(&self.current_state.text_matrix)?;
                        self.current_state.text_matrix = m;
                    }
                }
            }
            let textobj = if self.form_depth == 0 {
                textobj.with_source(self.text_draw_source())
            } else {
                textobj.with_nested_source(self.nested_draw_source()?)
            };
            device.show_text(textobj)?;
        } else {
            return Err(PdfError::PageOperatorError(
                "Text display requires a selected font".into(),
            ));
        }
        Ok(())
    }

    fn text_draw_source(&self) -> TextDrawSource {
        TextDrawSource {
            operation_index: self.operation_index,
            operator_byte_offset: self.operation_byte_offset,
        }
    }

    fn handle_show_text(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        let content = op.get_as_string(0)?;
        self.do_show_text(content.bytes(), device)
    }

    fn handle_move_to_nextline(&mut self) -> PdfResult<()> {
        let tx = 0.0_f32;
        let ty = -self.current_state.text_leading;
        let mat = Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty);
        self.current_state.text_matrix = mat.checked_mul(&self.current_state.text_line_matrix)?;
        self.current_state.text_line_matrix = self.current_state.text_matrix.clone();
        Ok(())
    }
    fn handle_set_text_matrix(&mut self, op: Operator) -> PdfResult<()> {
        let a = op.get_as_f32(0)?;
        let b = op.get_as_f32(1)?;
        let c = op.get_as_f32(2)?;
        let d = op.get_as_f32(3)?;
        let e = op.get_as_f32(4)?;
        let f = op.get_as_f32(5)?;
        let matrix = Matrix::new(a, b, c, d, e, f);
        self.current_state.text_matrix = matrix.clone();
        self.current_state.text_line_matrix = matrix;
        Ok(())
    }

    fn handle_move_textline(&mut self, op: Operator) -> PdfResult<()> {
        let tx = op.get_as_f32(0)?;
        let ty = op.get_as_f32(1)?;
        self.current_state.text_leading = -1.0 * ty;
        let mat = Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty);
        self.current_state.text_matrix = mat.checked_mul(&self.current_state.text_line_matrix)?;
        self.current_state.text_line_matrix = self.current_state.text_matrix.clone();
        Ok(())
    }

    fn handle_move_textpoint(&mut self, op: Operator) -> PdfResult<()> {
        let tx = op.get_as_f32(0)?;
        let ty = op.get_as_f32(1)?;
        let mat = Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty);
        self.current_state.text_matrix = mat.checked_mul(&self.current_state.text_line_matrix)?;
        self.current_state.text_line_matrix = self.current_state.text_matrix.clone();
        Ok(())
    }
    fn handle_set_text_rise(&mut self, op: Operator) -> PdfResult<()> {
        let rise = op.get_as_f32(0)?;
        self.current_state.text_rise = rise;
        Ok(())
    }

    fn handle_set_text_rendering(&mut self, op: Operator) -> PdfResult<()> {
        let mode = op
            .operand(0)?
            .as_number()
            .and_then(|n| n.as_u32_checked().ok())
            .filter(|mode| *mode <= 7)
            .ok_or_else(|| {
                PdfError::PageOperatorError("Tr requires an integer from 0 to 7".into())
            })? as u8;
        self.current_state
            .graphic_state
            .text_state
            .set_text_rendering_mode(mode);

        Ok(())
    }

    fn handle_set_font(&mut self, op: Operator) -> PdfResult<()> {
        let font_size = op.get_as_f32(1)?;
        self.current_state.graphic_state.text_state.font_size = font_size;
        let font_name = op.get_as_name(0)?;
        let font_resource = self.named_resource("Font", font_name.name())?;
        let font = Font::try_new(
            font_resource.as_dict().ok_or_else(|| {
                PdfError::PageResourceError("Font resource must be a dictionary".into())
            })?,
            self.ctx,
        )?;
        self.current_state.graphic_state.text_state.font = Some(font);
        Ok(())
    }
    fn handle_set_text_leading(&mut self, op: Operator) -> PdfResult<()> {
        let leading = op.get_as_f32(0)?;
        self.current_state.text_leading = leading;
        Ok(())
    }
    fn handle_set_text_horz_scale(&mut self, op: Operator) -> PdfResult<()> {
        let scale = op.get_as_f32(0)?;
        // Tz is a percentage, whereas advances use a unit scale factor.
        self.current_state.text_horz_scale = scale / 100.0;
        Ok(())
    }

    fn handle_setword_space(&mut self, op: Operator) -> PdfResult<()> {
        let space = op.get_as_f32(0)?;
        self.current_state.graphic_state.text_state.word_space = space;
        Ok(())
    }

    fn handle_set_character_space(&mut self, op: Operator) -> PdfResult<()> {
        let space = op.get_as_f32(0)?;
        self.current_state.graphic_state.text_state.char_space = space;
        Ok(())
    }

    fn handle_begin_text(&mut self) -> PdfResult<()> {
        self.current_state.set_text_matrix(Matrix::identity());
        self.current_state.text_line_matrix = Matrix::identity();
        Ok(())
    }

    fn handle_set_cmyk_fill(&mut self, op: Operator) -> PdfResult<()> {
        let c = op.get_as_f32(0)?;
        let m = op.get_as_f32(1)?;
        let y = op.get_as_f32(2)?;
        let k = op.get_as_f32(3)?;
        self.current_state
            .graphic_state
            .color_state
            .fill_color_space = ColorSpace::DeviceCmyk(DeviceCmyk::new());
        self.current_state
            .graphic_state
            .color_state
            .fill_color_value = ColorValue::new(vec![c, m, y, k]);
        Ok(())
    }

    fn handle_set_cmyk_stroke(&mut self, op: Operator) -> PdfResult<()> {
        let c = op.get_as_f32(0)?;
        let m = op.get_as_f32(1)?;
        let y = op.get_as_f32(2)?;
        let k = op.get_as_f32(3)?;
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_space = ColorSpace::DeviceCmyk(DeviceCmyk::new());
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_value = ColorValue::new(vec![c, m, y, k]);
        Ok(())
    }

    fn handle_set_rgb_fill(&mut self, op: Operator) -> PdfResult<()> {
        let r = op.get_as_f32(0)?;
        let g = op.get_as_f32(1)?;
        let b = op.get_as_f32(2)?;
        self.current_state
            .graphic_state
            .color_state
            .fill_color_space = ColorSpace::DeviceRgb(DeviceRgb::new());
        self.current_state
            .graphic_state
            .color_state
            .fill_color_value = ColorValue::new(vec![r, g, b]);
        Ok(())
    }

    fn handle_set_rgb_stroke(&mut self, op: Operator) -> PdfResult<()> {
        let r = op.get_as_f32(0)?;
        let g = op.get_as_f32(1)?;
        let b = op.get_as_f32(2)?;
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_space = ColorSpace::DeviceRgb(DeviceRgb::new());
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_value = ColorValue::new(vec![r, g, b]);
        Ok(())
    }

    fn handle_set_gray_fill(&mut self, op: Operator) -> PdfResult<()> {
        let gray_value = op.get_as_f32(0)?;
        self.current_state
            .graphic_state
            .color_state
            .fill_color_space = ColorSpace::DeviceGray(DeviceGray::new());
        self.current_state
            .graphic_state
            .color_state
            .fill_color_value = ColorValue::new(vec![gray_value]);
        Ok(())
    }

    fn handle_set_gray_stroke(&mut self, op: Operator) -> PdfResult<()> {
        let gray_value = op.get_as_f32(0)?;
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_space = ColorSpace::DeviceGray(DeviceGray::new());
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_value = ColorValue::new(vec![gray_value]);
        Ok(())
    }

    fn handle_set_colorvalue_ps_stroke(&mut self, op: Operator) -> PdfResult<()> {
        if matches!(
            self.current_state
                .graphic_state
                .color_state
                .stroke_color_space,
            ColorSpace::Pattern(_)
        ) {
            self.unsupported_operator(&op)
        } else {
            self.handle_set_colorvalue_stroke(op)
        }
    }
    fn handle_set_colorvalue_ps_fill(&mut self, op: Operator) -> PdfResult<()> {
        if matches!(
            self.current_state
                .graphic_state
                .color_state
                .fill_color_space,
            ColorSpace::Pattern(_)
        ) {
            self.unsupported_operator(&op)
        } else {
            self.handle_set_colorvalue_fill(op)
        }
    }

    fn handle_set_colorvalue_fill(&mut self, op: Operator) -> PdfResult<()> {
        let n = op.num_operands();
        let expected = self
            .current_state
            .graphic_state
            .color_state
            .fill_color_space
            .number_of_components()?;
        if n != expected {
            return Err(PdfError::PageOperatorError(format!(
                "fill color requires {expected} components, got {n}"
            )));
        }
        let mut values = Vec::new();
        for i in 0..n {
            let v = op.get_as_f32(i)?;
            values.push(v)
        }
        self.current_state
            .graphic_state
            .color_state
            .fill_color_value = ColorValue::new(values);
        Ok(())
    }

    fn handle_set_colorvalue_stroke(&mut self, op: Operator) -> PdfResult<()> {
        let n = op.num_operands();
        let expected = self
            .current_state
            .graphic_state
            .color_state
            .stroke_color_space
            .number_of_components()?;
        if n != expected {
            return Err(PdfError::PageOperatorError(format!(
                "stroke color requires {expected} components, got {n}"
            )));
        }
        let mut values = Vec::new();
        for i in 0..n {
            let v = op.get_as_f32(i)?;
            values.push(v)
        }
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_value = ColorValue::new(values);
        Ok(())
    }
    fn find_colorspace(&mut self, obj: &PdfObject) -> PdfResult<ColorSpace> {
        match obj {
            PdfObject::PdfName(name) => {
                if matches!(
                    name.name(),
                    "DeviceRGB" | "DeviceGray" | "DeviceCMYK" | "RGB" | "CMYK" | "G"
                ) {
                    return ColorSpace::try_new(obj, self.ctx);
                } else {
                    let colorobj = self.named_resource("ColorSpace", name.name())?;
                    let colorspace = ColorSpace::try_new(&colorobj, self.ctx)?;
                    return Ok(colorspace);
                }
            }
            _ => {
                return Err(PdfError::PageOperatorError(
                    "color space operand must be a name".into(),
                ));
            }
        }
    }

    pub fn handle_set_colorspace_fill(&mut self, op: Operator) -> PdfResult<()> {
        let obj = op.operand(0)?;
        //let colorspace = ColorSpace::try_new(obj, self.ctx)?;
        let colorspace = self.find_colorspace(obj)?;
        let default_value = colorspace.default_value()?;
        self.current_state
            .graphic_state
            .color_state
            .fill_color_value = default_value;
        self.current_state
            .graphic_state
            .color_state
            .fill_color_space = colorspace;
        Ok(())
    }
    pub fn handle_set_colorspace_stroke(&mut self, op: Operator) -> PdfResult<()> {
        let obj = op.operand(0)?;
        let colorspace = self.find_colorspace(obj)?;
        let default_value = colorspace.default_value()?;
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_value = default_value;
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_space = colorspace;
        Ok(())
    }
    fn handle_set_extend_gs(&mut self, op: Operator) -> PdfResult<()> {
        let name = op.get_as_name(0)?.name();
        let extgs_obj = self.named_resource("ExtGState", name)?;
        let extgs = extgs_obj
            .as_dict()
            .ok_or(PdfError::PageResourceError(format!(
                "Page ExtGs state is not a dict"
            )))?;
        // Reject the whole state before applying supported entries. In compatible
        // mode the diagnostic remains visible; unsupported transparency must not
        // be silently treated as opaque or partially applied.
        if !crate::page::all_state::supported_ext_gs_keys(extgs) {
            return self.unsupported_operator(&op);
        }
        if let Some(value) = extgs.get("BM") {
            let Some(mode) = crate::page::general_state::resolve_blend_mode(value, self.ctx)?
            else {
                return self.unsupported_operator(&op);
            };
            self.current_state.graphic_state.general_state.blend_mode = mode;
        }
        for key in ["CA", "ca"] {
            if let Some(value) = extgs.get(key) {
                let alpha = self
                    .ctx
                    .resolve_owned(value)?
                    .as_f32()
                    .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                    .ok_or_else(|| {
                        PdfError::PageResourceError(
                            "ExtGState alpha must be finite and in [0,1]".into(),
                        )
                    })?;
                if key == "CA" {
                    self.current_state.graphic_state.general_state.stroke_alpha = alpha;
                } else {
                    self.current_state.graphic_state.general_state.fill_alpha = alpha;
                }
            }
        }
        if let Some(value) = extgs.get("Type") {
            let value = self.ctx.resolve_owned(value)?;
            if value
                .as_name()
                .is_none_or(|name| name.name() != "ExtGState")
            {
                return Err(PdfError::PageResourceError(
                    "ExtGState Type must be ExtGState".into(),
                ));
            }
        }
        for (key, command) in [
            ("LW", "w"),
            ("LC", "J"),
            ("LJ", "j"),
            ("ML", "M"),
            ("FL", "i"),
        ] {
            if let Some(value) = extgs.get(key) {
                let value = self.ctx.resolve_owned(value)?;
                let mut operands = Vec::new();
                operands
                    .try_reserve_exact(1)
                    .map_err(|_| PdfError::ResourceLimit("ExtGState operands allocation".into()))?;
                operands.push(value);
                let operation = Operator::new(command.into(), operands);
                match key {
                    "LW" => self.handle_linewidth(operation)?,
                    "LC" => self.handle_linecap(operation)?,
                    "LJ" => self.handle_linejoin(operation)?,
                    "ML" => self.handle_miter_limit(operation)?,
                    "FL" => self.handle_set_flat(operation)?,
                    _ => unreachable!(),
                }
            }
        }
        if let Some(value) = extgs.get("D") {
            let value = self.ctx.resolve_owned(value)?;
            let array = value
                .as_array()
                .filter(|array| array.len() == 2)
                .ok_or_else(|| {
                    PdfError::PageResourceError("ExtGState D requires [array phase]".into())
                })?;
            let values = self.ctx.resolve_owned(array.get(0).unwrap())?;
            let values = values.as_array().ok_or_else(|| {
                PdfError::PageResourceError("ExtGState dash pattern must be an array".into())
            })?;
            if values.len() > self.ctx.limits().max_objects {
                return Err(PdfError::ResourceLimit("ExtGState dash entries".into()));
            }
            let mut resolved = crate::objects::pdf_array::PdfArray::default();
            for value in values {
                resolved.try_add_obj(self.ctx.resolve_owned(value)?)?;
            }
            let mut operands = Vec::new();
            operands.try_reserve_exact(2).map_err(|_| {
                PdfError::ResourceLimit("ExtGState dash operands allocation".into())
            })?;
            operands.push(PdfObject::PdfArray(resolved));
            operands.push(self.ctx.resolve_owned(array.get(1).unwrap())?);
            self.handle_set_dash(Operator::new("d".into(), operands))?;
        }
        if let Some(value) = extgs.get("Font") {
            let value = self.ctx.resolve_owned(value)?;
            let array = value
                .as_array()
                .filter(|array| array.len() == 2)
                .ok_or_else(|| PdfError::FontError("ExtGState Font requires [font size]".into()))?;
            let size = self
                .ctx
                .resolve_owned(array.get(1).unwrap())?
                .as_f32()
                .filter(|size| size.is_finite())
                .ok_or_else(|| PdfError::FontError("ExtGState font size must be finite".into()))?;
            let font = self.ctx.resolve_owned(array.get(0).unwrap())?;
            let dict = font
                .as_dict()
                .ok_or_else(|| PdfError::FontError("ExtGState font must be a dictionary".into()))?;
            let font = Font::try_new(dict, self.ctx)?;
            self.current_state.graphic_state.text_state.font = Some(font);
            self.current_state.graphic_state.text_state.font_size = size;
        }
        Ok(())
    }

    fn handle_set_flat(&mut self, op: Operator) -> PdfResult<()> {
        let flat = op.get_as_f32(0)?;
        if !(0.0..=100.0).contains(&flat) {
            return Err(PdfError::PageOperatorError(
                "Flatness must be between 0 and 100".into(),
            ));
        }
        self.current_state.graphic_state.general_state.flat_ness = flat;
        Ok(())
    }

    fn handle_set_dash(&mut self, op: Operator) -> PdfResult<()> {
        let dash_array = op.get_as_array(0)?;
        let limits = self.ctx.limits();
        if dash_array.len() > limits.max_objects
            || dash_array
                .len()
                .checked_mul(std::mem::size_of::<f32>())
                .is_none_or(|bytes| bytes > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit("dash pattern entries/bytes".into()));
        }
        let mut dash_vec = Vec::new();
        dash_vec
            .try_reserve_exact(dash_array.len())
            .map_err(|_| PdfError::ResourceLimit("dash pattern allocation".into()))?;
        for v in dash_array {
            dash_vec.push(
                v.as_f32()
                    .filter(|n| n.is_finite() && *n >= 0.0)
                    .ok_or_else(|| {
                        PdfError::PageOperatorError(
                            "Dash entries must be finite and nonnegative".into(),
                        )
                    })?,
            );
        }
        let dash_phrase = op.get_as_f32(1)?;
        let dash_total = dash_vec.iter().try_fold(0.0_f32, |sum, value| {
            let next = sum + value;
            if next.is_finite() {
                Ok(next)
            } else {
                Err(PdfError::PageOperatorError(
                    "Dash cycle length overflow".into(),
                ))
            }
        })?;
        if dash_vec.len() % 2 == 1 && !(dash_total * 2.0).is_finite() {
            return Err(PdfError::PageOperatorError(
                "Repeated odd dash cycle length overflow".into(),
            ));
        }
        if dash_phrase < 0.0 || (!dash_vec.is_empty() && dash_vec.iter().all(|v| *v == 0.0)) {
            return Err(PdfError::PageOperatorError(
                "Invalid dash phase or all-zero dash array".into(),
            ));
        }
        self.current_state.graphic_state.graph_state.dash_phrase = dash_phrase;
        self.current_state.graphic_state.graph_state.dash_array = dash_vec;
        Ok(())
    }

    fn handle_miter_limit(&mut self, op: Operator) -> PdfResult<()> {
        let limit = op.get_as_f32(0)?;
        if limit < 1.0 {
            return Err(PdfError::PageOperatorError(
                "Miter limit must be at least 1".into(),
            ));
        }
        self.current_state.graphic_state.graph_state.miter_limit = limit;
        Ok(())
    }

    fn handle_linejoin(&mut self, op: Operator) -> PdfResult<()> {
        let join = op
            .operand(0)?
            .as_u8()
            .filter(|value| *value <= 2)
            .ok_or_else(|| {
                PdfError::PageOperatorError("line join must be an integer 0–2".into())
            })?;
        self.current_state.graphic_state.graph_state.line_join = LineJoin::new_from_value(join);
        Ok(())
    }

    fn handle_linecap(&mut self, op: Operator) -> PdfResult<()> {
        let cap = op
            .operand(0)?
            .as_u8()
            .filter(|value| *value <= 2)
            .ok_or_else(|| PdfError::PageOperatorError("line cap must be an integer 0–2".into()))?;
        self.current_state.graphic_state.graph_state.line_cap = LineCap::new_from_value(cap);
        Ok(())
    }

    fn handle_linewidth(&mut self, op: Operator) -> PdfResult<()> {
        let width = op.get_as_f32(0)?;
        if width < 0.0 {
            return Err(PdfError::PageOperatorError(
                "line width must be nonnegative".into(),
            ));
        }
        self.current_state.graphic_state.graph_state.line_width = width;
        Ok(())
    }

    fn handle_fill_and_stroke_path(
        &mut self,
        device: &mut dyn Device,
        filletype: FillType,
    ) -> PdfResult<()> {
        if self.current_path.is_empty() {
            return self.finish_empty_path();
        }
        device.preflight_draw(&self.current_state.graphic_state, Some(&self.current_path))?;
        let ctm = self.current_state.ctm();
        let matrix = ctm.checked_mul(&self.page.page_matrix)?;
        let state = self.current_state.graphic_state.clone();
        let pathobj = PathObject::new(
            self.current_path.clone(),
            matrix.clone(),
            state,
            filletype,
            true,
        );
        let pathobj = if self.form_depth == 0 {
            pathobj.with_source(self.path_draw_source()?)
        } else {
            pathobj.with_nested_source(self.nested_path_source()?)
        };
        device.fill_and_stroke_path(pathobj)?;
        self.finish_painted_path(matrix);
        Ok(())
    }

    fn handle_fill_path(&mut self, device: &mut dyn Device, filletype: FillType) -> PdfResult<()> {
        if self.current_path.is_empty() {
            return self.finish_empty_path();
        }
        device.preflight_draw(&self.current_state.graphic_state, Some(&self.current_path))?;
        let ctm = self.current_state.ctm();
        let matrix = ctm.checked_mul(&self.page.page_matrix)?;
        let state = self.current_state.graphic_state.clone();
        let pathobj = PathObject::new(
            self.current_path.clone(),
            matrix.clone(),
            state,
            filletype,
            false,
        );
        let pathobj = if self.form_depth == 0 {
            pathobj.with_source(self.path_draw_source()?)
        } else {
            pathobj.with_nested_source(self.nested_path_source()?)
        };
        device.fill_path(pathobj)?;
        self.finish_painted_path(matrix);
        Ok(())
    }

    // s
    fn handle_close_stroke_path(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        //
        self.handle_close_path()?;
        self.handle_stroke_path(device)?;
        Ok(())
    }
    // S
    fn handle_stroke_path(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        if self.current_path.is_empty() {
            return self.finish_empty_path();
        }
        device.preflight_draw(&self.current_state.graphic_state, Some(&self.current_path))?;

        let ctm = self.current_state.ctm();
        let matrix = ctm.checked_mul(&self.page.page_matrix)?;

        let state = self.current_state.graphic_state.clone();
        let path_obj = PathObject::new(
            self.current_path.clone(),
            matrix.clone(),
            state,
            FillType::NoFill,
            true,
        );
        let path_obj = if self.form_depth == 0 {
            path_obj.with_source(self.path_draw_source()?)
        } else {
            path_obj.with_nested_source(self.nested_path_source()?)
        };
        device.stroke_path(path_obj)?;
        self.finish_painted_path(matrix);
        Ok(())
    }

    fn path_draw_source(&self) -> PdfResult<PathDrawSource> {
        let mut indices = Vec::new();
        indices
            .try_reserve_exact(self.path_source_indices.len())
            .map_err(|_| PdfError::ResourceLimit("path draw source allocation".into()))?;
        indices.extend_from_slice(&self.path_source_indices);
        Ok(PathDrawSource {
            geometry_operation_indices: indices,
            painting_operation_index: self.operation_index,
            painting_byte_offset: self.operation_byte_offset,
        })
    }

    fn nested_draw_source(&self) -> PdfResult<Option<super::source_provenance::NestedDrawSource>> {
        if self.invocation_path.is_empty() {
            return Ok(None);
        }
        let bytes = self
            .invocation_path
            .len()
            .checked_mul(std::mem::size_of::<super::source_provenance::FormInvocation>())
            .ok_or_else(|| PdfError::ResourceLimit("Form source path size".into()))?;
        if bytes > self.ctx.limits().max_decoded_bytes {
            return Err(PdfError::ResourceLimit("Form source path bytes".into()));
        }
        let mut invocations = Vec::new();
        invocations
            .try_reserve_exact(self.invocation_path.len())
            .map_err(|_| PdfError::ResourceLimit("Form source path allocation".into()))?;
        invocations.extend_from_slice(&self.invocation_path);
        Ok(Some(super::source_provenance::NestedDrawSource {
            invocations,
            geometry_operation_indices: Vec::new(),
            operation_index: self.operation_index,
            operator_byte_offset: self.operation_byte_offset,
        }))
    }

    fn nested_path_source(&self) -> PdfResult<Option<super::source_provenance::NestedDrawSource>> {
        let Some(mut source) = self.nested_draw_source()? else {
            return Ok(None);
        };
        let bytes = self
            .path_source_indices
            .len()
            .checked_mul(std::mem::size_of::<usize>())
            .and_then(|n| source.retained_bytes().ok().and_then(|b| b.checked_add(n)))
            .ok_or_else(|| PdfError::ResourceLimit("nested path source size overflow".into()))?;
        if bytes > self.ctx.limits().max_decoded_bytes {
            return Err(PdfError::ResourceLimit("nested path source bytes".into()));
        }
        source
            .geometry_operation_indices
            .try_reserve_exact(self.path_source_indices.len())
            .map_err(|_| PdfError::ResourceLimit("nested path source allocation".into()))?;
        source
            .geometry_operation_indices
            .extend_from_slice(&self.path_source_indices);
        Ok(Some(source))
    }

    fn finish_painted_path(&mut self, matrix: Matrix) {
        self.path_source_indices.clear();
        if self.path_clip_type != FillType::NoFill {
            self.current_state.graphic_state.clip_path.add_path(
                self.current_path.clone(),
                self.path_clip_type.clone(),
                matrix,
            );
        }
        self.path_clip_type = FillType::NoFill;
        self.current_path = PdfPath::default();
        self.current_path_point = Point::default();
        self.path_start = Point::default();
    }

    fn finish_empty_path(&mut self) -> PdfResult<()> {
        let matrix = self
            .current_state
            .ctm()
            .checked_mul(&self.page.page_matrix)?;
        self.finish_painted_path(matrix);
        Ok(())
    }

    // re
    fn handle_rect(&mut self, op: Operator) -> PdfResult<()> {
        let x = op.get_as_f32(0)?;
        let y = op.get_as_f32(1)?;
        let width = op.get_as_f32(2)?;
        let height = op.get_as_f32(3)?;
        let right = x + width;
        let top = y + height;
        if !right.is_finite() || !top.is_finite() {
            return Err(PdfError::PageOperatorError(
                "rectangle coordinate overflow".into(),
            ));
        }
        let lower_left = Point::new(x, y);
        let lower_right = Point::new(right, y);
        let upper_right = Point::new(right, top);
        let upper_left = Point::new(x, top);
        let mut subpath = SubPath::new(vec![Segment::MoveTo(lower_left.clone())], false);
        self.path_start = lower_left.clone();
        self.current_path_point = lower_left.clone();
        subpath.add_segment(Segment::LineTo(lower_right));
        subpath.add_segment(Segment::LineTo(upper_right));
        subpath.add_segment(Segment::LineTo(upper_left));
        subpath.close();
        self.current_path.add_subpath(subpath);
        Ok(())
    }

    // W
    fn handle_clip(&mut self) -> PdfResult<()> {
        self.check_clip_limit()?;
        self.path_clip_type = FillType::Winding;
        Ok(())
    }
    // W*
    fn handle_eoclip(&mut self) -> PdfResult<()> {
        self.check_clip_limit()?;
        self.path_clip_type = FillType::EvenOdd;
        Ok(())
    }

    fn check_clip_limit(&self) -> PdfResult<()> {
        if self.current_state.graphic_state.clip_path.elements().len()
            >= self.ctx.limits().max_clip_paths
        {
            return Err(PdfError::ResourceLimit("cumulative clipping paths".into()));
        }
        Ok(())
    }

    fn handle_end_path(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        let ctm = self.current_state.ctm();
        let matrix = ctm.checked_mul(&self.page.page_matrix)?;
        self.finish_painted_path(matrix);
        Ok(())
    }

    fn handle_close_path(&mut self) -> PdfResult<()> {
        if self.current_path.is_empty() {
            return Ok(());
        }
        let last = self.current_path.last_mut_subpath().unwrap();
        if !last.is_closed() {
            last.close();
        }
        self.current_path_point = self.path_start.clone();
        Ok(())
    }

    // y
    fn curve_to_y(&mut self, op: Operator) -> PdfResult<()> {
        if self.current_path.is_empty() {
            return Ok(());
        }

        let x1 = op.get_as_f32(0)?;
        let y1 = op.get_as_f32(1)?;
        let x3 = op.get_as_f32(2)?;
        let y3 = op.get_as_f32(3)?;

        let p1 = Point::new(x1, y1);
        let p3 = Point::new(x3, y3);
        let bezier = Bezier::new(p1, p3.clone(), p3.clone());
        self.current_path_point = p3;
        let last = self.current_path.last_mut_subpath().unwrap();
        last.add_segment(Segment::CurveTo(bezier));
        Ok(())
    }

    fn curve_to_v(&mut self, op: Operator) -> PdfResult<()> {
        if self.current_path.is_empty() {
            return Ok(());
        }
        let x2 = op.get_as_f32(0)?;
        let y2 = op.get_as_f32(1)?;
        let x3 = op.get_as_f32(2)?;
        let y3 = op.get_as_f32(3)?;
        let p2 = Point::new(x2, y2);
        let p3 = Point::new(x3, y3);
        let bezier = Bezier::new(self.current_path_point.clone(), p2, p3.clone());
        let last = self.current_path.last_mut_subpath().unwrap();
        self.current_path_point = p3;
        last.add_segment(Segment::CurveTo(bezier));
        Ok(())
    }

    // c
    fn curve_to_c(&mut self, op: Operator) -> PdfResult<()> {
        if self.current_path.is_empty() {
            return Ok(());
        }
        let x1 = op.get_as_f32(0)?;
        let y1 = op.get_as_f32(1)?;
        let x2 = op.get_as_f32(2)?;
        let y2 = op.get_as_f32(3)?;
        let x3 = op.get_as_f32(4)?;
        let y3 = op.get_as_f32(5)?;
        let p1 = Point::new(x1, y1);
        let p2 = Point::new(x2, y2);
        let p3 = Point::new(x3, y3);
        let last = self.current_path.last_mut_subpath().unwrap();
        let bezier = Bezier::new(p1, p2, p3.clone());
        last.add_segment(Segment::CurveTo(bezier));
        self.current_path_point = p3;
        Ok(())
    }

    fn line_to(&mut self, op: Operator) -> PdfResult<()> {
        let x = op.get_as_f32(0)?;
        let y = op.get_as_f32(1)?;
        let point = Point::new(x, y);
        self.current_path_point = point.clone();
        let seg = Segment::LineTo(point);
        if self.current_path.is_empty() {
            return Ok(());
        }
        let last = self.current_path.last_mut_subpath().unwrap();
        last.add_segment(seg);
        Ok(())
    }

    // m
    fn move_to(&mut self, op: Operator) -> PdfResult<()> {
        let x = op.get_as_f32(0)?;
        let y = op.get_as_f32(1)?;
        let point = Point::new(x, y);
        self.path_start = point.clone();
        self.current_path_point = point.clone();
        let seg = Segment::MoveTo(point);
        let subpath = SubPath::new(vec![seg], false);
        self.current_path.add_subpath(subpath);

        //
        Ok(())
    }

    // q
    fn save_sate(&mut self) -> PdfResult<()> {
        if self.state_stack.len() >= self.ctx.limits().max_depth {
            return Err(PdfError::ResourceLimit("graphics state stack depth".into()));
        }
        let total = self
            .stack_clip_segments
            .checked_add(Self::clip_segment_count(&self.current_state)?)
            .filter(|total| *total <= self.ctx.limits().max_path_segments)
            .ok_or_else(|| PdfError::ResourceLimit("graphics stack clipping segments".into()))?;
        self.state_stack
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("graphics state stack allocation".into()))?;
        self.state_stack.push(self.current_state.clone());
        self.stack_clip_segments = total;
        Ok(())
    }
    // Q
    fn restore_state(&mut self) -> PdfResult<()> {
        let mut state = self
            .state_stack
            .pop()
            .ok_or_else(|| PdfError::PageOperatorError("Q without matching q".into()))?;
        self.stack_clip_segments = self
            .stack_clip_segments
            .checked_sub(Self::clip_segment_count(&state)?)
            .ok_or_else(|| {
                PdfError::PageOperatorError("graphics stack clipping accounting mismatch".into())
            })?;
        // Text/line matrices are not part of the saved graphics state.
        state.text_matrix = self.current_state.text_matrix.clone();
        state.text_line_matrix = self.current_state.text_line_matrix.clone();
        self.current_state = state;
        Ok(())
    }

    fn clip_segment_count(state: &AllState) -> PdfResult<usize> {
        let mut count = 0usize;
        for element in state.graphic_state.clip_path.elements() {
            for subpath in element.path.subpaths() {
                count = count.checked_add(subpath.segments().len()).ok_or_else(|| {
                    PdfError::ResourceLimit("graphics clipping segment overflow".into())
                })?;
            }
        }
        Ok(count)
    }

    fn concat_matrix(&mut self, op: Operator) -> PdfResult<()> {
        let a = op.get_as_f32(0)?;
        let b = op.get_as_f32(1)?;
        let c = op.get_as_f32(2)?;
        let d = op.get_as_f32(3)?;
        let e = op.get_as_f32(4)?;
        let f = op.get_as_f32(5)?;
        let matrix = Matrix::new(a, b, c, d, e, f);
        let ctm = matrix.checked_mul(&self.current_state.ctm)?;
        self.current_state.ctm = ctm;
        Ok(())
    }

    fn handle_do(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        let name = op.get_as_name(0)?.name();
        let xobject = self.named_resource("XObject", name)?;
        if xobject.as_stream().is_none() {
            return Err(PdfError::PageResourceError(
                "XObject must be a stream".into(),
            ));
        }
        let subtype_object =
            self.ctx
                .resolve(xobject.get_attr("Subtype").ok_or_else(|| {
                    PdfError::PageResourceError("XObject Subtype is missing".into())
                })?)?;
        let subtype = subtype_object
            .as_name()
            .ok_or_else(|| PdfError::PageResourceError("XObject Subtype must be a name".into()))?
            .name();

        match subtype {
            "Image" => {
                device.preflight_draw(&self.current_state.graphic_state, None)?;
                let pdfimage = PdfImage::try_new(name.to_string(), &xobject, self.ctx)?;
                let matrix = self.current_state.ctm();
                let image_to_user = Matrix::new(1.0, 0.0, 0.0, -1.0, 0.0, 1.0);
                let matrix = image_to_user.checked_mul(&matrix)?;
                let image_matrix = matrix.checked_mul(&self.page.page_matrix)?;
                let imageobject = ImageObject::new(
                    image_matrix,
                    pdfimage,
                    self.current_state.graphic_state.clone(),
                )
                .with_source(
                    op.byte_offset()
                        .filter(|_| self.form_depth == 0)
                        .map(|operator_byte_offset| ImageDrawSource {
                            operation_index: self.operation_index,
                            operator_byte_offset,
                        }),
                );
                let imageobject = imageobject.with_nested_source(self.nested_draw_source()?);
                device.do_image(imageobject)?;
            }
            "Form" => {
                if self.in_text_object {
                    return Err(PdfError::PageOperatorError(
                        "Form invocation inside BT/ET".into(),
                    ));
                }
                let limits = self.ctx.limits();
                let budget = self.execution_budget.child(&limits)?;
                let depth = self.form_depth + 1;
                // Native recursion is bounded until an iterative invocation engine replaces it.
                if depth > 128 {
                    return Err(PdfError::ResourceLimit(
                        "Form implementation depth (128)".into(),
                    ));
                }
                let id = self.resource_entry("XObject", name)?.as_object_id();
                if id.is_some_and(|id| self.active_forms.contains(&id)) {
                    return Err(PdfError::PageResourceError("Cyclic Form invocation".into()));
                }
                let form = crate::page::pdf_form::PdfForm::try_new(&xobject, self.ctx)?;
                let state =
                    form.invocation_state(&self.current_state, &self.page.page_matrix, &limits)?;
                let page_resources = self.page.resource()?;
                let scope = form.resource_scope(&page_resources).clone();
                let data = xobject.as_stream().unwrap().decode_data(self.ctx)?;
                budget.decoded(data.len(), &limits)?;
                let parser = ContentParser::new(crate::parser::syntax::SyntaxParser::with_options(
                    crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(data))?,
                    self.ctx.options(),
                ));
                let mut child =
                    Interpreter::new(self.page, self.ctx, parser).with_resource_scope(scope);
                child.execution_budget = budget;
                child.form_depth = depth;
                child
                    .invocation_path
                    .try_reserve_exact(self.invocation_path.len() + 1)
                    .map_err(|_| {
                        PdfError::ResourceLimit("Form invocation source allocation".into())
                    })?;
                child
                    .invocation_path
                    .extend_from_slice(&self.invocation_path);
                child
                    .invocation_path
                    .push(super::source_provenance::FormInvocation {
                        form_id: id,
                        operation_index: self.operation_index,
                        operator_byte_offset: self.operation_byte_offset,
                    });
                child
                    .active_forms
                    .try_reserve(self.active_forms.len() + usize::from(id.is_some()))
                    .map_err(|_| PdfError::ResourceLimit("Form cycle path allocation".into()))?;
                child.active_forms.extend_from_slice(&self.active_forms);
                if let Some(id) = id {
                    child.active_forms.push(id);
                }
                child.current_state = state;
                child.run_content(device)?;
                if !child.diagnostics.is_empty() {
                    return Err(PdfError::UnsupportedFeature(
                        "Form diagnostics require nested source paths".into(),
                    ));
                }
            }
            _ => {
                return Err(PdfError::UnsupportedFeature(format!(
                    "xobject subtype not supported: {subtype}"
                )));
            }
        }
        Ok(())
    }
}
