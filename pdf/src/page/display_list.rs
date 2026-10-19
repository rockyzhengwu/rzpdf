use crate::{
    device::Device,
    error::PdfResult,
    page::{graphic_state::FillType, page_object::PageObject},
};

#[derive(Debug, Clone, Default)]
pub struct DisplayList {
    optional_content_selection: Option<crate::document::optional_content::OptionalContentSelection>,
    provenance: Option<super::source_provenance::PageSourceProvenance>,
    page_width: f32,
    page_height: f32,
    objects: Vec<PageObject>,
    diagnostics: Vec<crate::page::diagnostic::ContentDiagnostic>,
    retained_payload_bytes: usize,
    retained_path_segments: usize,
}

impl DisplayList {
    /// Source stamps alone do not distinguish two configurations of one PDF.
    pub fn optional_content_selection(
        &self,
    ) -> Option<&crate::document::optional_content::OptionalContentSelection> {
        self.optional_content_selection.as_ref()
    }
    pub(crate) fn set_optional_content_selection(
        &mut self,
        selection: crate::document::optional_content::OptionalContentSelection,
    ) {
        self.optional_content_selection = Some(selection);
    }
    pub(crate) fn clear_edit_provenance(&mut self) {
        self.provenance = None;
        for object in &mut self.objects {
            match object {
                PageObject::Image(v) => v.clear_edit_source(),
                PageObject::Text(v) => v.clear_edit_source(),
                PageObject::Path(v) => v.clear_edit_source(),
                PageObject::Shading(v) => v.clear_edit_source(),
                _ => {}
            }
        }
    }
    /// Allocation-free image geometry candidates, last-painted first. Non-image
    /// occlusion and clip/mask/alpha visibility are not evaluated. Check collection
    /// provenance before using returned images' source indices for editing.
    pub fn image_geometry_hits(
        &self,
        x: f32,
        y: f32,
    ) -> impl Iterator<Item = PdfResult<(usize, &super::image_object::ImageObject)>> {
        self.objects
            .iter()
            .enumerate()
            .rev()
            .filter_map(move |(index, object)| {
                let PageObject::Image(image) = object else {
                    return None;
                };
                match image.geometry_contains_display_point(x, y) {
                    Ok(true) => Some(Ok((index, image))),
                    Ok(false) => None,
                    Err(error) => Some(Err(error)),
                }
            })
    }
    pub fn source_stamp(&self) -> Option<&crate::document::identity::DocumentStamp> {
        self.provenance.as_ref().map(|source| &source.stamp)
    }
    pub fn matches_program(&self, program: &super::content_program::ContentProgram) -> bool {
        self.provenance
            .as_ref()
            .is_some_and(|source| source.matches_program(program))
    }
    pub(crate) fn set_provenance(
        &mut self,
        provenance: super::source_provenance::PageSourceProvenance,
    ) {
        self.provenance = Some(provenance);
    }
    pub fn new(page_width: f32, page_height: f32, objects: Vec<PageObject>) -> Self {
        Self {
            provenance: None,
            optional_content_selection: None,
            page_width,
            page_height,
            objects,
            diagnostics: Vec::new(),
            retained_payload_bytes: 0,
            retained_path_segments: 0,
        }
    }

    pub fn page_width(&self) -> f32 {
        self.page_width
    }

    pub(crate) fn collection_usage(&self) -> (usize, usize) {
        (self.retained_payload_bytes, self.retained_path_segments)
    }
    pub(crate) fn set_collection_usage(&mut self, bytes: usize, segments: usize) {
        self.retained_payload_bytes = bytes;
        self.retained_path_segments = segments;
    }

    pub fn page_height(&self) -> f32 {
        self.page_height
    }

    pub fn objects(&self) -> &[PageObject] {
        &self.objects
    }

    /// Consume the list and move its objects without cloning decoded payloads.
    pub fn into_objects(self) -> Vec<PageObject> {
        self.objects
    }

    /// Move interpreted objects and diagnostics together without payload copies.
    pub fn into_objects_with_diagnostics(
        self,
    ) -> (
        Vec<PageObject>,
        Vec<crate::page::diagnostic::ContentDiagnostic>,
    ) {
        (self.objects, self.diagnostics)
    }

    pub fn diagnostics(&self) -> &[crate::page::diagnostic::ContentDiagnostic] {
        &self.diagnostics
    }

    pub(crate) fn set_diagnostics(
        &mut self,
        diagnostics: Vec<crate::page::diagnostic::ContentDiagnostic>,
    ) {
        self.diagnostics = diagnostics;
    }

    pub fn replay(&self, device: &mut dyn Device) -> PdfResult<()> {
        device.start_page(self.page_width, self.page_height)?;
        self.replay_contents(device)?;
        device.end_page()
    }

    /// Replay into an already started page (e.g. a caller-owned transparency
    /// group). Only groups opened by this list are unwound on failure.
    pub fn replay_contents(&self, device: &mut dyn Device) -> PdfResult<()> {
        let mut device = crate::device::painting::PaintingDevice(device);
        let mut groups = 0usize;
        let result = (|| -> PdfResult<()> {
            for object in &self.objects {
                match object {
                    PageObject::Image(image) => device.do_image(image.clone())?,
                    PageObject::Text(text) => device.show_text(text.clone())?,
                    PageObject::Path(path) => match (path.stroke(), path.fill_type()) {
                        (true, FillType::NoFill) => device.stroke_path(path.clone())?,
                        (false, _) => device.fill_path(path.clone())?,
                        _ => device.fill_and_stroke_path(path.clone())?,
                    },
                    PageObject::Shading(shading) => device.do_shading(shading.clone())?,
                    PageObject::Form => {}
                    PageObject::GroupBegin(group) => {
                        let next = groups.checked_add(1).ok_or_else(|| {
                            crate::error::PdfError::ResourceLimit("display group depth".into())
                        })?;
                        device.begin_group(group.clone())?;
                        groups = next;
                    }
                    PageObject::GroupEnd => {
                        if groups == 0 {
                            return Err(crate::error::PdfError::PageOperatorError(
                                "unmatched display group end".into(),
                            ));
                        }
                        // The backend consumes its boundary even if compositing fails.
                        groups -= 1;
                        device.end_group(true)?;
                    }
                }
            }
            if groups != 0 {
                return Err(crate::error::PdfError::PageOperatorError(
                    "unclosed display group".into(),
                ));
            }
            Ok(())
        })();
        if result.is_err() {
            while groups > 0 {
                let _ = device.end_group(false);
                groups -= 1;
            }
        }
        result?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        error::PdfError,
        page::{image_object::ImageObject, path_object::PathObject, text_object::TextObject},
    };

    struct FailingDevice {
        fail_on: &'static str,
        calls: Vec<&'static str>,
        path_styles: Vec<&'static str>,
    }
    impl FailingDevice {
        fn event(&mut self, event: &'static str) -> PdfResult<()> {
            self.calls.push(event);
            if event == self.fail_on {
                Err(PdfError::IOError(format!("failed {event}")))
            } else {
                Ok(())
            }
        }
    }
    impl Device for FailingDevice {
        fn start_page(&mut self, _: f32, _: f32) -> PdfResult<()> {
            self.event("start")
        }
        fn end_page(&mut self) -> PdfResult<()> {
            self.event("end")
        }
        fn do_image(&mut self, _: ImageObject) -> PdfResult<()> {
            self.event("image")
        }
        fn show_text(&mut self, _: TextObject) -> PdfResult<()> {
            self.event("text")
        }
        fn stroke_path(&mut self, _: PathObject) -> PdfResult<()> {
            self.path_styles.push("stroke");
            self.event("path")
        }
        fn fill_path(&mut self, _: PathObject) -> PdfResult<()> {
            self.path_styles.push("fill");
            self.event("path")
        }
        fn fill_and_stroke_path(&mut self, _: PathObject) -> PdfResult<()> {
            self.path_styles.push("fill-and-stroke");
            self.event("path")
        }
        fn clip_path(&mut self, _: PathObject) -> PdfResult<()> {
            self.event("clip")
        }
    }

    #[test]
    fn shading_replay_requires_backend_support_but_keeps_none_objects() {
        use crate::{
            color::shading::Shading,
            device::trace::TraceDevice,
            document::PdfDocument,
            geom::matrix::Matrix,
            io::stream_reader::StreamReader,
            page::{graphic_state::GraphicState, shading_object::ShadingObject},
            parser::syntax::SyntaxParser,
        };
        let doc = PdfDocument::new().unwrap();
        for none in [false, true] {
            let space = if none {
                "[/Separation /None /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]"
            } else {
                "/DeviceGray"
            };
            let source = format!(
                "<< /ShadingType 2 /ColorSpace {space} /Coords [0 0 10 0] /Function << /FunctionType 2 /Domain [0 1] /N 1 >> >>"
            );
            let object = SyntaxParser::new(
                StreamReader::try_new(std::io::Cursor::new(source.into_bytes())).unwrap(),
            )
            .get_object()
            .unwrap();
            let shading = ShadingObject::new(
                "S".into(),
                Matrix::default(),
                Shading::try_new(&object, doc.ctx()).unwrap(),
                GraphicState::default(),
            );
            let list = DisplayList::new(10.0, 10.0, vec![PageObject::Shading(shading)]);
            assert_eq!(list.objects().len(), 1);
            let mut unsupported = FailingDevice {
                fail_on: "",
                calls: Vec::new(),
                path_styles: Vec::new(),
            };
            let result = list.replay(&mut unsupported);
            if none {
                result.unwrap();
            } else {
                assert!(matches!(result, Err(PdfError::UnsupportedFeature(_))));
            }
            let mut trace = TraceDevice::new();
            list.replay(&mut trace).unwrap();
            assert_eq!(trace.objects().len(), usize::from(!none));
        }
    }

    #[test]
    fn none_path_dispatch_matches_direct_interpretation_without_losing_objects() {
        use crate::{
            document::PdfDocument,
            io::stream_reader::StreamReader,
            page::{content_interpreter::Interpreter, content_parser::ContentParser},
            parser::syntax::SyntaxParser,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let object = SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(
                b"[/Separation /None /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]"
                    .to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap();
        let name = doc.resources().register_color_space(0, object).unwrap();
        let content = format!("/{name} cs 1 0 0 RG 0 0 10 10 re B 0 1 0 rg /{name} CS 10 0 10 10 re B /{name} cs 20 0 10 10 re B 0 0 1 rg 30 0 10 10 re f").into_bytes();
        doc.replace_page_contents(0, content.clone()).unwrap();
        let mut page = doc.get_page(0).unwrap();
        let list = page.display_list().unwrap();
        assert_eq!(list.objects().len(), 4);
        let device = || FailingDevice {
            fail_on: "none",
            calls: Vec::new(),
            path_styles: Vec::new(),
        };
        let mut replayed = device();
        list.replay(&mut replayed).unwrap();
        assert_eq!(replayed.path_styles, ["stroke", "fill", "fill"]);
        let parser = ContentParser::new(SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(content)).unwrap(),
        ));
        let mut direct = device();
        Interpreter::new(&page, doc.ctx(), parser)
            .run(&mut direct)
            .unwrap();
        assert_eq!(direct.path_styles, replayed.path_styles);
        assert_eq!(direct.calls, replayed.calls);
        assert_eq!(list.objects().len(), 4);
    }

    #[test]
    fn replay_preserves_all_path_painting_modes_and_stops_on_draw_error() {
        use crate::{
            document::PdfDocument,
            io::stream_reader::StreamReader,
            page::{content_interpreter::Interpreter, content_parser::ContentParser},
            parser::syntax::SyntaxParser,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let modes = ["S", "s", "f", "F", "f*", "B", "B*", "b", "b*"];
        let content = modes
            .iter()
            .map(|mode| format!("10 20 30 40 re {mode}\n"))
            .collect::<String>()
            .into_bytes();
        doc.replace_page_contents(0, content.clone()).unwrap();
        let mut page = doc.get_page(0).unwrap();
        let list = page.display_list().unwrap();
        let mut replayed = FailingDevice {
            fail_on: "none",
            calls: Vec::new(),
            path_styles: Vec::new(),
        };
        list.replay(&mut replayed).unwrap();
        assert_eq!(
            replayed.path_styles,
            [
                "stroke",
                "stroke",
                "fill",
                "fill",
                "fill",
                "fill-and-stroke",
                "fill-and-stroke",
                "fill-and-stroke",
                "fill-and-stroke"
            ]
        );
        let reader = StreamReader::try_new(std::io::Cursor::new(content)).unwrap();
        let parser = ContentParser::new(SyntaxParser::new(reader));
        let mut direct = FailingDevice {
            fail_on: "none",
            calls: Vec::new(),
            path_styles: Vec::new(),
        };
        Interpreter::new(&page, doc.ctx(), parser)
            .run(&mut direct)
            .unwrap();
        assert_eq!(direct.path_styles, replayed.path_styles);
        assert_eq!(direct.calls, replayed.calls);
        // Each distinct callback must stop replay immediately on failure.
        for (object_index, style) in [(0, "stroke"), (2, "fill"), (5, "fill-and-stroke")] {
            let one = DisplayList::new(
                100.,
                100.,
                vec![
                    list.objects()[object_index].clone(),
                    list.objects()[object_index].clone(),
                ],
            );
            let mut failed = FailingDevice {
                fail_on: "path",
                calls: Vec::new(),
                path_styles: Vec::new(),
            };
            assert!(one.replay(&mut failed).is_err());
            assert_eq!(failed.path_styles, [style]);
            assert_eq!(failed.calls, ["start", "path"]);
        }
    }

    #[test]
    fn replay_propagates_start_draw_and_end_failures() {
        let list = DisplayList::new(
            100.0,
            100.0,
            vec![
                PageObject::Path(PathObject::default()),
                PageObject::Path(PathObject::default()),
            ],
        );
        for (event, expected) in [
            ("start", vec!["start"]),
            ("path", vec!["start", "path"]),
            ("end", vec!["start", "path", "path", "end"]),
        ] {
            let mut device = FailingDevice {
                fail_on: event,
                calls: Vec::new(),
                path_styles: Vec::new(),
            };
            let error = list.replay(&mut device).unwrap_err();
            assert!(error.to_string().contains(event));
            assert_eq!(device.calls, expected);
        }
    }

    #[test]
    fn direct_interpreter_propagates_draw_failure() {
        use crate::{
            document::PdfDocument,
            io::stream_reader::StreamReader,
            page::{content_interpreter::Interpreter, content_parser::ContentParser},
            parser::syntax::SyntaxParser,
        };
        let doc = PdfDocument::open("../tests/path-test.pdf").unwrap();
        let page = doc.get_page(0).unwrap();
        let reader =
            StreamReader::try_new(std::io::Cursor::new(b"0 0 10 10 re f".to_vec())).unwrap();
        let parser = ContentParser::new(SyntaxParser::new(reader));
        let mut interpreter = Interpreter::new(&page, doc.ctx(), parser);
        let mut device = FailingDevice {
            fail_on: "path",
            calls: Vec::new(),
            path_styles: Vec::new(),
        };
        assert!(
            interpreter
                .run(&mut device)
                .unwrap_err()
                .to_string()
                .contains("path")
        );
        assert_eq!(device.calls, vec!["start", "path"]);
    }
}
