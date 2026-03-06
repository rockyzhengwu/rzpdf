use crate::{
    device::Device,
    font::GlyphDesc,
    page::{
        graphic_state::FillType, image_object::ImageObject, page_object::PageObject,
        path_object::PathObject, text_object::TextObject,
    },
    path::pdf_path::Segment,
};

use quick_xml::Writer;
use quick_xml::events::{BytesEnd, BytesStart, Event};
use std::io::Cursor;

pub struct TraceDevice {
    objects: Vec<PageObject>,
    page_width: f32,
    page_height: f32,
    writer: Writer<Cursor<Vec<u8>>>,
}

impl Device for TraceDevice {
    fn do_image(&mut self, imageobject: ImageObject) {
        self.add_xml_image(&imageobject);
        self.objects.push(PageObject::Image(imageobject));
    }

    fn stroke_path(&mut self, pathobject: PathObject) {
        let name = "stroke_path";
        self.add_xml_path(name, &pathobject);
        self.objects.push(PageObject::Path(pathobject));
    }

    fn fill_path(&mut self, pathobject: PathObject) {
        let name = "fill_path";
        self.add_xml_path(name, &pathobject);
        self.objects.push(PageObject::Path(pathobject))
    }

    fn fill_and_stroke_path(&mut self, pathobject: PathObject) {
        let name = "stroke_and_fill_path";
        self.add_xml_path(name, &pathobject);
        self.objects.push(PageObject::Path(pathobject));
    }

    fn start_page(&mut self, page_width: f32, page_height: f32) {
        self.page_width = page_width;
        self.page_height = page_height;
        let mut page = BytesStart::new("page");
        page.push_attribute(("width", format!("{}", self.page_width).as_str()));
        page.push_attribute(("height", format!("{}", self.page_height).as_str()));
        self.writer.write_event(Event::Start(page)).unwrap();
    }
    fn end_page(&mut self) {
        self.writer
            .write_event(Event::End(BytesEnd::new("page")))
            .unwrap();
    }

    fn clip_path(&mut self, patobject: PathObject) {}

    fn show_text(&mut self, textobject: TextObject) {
        self.add_xml_text(&textobject);
        self.objects.push(PageObject::Text(textobject));
    }
}

impl TraceDevice {
    pub fn new() -> Self {
        let writer = Writer::new(Cursor::new(Vec::new()));
        TraceDevice {
            objects: Vec::new(),
            page_width: 0.0,
            page_height: 0.0,
            writer,
        }
    }
    pub fn objects(&self) -> &[PageObject] {
        &self.objects
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

    fn add_xml_text(&mut self, textobject: &TextObject) {
        let mut text_item = BytesStart::new("text");
        let render_mode = format!("{:?}", textobject.state.text_state.text_rendering_mode);
        text_item.push_attribute(("render_mode", render_mode.as_str()));
        self.writer.write_event(Event::Start(text_item)).unwrap();
        let font = &textobject.state.text_state.font;
        if font.is_none() {
            return;
        }
        for char in textobject.items.iter() {
            let mut char_item = BytesStart::new("char");
            let x = char.pos().x();
            let y = char.pos().y();
            char_item.push_attribute(("x", format!("{}", x).as_str()));
            char_item.push_attribute(("y", format!("{}", y).as_str()));
            if let Some(unicode) = char.unicode() {
                char_item.push_attribute(("unicode", format!("{}", unicode).as_str()));
            }
            if let Some(fo) = font {
                if let Some(glyph) = fo.get_glyph(char.charcode()) {
                    match glyph {
                        GlyphDesc::Name(name) => {
                            char_item.push_attribute(("glyph", format!("{}", name).as_str()));
                        }
                        GlyphDesc::Gid(gid) => {
                            char_item.push_attribute(("glyph", format!("{}", gid).as_str()));
                        }
                    }
                }
            }
            self.writer.write_event(Event::Start(char_item)).unwrap();
            self.writer
                .write_event(Event::End(BytesEnd::new("char")))
                .unwrap();
        }
        self.writer
            .write_event(Event::End(BytesEnd::new("text")))
            .unwrap();
    }

    fn add_xml_image(&mut self, imageobject: &ImageObject) {
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
        self.writer.write_event(Event::Start(image_item)).unwrap();
        self.writer
            .write_event(Event::End(BytesEnd::new("image")))
            .unwrap();
    }

    pub fn add_xml_path(&mut self, name: &str, p: &PathObject) {
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
        self.writer.write_event(Event::Start(path_item)).unwrap();
        for subpath in p.pdf_path().subpaths() {
            for segment in subpath.segments() {
                match segment {
                    Segment::MoveTo(point) => {
                        let mut seg_ele = BytesStart::new("moveto");
                        seg_ele.push_attribute(("x", format!("{}", point.x()).as_str()));
                        seg_ele.push_attribute(("y", format!("{}", point.y()).as_str()));
                        self.writer.write_event(Event::Start(seg_ele)).unwrap();
                        self.writer
                            .write_event(Event::End(BytesEnd::new("moveto")))
                            .unwrap();
                    }
                    Segment::LineTo(point) => {
                        let mut seg_ele = BytesStart::new("lineto");
                        seg_ele.push_attribute(("x", format!("{}", point.x()).as_str()));
                        seg_ele.push_attribute(("y", format!("{}", point.y()).as_str()));
                        self.writer.write_event(Event::Start(seg_ele)).unwrap();
                        self.writer
                            .write_event(Event::End(BytesEnd::new("lineto")))
                            .unwrap();
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
                        self.writer.write_event(Event::Start(seg_ele)).unwrap();
                        self.writer
                            .write_event(Event::End(BytesEnd::new("curveto")))
                            .unwrap();
                    }
                }
            }
        }
        self.writer
            .write_event(Event::End(BytesEnd::new(name)))
            .unwrap();
    }

    pub fn to_xml(self) -> String {
        let buffer = self.writer.into_inner().into_inner();
        String::from_utf8(buffer).unwrap()
    }
}
