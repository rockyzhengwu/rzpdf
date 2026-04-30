use std::io::{Cursor, Read, Seek};

use crate::{
    device::Device,
    error::{PdfError, PdfResult},
    geom::{matrix::Matrix, rectangle::Rectangle},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict},
    page::{content_interpreter::Interpreter, content_parser::ContentParser},
    parser::syntax::SyntaxParser,
    pdf_context::PDFContext,
};

pub mod all_state;
pub mod clip_path;
pub mod color_state;
pub mod display_list;
pub mod content_builder;
pub mod content_interpreter;
pub mod content_parser;
pub mod content_syntax;
pub mod general_state;
pub mod graph_state;
pub mod graphic_state;
pub mod image_object;
pub mod opcode;
pub mod operator;
pub mod page_object;
pub mod path_object;
pub mod pdf_image;
pub mod text_object;
pub mod text_state;

#[derive(Debug)]
pub struct PdfPage<'a, R: Seek + Read> {
    page_node: PdfObject,
    ctx: &'a PDFContext<R>,
    page_width: f32,
    page_height: f32,
    bbox: Rectangle,
    page_matrix: Matrix,
}

impl<'a, R: Seek + Read> PdfPage<'a, R> {
    pub fn new(page_node: PdfObject, ctx: &'a PDFContext<R>) -> PdfPage<'a, R> {
        Self {
            page_node,
            ctx,
            page_width: 612.0,
            page_height: 792.0,
            bbox: Rectangle::new_a4(),
            page_matrix: Matrix::default(),
        }
    }

    fn pagedict(&self) -> PdfResult<PdfDict> {
        self.ctx
            .resolve_owned(&self.page_node)?
            .into_dict()
            .ok_or(PdfError::DocumentError("page node is not a dict".to_string()))
    }

    fn update_dimensions(&mut self) -> PdfResult<()> {
        if let Some(mediaobj) = self.get_pageattr("MediaBox")? {
            self.bbox = Rectangle::try_from(&mediaobj)?;
        }
        if let Some(cropobj) = self.get_pageattr("CropBox")? {
            let cropbox = Rectangle::try_from(&cropobj)?;
            self.bbox.intersect(&cropbox);
        }
        self.page_width = self.bbox.width();
        self.page_height = self.bbox.height();
        let rotation = self.get_pagerotation()?;
        match rotation {
            0 => {
                self.page_matrix =
                    Matrix::new(1.0, 0.0, 0.0, -1.0, self.bbox.llx(), self.bbox.ury());
            }
            1 => {
                let h = self.page_height;
                self.page_height = self.page_width;
                self.page_width = h;
                self.page_matrix =
                    Matrix::new(0.0, -1.0, 1.0, 0.0, self.bbox.lly(), self.bbox.urx());
            }
            2 => {
                self.page_matrix =
                    Matrix::new(-1.0, 0.0, 0.0, -1.0, self.bbox.urx(), self.bbox.ury());
            }
            3 => {
                let h = self.page_height;
                self.page_height = self.page_width;
                self.page_width = h;
                self.page_matrix =
                    Matrix::new(0.0, 1.0, -1.0, 0.0, self.bbox.ury(), self.bbox.llx());
            }
            _ => {}
        }
        Ok(())
    }

    fn get_pagerotation(&self) -> PdfResult<i32> {
        if let Some(rotate) = self.get_pageattr("Rotate")? {
            let v = rotate.as_i32().unwrap_or(0);
            let r = (v / 90) % 4;
            return Ok(r);
        } else {
            Ok(0)
        }
    }

    fn get_pageattr(&self, name: &str) -> PdfResult<Option<PdfObject>> {
        let mut page_dict = self.pagedict()?;
        loop {
            if let Some(obj) = page_dict.get(name) {
                return self.ctx.resolve_owned(obj).map(Some);
            }
            if let Some(parent) = page_dict.get("Parent") {
                let parent = self.ctx.resolve_owned(parent)?;
                page_dict = parent.into_dict().ok_or(PdfError::PageParentIsNotDict)?;
            } else {
                break;
            }
        }
        Ok(None)
    }

    pub fn resource(&self) -> PdfResult<PdfDict> {
        if let Some(resource) = self.get_pageattr("Resources")? {
            let resource_dict = resource
                .into_dict()
                .ok_or(PdfError::PageResourcesIsNotDict)?;
            return Ok(resource_dict);
        }
        return Err(PdfError::PageResourceError(format!(
            "Page Resources is not found"
        )));
    }
    pub fn get_resource_color(&self, name: &str) -> PdfResult<PdfObject> {
        let resource = self.resource()?;
        if let Some(colors) = resource.get("ColorSpace") {
            let color_dict =
                self.ctx
                    .resolve_owned(colors)?
                    .into_dict()
                    .ok_or(PdfError::PageResourceError(
                        "Colorspace is not dict".to_string(),
                    ))?;
            if let Some(obj) = color_dict.get(name) {
                return self.ctx.resolve_owned(obj);
            }
        }
        Err(PdfError::PageResourceError(format!(
            "Colospace resource {0} not found",
            name
        )))
    }
    pub fn get_resource_pattern(&self, name: &str) -> PdfResult<PdfObject> {
        let resource = self.resource()?;
        if let Some(pattern) = resource.get("Pattern") {
            let pattern_dict = self.ctx.resolve_owned(pattern)?;
            if let Some(p) = pattern_dict.get_attr(name) {
                return self.ctx.resolve_owned(p);
            }
        }
        Err(PdfError::PageResourceError(format!(
            "Pattern {0} not found",
            name
        )))
    }

    pub fn get_resource_extgstate(&self, name: &str) -> PdfResult<PdfObject> {
        let resource = self.resource()?;
        if let Some(extend) = resource.get("ExtGState") {
            let extend_dict =
                self.ctx
                    .resolve_owned(extend)?
                    .into_dict()
                    .ok_or(PdfError::PageResourceError(format!(
                        "Extend in Resource is not dict"
                    )))?;
            if let Some(obj) = extend_dict.get(name) {
                return self.ctx.resolve_owned(obj);
            }
        }
        Err(PdfError::PageResourceError(format!(
            "extGState {0} not found",
            name
        )))
    }

    pub fn get_xobject(&self, objname: &str) -> PdfResult<PdfObject> {
        let resource = self.resource()?;
        if let Some(xobject) = resource.get("XObject") {
            let xobject_dict = self
                .ctx
                .resolve_owned(xobject)?
                .into_dict()
                .ok_or(PdfError::PageXobjectIsNotDict)?;
            if let Some(xobj) = xobject_dict.get(objname) {
                return self.ctx.resolve_owned(xobj);
            }
        }
        Err(PdfError::PageXobjectNotFound)
    }

    pub fn get_resource_font(&self, fontname: &str) -> PdfResult<Option<PdfObject>> {
        let resource = self.resource()?;
        if let Some(fonts) = resource.get("Font") {
            let fonts_dict =
                self.ctx
                    .resolve_owned(fonts)?
                    .into_dict()
                    .ok_or(PdfError::PageResourceError(format!(
                        "page font is not dict"
                    )))?;
            if let Some(font) = fonts_dict.get(fontname) {
                return self.ctx.resolve_owned(font).map(Some);
            }
        }
        Ok(None)
    }

    fn content_streams(&self) -> PdfResult<Vec<Vec<u8>>> {
        let mut result = Vec::new();
        if let Some(contents) = self.pagedict()?.get("Contents") {
            match contents {
                PdfObject::PdfArray(contents_array) => {
                    for c in contents_array.into_iter() {
                        let stream_obj = self.ctx.resolve_owned(c)?;
                        let cobj = stream_obj
                            .as_stream()
                            .ok_or(PdfError::PageContentIsNotStream)?;
                        let data = cobj.decode_data(self.ctx)?;
                        result.push(data);
                    }
                }
                PdfObject::PdfReference(_) => {
                    let sobj = self.ctx.resolve_owned(contents)?;
                    match sobj {
                        PdfObject::PdfArray(contents_array) => {
                            for c in contents_array.into_iter() {
                                let stream_obj = self.ctx.resolve_owned(c)?;
                                let cobj = stream_obj
                                    .as_stream()
                                    .ok_or(PdfError::PageContentIsNotStream)?;
                                let data = cobj.decode_data(self.ctx)?;
                                result.push(data);
                            }
                        }
                        PdfObject::PdfStream(content_stream) => {
                            let data = content_stream.decode_data(self.ctx)?;
                            result.push(data);
                        }
                        _ => {
                            return Err(PdfError::PageContentIsNotStream);
                        }
                    }
                }
                _ => {
                    return Err(PdfError::PageContentIsNotStream);
                }
            }
        } else {
            return Ok(result);
        }
        Ok(result)
    }

    pub fn display_list(&mut self) -> PdfResult<display_list::DisplayList> {
        self.update_dimensions()?;
        let contents = self.content_streams()?;
        let mut all_content = Vec::new();
        for content in contents {
            all_content.extend(content);
        }
        let reader = StreamReader::try_new(Cursor::new(all_content))?;
        let syntax = SyntaxParser::new(reader);
        let content_parser = ContentParser::new(syntax);
        let mut interpreter = Interpreter::new(self, &self.ctx, content_parser);
        interpreter.run_to_display_list()
    }

    pub fn display(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        self.display_list()?.replay(device)
    }
}
