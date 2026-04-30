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
        image_object::ImageObject,
        operator::Operator,
        path_object::PathObject,
        pdf_image::PdfImage,
        text_object::{CharItem, TextObject},
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
    current_state: AllState,
    current_path: PdfPath,
    current_path_point: Point,
    path_clip_type: FillType,
    path_start: Point,
}

impl<'a, R: Seek + Read> Interpreter<'a, R> {
    pub fn new(page: &'a PdfPage<'a, R>, ctx: &'a PDFContext<R>, parser: ContentParser) -> Self {
        Interpreter {
            page,
            ctx,
            parser,
            current_state: AllState::default(),
            state_stack: Vec::new(),
            current_path: PdfPath::default(),
            current_path_point: Point::default(),
            path_clip_type: FillType::default(),
            path_start: Point::default(),
        }
    }

    pub fn run(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        device.start_page(self.page.page_width, self.page.page_height);
        loop {
            if let Some(op) = self.parser.read_operator()? {
                self.invoke_oeration(op, device)?;
            } else {
                break;
            }
        }
        device.end_page();
        Ok(())
    }

    pub fn run_to_display_list(&mut self) -> PdfResult<DisplayList> {
        let mut device = TraceDevice::new();
        self.run(&mut device)?;
        Ok(device.into_display_list())
    }

    fn invoke_oeration(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
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
            "ri" => self.handle_set_render_intent()?,
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

            "BT" => self.handle_begin_text()?,
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
            "ET" => {}

            _ => {
                println!("not implemented op:{:?}", op);
            }
        }
        Ok(())
    }

    fn handle_show_text_array(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        if let Some(font) = &self.current_state.graphic_state.text_state.font {
            let font_size = self.current_state.graphic_state.text_state.font_size;
            let char_space = self.current_state.graphic_state.text_state.char_space;
            let word_space = self.current_state.graphic_state.text_state.word_space;
            let horz_scale = self.current_state.text_horz_scale;
            let mut textobj = TextObject::new(self.current_state.graphic_state.clone());

            let params = op.operand(0)?.as_array().unwrap();
            let wmd = font.writting_mode();
            let matrix = self.current_state.text_matrix.mul(&self.current_state.ctm);
            let matrix = matrix.mul(&self.page.page_matrix);

            textobj.matrix = matrix.clone();
            let ox = matrix.e();
            let oy = matrix.f();
            textobj.origin_x = ox;
            textobj.origin_y = oy;

            for p in params.into_iter() {
                match p {
                    PdfObject::PdfString(s) => {
                        let content = s.bytes();
                        let chars = font.chars(content)?;
                        for char in chars.iter() {
                            let unicode = font.unicode(char)?;
                            let matrix =
                                self.current_state.text_matrix.mul(&self.current_state.ctm);
                            let matrix = matrix.mul(&self.page.page_matrix);

                            let pos = Point::new(matrix.e(), matrix.f());
                            let charitem =
                                CharItem::new(pos, char.to_owned(), Some(unicode.clone()));
                            textobj.add_item(charitem);
                            let mut displacement = char.width() * font_size / 1000.0 + char_space;
                            if unicode == " " {
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
                                    let m = translation.mul(&self.current_state.text_matrix);
                                    self.current_state.text_matrix = m;
                                }
                                WritingMode::Vertical => {
                                    let translation =
                                        Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, displacement);
                                    let m = translation.mul(&self.current_state.text_matrix);
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
                                let rm = Matrix::new(1.0, 0.0, 0.0, 0.0, -tj, 0.0);
                                let text_matrix = rm.mul(&self.current_state.text_matrix);
                                self.current_state.text_matrix = text_matrix;
                            }
                            WritingMode::Vertical => {
                                let rm = Matrix::new(1.0, 0.0, 0.0, 0.0, 0.0, -tj);
                                let text_matrix = rm.mul(&self.current_state.text_matrix);
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
            device.show_text(textobj);
        }

        Ok(())
    }
    fn handle_show_text_nextline_space(
        &mut self,
        op: Operator,
        device: &mut dyn Device,
    ) -> PdfResult<()> {
        let aw = op.get_as_f32(0).unwrap();
        let ac = op.get_as_f32(1).unwrap();
        self.current_state.graphic_state.text_state.word_space = aw;
        self.current_state.graphic_state.text_state.char_space = ac;
        let content = op.operand(2)?.as_string().unwrap();
        self.do_show_text(content.bytes(), device)
    }

    fn handle_show_text_nextline(
        &mut self,
        op: Operator,
        device: &mut dyn Device,
    ) -> PdfResult<()> {
        self.handle_move_to_nextline()?;
        let content = op.operand(0)?.as_string().unwrap();
        self.do_show_text(content.bytes(), device)
    }
    fn do_show_text(&mut self, content: &[u8], device: &mut dyn Device) -> PdfResult<()> {
        let font_size = self.current_state.graphic_state.text_state.font_size;
        let char_space = self.current_state.graphic_state.text_state.char_space;
        let word_space = self.current_state.graphic_state.text_state.word_space;
        let horz_scale = self.current_state.text_horz_scale;
        let mut textobj = TextObject::new(self.current_state.graphic_state.clone());

        let matrix = self.current_state.text_matrix.mul(&self.current_state.ctm);
        let matrix = matrix.mul(&self.page.page_matrix);
        textobj.matrix = matrix.clone();
        let ox = matrix.e();
        let oy = matrix.f();
        textobj.origin_x = ox;
        textobj.origin_y = oy;
        if let Some(font) = &self.current_state.graphic_state.text_state.font {
            let chars = font.chars(content)?;
            for char in chars.iter() {
                let unicode = font.unicode(char)?;
                let matrix = self.current_state.text_matrix.mul(&self.current_state.ctm);
                let matrix = matrix.mul(&self.page.page_matrix);

                let pos = Point::new(matrix.e(), matrix.f());
                let charitem = CharItem::new(pos, char.to_owned(), Some(unicode.clone()));
                textobj.add_item(charitem);
                let mut displacement = char.width() * font_size / 1000.0 + char_space;
                if unicode == " " {
                    displacement += word_space;
                }
                match font.writting_mode() {
                    WritingMode::Horizontal => {
                        let translation =
                            Matrix::new(1.0, 0.0, 0.0, 1.0, displacement * horz_scale, 0.0);
                        let m = translation.mul(&self.current_state.text_matrix);
                        self.current_state.text_matrix = m;
                    }
                    WritingMode::Vertical => {
                        let translation = Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, displacement);
                        let m = translation.mul(&self.current_state.text_matrix);
                        self.current_state.text_matrix = m;
                    }
                }
            }
            device.show_text(textobj);
        } else {
            return Ok(());
        }
        Ok(())
    }

    fn handle_show_text(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        let content = op.operand(0)?.as_string().unwrap();
        self.do_show_text(content.bytes(), device)
    }

    fn handle_move_to_nextline(&mut self) -> PdfResult<()> {
        let tx = 0.0_f32;
        let ty = -self.current_state.text_leading;
        let mat = Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty);
        self.current_state.text_matrix = mat.mul(&self.current_state.text_line_matrix);
        self.current_state.text_line_matrix = self.current_state.text_matrix.clone();
        Ok(())
    }
    fn handle_set_text_matrix(&mut self, op: Operator) -> PdfResult<()> {
        let a = op.operand(0).unwrap().as_f32().unwrap();
        let b = op.operand(1).unwrap().as_f32().unwrap();
        let c = op.operand(2).unwrap().as_f32().unwrap();
        let d = op.operand(3).unwrap().as_f32().unwrap();
        let e = op.operand(4).unwrap().as_f32().unwrap();
        let f = op.operand(5).unwrap().as_f32().unwrap();
        let matrix = Matrix::new(a, b, c, d, e, f);
        self.current_state.text_matrix = matrix.clone();
        self.current_state.text_line_matrix = matrix;
        Ok(())
    }

    fn handle_move_textline(&mut self, op: Operator) -> PdfResult<()> {
        let tx = op.get_as_f32(0).unwrap();
        let ty = op.get_as_f32(1).unwrap();
        self.current_state.text_leading = -1.0 * ty;
        let mat = Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty);
        self.current_state.text_matrix = mat.mul(&self.current_state.text_line_matrix);
        self.current_state.text_line_matrix = self.current_state.text_matrix.clone();
        Ok(())
    }

    fn handle_move_textpoint(&mut self, op: Operator) -> PdfResult<()> {
        let tx = op.get_as_f32(0).unwrap();
        let ty = op.get_as_f32(1).unwrap();
        let mat = Matrix::new(1.0, 0.0, 0.0, 1.0, tx, ty);
        self.current_state.text_matrix = mat.mul(&self.current_state.text_line_matrix);
        self.current_state.text_line_matrix = self.current_state.text_matrix.clone();
        Ok(())
    }
    fn handle_set_text_rise(&mut self, op: Operator) -> PdfResult<()> {
        let rise = op.get_as_f32(0).unwrap();
        self.current_state.text_rise = rise;
        Ok(())
    }

    fn handle_set_text_rendering(&mut self, op: Operator) -> PdfResult<()> {
        let mode = op.operand(0).unwrap().as_number().unwrap().get_u8();
        self.current_state
            .graphic_state
            .text_state
            .set_text_rendering_mode(mode);

        Ok(())
    }

    fn handle_set_font(&mut self, op: Operator) -> PdfResult<()> {
        let font_size = op.get_as_f32(1).unwrap();
        self.current_state.graphic_state.text_state.font_size = font_size;
        let font_name = op.operand(0).unwrap().as_name().unwrap();
        let font_resource =
            self.page
                .get_resource_font(font_name.name())?
                .ok_or(PdfError::PageResourceError(format!(
                    "font {} not found",
                    font_name.name()
                )))?;
        let font = Font::try_new(font_resource.as_dict().unwrap(), self.ctx)?;
        self.current_state.graphic_state.text_state.font = Some(font);
        Ok(())
    }
    fn handle_set_text_leading(&mut self, op: Operator) -> PdfResult<()> {
        let leading = op.get_as_f32(0).unwrap();
        self.current_state.text_leading = leading;
        Ok(())
    }
    fn handle_set_text_horz_scale(&mut self, op: Operator) -> PdfResult<()> {
        let scale = op.get_as_f32(0).unwrap();
        self.current_state.text_horz_scale = scale;
        // TODO onchange TextMatrix
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
        //match self.current_state.graphic_state.color_state.stroke_color_space
        // SCN TODO
        Ok(())
    }
    fn handle_set_colorvalue_ps_fill(&mut self, op: Operator) -> PdfResult<()> {
        match self
            .current_state
            .graphic_state
            .color_state
            .fill_color_space
        {
            ColorSpace::Pattern(_) => {
                let name = op.operand(0)?.as_name().unwrap().name();
                let pattern = self.page.get_resource_pattern(name)?;
                unimplemented!("pattern colorspace");
            }
            _ => {}
        }
        // scn TODO
        Ok(())
    }

    fn handle_set_colorvalue_fill(&mut self, op: Operator) -> PdfResult<()> {
        let n = op.num_operands();
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
                    "DeviceRgb" | "DeviceGray" | "DeviceCmyk" | "RGB" | "CMYK" | "G"
                ) {
                    let colorspace = ColorSpace::try_new(obj, self.ctx)?;
                    self.current_state
                        .graphic_state
                        .color_state
                        .stroke_color_space = colorspace;
                } else {
                    let colorobj = self.page.get_resource_color(name.name())?;
                    println!("colorobj:{:?}", colorobj);
                    let colorspace = ColorSpace::try_new(&colorobj, self.ctx)?;
                    return Ok(colorspace);
                }
            }
            _ => {}
        }
        Ok(ColorSpace::DeviceGray(DeviceGray::default()))
    }

    pub fn handle_set_colorspace_fill(&mut self, op: Operator) -> PdfResult<()> {
        let obj = op.operand(0)?;
        //let colorspace = ColorSpace::try_new(obj, self.ctx)?;
        let colorspace = self.find_colorspace(obj)?;
        println!("colorspace:{:?},{:?}", obj, colorspace);
        self.current_state
            .graphic_state
            .color_state
            .fill_color_space = colorspace;
        Ok(())
    }
    pub fn handle_set_colorspace_stroke(&mut self, op: Operator) -> PdfResult<()> {
        let obj = op.operand(0)?;
        let colorspace = self.find_colorspace(obj)?;
        self.current_state
            .graphic_state
            .color_state
            .stroke_color_space = colorspace;
        Ok(())
    }
    fn handle_set_extend_gs(&mut self, op: Operator) -> PdfResult<()> {
        let name = op.operand(0)?.as_name().unwrap().name();
        let extgs_obj = self.page.get_resource_extgstate(name)?;
        let extgs = extgs_obj.as_dict().ok_or(
            PdfError::PageResourceError(format!("Page ExtGs state is not a dict")),
        )?;
        self.current_state.process_ext_gs(extgs)?;
        Ok(())
    }

    fn handle_set_flat(&mut self, op: Operator) -> PdfResult<()> {
        let flat = op.get_as_f32(0)?;
        self.current_state.graphic_state.general_state.flat_ness = flat;
        Ok(())
    }

    fn handle_set_render_intent(&mut self) -> PdfResult<()> {
        unimplemented!()
    }

    fn handle_set_dash(&mut self, op: Operator) -> PdfResult<()> {
        let dash_array = op.operand(0)?.as_array().unwrap();
        let dash_vec = dash_array
            .into_iter()
            .map(|v| v.as_f32().unwrap())
            .collect();
        let dash_phrase = op.operand(1)?.as_f32().unwrap();
        self.current_state.graphic_state.graph_state.dash_phrase = dash_phrase;
        self.current_state.graphic_state.graph_state.dash_array = dash_vec;
        Ok(())
    }

    fn handle_miter_limit(&mut self, op: Operator) -> PdfResult<()> {
        let limit = op.operand(0)?.as_f32().unwrap_or(10.0);
        self.current_state.graphic_state.graph_state.miter_limit = limit;
        Ok(())
    }

    fn handle_linejoin(&mut self, op: Operator) -> PdfResult<()> {
        let join = op.operand(0)?.as_u8().unwrap_or(0);
        self.current_state.graphic_state.graph_state.line_join = LineJoin::new_from_value(join);
        Ok(())
    }

    fn handle_linecap(&mut self, op: Operator) -> PdfResult<()> {
        let cap = op.operand(0)?.as_u8().unwrap_or(0);
        self.current_state.graphic_state.graph_state.line_cap = LineCap::new_from_value(cap);
        Ok(())
    }

    fn handle_linewidth(&mut self, op: Operator) -> PdfResult<()> {
        let width = op.get_as_f32(0)?;
        self.current_state.graphic_state.graph_state.line_width = width;
        Ok(())
    }

    fn handle_fill_and_stroke_path(
        &mut self,
        device: &mut dyn Device,
        filletype: FillType,
    ) -> PdfResult<()> {
        let ctm = self.current_state.ctm();
        let matrix = ctm.mul(&self.page.page_matrix);
        let state = self.current_state.graphic_state.clone();
        let pathobj = PathObject::new(self.current_path.clone(), matrix, state, filletype, true);
        device.fill_and_stroke_path(pathobj);
        self.current_path = PdfPath::default();
        Ok(())
    }

    fn handle_fill_path(&mut self, device: &mut dyn Device, filletype: FillType) -> PdfResult<()> {
        if self.current_path.is_empty() {
            return Ok(());
        }
        let ctm = self.current_state.ctm();
        let matrix = ctm.mul(&self.page.page_matrix);
        if self.path_clip_type != FillType::NoFill {
            self.current_state.graphic_state.clip_path.add_path(
                self.current_path.clone(),
                self.path_clip_type.clone(),
                matrix.clone(),
            );
            self.path_clip_type = FillType::NoFill;
        }

        let ctm = self.current_state.ctm();
        let matrix = ctm.mul(&self.page.page_matrix);
        let state = self.current_state.graphic_state.clone();
        let pathobj = PathObject::new(self.current_path.clone(), matrix, state, filletype, false);
        device.fill_path(pathobj);
        self.current_path = PdfPath::default();
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
            return Ok(());
        }

        let ctm = self.current_state.ctm();
        let matrix = ctm.mul(&self.page.page_matrix);

        if self.path_clip_type != FillType::NoFill {
            self.current_state.graphic_state.clip_path.add_path(
                self.current_path.clone(),
                self.path_clip_type.clone(),
                matrix.clone(),
            );
            self.path_clip_type = FillType::NoFill;
        }

        let ctm = self.current_state.ctm();
        let matrix = ctm.mul(&self.page.page_matrix);

        let state = self.current_state.graphic_state.clone();
        let path_obj = PathObject::new(
            self.current_path.clone(),
            matrix,
            state,
            FillType::NoFill,
            true,
        );
        device.stroke_path(path_obj);
        self.current_path = PdfPath::default();
        Ok(())
    }

    // re
    fn handle_rect(&mut self, op: Operator) -> PdfResult<()> {
        let x = op.get_as_f32(0)?;
        let y = op.get_as_f32(1)?;
        let width = op.get_as_f32(2)?;
        let height = op.get_as_f32(3)?;
        let lower_left = Point::new(x, y);
        let lower_right = Point::new(x + width, y);
        let upper_right = Point::new(x + width, y + height);
        let upper_left = Point::new(x, y + height);
        let mut subpath = SubPath::new(vec![Segment::MoveTo(lower_left.clone())], false);
        self.current_path_point = lower_left.clone();
        subpath.add_segment(Segment::LineTo(lower_right));
        subpath.add_segment(Segment::LineTo(upper_right));
        subpath.add_segment(Segment::LineTo(upper_left));
        subpath.add_segment(Segment::LineTo(lower_left));
        self.current_path.add_subpath(subpath);
        Ok(())
    }

    // W
    fn handle_clip(&mut self) -> PdfResult<()> {
        self.path_clip_type = FillType::Winding;
        Ok(())
    }
    // W*
    fn handle_eoclip(&mut self) -> PdfResult<()> {
        self.path_clip_type = FillType::EvenOdd;
        Ok(())
    }

    fn handle_end_path(&mut self, device: &mut dyn Device) -> PdfResult<()> {
        let path_clip_type = self.path_clip_type.clone();
        self.path_clip_type = FillType::NoFill;
        if self.current_path.is_empty() || path_clip_type == FillType::NoFill {
            return Ok(());
        }
        let ctm = self.current_state.ctm();
        let matrix = ctm.mul(&self.page.page_matrix);
        self.current_state
            .mut_graphic_state()
            .mut_clippath()
            .add_path(self.current_path.clone(), path_clip_type, matrix);
        self.current_path = PdfPath::default();
        self.current_path_point = Point::default();
        self.path_start = Point::default();

        Ok(())
    }

    fn handle_close_path(&mut self) -> PdfResult<()> {
        if self.current_path.is_empty() {
            return Ok(());
        }
        let last = self.current_path.last_mut_subpath().unwrap();
        last.add_segment(Segment::LineTo(self.path_start.clone()));
        last.close();
        self.path_start = Point::default();
        self.current_path_point = Point::default();
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
        let bezier = Bezier::new(self.current_path_point.clone(), p1, p3.clone());
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
        let x = op.operand(0)?.as_f32().unwrap();
        let y = op.operand(1)?.as_f32().unwrap();
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
        let x = op.operand(0)?.as_f32().unwrap();
        let y = op.operand(1)?.as_f32().unwrap();
        let point = Point::new(x, y);
        self.path_start = point.clone();
        self.current_path_point = point.clone();
        let seg = Segment::MoveTo(point);
        if self.current_path.is_empty() {
            let subpath = SubPath::new(vec![seg], false);
            self.current_path.add_subpath(subpath);
        } else {
            let last = self.current_path.last_mut_subpath().unwrap();
            if last.is_closed() {
                let subpath = SubPath::new(vec![seg], false);
                self.current_path.add_subpath(subpath);
            } else {
                last.add_segment(seg);
            }
        }

        //
        Ok(())
    }

    // q
    fn save_sate(&mut self) -> PdfResult<()> {
        self.state_stack.push(self.current_state.clone());
        //self.current_state = AllState::default();
        Ok(())
    }
    // Q
    fn restore_state(&mut self) -> PdfResult<()> {
        let state = self.state_stack.pop().unwrap();
        self.current_state = state;
        Ok(())
    }

    fn concat_matrix(&mut self, op: Operator) -> PdfResult<()> {
        let a = op.operand(0).unwrap().as_f32().unwrap();
        let b = op.operand(1).unwrap().as_f32().unwrap();
        let c = op.operand(2).unwrap().as_f32().unwrap();
        let d = op.operand(3).unwrap().as_f32().unwrap();
        let e = op.operand(4).unwrap().as_f32().unwrap();
        let f = op.operand(5).unwrap().as_f32().unwrap();
        let matrix = Matrix::new(a, b, c, d, e, f);
        let ctm = matrix.mul(&self.current_state.ctm);
        self.current_state.ctm = ctm;
        Ok(())
    }

    fn handle_do(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        let name = op.operand(0)?.as_name().unwrap().name();
        let xobject = self.page.get_xobject(name)?;
        let subtype = xobject
            .get_attr("Subtype")
            .unwrap()
            .as_name()
            .unwrap()
            .name();

        match subtype {
            "Image" => {
                let pdfimage = PdfImage::try_new(name.to_string(), &xobject, self.ctx)?;
                let matrix = self.current_state.ctm();
                let image_to_user = Matrix::new(1.0, 0.0, 0.0, -1.0, 0.0, 1.0);
                let matrix = image_to_user.mul(&matrix);
                let image_matrix = matrix.mul(&self.page.page_matrix);
                let imageobject = ImageObject::new(
                    image_matrix,
                    pdfimage,
                    self.current_state.graphic_state.clone(),
                );
                device.do_image(imageobject);
            }
            "Form" => {
                return Err(PdfError::PageXobjectNotFound);
            }
            _ => {
                return Err(PdfError::PageResourceError(format!(
                    "xobject subtype not supported: {subtype}"
                )));
            }
        }
        Ok(())
    }
}
