use std::io::{Cursor, Read, Seek};

use crate::{
    device::Device,
    error::{PdfError, PdfResult},
    geom::{matrix::Matrix, point::Point, rectangle::Rectangle},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_dict::PdfDict},
    page::{content_interpreter::Interpreter, content_parser::ContentParser},
    parser::syntax::SyntaxParser,
    pdf_context::PDFContext,
};

pub mod all_state;
pub mod clip_path;
pub mod color_state;
pub mod content_builder;
pub mod content_interpreter;
pub mod content_parser;
pub mod content_program;
pub mod content_syntax;
pub mod diagnostic;
pub mod display_list;
pub(crate) mod execution_budget;
pub(crate) mod font_binding;
pub use font_binding::BudgetedFont as InspectedFont;
pub mod general_state;
pub mod graph_state;
pub mod graphic_state;
pub mod image_extraction;
pub mod image_object;
pub mod inline_image;
pub mod opcode;
pub mod operator;
pub mod page_builder;
pub mod page_object;
pub mod path_object;
pub mod pdf_form;
pub mod pdf_image;
pub mod shading_object;
pub mod source_provenance;
pub mod text_extraction;
pub mod text_object;
pub mod text_state;

/// Effective page geometry, independent of content decoding or interpretation.
#[derive(Debug, Clone)]
pub struct PageGeometry {
    /// MediaBox intersected with CropBox, in unrotated PDF user coordinates.
    pub visible_box: Rectangle,
    /// Normalized clockwise degrees: 0, 90, 180 or 270.
    pub rotation: i32,
    /// Default user-space unit size in multiples of 1/72 inch (PDF 1.6).
    pub user_unit: f32,
    pub display_width: f32,
    pub display_height: f32,
    /// Maps PDF user space to top-left-origin, y-down display coordinates.
    pub user_to_display: Matrix,
}

impl PageGeometry {
    /// Convert a display-space point to unrotated PDF user space.
    /// Does not clamp to the visible box or perform content hit testing.
    pub fn display_to_user_point(&self, point: &Point) -> PdfResult<Point> {
        self.user_to_display
            .checked_inverse()?
            .checked_transform(point)
    }
}

/// Shared read/edit normalization; parse the integer lexeme, never rounded f32.
pub(crate) fn normalized_page_rotation(rotate: &PdfObject) -> PdfResult<i32> {
    let number = rotate
        .as_number()
        .ok_or_else(|| PdfError::PageResourceError("Rotate must be an integer".into()))?;
    if !number.is_integer() {
        return Err(PdfError::PageResourceError(
            "Rotate must be an integer".into(),
        ));
    }
    let value = number.lexeme().parse::<i64>().map_err(|_| {
        PdfError::PageResourceError("Rotate is outside signed integer range".into())
    })?;
    if value % 90 != 0 {
        return Err(PdfError::PageResourceError(
            "Rotate must be a multiple of 90".into(),
        ));
    }
    Ok((value / 90).rem_euclid(4) as i32 * 90)
}

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
            .ok_or(PdfError::DocumentError(
                "page node is not a dict".to_string(),
            ))
    }

    fn pagedict_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfDict, crate::budget::MemoryLease)> {
        let (object, lease) = self
            .ctx
            .resolve_owned_with_budget(&self.page_node, budget)?;
        let dict = object
            .into_dict()
            .ok_or_else(|| PdfError::DocumentError("page node is not a dict".into()))?;
        Ok((dict, lease))
    }

    fn update_dimensions(&mut self) -> PdfResult<()> {
        let mut bbox = Rectangle::new_a4();
        if let Some(mediaobj) = self.get_pageattr("MediaBox")? {
            bbox = Rectangle::try_from_with_context(&mediaobj, self.ctx)?;
        }
        if let Some(cropobj) = self.get_pageattr("CropBox")? {
            let cropbox = Rectangle::try_from_with_context(&cropobj, self.ctx)?;
            bbox.intersect(&cropbox);
        }
        let width = bbox.width();
        let height = bbox.height();
        if !width.is_finite() || !height.is_finite() || width <= 0.0 || height <= 0.0 {
            return Err(PdfError::PageResourceError(
                "Page boxes have an empty or invalid intersection".into(),
            ));
        }
        let rotation = self.get_pagerotation()?;
        let matrix = match rotation {
            0 => Matrix::new(1., 0., 0., -1., -bbox.llx(), bbox.ury()),
            1 => Matrix::new(0., 1., 1., 0., -bbox.lly(), -bbox.llx()),
            2 => Matrix::new(-1., 0., 0., 1., bbox.urx(), -bbox.lly()),
            3 => Matrix::new(0., -1., -1., 0., bbox.ury(), bbox.urx()),
            _ => unreachable!("validated page rotation"),
        };
        let (width, height) = if rotation % 2 == 0 {
            (width, height)
        } else {
            (height, width)
        };
        let unit = self.page_user_unit()?;
        let scaled_width = width * unit;
        let scaled_height = height * unit;
        if !scaled_width.is_finite()
            || !scaled_height.is_finite()
            || scaled_width <= 0.
            || scaled_height <= 0.
        {
            return Err(PdfError::PageResourceError(
                "UserUnit produces invalid display dimensions".into(),
            ));
        }
        let matrix = matrix.checked_mul(&Matrix::new(unit, 0., 0., unit, 0., 0.))?;
        // Commit all cached geometry together only after every fallible check.
        self.bbox = bbox;
        self.page_matrix = matrix;
        self.page_width = scaled_width;
        self.page_height = scaled_height;
        Ok(())
    }

    fn page_user_unit(&self) -> PdfResult<f32> {
        let page = self.pagedict()?;
        let Some(value) = page.get("UserUnit") else {
            return Ok(1.);
        };
        let value = self.ctx.resolve_owned(value)?;
        if matches!(value, PdfObject::PdfNull) {
            return Ok(1.);
        }
        value
            .as_f32()
            .filter(|unit| unit.is_finite() && *unit > 0.)
            .ok_or_else(|| {
                PdfError::PageResourceError("UserUnit must be a finite positive number".into())
            })
    }

    fn get_pagerotation(&self) -> PdfResult<i32> {
        if let Some(rotate) = self.get_pageattr("Rotate")? {
            Ok(normalized_page_rotation(&rotate)? / 90)
        } else {
            Ok(0)
        }
    }

    fn get_pageattr(&self, name: &str) -> PdfResult<Option<PdfObject>> {
        crate::document::page_tree::PageTree::inherited_attribute(self.ctx, &self.page_node, name)
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
    pub(crate) fn resource_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(PdfDict, crate::budget::MemoryLease)> {
        let (object, lease) =
            crate::document::page_tree::PageTree::inherited_attribute_with_budget(
                self.ctx,
                &self.page_node,
                "Resources",
                budget,
            )?
            .ok_or_else(|| PdfError::PageResourceError("Page Resources is not found".into()))?;
        let dict = object.into_dict().ok_or(PdfError::PageResourcesIsNotDict)?;
        Ok((dict, lease))
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

    pub(crate) fn content_bytes(&self) -> PdfResult<Vec<u8>> {
        let mut result = Vec::new();
        let mut push = |data: Vec<u8>| -> PdfResult<()> {
            crate::filter::check_output_size(
                result.len(),
                data.len(),
                self.ctx.limits().max_decoded_bytes,
                "page contents",
            )?;
            result
                .try_reserve_exact(data.len())
                .map_err(|_| PdfError::ResourceLimit("page content allocation".into()))?;
            result.extend_from_slice(&data);
            Ok(())
        };
        if let Some(contents) = self.pagedict()?.get("Contents") {
            match self.ctx.resolve_owned(contents)? {
                PdfObject::PdfArray(contents_array) => {
                    crate::filter::check_output_size(
                        0,
                        contents_array.len(),
                        self.ctx.limits().max_objects,
                        "page content stream count",
                    )?;
                    for c in contents_array.into_iter() {
                        let stream_obj = self.ctx.resolve_owned(c)?;
                        let cobj = stream_obj
                            .as_stream()
                            .ok_or(PdfError::PageContentIsNotStream)?;
                        let data = cobj.decode_data(self.ctx)?;
                        push(data)?;
                    }
                }
                PdfObject::PdfStream(content_stream) => {
                    push(content_stream.decode_data(self.ctx)?)?;
                }
                PdfObject::PdfNull => {}
                _ => {
                    return Err(PdfError::PageContentIsNotStream);
                }
            }
        } else {
            return Ok(result);
        }
        Ok(result)
    }

    /// Query latest effective geometry without reading Contents or fonts.
    /// Inheritance and indirect box coordinates use the same validation as display.
    pub fn geometry(&mut self) -> PdfResult<PageGeometry> {
        self.update_dimensions()?;
        Ok(PageGeometry {
            visible_box: self.bbox.clone(),
            rotation: self.get_pagerotation()? * 90,
            user_unit: self.page_user_unit()?,
            display_width: self.page_width,
            display_height: self.page_height,
            user_to_display: self.page_matrix.clone(),
        })
    }

    pub fn display_list(&mut self) -> PdfResult<display_list::DisplayList> {
        self.update_dimensions()?;
        let all_content = self.content_bytes()?;
        let reader = StreamReader::try_new(Cursor::new(all_content))?;
        let syntax = SyntaxParser::with_options(reader, self.ctx.options());
        let content_parser = ContentParser::new(syntax);
        let mut interpreter = Interpreter::new(self, &self.ctx, content_parser);
        interpreter.run_to_display_list()
    }

    pub fn display(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        self.display_list()?.replay(device)
    }
}

#[cfg(test)]
mod geometry_tests {
    use super::*;
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::{collections::HashMap, io::Cursor};

    #[test]
    fn rotation_and_cropbox_geometry_are_validated() {
        let parser = PdfParser::new(SyntaxParser::new(
            StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let ctx = PDFContext::try_new(
            parser,
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let mut streams = crate::objects::pdf_array::PdfArray::default();
        for bytes in [b"0 0 1".as_slice(), b"0 10 re S".as_slice()] {
            streams.add_obj(PdfObject::PdfStream(
                crate::objects::pdf_stream::PdfStream::new(PdfDict::default(), bytes.to_vec()),
            ));
        }
        let mut dict = PdfDict::default();
        dict.insert("Contents".into(), PdfObject::PdfArray(streams));
        let page = PdfPage::new(PdfObject::PdfDict(dict), &ctx);
        assert_eq!(page.content_bytes().unwrap(), b"0 0 10 10 re S");
        for contents in [
            PdfObject::PdfNull,
            PdfObject::PdfStream(crate::objects::pdf_stream::PdfStream::new(
                PdfDict::default(),
                Vec::new(),
            )),
        ] {
            let mut dict = PdfDict::default();
            dict.insert("Contents".into(), contents);
            assert!(
                PdfPage::new(PdfObject::PdfDict(dict), &ctx)
                    .content_bytes()
                    .unwrap()
                    .is_empty()
            );
        }
        let mut dict = PdfDict::default();
        dict.insert(
            "Contents".into(),
            PdfObject::PdfStream(crate::objects::pdf_stream::PdfStream::new(
                PdfDict::default(),
                b"q Q".to_vec(),
            )),
        );
        assert_eq!(
            PdfPage::new(PdfObject::PdfDict(dict), &ctx)
                .content_bytes()
                .unwrap(),
            b"q Q"
        );
        let mut reused = PdfPage::new(parse("<< /CropBox [10 20 80 80] >>"), &ctx);
        reused.update_dimensions().unwrap();
        assert_eq!((reused.page_width, reused.page_height), (70.0, 60.0));
        reused.page_node = parse("<< /CropBox [0 0 200 100] >>");
        reused.update_dimensions().unwrap();
        assert_eq!((reused.page_width, reused.page_height), (200.0, 100.0));
        let old_matrix = reused.page_matrix.clone();
        reused.page_node = parse("<< /MediaBox [0 0 500 500] /Rotate 45 >>");
        assert!(reused.update_dimensions().is_err());
        assert_eq!((reused.page_width, reused.page_height), (200.0, 100.0));
        assert_eq!((reused.bbox.urx(), reused.bbox.ury()), (200.0, 100.0));
        assert_eq!(
            (reused.page_matrix.e(), reused.page_matrix.f()),
            (old_matrix.e(), old_matrix.f())
        );
        for invalid in [
            "<< /MediaBox [10 20 500 500] /Rotate 90 /UserUnit 0 >>",
            "<< /MediaBox [10 20 500 500] /Rotate 270 /UserUnit -1 >>",
            "<< /MediaBox [10 20 500 500] /UserUnit /Invalid >>",
            "<< /MediaBox [10 20 500 500] /UserUnit 100000000000000000000000000000000000000 >>",
            "<< /MediaBox [1000000000000000000000000000000 0 1000001000000000000000000000000 1] /UserUnit 1000000000 >>",
        ] {
            reused.page_node = parse(invalid);
            assert!(reused.update_dimensions().is_err(), "{invalid}");
            assert_eq!((reused.page_width, reused.page_height), (200., 100.));
            assert_eq!(
                (
                    reused.bbox.llx(),
                    reused.bbox.lly(),
                    reused.bbox.urx(),
                    reused.bbox.ury()
                ),
                (0., 0., 200., 100.)
            );
            assert_eq!(
                [
                    reused.page_matrix.a(),
                    reused.page_matrix.b(),
                    reused.page_matrix.c(),
                    reused.page_matrix.d(),
                    reused.page_matrix.e(),
                    reused.page_matrix.f()
                ],
                [
                    old_matrix.a(),
                    old_matrix.b(),
                    old_matrix.c(),
                    old_matrix.d(),
                    old_matrix.e(),
                    old_matrix.f()
                ]
            );
        }
        reused.page_node = parse("<< >>");
        reused.update_dimensions().unwrap();
        assert_eq!((reused.page_width, reused.page_height), (612.0, 792.0));
        for (rotation, expected) in [
            (0, [(0.0, 60.0), (70.0, 0.0)]),
            (90, [(0.0, 0.0), (60.0, 70.0)]),
            (180, [(70.0, 0.0), (0.0, 60.0)]),
            (270, [(60.0, 70.0), (0.0, 0.0)]),
        ] {
            let source =
                format!("<< /MediaBox [0 0 200 100] /CropBox [10 20 80 80] /Rotate {rotation} >>");
            let mut page = PdfPage::new(parse(&source), &ctx);
            page.update_dimensions().unwrap();
            for ((x, y), (expected_x, expected_y)) in
                [(10.0, 20.0), (80.0, 80.0)].into_iter().zip(expected)
            {
                let point = page
                    .page_matrix
                    .checked_transform(&crate::geom::point::Point::new(x, y))
                    .unwrap();
                assert_eq!(
                    (point.x(), point.y()),
                    (expected_x, expected_y),
                    "rotation {rotation}"
                );
            }
        }
        let mut page = PdfPage::new(
            parse("<< /MediaBox [0 0 200 100] /CropBox [10 20 80 90] /Rotate -90 >>"),
            &ctx,
        );
        page.update_dimensions().unwrap();
        assert_eq!((page.page_width, page.page_height), (70.0, 70.0));
        assert_eq!(page.get_pagerotation().unwrap(), 3);
        let mut page = PdfPage::new(
            parse("<< /MediaBox [0 0 200 100] /CropBox [10 20 80 80] /Rotate 450 >>"),
            &ctx,
        );
        page.update_dimensions().unwrap();
        assert_eq!((page.page_width, page.page_height), (60.0, 70.0));
        for source in [
            "<< /Rotate /bad >>",
            "<< /Rotate 90.5 >>",
            "<< /Rotate 45 >>",
            "<< /MediaBox [0 0 0 10] >>",
            "<< /MediaBox [0 0 10 10] /CropBox [20 20 30 30] >>",
        ] {
            assert!(
                PdfPage::new(parse(source), &ctx)
                    .update_dimensions()
                    .is_err(),
                "{source}"
            );
        }
    }
}
