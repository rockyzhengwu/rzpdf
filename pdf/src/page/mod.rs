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
    pagedict: &'a PdfDict,
    ctx: &'a PDFContext<R>,
    page_width: f32,
    page_height: f32,
    bbox: Rectangle,
    page_matrix: Matrix,
}

impl<'a, R: Seek + Read> PdfPage<'a, R> {
    pub fn new(pagedict: &'a PdfDict, ctx: &'a PDFContext<R>) -> PdfPage<'a, R> {
        Self {
            pagedict,
            ctx,
            page_width: 612.0,
            page_height: 792.0,
            bbox: Rectangle::new_a4(),
            page_matrix: Matrix::default(),
        }
    }

    fn update_dimensions(&mut self) -> PdfResult<()> {
        if let Some(mediaobj) = self.get_pageattr("MediaBox")? {
            self.bbox = Rectangle::try_from(mediaobj)?;
        }
        if let Some(cropobj) = self.get_pageattr("CropBox")? {
            let cropbox = Rectangle::try_from(cropobj)?;
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

    fn get_pageattr(&self, name: &str) -> PdfResult<Option<&PdfObject>> {
        let mut page_dict = self.pagedict;
        loop {
            if let Some(obj) = page_dict.get(name) {
                let resolve_obj = self.ctx.resolve(obj)?;
                return Ok(Some(resolve_obj));
            } else {
                if let Some(parent) = page_dict.get("Parent") {
                    let parent = self.ctx.resolve(parent)?;
                    page_dict = parent.as_dict().ok_or(PdfError::PageParentIsNotDict)?;
                } else {
                    break;
                }
            }
        }
        return Ok(None);
    }

    pub fn resource(&self) -> PdfResult<&PdfDict> {
        if let Some(resource) = self.get_pageattr("Resources")? {
            let resource_dict = resource.as_dict().ok_or(PdfError::PageResourcesIsNotDict)?;
            return Ok(resource_dict);
        }
        return Err(PdfError::PageResourceError(format!(
            "Page Resources is not found"
        )));
    }
    pub fn get_resource_color(&self, name: &str) -> PdfResult<&PdfObject> {
        if let Some(colors) = self.resource()?.get("ColorSpace") {
            let color_dict =
                self.ctx
                    .resolve(colors)?
                    .as_dict()
                    .ok_or(PdfError::PageResourceError(
                        "Colorspace is not dict".to_string(),
                    ))?;
            if let Some(obj) = color_dict.get(name) {
                let colorobj = self.ctx.resolve(obj)?;
                return Ok(colorobj);
            }
        }
        Err(PdfError::PageResourceError(format!(
            "Colospace resource {0} not found",
            name
        )))
    }
    pub fn get_resource_pattern(&self, name: &str) -> PdfResult<&PdfObject> {
        if let Some(pattern) = self.resource()?.get("Pattern") {
            let pattern_dict = self.ctx.resolve(pattern)?;
            if let Some(p) = pattern_dict.get_attr(name) {
                let pobj = self.ctx.resolve(p)?;
                return Ok(pobj);
            }
        }
        Err(PdfError::PageResourceError(format!(
            "Pattern {0} not found",
            name
        )))
    }

    pub fn get_resource_extgstate(&self, name: &str) -> PdfResult<&PdfObject> {
        if let Some(extend) = self.resource()?.get("ExtGState") {
            let extend_dict =
                self.ctx
                    .resolve(extend)?
                    .as_dict()
                    .ok_or(PdfError::PageResourceError(format!(
                        "Extend in Resource is not dict"
                    )))?;
            if let Some(obj) = extend_dict.get(name) {
                let resobj = self.ctx.resolve(obj)?;
                return Ok(resobj);
            }
        }
        Err(PdfError::PageResourceError(format!(
            "extGState {0} not found",
            name
        )))
    }

    pub fn get_xobject(&self, objname: &str) -> PdfResult<&PdfObject> {
        let resource = self.resource()?;
        if let Some(xobject) = resource.get("XObject") {
            let xobject_dict = self
                .ctx
                .resolve(xobject)?
                .as_dict()
                .ok_or(PdfError::PageXobjectIsNotDict)?;
            if let Some(xobj) = xobject_dict.get(objname) {
                let xxobj = self.ctx.resolve(xobj)?;
                return Ok(xxobj);
            }
        }
        Err(PdfError::PageXobjectNotFound)
    }

    pub fn get_resource_font(&self, fontname: &str) -> PdfResult<Option<&PdfObject>> {
        let resource = self.resource()?;
        if let Some(fonts) = resource.get("Font") {
            let fonts_dict =
                self.ctx
                    .resolve(fonts)?
                    .as_dict()
                    .ok_or(PdfError::PageResourceError(format!(
                        "page font is not dict"
                    )))?;
            if let Some(font) = fonts_dict.get(fontname) {
                return Ok(Some(self.ctx.resolve(font)?));
            }
        }
        Ok(None)
    }

    fn content_streams(&self) -> PdfResult<Vec<Vec<u8>>> {
        let mut result = Vec::new();
        if let Some(contents) = self.pagedict.get("Contents") {
            match contents {
                PdfObject::PdfArray(contents_array) => {
                    for c in contents_array.into_iter() {
                        let cobj = self
                            .ctx
                            .resolve(c)?
                            .as_stream()
                            .ok_or(PdfError::PageContentIsNotStream)?;
                        let data = cobj.decode_data(self.ctx)?;
                        result.push(data);
                    }
                }
                PdfObject::PdfReference(_) => {
                    let sobj = self.ctx.resolve(contents)?;
                    match sobj {
                        PdfObject::PdfArray(contents_array) => {
                            for c in contents_array.into_iter() {
                                let cobj = self
                                    .ctx
                                    .resolve(c)?
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
                    panic!("Page Content stream need to be an array or a stream object");
                }
            }
        } else {
            return Ok(result);
        }
        Ok(result)
    }

    pub fn display(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        self.update_dimensions()?;
        let contents = self.content_streams()?;
        let mut all_content = Vec::new();
        for content in contents {
            all_content.extend(content);
        }
        println!("{}", String::from_utf8(all_content.clone()).unwrap());
        let reader = StreamReader::try_new(Cursor::new(all_content))?;
        let syntax = SyntaxParser::new(reader);
        let content_parser = ContentParser::new(syntax);
        let mut interpreter = Interpreter::new(self, &self.ctx, content_parser);
        interpreter.run(device)?;
        Ok(())
    }
}
