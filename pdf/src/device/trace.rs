use crate::{
    device::Device,
    error::{PdfError, PdfResult},
    font::GlyphDesc,
    page::{
        display_list::DisplayList, graphic_state::FillType, image_object::ImageObject,
        page_object::PageObject, path_object::PathObject, text_object::TextObject,
    },
    path::pdf_path::Segment,
};

use quick_xml::Writer;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use std::io::Cursor;

pub struct TraceDevice {
    collect_xml: bool,
    limits: Option<crate::parser::options::Limits>,
    retained_payload_bytes: usize,
    retained_images: std::collections::HashSet<usize>,
    retained_resources: std::collections::HashSet<(u8, usize)>,
    pending_resources: Vec<(u8, usize)>,
    retained_path_segments: usize,
    objects: Vec<PageObject>,
    page_width: f32,
    page_height: f32,
    writer: Writer<Cursor<Vec<u8>>>,
}

impl Default for TraceDevice {
    fn default() -> Self {
        Self::new()
    }
}

impl Device for TraceDevice {
    fn begin_group(&mut self, group: crate::page::transparency::GroupPaint) -> PdfResult<()> {
        self.reserve_object()?;
        let mut state = crate::page::graphic_state::GraphicState {
            clip_path: group.clip.clone(),
            ..Default::default()
        };
        state.general_state.soft_mask = group.soft_mask.clone();
        let mut payload = self.next_draw_bytes(None, &state)?;
        if let Some(profile) = &group.blending_profile {
            payload = payload
                .checked_add(self.resource_bytes(
                    0,
                    profile.storage_identity(),
                    profile.retained_source_bytes(),
                )?)
                .ok_or_else(|| PdfError::ResourceLimit("group profile storage".into()))?;
        }
        if self
            .limits
            .is_some_and(|limits| payload > limits.max_display_list_bytes)
        {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "display_list_bytes",
                stage: "page_prepare",
                limit: self.limits.unwrap().max_display_list_bytes as u64,
                actual: payload as u64,
            });
        }
        let segments = self.next_retained_segments(None, &state)?;
        self.retained_path_segments = segments;
        self.retained_payload_bytes = payload;
        self.commit_resources();
        self.objects.push(PageObject::GroupBegin(group));
        Ok(())
    }
    fn end_group(&mut self, commit: bool) -> PdfResult<()> {
        if !commit {
            return Err(PdfError::PageOperatorError(
                "aborted group display list".into(),
            ));
        }
        self.reserve_object()?;
        self.objects.push(PageObject::GroupEnd);
        Ok(())
    }
    fn do_shading(&mut self, shading: crate::page::shading_object::ShadingObject) -> PdfResult<()> {
        self.reserve_object()?;
        let segments = self.next_retained_segments(None, &shading.graphic_state)?;
        let base = self.next_draw_bytes(shading.nested_source(), &shading.graphic_state)?;
        let shared = self.resource_bytes(
            3,
            std::sync::Arc::as_ptr(&shading.shading) as usize,
            shading.retained_payload_bytes()?,
        )?;
        let total = base
            .checked_add(shared)
            .ok_or_else(|| PdfError::ResourceLimit("display retained payload overflow".into()))?;
        if self
            .limits
            .is_some_and(|limits| total > limits.max_display_list_bytes)
        {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "display_list_bytes",
                stage: "page_prepare",
                limit: self.limits.unwrap().max_display_list_bytes as u64,
                actual: total as u64,
            });
        }
        if self.collect_xml {
            let mut event = BytesStart::new("shading");
            event.push_attribute(("resource", shading.resource_name.as_str()));
            self.writer.write_event(Event::Empty(event))?;
        }
        self.retained_payload_bytes = total;
        self.commit_resources();
        self.retained_path_segments = segments;
        self.objects.push(PageObject::Shading(shading));
        Ok(())
    }
    fn preflight_draw(
        &self,
        state: &crate::page::graphic_state::GraphicState,
        path: Option<&crate::path::pdf_path::PdfPath>,
    ) -> PdfResult<()> {
        if self
            .limits
            .is_some_and(|limits| self.objects.len() >= limits.max_objects)
        {
            return Err(PdfError::ResourceLimit("display object count".into()));
        }
        self.next_retained_segments(path, state)?;
        Ok(())
    }
    fn do_image(&mut self, imageobject: ImageObject) -> PdfResult<()> {
        self.reserve_object()?;
        let segments = self.next_retained_segments(None, &imageobject.graphic_state)?;
        let identity = std::rc::Rc::as_ptr(&imageobject.image) as usize;
        let first = !self.retained_images.contains(&identity);
        let total = self
            .next_draw_bytes(imageobject.nested_source(), &imageobject.graphic_state)?
            .checked_add(if first {
                imageobject.image.retained_sample_bytes()?
            } else {
                0
            })
            .ok_or_else(|| PdfError::ResourceLimit("display retained payload overflow".into()))?;
        if self
            .limits
            .is_some_and(|limits| total > limits.max_display_list_bytes)
        {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "display_list_bytes",
                stage: "page_prepare",
                limit: self.limits.unwrap().max_display_list_bytes as u64,
                actual: total as u64,
            });
        }
        self.add_xml_image(&imageobject)?;
        if first {
            self.retained_images
                .try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("display image identities".into()))?;
            self.retained_images.insert(identity);
        }
        self.retained_payload_bytes = total;
        self.commit_resources();
        self.retained_path_segments = segments;
        self.objects.push(PageObject::Image(imageobject));
        Ok(())
    }

    fn stroke_path(&mut self, pathobject: PathObject) -> PdfResult<()> {
        let payload =
            self.next_draw_bytes(pathobject.nested_source(), pathobject.graphic_state())?;
        self.reserve_object()?;
        let segments =
            self.next_retained_segments(Some(pathobject.pdf_path()), pathobject.graphic_state())?;
        let name = "stroke_path";
        self.add_xml_path(name, &pathobject)?;
        self.retained_path_segments = segments;
        self.retained_payload_bytes = payload;
        self.commit_resources();
        self.objects.push(PageObject::Path(pathobject));
        Ok(())
    }

    fn fill_path(&mut self, pathobject: PathObject) -> PdfResult<()> {
        let payload =
            self.next_draw_bytes(pathobject.nested_source(), pathobject.graphic_state())?;
        self.reserve_object()?;
        let segments =
            self.next_retained_segments(Some(pathobject.pdf_path()), pathobject.graphic_state())?;
        let name = "fill_path";
        self.add_xml_path(name, &pathobject)?;
        self.retained_path_segments = segments;
        self.retained_payload_bytes = payload;
        self.commit_resources();
        self.objects.push(PageObject::Path(pathobject));
        Ok(())
    }

    fn fill_and_stroke_path(&mut self, pathobject: PathObject) -> PdfResult<()> {
        let payload =
            self.next_draw_bytes(pathobject.nested_source(), pathobject.graphic_state())?;
        self.reserve_object()?;
        let segments =
            self.next_retained_segments(Some(pathobject.pdf_path()), pathobject.graphic_state())?;
        let name = "stroke_and_fill_path";
        self.add_xml_path(name, &pathobject)?;
        self.retained_path_segments = segments;
        self.retained_payload_bytes = payload;
        self.commit_resources();
        self.objects.push(PageObject::Path(pathobject));
        Ok(())
    }

    fn start_page(&mut self, page_width: f32, page_height: f32) -> PdfResult<()> {
        self.page_width = page_width;
        self.page_height = page_height;
        if !self.collect_xml {
            return Ok(());
        }
        let mut page = BytesStart::new("page");
        page.push_attribute(("width", format!("{}", self.page_width).as_str()));
        page.push_attribute(("height", format!("{}", self.page_height).as_str()));
        self.writer.write_event(Event::Start(page))?;
        Ok(())
    }
    fn end_page(&mut self) -> PdfResult<()> {
        if !self.collect_xml {
            return Ok(());
        }
        self.writer.write_event(Event::End(BytesEnd::new("page")))?;
        Ok(())
    }

    fn clip_path(&mut self, _patobject: PathObject) -> PdfResult<()> {
        Ok(())
    }

    fn show_text(&mut self, textobject: TextObject) -> PdfResult<()> {
        self.reserve_object()?;
        let segments = self.next_retained_segments(None, &textobject.state)?;
        let mut total = self.next_draw_bytes(textobject.nested_source(), &textobject.state)?;
        total = total
            .checked_add(textobject.language().map_or(0, str::len))
            .ok_or_else(|| PdfError::ResourceLimit("display language bytes overflow".into()))?;
        for item in &textobject.items {
            total = total
                .checked_add(item.unicode().map_or(0, |text| text.len()))
                .ok_or_else(|| PdfError::ResourceLimit("display Unicode bytes overflow".into()))?;
        }
        if self
            .limits
            .is_some_and(|limits| total > limits.max_display_list_bytes)
        {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "display_list_bytes",
                stage: "page_prepare",
                limit: self.limits.unwrap().max_display_list_bytes as u64,
                actual: total as u64,
            });
        }
        self.add_xml_text(&textobject)?;
        self.retained_payload_bytes = total;
        self.commit_resources();
        self.retained_path_segments = segments;
        self.objects.push(PageObject::Text(textobject));
        Ok(())
    }
}

impl TraceDevice {
    pub(crate) fn pattern_storage(&self) -> PdfResult<(usize, usize)> {
        let bytes = self
            .objects
            .capacity()
            .checked_mul(std::mem::size_of::<PageObject>())
            .and_then(|v| {
                self.retained_path_segments
                    .checked_mul(std::mem::size_of::<Segment>())
                    .and_then(|s| v.checked_add(s))
            })
            .and_then(|v| v.checked_add(self.retained_payload_bytes))
            .ok_or_else(|| PdfError::ResourceLimit("pattern retained storage overflow".into()))?;
        if self
            .limits
            .is_some_and(|limits| bytes > limits.max_display_list_bytes)
        {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "display_list_bytes",
                stage: "page_prepare",
                limit: self.limits.unwrap().max_display_list_bytes as u64,
                actual: bytes as u64,
            });
        }
        Ok((bytes, self.retained_path_segments))
    }

    fn resource_bytes(&mut self, kind: u8, identity: usize, bytes: usize) -> PdfResult<usize> {
        let key = (kind, identity);
        if self.retained_resources.contains(&key) || self.pending_resources.contains(&key) {
            return Ok(0);
        }
        self.pending_resources
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("display resource identities".into()))?;
        self.retained_resources
            .try_reserve(self.pending_resources.len() + 1)
            .map_err(|_| PdfError::ResourceLimit("display resource identities".into()))?;
        self.pending_resources.push(key);
        Ok(bytes)
    }
    fn commit_resources(&mut self) {
        self.retained_resources
            .extend(self.pending_resources.drain(..));
    }
    fn next_draw_bytes(
        &mut self,
        source: Option<&crate::page::source_provenance::NestedDrawSource>,
        state: &crate::page::graphic_state::GraphicState,
    ) -> PdfResult<usize> {
        self.pending_resources.clear();
        let mut total = self.next_nested_source_bytes(source)?;
        if let Some(mask) = &state.general_state.soft_mask {
            total = total
                .checked_add(self.resource_bytes(
                    1,
                    std::sync::Arc::as_ptr(mask) as usize,
                    mask.retained_bytes,
                )?)
                .ok_or_else(|| PdfError::ResourceLimit("soft mask draw payload overflow".into()))?;
        }
        for space in [
            &state.color_state.fill_color_space,
            &state.color_state.stroke_color_space,
        ] {
            space.visit_profiles(&mut |profile| {
                total = total
                    .checked_add(self.resource_bytes(
                        0,
                        profile.storage_identity(),
                        profile.retained_source_bytes(),
                    )?)
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("colour profile draw payload overflow".into())
                    })?;
                Ok(())
            })?;
            if let crate::color::colorspace::ColorSpace::Pattern(space) = space
                && let Some(paint) = space.paint()
            {
                total = total
                    .checked_add(self.resource_bytes(
                        2,
                        paint as *const _ as usize,
                        paint.retained_bytes,
                    )?)
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("pattern draw payload overflow".into())
                    })?;
            }
        }
        if self
            .limits
            .is_some_and(|limits| total > limits.max_display_list_bytes)
        {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "display_list_bytes",
                stage: "page_prepare",
                limit: self.limits.unwrap().max_display_list_bytes as u64,
                actual: total as u64,
            });
        }
        Ok(total)
    }
    fn next_nested_source_bytes(
        &self,
        source: Option<&crate::page::source_provenance::NestedDrawSource>,
    ) -> PdfResult<usize> {
        let bytes = source.map(|s| s.retained_bytes()).transpose()?.unwrap_or(0);
        let total = self
            .retained_payload_bytes
            .checked_add(bytes)
            .ok_or_else(|| {
                PdfError::ResourceLimit("display nested source bytes overflow".into())
            })?;
        if self
            .limits
            .is_some_and(|limits| total > limits.max_display_list_bytes)
        {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "display_list_bytes",
                stage: "page_prepare",
                limit: self.limits.unwrap().max_display_list_bytes as u64,
                actual: total as u64,
            });
        }
        Ok(total)
    }
    pub fn new() -> Self {
        let writer = Writer::new(Cursor::new(Vec::new()));
        TraceDevice {
            collect_xml: true,
            limits: None,
            retained_payload_bytes: 0,
            retained_images: Default::default(),
            retained_resources: Default::default(),
            pending_resources: Vec::new(),
            retained_path_segments: 0,
            objects: Vec::new(),
            page_width: 0.0,
            page_height: 0.0,
            writer,
        }
    }
    /// Internal display collection does not build the unused debugging XML copy.
    pub(crate) fn for_display_list(limits: crate::parser::options::Limits) -> Self {
        let mut device = Self::new();
        device.collect_xml = false;
        device.limits = Some(limits);
        device
    }
    fn next_retained_segments(
        &self,
        path: Option<&crate::path::pdf_path::PdfPath>,
        state: &crate::page::graphic_state::GraphicState,
    ) -> PdfResult<usize> {
        let mut total = self.retained_path_segments;
        if let Some(mask) = &state.general_state.soft_mask {
            total = total.checked_add(mask.retained_segments).ok_or_else(|| {
                PdfError::ResourceLimit("soft mask draw segments overflow".into())
            })?;
        }
        for space in [
            &state.color_state.fill_color_space,
            &state.color_state.stroke_color_space,
        ] {
            if let crate::color::colorspace::ColorSpace::Pattern(space) = space
                && let Some(paint) = space.paint()
            {
                total = total.checked_add(paint.retained_segments).ok_or_else(|| {
                    PdfError::ResourceLimit("pattern draw segments overflow".into())
                })?;
            }
        }
        for path in path
            .into_iter()
            .chain(state.clip_path.elements().iter().map(|clip| &clip.path))
        {
            for subpath in path.subpaths() {
                total = total.checked_add(subpath.segments().len()).ok_or_else(|| {
                    PdfError::ResourceLimit("display retained path segments overflow".into())
                })?;
            }
        }
        if self
            .limits
            .is_some_and(|limits| total > limits.max_path_segments)
        {
            return Err(PdfError::ResourceLimit(
                "display retained path segments".into(),
            ));
        }
        Ok(total)
    }
    fn reserve_object(&mut self) -> PdfResult<()> {
        if self
            .limits
            .is_some_and(|limits| self.objects.len() >= limits.max_objects)
        {
            return Err(PdfError::ResourceLimit("display object count".into()));
        }
        self.objects
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("display object allocation".into()))
    }
    pub fn objects(&self) -> &[PageObject] {
        &self.objects
    }

    pub fn into_display_list(self) -> DisplayList {
        let mut list = DisplayList::new(self.page_width, self.page_height, self.objects);
        list.set_collection_usage(self.retained_payload_bytes, self.retained_path_segments);
        list
    }

    pub fn pathname(&self, path: &PathObject) -> &str {
        match path.fill_type() {
            FillType::NoFill => {
                if path.stroke() {
                    "stroke_path"
                } else {
                    "fill_path"
                }
            }
            _ => {
                if path.stroke() {
                    "stroke_and_fill_path"
                } else {
                    "fill_path"
                }
            }
        }
    }

    fn add_xml_text(&mut self, textobject: &TextObject) -> PdfResult<()> {
        if !self.collect_xml {
            return Ok(());
        }
        let mut text_item = BytesStart::new("text");
        let render_mode = format!("{:?}", textobject.state.text_state.text_rendering_mode);
        text_item.push_attribute(("render_mode", render_mode.as_str()));
        self.writer.write_event(Event::Start(text_item))?;
        let font = &textobject.state.text_state.font;
        for char in textobject.items.iter() {
            let mut char_item = BytesStart::new("char");
            let x = char.pos().x();
            let y = char.pos().y();
            char_item.push_attribute(("x", format!("{}", x).as_str()));
            char_item.push_attribute(("y", format!("{}", y).as_str()));
            if let Some(unicode) = char.unicode() {
                char_item.push_attribute(("unicode", unicode.to_string().as_str()));
            }
            if let Some(fo) = font
                && let Some(glyph) = fo.get_glyph(char.charcode())
            {
                match glyph {
                    GlyphDesc::Name(name) => {
                        char_item.push_attribute(("glyph", name.to_string().as_str()));
                    }
                    GlyphDesc::Gid(gid) => {
                        char_item.push_attribute(("glyph", format!("{}", gid).as_str()));
                    }
                }
            }
            self.writer.write_event(Event::Start(char_item))?;
            self.writer.write_event(Event::End(BytesEnd::new("char")))?;
        }
        self.writer.write_event(Event::End(BytesEnd::new("text")))?;
        Ok(())
    }

    fn add_xml_image(&mut self, imageobject: &ImageObject) -> PdfResult<()> {
        if !self.collect_xml {
            return Ok(());
        }
        let mut image_item = BytesStart::new("image");
        if imageobject.image.is_mask() {
            image_item.push_attribute(("mask", "true"));
        }
        let matrix = &imageobject.matrix;
        image_item.push_attribute(("width", format!("{}", imageobject.image.width()).as_str()));
        image_item.push_attribute(("height", format!("{}", imageobject.image.height()).as_str()));
        image_item.push_attribute((
            "matrix",
            format!(
                "{},{},{},{},{},{}",
                matrix.a(),
                matrix.b(),
                matrix.c(),
                matrix.d(),
                matrix.e(),
                matrix.f()
            )
            .as_str(),
        ));
        self.writer.write_event(Event::Start(image_item))?;
        self.writer
            .write_event(Event::End(BytesEnd::new("image")))?;
        Ok(())
    }

    pub fn add_xml_path(&mut self, name: &str, p: &PathObject) -> PdfResult<()> {
        if !self.collect_xml {
            return Ok(());
        }
        let mut path_item = BytesStart::new(name);
        if p.stroke() {
            let linewidth = p.graphic_state().graph_state.line_width;
            path_item.push_attribute(("linewidth", format!("{}", linewidth).as_str()));
            let dash_phrase = p.graphic_state().graph_state.dash_phrase;
            let dash_array = &p.graphic_state().graph_state.dash_array;
            if !dash_array.is_empty() || dash_phrase != 0.0 {
                path_item.push_attribute(("dash_phrase", format!("{}", dash_phrase).as_str()));
                path_item.push_attribute(("dash_array", format!("{:?}", dash_array).as_str()));
            }
        }
        let matrix = p.matrix();
        path_item.push_attribute((
            "matrix",
            format!(
                "{},{},{},{},{},{}",
                matrix.a(),
                matrix.b(),
                matrix.c(),
                matrix.d(),
                matrix.e(),
                matrix.f()
            )
            .as_str(),
        ));
        self.writer.write_event(Event::Start(path_item))?;
        for subpath in p.pdf_path().subpaths() {
            for segment in subpath.segments() {
                match segment {
                    Segment::Close => {
                        self.writer
                            .write_event(Event::Empty(BytesStart::new("close")))?;
                    }
                    Segment::MoveTo(point) => {
                        let mut seg_ele = BytesStart::new("moveto");
                        seg_ele.push_attribute(("x", format!("{}", point.x()).as_str()));
                        seg_ele.push_attribute(("y", format!("{}", point.y()).as_str()));
                        self.writer.write_event(Event::Start(seg_ele))?;
                        self.writer
                            .write_event(Event::End(BytesEnd::new("moveto")))?;
                    }
                    Segment::LineTo(point) => {
                        let mut seg_ele = BytesStart::new("lineto");
                        seg_ele.push_attribute(("x", format!("{}", point.x()).as_str()));
                        seg_ele.push_attribute(("y", format!("{}", point.y()).as_str()));
                        self.writer.write_event(Event::Start(seg_ele))?;
                        self.writer
                            .write_event(Event::End(BytesEnd::new("lineto")))?;
                    }
                    Segment::CurveTo(bezier) => {
                        let mut seg_ele = BytesStart::new("curveto");
                        let start = bezier.p1();
                        seg_ele.push_attribute(("x1", format!("{}", start.x()).as_str()));
                        seg_ele.push_attribute(("y1", format!("{}", start.y()).as_str()));

                        let p2 = bezier.p2();
                        seg_ele.push_attribute(("x2", format!("{}", p2.x()).as_str()));
                        seg_ele.push_attribute(("y2", format!("{}", p2.y()).as_str()));

                        let p3 = bezier.p3();
                        seg_ele.push_attribute(("x3", format!("{}", p3.x()).as_str()));
                        seg_ele.push_attribute(("y3", format!("{}", p3.y()).as_str()));
                        self.writer.write_event(Event::Start(seg_ele))?;
                        self.writer
                            .write_event(Event::End(BytesEnd::new("curveto")))?;
                    }
                }
            }
        }
        self.writer.write_event(Event::End(BytesEnd::new(name)))?;
        Ok(())
    }

    pub fn to_xml(self) -> String {
        let buffer = self.writer.into_inner().into_inner();
        String::from_utf8(buffer).unwrap()
    }
}

#[cfg(test)]
mod collection_tests {
    use super::*;

    #[test]
    #[allow(clippy::arc_with_non_send_sync)] // ShadingObject owns an Arc in the public, single-threaded model.
    fn shading_collection_counts_shared_function_payload_once() {
        use crate::{
            color::shading::Shading,
            document::PdfDocument,
            geom::matrix::Matrix,
            io::stream_reader::StreamReader,
            page::{graphic_state::GraphicState, shading_object::ShadingObject},
            parser::syntax::SyntaxParser,
        };
        let doc = PdfDocument::new().unwrap();
        let object = SyntaxParser::new(StreamReader::try_new(Cursor::new(b"<< /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 10 0] /Function << /FunctionType 2 /Domain [0 1] /N 1 >> >>".to_vec())).unwrap()).get_object().unwrap();
        let shading = ShadingObject::new(
            "S".into(),
            Matrix::default(),
            Shading::try_new(&object, doc.ctx()).unwrap(),
            GraphicState::default(),
        );
        let bytes = shading.retained_payload_bytes().unwrap();
        assert!(bytes > 0);
        let clone = shading.clone();
        assert!(std::sync::Arc::ptr_eq(&clone.shading, &shading.shading));
        let mut trace = TraceDevice::for_display_list(crate::parser::options::Limits {
            max_display_list_bytes: bytes,
            ..Default::default()
        });
        trace.start_page(10.0, 10.0).unwrap();
        trace.do_shading(shading.clone()).unwrap();
        trace.do_shading(clone).unwrap();
        assert_eq!(trace.objects().len(), 2);
        let mut independent = shading.clone();
        independent.shading = std::sync::Arc::new((*independent.shading).clone());
        assert!(matches!(
            trace.do_shading(independent),
            Err(PdfError::ResourceLimitExceeded {
                reason: "display_list_bytes",
                ..
            })
        ));
        // The failed draw must not publish its pending resource identities.
        trace.do_shading(shading.clone()).unwrap();
        assert_eq!(trace.objects().len(), 3);
        assert_eq!(trace.to_xml(), "");
        let mut trace = TraceDevice::for_display_list(crate::parser::options::Limits {
            max_objects: 1,
            ..Default::default()
        });
        trace.do_shading(shading.clone()).unwrap();
        assert!(
            matches!(trace.do_shading(shading), Err(PdfError::ResourceLimit(message)) if message == "display object count")
        );
    }

    #[test]
    fn display_collection_is_bounded_and_does_not_duplicate_xml() {
        let mut display = TraceDevice::for_display_list(crate::parser::options::Limits {
            max_objects: 1,
            ..Default::default()
        });
        display.start_page(10., 10.).unwrap();
        display.fill_path(PathObject::default()).unwrap();
        assert!(matches!(
            display.fill_path(PathObject::default()),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(display.objects().len(), 1);
        display.end_page().unwrap();
        assert_eq!(display.to_xml(), "");

        let mut debug = TraceDevice::new();
        debug.start_page(10., 10.).unwrap();
        debug.fill_path(PathObject::default()).unwrap();
        debug.end_page().unwrap();
        let xml = debug.to_xml();
        assert!(xml.contains("<page"));
        assert!(xml.contains("fill_path"));
        assert!(xml.ends_with("</page>"));
    }
}
