use std::io::{Read, Seek};
mod marked_text;
mod pattern;
mod soft_mask;
mod text_clip;

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
    marked_content_depth: usize,
    language: Option<std::rc::Rc<str>>,
    language_initialized: bool,
    language_stack: Vec<Option<std::rc::Rc<str>>>,
    optional_content: Option<crate::document::optional_content::OptionalContentVisibility>,
    optional_visible: bool,
    optional_stack: Vec<bool>,
    semantic_capture: Option<std::rc::Rc<std::cell::RefCell<super::marked_text::Capture>>>,
    semantic_mark_context: Vec<(bool, usize)>,
    text_chars: usize,
    path_segments: usize,
    operation_index: usize,
    operation_byte_offset: Option<u64>,
    path_source_indices: Vec<usize>,
    resource_scope: Option<crate::objects::pdf_dict::PdfDict>,
    initial_ctm: Matrix,
    pattern_depth: usize,
    pattern_uncoloured: bool,
    pending_text_clip: Option<PdfPath>,
    execution_budget: super::execution_budget::ExecutionBudget,
    form_depth: usize,
    active_forms: Vec<crate::objects::object_id::ObjectId>,
    invocation_path: Vec<super::source_provenance::FormInvocation>,
    type3_depth: usize,
    type3_uncoloured: bool,
    type3_budget: crate::budget::OperationBudget,
    // Cache is local to a resource invocation and invalidated after edits.
    font_cache_revision: u64,
    image_cache: std::collections::HashMap<String, std::rc::Rc<PdfImage>>,
    image_cache_bytes: usize,
    font_cache: std::collections::HashMap<String, std::rc::Rc<Font>>,
}

impl<'a, R: Seek + Read> Interpreter<'a, R> {
    pub fn new(page: &'a PdfPage<'a, R>, ctx: &'a PDFContext<R>, parser: ContentParser) -> Self {
        Interpreter {
            font_cache_revision: ctx.revision(),
            image_cache: Default::default(),
            image_cache_bytes: 0,
            font_cache: std::collections::HashMap::new(),
            invocation_path: Vec::new(),
            type3_depth: 0,
            type3_uncoloured: false,
            type3_budget: ctx.limits().operation_budget(),
            form_depth: 0,
            active_forms: Vec::new(),
            execution_budget: super::execution_budget::ExecutionBudget::new(),
            resource_scope: None,
            initial_ctm: Matrix::identity(),
            pattern_depth: 0,
            pattern_uncoloured: false,
            pending_text_clip: None,
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
            marked_content_depth: 0,
            language: None,
            language_initialized: false,
            language_stack: Vec::new(),
            optional_content: None,
            optional_visible: true,
            optional_stack: Vec::new(),
            semantic_capture: None,
            semantic_mark_context: Vec::new(),
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
        self.initialize_language()?;
        let mut operations = 0usize;
        loop {
            use super::preparation_profile::PreparationPhase;
            let context = self.ctx;
            let op = {
                let _span = context.preparation_span(PreparationPhase::OperatorParsing);
                self.parser.read_operator()?
            };
            let Some(op) = op else {
                break;
            };
            let phase = match op.name() {
                "Tf" => PreparationPhase::Fonts,
                "Tj" | "TJ" | "'" | "\"" => PreparationPhase::Text,
                "Do" | "BI" => PreparationPhase::XObjects,
                _ => PreparationPhase::OtherOperators,
            };
            let _span = context.preparation_span(phase);
            self.execution_budget.operation(&self.ctx.limits())?;
            self.operation_index = operations;
            self.operation_byte_offset = op.byte_offset();
            operations += 1;
            let visible = self.optional_visible;
            self.invoke_oeration(
                op,
                &mut crate::device::visibility::VisibilityDevice { device, visible },
            )?;
        }
        if self.in_text_object {
            return Err(PdfError::PageOperatorError(
                "content ends before matching ET".into(),
            ));
        }
        if self.marked_content_depth != 0 {
            return Err(PdfError::PageOperatorError(
                "content ends before matching EMC".into(),
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
        self.font_cache.clear();
        self.image_cache.clear();
        self.image_cache_bytes = 0;
        self.resource_scope = Some(scope);
        self
    }

    pub(crate) fn with_execution_budget(
        mut self,
        budget: super::execution_budget::ExecutionBudget,
    ) -> Self {
        self.execution_budget = budget;
        self
    }

    pub fn with_optional_content(
        mut self,
        visibility: &crate::document::optional_content::OptionalContentVisibility,
    ) -> PdfResult<Self> {
        visibility.validate(self.ctx)?;
        self.optional_content = Some(visibility.clone());
        Ok(self)
    }

    fn begin_optional_mark(&mut self, op: &Operator) -> PdfResult<()> {
        if self.optional_content.is_none() {
            return Ok(());
        }
        if op.num_operands() != if op.name() == "BDC" { 2 } else { 1 } {
            return Err(PdfError::PageOperatorError(
                "optional content BMC/BDC operands".into(),
            ));
        }
        if self.optional_stack.len() >= self.ctx.limits().max_depth {
            return Err(PdfError::ResourceLimit(
                "optional content marked nesting".into(),
            ));
        }
        let mut visible = true;
        if op.name() == "BDC" && op.get_as_name(0)?.name() == "OC" {
            let name = op.get_as_name(1)?.name();
            let value = self.resource_entry("Properties", name)?;
            visible = self
                .optional_content
                .as_ref()
                .unwrap()
                .visible(self.ctx, &value)?;
        }
        self.optional_stack.push(self.optional_visible);
        self.optional_visible &= visible;
        Ok(())
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
        #[cfg(feature = "prepare-diagnostics")]
        self.run_unfiltered(&mut super::preparation_profile::Collector {
            context: self.ctx,
            device: &mut device,
        })?;
        #[cfg(not(feature = "prepare-diagnostics"))]
        self.run_unfiltered(&mut device)?;
        let mut list = device.into_display_list();
        if let Some(visibility) = &self.optional_content {
            list.set_optional_content_selection(visibility.selection().clone());
        }
        list.set_provenance(self.page.source_provenance());
        list.set_diagnostics(self.diagnostics.clone());
        Ok(list)
    }

    pub fn diagnostics(&self) -> &[crate::page::diagnostic::ContentDiagnostic] {
        &self.diagnostics
    }

    fn invoke_oeration(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        if self.pattern_uncoloured
            && matches!(
                op.name(),
                "rg" | "RG"
                    | "g"
                    | "G"
                    | "k"
                    | "K"
                    | "cs"
                    | "CS"
                    | "sc"
                    | "SC"
                    | "scn"
                    | "SCN"
                    | "sh"
            )
        {
            if self.ctx.options().mode == crate::parser::syntax::ParseMode::Strict
                || op.name() == "sh"
            {
                return Err(PdfError::ColorError(
                    "uncoloured pattern cannot specify colour".into(),
                ));
            }
            return self.unsupported_operator_reason(
                &op,
                Some("ignored colour change in uncoloured pattern; caller tint retained"),
            );
        }
        if self.type3_depth != 0 {
            if !matches!(
                op.name(),
                "q" | "Q"
                    | "cm"
                    | "w"
                    | "J"
                    | "j"
                    | "M"
                    | "d"
                    | "i"
                    | "m"
                    | "l"
                    | "c"
                    | "v"
                    | "y"
                    | "h"
                    | "re"
                    | "S"
                    | "s"
                    | "f"
                    | "F"
                    | "f*"
                    | "B"
                    | "B*"
                    | "b"
                    | "b*"
                    | "n"
                    | "W"
                    | "W*"
                    | "Do"
                    | "BI"
                    | "rg"
                    | "RG"
                    | "g"
                    | "G"
                    | "k"
                    | "K"
                    | "cs"
                    | "CS"
                    | "sc"
                    | "SC"
                    | "scn"
                    | "SCN"
            ) {
                return Err(PdfError::UnsupportedFeature(format!(
                    "Type3 glyph operator {} outside graphics baseline",
                    op.name()
                )));
            }
            if self.type3_uncoloured
                && matches!(
                    op.name(),
                    "rg" | "RG" | "g" | "G" | "k" | "K" | "cs" | "CS" | "sc" | "SC" | "scn" | "SCN"
                )
            {
                return Err(PdfError::FontError("d1 glyph cannot change colour".into()));
            }
        }
        if let Some(expected) = crate::page::opcode::operand_count(op.name())
            && op.num_operands() != expected
        {
            return Err(PdfError::PageOperatorError(format!(
                "{} requires {expected} operands, got {}",
                op.name(),
                op.num_operands()
            )));
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
            "Q" => {
                if self
                    .semantic_mark_context
                    .last()
                    .is_some_and(|(_, depth)| *depth >= self.state_stack.len())
                {
                    return Err(PdfError::PageOperatorError(
                        "Q crosses a marked-content boundary".into(),
                    ));
                }
                self.restore_state()?;
            }
            "cm" => self.concat_matrix(op)?,
            "w" => self.handle_linewidth(op)?,
            "J" => self.handle_linecap(op)?,
            "j" => self.handle_linejoin(op)?,
            "M" => self.handle_miter_limit(op)?,

            "Do" => self.handle_do(op, device)?,
            "BI" => self.handle_inline_image(op, device)?,
            "sh" => self.handle_shading(op, device)?,
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
            "ri" => {
                let name = op.get_as_name(0)?.name();
                if let Some(intent) = crate::color::conversion::RenderingIntent::from_pdf_name(name)
                {
                    self.current_state
                        .graphic_state
                        .general_state
                        .rendering_intent = intent;
                } else {
                    self.unsupported_operator(&op)?;
                }
            }
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
                if self
                    .semantic_mark_context
                    .last()
                    .is_some_and(|(inside, _)| *inside)
                {
                    return Err(PdfError::PageOperatorError(
                        "ET crosses a marked-content boundary".into(),
                    ));
                }
                if !self.in_text_object {
                    return Err(PdfError::PageOperatorError("ET without matching BT".into()));
                }
                self.finish_text_clip()?;
                self.in_text_object = false;
            }

            // Metadata does not change glyph painting. Logical replacements
            // are applied only by explicit marked-text capture.
            "MP" | "DP" => {
                let expected = if op.name() == "DP" { 2 } else { 1 };
                if op.num_operands() != expected {
                    return Err(PdfError::PageOperatorError(
                        "marked-content point operands".into(),
                    ));
                }
                op.get_as_name(0)?;
                if op.name() == "DP" {
                    self.validate_selected_properties(&op)?;
                }
                // A point has no content extent: never push a BMC/BDC frame or
                // change optional visibility / replace subsequent glyph text.
            }
            "BMC" => {
                self.push_language_mark(&op)?;
                self.begin_optional_mark(&op)?;
                if self.semantic_capture.is_some() {
                    self.capture_mark(&op)?;
                }
                op.get_as_name(0)?;
                self.marked_content_depth = self
                    .marked_content_depth
                    .checked_add(1)
                    .filter(|depth| *depth <= self.ctx.limits().max_content_operations)
                    .ok_or_else(|| PdfError::ResourceLimit("marked content depth".into()))?;
            }
            "BDC" => {
                op.get_as_name(0)?;
                self.begin_optional_mark(&op)?;
                let mcid_only = op.operand(1)?.as_dict().is_some_and(|dict| {
                    dict.into_iter().count() == 1
                        && dict.get("MCID").and_then(PdfObject::as_u32).is_some()
                });
                if self.semantic_capture.is_some() {
                    self.capture_mark(&op)?;
                } else if !mcid_only {
                    if self.optional_content.is_some() || op.get_as_name(0)?.name() != "OC" {
                        self.validate_selected_properties(&op)?;
                    } else {
                        self.unsupported_operator(&op)?;
                    }
                }
                self.push_language_mark(&op)?;
                self.marked_content_depth = self
                    .marked_content_depth
                    .checked_add(1)
                    .filter(|depth| *depth <= self.ctx.limits().max_content_operations)
                    .ok_or_else(|| PdfError::ResourceLimit("marked content depth".into()))?;
            }
            "EMC" => {
                if (self.semantic_capture.is_some() || self.optional_content.is_some())
                    && op.num_operands() != 0
                {
                    return Err(PdfError::PageOperatorError("EMC operands".into()));
                }
                self.marked_content_depth =
                    self.marked_content_depth.checked_sub(1).ok_or_else(|| {
                        PdfError::PageOperatorError("EMC without matching BMC".into())
                    })?;
                self.language = self
                    .language_stack
                    .pop()
                    .ok_or_else(|| PdfError::PageOperatorError("language unmatched EMC".into()))?;
                if self.semantic_capture.is_some() {
                    self.capture_mark(&op)?;
                }
                if self.optional_content.is_some() {
                    self.optional_visible = self.optional_stack.pop().ok_or_else(|| {
                        PdfError::PageOperatorError("optional content unmatched EMC".into())
                    })?;
                }
            }

            _ => {
                self.unsupported_operator(&op)?;
            }
        }
        Ok(())
    }

    fn unsupported_operator(&mut self, op: &Operator) -> PdfResult<()> {
        self.unsupported_operator_reason(op, None)
    }

    fn unsupported_operator_reason(
        &mut self,
        op: &Operator,
        reason: Option<&str>,
    ) -> PdfResult<()> {
        use crate::page::diagnostic::{ContentDiagnostic, DiagnosticKind};
        crate::filter::check_output_size(
            self.diagnostics.len(),
            1,
            self.ctx.limits().max_content_diagnostics,
            "page content diagnostics",
        )?;
        let prefix = "unsupported content operator ";
        let suffix = match reason {
            Some(reason) => format!(" at byte {:?}: {reason}", op.byte_offset()),
            None => format!(" at byte {:?}", op.byte_offset()),
        };
        let message_bytes = op
            .name()
            .len()
            .checked_add(prefix.len())
            .and_then(|n| n.checked_add(suffix.len()))
            .ok_or_else(|| PdfError::ResourceLimit("diagnostic size overflow".into()))?;
        let source = self.nested_draw_source()?;
        let source_bytes = source
            .as_ref()
            .map(|s| s.retained_bytes())
            .transpose()?
            .unwrap_or(0);
        let retained_bytes = message_bytes
            .checked_add(op.name().len())
            .and_then(|n| n.checked_add(source_bytes))
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
            source,
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
            textobj.set_language(self.language.clone());
            textobj.horizontal_scale = horz_scale;

            let wmd = font.writting_mode();
            let matrix = Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, self.current_state.text_rise)
                .checked_mul(&self.current_state.text_matrix)?
                .checked_mul(&self.current_state.ctm)?;
            let matrix = matrix.checked_mul(&self.page.page_matrix)?;

            textobj.matrix = matrix.clone();
            textobj.paint_matrix = self.current_state.ctm.checked_mul(&self.page.page_matrix)?;
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
                            let advance = font.text_advance(
                                char, font_size, char_space, word_space, horz_scale,
                            )?;
                            self.current_state.text_matrix =
                                Matrix::new(1., 0., 0., 1., advance[0], advance[1])
                                    .checked_mul(&self.current_state.text_matrix)?;
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
            let textobj = if self.form_depth == 0 && self.type3_depth == 0 {
                textobj.with_source(self.text_draw_source())
            } else {
                textobj.with_nested_source(self.nested_draw_source()?)
            };
            self.emit_text_object(textobj, device)?;
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
        textobj.set_language(self.language.clone());
        textobj.horizontal_scale = horz_scale;

        let matrix = Matrix::new(1.0, 0.0, 0.0, 1.0, 0.0, self.current_state.text_rise)
            .checked_mul(&self.current_state.text_matrix)?
            .checked_mul(&self.current_state.ctm)?;
        let matrix = matrix.checked_mul(&self.page.page_matrix)?;
        textobj.matrix = matrix.clone();
        textobj.paint_matrix = self.current_state.ctm.checked_mul(&self.page.page_matrix)?;
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
                let advance =
                    font.text_advance(char, font_size, char_space, word_space, horz_scale)?;
                self.current_state.text_matrix =
                    Matrix::new(1., 0., 0., 1., advance[0], advance[1])
                        .checked_mul(&self.current_state.text_matrix)?;
            }
            let textobj = if self.form_depth == 0 && self.type3_depth == 0 {
                textobj.with_source(self.text_draw_source())
            } else {
                textobj.with_nested_source(self.nested_draw_source()?)
            };
            self.emit_text_object(textobj, device)?;
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

    fn emit_text_object(&mut self, mut text: TextObject, device: &mut dyn Device) -> PdfResult<()> {
        if let Some(capture) = self.semantic_capture.clone() {
            capture
                .borrow_mut()
                .text(&text, self.semantic_location()?, self.optional_visible)?;
        }
        if let Some(Font::Type3(font)) = text.state.text_state.font.as_deref() {
            if text.state.text_state.text_rendering_mode
                != super::text_state::TextRenderingMode::Fill
            {
                return Err(PdfError::UnsupportedFeature(
                    "Type3 non-default text rendering mode outside baseline".into(),
                ));
            }
            let general = &text.state.general_state;
            if general.fill_alpha != general.stroke_alpha
                || general.soft_mask.is_some()
                || general.blend_mode != super::general_state::BlendMode::Normal
            {
                return Err(PdfError::UnsupportedFeature(
                    "Type3 unequal alpha, blend mode or soft mask outside baseline".into(),
                ));
            }
            let limits = self.ctx.limits();
            for item in &text.items {
                let code = u8::try_from(item.charcode().code())
                    .map_err(|_| PdfError::FontError("Type3 code range".into()))?;
                let work = self.execution_budget.child(&limits)?;
                let depth = self.type3_depth + self.form_depth + 1;
                if depth > 128 {
                    return Err(PdfError::ResourceLimit(
                        "Type3 implementation depth (128)".into(),
                    ));
                }
                work.operation(&limits)?; // d0/d1, checked by the model below.
                let program = font.glyph_program_with_decode_check(code, self.ctx, |bytes| {
                    work.decoded(bytes, &limits)
                })?;
                if program.metrics().width[0] != font.width(code)? {
                    if matches!(
                        self.ctx.read_provenance(),
                        crate::parser::diagnostic::ReadProvenance::Parsed(
                            crate::parser::syntax::ParseMode::Compatible
                        )
                    ) {
                        // Advance remains governed by Widths, never the damaged d0/d1 metric.
                        self.ctx.record_structure_recovery(
                            crate::parser::diagnostic::StructureReason::FontWidths,
                        )?;
                    } else {
                        return Err(PdfError::FontError(
                            "Type3 CharProc width disagrees with Widths".into(),
                        ));
                    }
                }
                let mut child = Interpreter::new(
                    self.page,
                    self.ctx,
                    ContentParser::new(crate::parser::syntax::SyntaxParser::with_options(
                        crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                            Vec::new(),
                        ))?,
                        self.ctx.options(),
                    )),
                );
                child.language = self.language.clone();
                child.language_initialized = true;
                child.type3_depth = self.type3_depth + 1;
                child.optional_content = self.optional_content.clone();
                child.optional_visible = self.optional_visible;
                child.form_depth = self.form_depth;
                child.type3_uncoloured = program.metrics().bbox.is_some();
                child.pattern_depth = self.pattern_depth;
                child.pattern_uncoloured = self.pattern_uncoloured;
                child.type3_budget = self.type3_budget.clone();
                child.execution_budget = work;
                // Type3 Resources omitted: use the page resource dictionary
                // (PDF 1.7 Table 5.9). An explicit empty scope stays empty.
                child.resource_scope = Some(match font.resources() {
                    Some(scope) => scope.clone(),
                    None => self.page.resource()?,
                });
                child.active_forms = self.active_forms.clone();
                child.current_state.graphic_state = text.state.clone();
                child.current_state.graphic_state.text_state.font = None;
                let matrix = font.font_matrix();
                let glyph = Matrix::new(
                    matrix[0], matrix[1], matrix[2], matrix[3], matrix[4], matrix[5],
                );
                let size = text.state.text_state.font_size;
                let baseline = Matrix::new(
                    text.matrix.a(),
                    text.matrix.b(),
                    text.matrix.c(),
                    text.matrix.d(),
                    item.pos().x(),
                    item.pos().y(),
                );
                child.current_state.ctm = glyph
                    .checked_mul(&Matrix::new(
                        size * text.horizontal_scale,
                        0.,
                        0.,
                        size,
                        0.,
                        0.,
                    ))?
                    .checked_mul(&baseline)?
                    .checked_mul(&self.page.page_matrix.checked_inverse()?)?;
                let grouped = general.fill_alpha != 1.;
                if grouped {
                    device.begin_group(super::transparency::GroupPaint {
                        blending_profile: None,
                        rendering_intent: self
                            .current_state
                            .graphic_state
                            .general_state
                            .rendering_intent,
                        isolated: true,
                        knockout: false,
                        alpha: general.fill_alpha,
                        blend_mode: super::general_state::BlendMode::Normal,
                        clip: text.state.clip_path.clone(),
                        soft_mask: None,
                    })?;
                    child.current_state.graphic_state.general_state.fill_alpha = 1.;
                    child.current_state.graphic_state.general_state.stroke_alpha = 1.;
                }
                let result = (|| -> PdfResult<()> {
                    for (index, operation) in
                        program.program().operations().iter().enumerate().skip(1)
                    {
                        child.execution_budget.operation(&limits)?;
                        child.operation_index = index;
                        child.operation_byte_offset = operation.operator.byte_offset();
                        let visible = child.optional_visible;
                        child.invoke_oeration(
                            operation.operator.clone(),
                            &mut crate::device::visibility::VisibilityDevice { device, visible },
                        )?;
                    }
                    // Empty parser: applies the normal balanced q/Q/BT/marked-content checks.
                    child.run_content(device)?;
                    if !child.diagnostics.is_empty() {
                        return Err(PdfError::UnsupportedFeature(
                            "Type3 glyph diagnostics".into(),
                        ));
                    }
                    Ok(())
                })();
                let cleanup = if grouped {
                    device.end_group(result.is_ok())
                } else {
                    Ok(())
                };
                result?;
                cleanup?;
            }
            return device.show_text(text.with_type3_metadata());
        }
        self.accumulate_text_clip(&mut text)?;
        device.show_text(text)
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
        self.current_state.text_leading = -ty;
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
        if self.font_cache_revision != self.ctx.revision() {
            self.font_cache.clear();
            self.image_cache.clear();
            self.image_cache_bytes = 0;
            self.font_cache_revision = self.ctx.revision();
        }
        let font = if let Some(font) = self.font_cache.get(font_name.name()) {
            font.clone()
        } else {
            if self.font_cache.len() >= self.ctx.limits().max_objects {
                return Err(PdfError::ResourceLimit("font cache entries".into()));
            }
            let entry = self.resource_entry("Font", font_name.name())?;
            let identity = entry.as_object_id();
            let shared = identity.and_then(|id| self.ctx.cached_font(id));
            let font = if let Some(font) = shared {
                font
            } else {
                let resource = self.ctx.resolve(&entry)?;
                let font =
                    std::rc::Rc::new(self.load_font(resource.as_dict().ok_or_else(|| {
                        PdfError::PageResourceError("Font resource must be a dictionary".into())
                    })?)?);
                if let Some(id) = identity {
                    self.ctx.retain_font(id, font.clone());
                }
                font
            };
            self.font_cache
                .insert(font_name.name().to_owned(), font.clone());
            font
        };
        self.current_state.graphic_state.text_state.font = Some(font);
        Ok(())
    }
    fn load_font(&self, dict: &crate::objects::pdf_dict::PdfDict) -> PdfResult<Font> {
        let subtype = dict
            .get("Subtype")
            .map(|value| self.ctx.resolve(value))
            .transpose()?;
        if subtype
            .as_ref()
            .and_then(PdfObject::as_name)
            .is_some_and(|name| name.name() == "Type3")
        {
            Ok(Font::Type3(std::rc::Rc::new(
                crate::font::type3::Type3Font::try_new_with_budget(
                    dict,
                    self.ctx,
                    &self.type3_budget,
                )?,
            )))
        } else {
            Font::try_new(dict, self.ctx)
        }
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
        self.pending_text_clip = None;
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
            self.select_pattern(&op, true)
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
            self.select_pattern(&op, false)
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
                    "DeviceRGB" | "DeviceGray" | "DeviceCMYK" | "RGB" | "CMYK" | "G" | "Pattern"
                ) {
                    ColorSpace::try_new(obj, self.ctx)
                } else {
                    let colorobj = self.named_resource("ColorSpace", name.name())?;
                    let colorspace = ColorSpace::try_new(&colorobj, self.ctx)?;
                    Ok(colorspace)
                }
            }
            _ => Err(PdfError::PageOperatorError(
                "color space operand must be a name".into(),
            )),
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
        let extgs = extgs_obj.as_dict().ok_or(PdfError::PageResourceError(
            "Page ExtGs state is not a dict".to_string(),
        ))?;
        // Reject the whole state before applying supported entries. In compatible
        // mode the diagnostic remains visible; unsupported transparency must not
        // be silently treated as opaque or partially applied.
        if !crate::page::all_state::supported_ext_gs_keys(extgs) {
            let key = extgs
                .into_iter()
                .find(|(key, _)| !crate::page::all_state::supported_ext_gs_key(key))
                .unwrap()
                .0;
            return self.unsupported_operator_reason(&op, Some(&format!("ExtGState field {key}")));
        }
        // BG/UCR affect RGB -> CMYK printing, not conversion to an RGB raster.
        // Parse and retain the scalar functions; do not pretend they are defaults.
        // The *2 entry overrides its legacy counterpart, including /Default.
        let mut print_functions = [None, None];
        for (index, (legacy, modern)) in [("BG", "BG2"), ("UCR", "UCR2")].into_iter().enumerate() {
            if let Some(value) = extgs.get(modern).or_else(|| extgs.get(legacy)) {
                let resolved = self.ctx.resolve(value)?;
                let function = if resolved.as_name().is_some_and(|n| n.name() == "Default") {
                    None
                } else {
                    let function = crate::function::create_function(&resolved, self.ctx)?;
                    if function.dimensions() != (1, 1) {
                        return Err(PdfError::PageResourceError(format!(
                            "ExtGState {modern} requires a scalar function"
                        )));
                    }
                    let limits = self.ctx.limits();
                    self.execution_budget
                        .decoded(function.retained_payload_bytes(&limits)?, &limits)?;
                    Some(std::sync::Arc::new(function))
                };
                print_functions[index] = Some(function);
            }
        }
        // Transfer functions and halftones can affect visible RGB output and
        // remain explicit unsupported features unless neutral.
        for (legacy, modern) in [("TR", "TR2"), ("HT", "HT")] {
            let selected = extgs.get(modern).or_else(|| extgs.get(legacy));
            if let Some(value) = selected {
                let value = self.ctx.resolve(value)?;
                let neutral = value.as_name().is_some_and(|name| {
                    name.name() == "Default" || ((legacy == "TR") && name.name() == "Identity")
                });
                let neutral = neutral
                    || (legacy == "TR"
                        && value.as_array().is_some_and(|a| {
                            a.len() == 4
                                && a.into_iter()
                                    .all(|v| v.as_name().is_some_and(|n| n.name() == "Identity"))
                        }));
                if !neutral {
                    self.unsupported_operator_reason(
                        &op,
                        Some(&format!(
                            "ExtGState {modern} nondefault print function/halftone"
                        )),
                    )?;
                }
            }
        }
        // Nondefault AIS/TK need separate shape/text compositing support.
        // Validate them before applying any part of this state.
        for (key, default) in [("AIS", false), ("TK", true)] {
            if let Some(value) = extgs.get(key) {
                let value = self.ctx.resolve(value)?;
                let value = value
                    .as_bool()
                    .ok_or_else(|| {
                        PdfError::PageResourceError(format!("ExtGState {key} must be boolean"))
                    })?
                    .value();
                if value != default {
                    return self.unsupported_operator_reason(
                        &op,
                        Some(&format!("ExtGState {key}={value}")),
                    );
                }
            }
        }
        let mut general = self.current_state.graphic_state.general_state.clone();
        for key in ["OP", "op", "SA"] {
            if let Some(value) = extgs.get(key) {
                let value = self.ctx.resolve(value)?;
                let value = value
                    .as_bool()
                    .ok_or_else(|| {
                        PdfError::PageResourceError(format!("ExtGState {key} must be boolean"))
                    })?
                    .value();
                match key {
                    "OP" => {
                        general.stroke_overprint = value;
                        if !extgs.contains_key("op") {
                            general.fill_overprint = value;
                        }
                    }
                    "op" => general.fill_overprint = value,
                    "SA" => general.stroke_adjustment = value,
                    _ => unreachable!(),
                }
            }
        }
        if let Some(value) = extgs.get("OPM") {
            general.overprint_mode = self
                .ctx
                .resolve(value)?
                .as_u8()
                .filter(|value| *value <= 1)
                .ok_or_else(|| {
                    PdfError::PageResourceError("ExtGState OPM must be 0 or 1".into())
                })?;
        }
        if let Some(value) = extgs.get("SM") {
            general.smoothness = self
                .ctx
                .resolve(value)?
                .as_f32()
                .filter(|value| value.is_finite() && (0.0..=1.0).contains(value))
                .ok_or_else(|| {
                    PdfError::PageResourceError("ExtGState SM must be finite and in [0,1]".into())
                })?;
        }
        let mask = extgs
            .get("SMask")
            .map(|value| self.compile_soft_mask(value))
            .transpose()?;
        let intent = if let Some(value) = extgs.get("RI") {
            let value = self.ctx.resolve(value)?;
            let name = value
                .as_name()
                .ok_or_else(|| PdfError::PageResourceError("ExtGState RI must be a name".into()))?
                .name();
            let Some(intent) = crate::color::conversion::RenderingIntent::from_pdf_name(name)
            else {
                return self.unsupported_operator(&op);
            };
            Some(intent)
        } else {
            None
        };
        if let Some(value) = extgs.get("BM") {
            let Some(mode) = crate::page::general_state::resolve_blend_mode(value, self.ctx)?
            else {
                return self.unsupported_operator(&op);
            };
            self.current_state.graphic_state.general_state.blend_mode = mode;
        }
        // Preserve the print-device state through q/Q and into display objects.
        self.current_state
            .graphic_state
            .general_state
            .stroke_overprint = general.stroke_overprint;
        self.current_state
            .graphic_state
            .general_state
            .fill_overprint = general.fill_overprint;
        self.current_state
            .graphic_state
            .general_state
            .overprint_mode = general.overprint_mode;
        self.current_state
            .graphic_state
            .general_state
            .stroke_adjustment = general.stroke_adjustment;
        self.current_state.graphic_state.general_state.smoothness = general.smoothness;
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
            let font = std::rc::Rc::new(self.load_font(dict)?);
            self.current_state.graphic_state.text_state.font = Some(font);
            self.current_state.graphic_state.text_state.font_size = size;
        }
        let [black, undercolor] = print_functions;
        if let Some(function) = black {
            self.current_state
                .graphic_state
                .general_state
                .black_generation = function;
        }
        if let Some(function) = undercolor {
            self.current_state
                .graphic_state
                .general_state
                .undercolor_removal = function;
        }
        if let Some(mask) = mask {
            self.current_state.graphic_state.general_state.soft_mask = mask;
        }
        if let Some(intent) = intent {
            self.current_state
                .graphic_state
                .general_state
                .rendering_intent = intent;
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
        let pathobj = if self.form_depth == 0 && self.type3_depth == 0 {
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
        let pathobj = if self.form_depth == 0 && self.type3_depth == 0 {
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
        let path_obj = if self.form_depth == 0 && self.type3_depth == 0 {
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
        if self.type3_depth != 0 {
            return Ok(None);
        }
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

    fn handle_end_path(&mut self, _device: &mut dyn Device) -> PdfResult<()> {
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

    fn handle_shading(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        if self.in_text_object {
            return Err(PdfError::PageOperatorError(
                "shading invocation inside BT/ET".into(),
            ));
        }
        let name = op.get_as_name(0)?.name();
        device.preflight_draw(&self.current_state.graphic_state, None)?;
        let object = self.named_resource("Shading", name)?;
        let shading = match crate::color::shading::Shading::try_new(&object, self.ctx) {
            Ok(shading) => shading,
            Err(PdfError::UnsupportedFeature(_)) => return self.unsupported_operator(&op),
            Err(error) => return Err(error),
        };
        let matrix = self
            .current_state
            .ctm()
            .checked_mul(&self.page.page_matrix)?;
        let object = crate::page::shading_object::ShadingObject::new(
            name.to_string(),
            matrix,
            shading,
            self.current_state.graphic_state.clone(),
        )
        .with_source(
            op.byte_offset()
                .filter(|_| self.form_depth == 0 && self.type3_depth == 0)
                .map(
                    |operator_byte_offset| crate::page::shading_object::ShadingDrawSource {
                        operation_index: self.operation_index,
                        operator_byte_offset,
                    },
                ),
        )
        .with_nested_source(self.nested_draw_source()?);
        device.do_shading(object)
    }

    fn handle_inline_image(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        if self.in_text_object {
            return Err(PdfError::PageOperatorError(
                "inline image inside BT/ET".into(),
            ));
        }
        let image = op.inline_image().ok_or_else(|| {
            PdfError::PageOperatorError("BI requires a typed inline payload".into())
        })?;
        device.preflight_draw(&self.current_state.graphic_state, None)?;
        let mut dict = image.normalized_dict();
        if let Some(name) = dict.get("ColorSpace").and_then(PdfObject::as_name) {
            let default_key = match name.name() {
                "DeviceGray" => Some("DefaultGray"),
                "DeviceRGB" => Some("DefaultRGB"),
                "DeviceCMYK" => Some("DefaultCMYK"),
                _ => None,
            };
            if let Some(key) = default_key {
                let resources = self
                    .resource_scope
                    .clone()
                    .map(Ok)
                    .unwrap_or_else(|| self.page.resource())?;
                if let Some(colors) = resources.get("ColorSpace") {
                    let colors = self.ctx.resolve(colors)?;
                    let colors = colors.as_dict().ok_or_else(|| {
                        PdfError::PageResourceError(
                            "ColorSpace resources must be a dictionary".into(),
                        )
                    })?;
                    if colors
                        .get(key)
                        .is_some_and(|v| !matches!(v, PdfObject::PdfNull))
                    {
                        return Err(PdfError::UnsupportedFeature(
                            "inline default colour override outside v0.6 baseline".into(),
                        ));
                    }
                }
            }
            if !matches!(name.name(), "DeviceGray" | "DeviceRGB" | "DeviceCMYK") {
                let color = self.named_resource("ColorSpace", name.name())?;
                dict.insert("ColorSpace".into(), color);
            }
        }
        let stream = PdfObject::PdfStream(crate::objects::pdf_stream::PdfStream::new(
            dict,
            image.encoded_data().to_vec(),
        ));
        let image_matrix = self
            .current_state
            .ctm()
            .checked_mul(&self.page.page_matrix)?;
        let decoded = if self.pattern_depth == 0 && self.type3_depth == 0 {
            PdfImage::try_new_for_render(String::new(), &stream, self.ctx, &image_matrix)?
        } else {
            PdfImage::try_new(String::new(), &stream, self.ctx)?
        }
        .mark_inline();
        if (self.type3_uncoloured || self.pattern_uncoloured) && !decoded.is_mask() {
            return Err(PdfError::FontError(
                "d1 glyph image must be a stencil".into(),
            ));
        }
        let matrix = Matrix::new(1., 0., 0., -1., 0., 1.)
            .checked_mul(self.current_state.ctm())?
            .checked_mul(&self.page.page_matrix)?;
        let drawing = ImageObject::new(matrix, decoded, self.current_state.graphic_state.clone())
            .with_source(
                op.byte_offset()
                    .filter(|_| self.form_depth == 0 && self.type3_depth == 0)
                    .map(|operator_byte_offset| ImageDrawSource {
                        operation_index: self.operation_index,
                        operator_byte_offset,
                    }),
            )
            .with_nested_source(self.nested_draw_source()?);
        device.do_image(drawing)
    }

    fn handle_do(&mut self, op: Operator, device: &mut dyn Device) -> PdfResult<()> {
        let name = op.get_as_name(0)?.name();
        let object = self.named_resource("XObject", name)?;
        let previous = self.optional_visible;
        if let Some(value) = object.as_stream().and_then(|s| s.dict().get("OC"))
            && let Some(visibility) = self.optional_content.as_ref()
        {
            self.optional_visible &= visibility.visible(self.ctx, value)?;
        }
        let visible = self.optional_visible;
        let result = self.handle_do_inner(
            op,
            object,
            &mut crate::device::visibility::VisibilityDevice { device, visible },
        );
        self.optional_visible = previous;
        result
    }

    fn handle_do_inner(
        &mut self,
        op: Operator,
        xobject: PdfObject,
        device: &mut dyn Device,
    ) -> PdfResult<()> {
        let name = op.get_as_name(0)?.name();
        if self.semantic_capture.is_some()
            && self.optional_content.is_none()
            && xobject
                .as_stream()
                .is_some_and(|stream| stream.dict().contains_key("OC"))
        {
            return Err(PdfError::UnsupportedFeature(
                "marked extraction XObject optional content visibility".into(),
            ));
        }
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
                if self.font_cache_revision != self.ctx.revision() {
                    self.font_cache.clear();
                    self.image_cache.clear();
                    self.image_cache_bytes = 0;
                    self.font_cache_revision = self.ctx.revision();
                }
                let pdfimage = if let Some(image) = self.image_cache.get(name) {
                    image.clone()
                } else {
                    let image_matrix = self
                        .current_state
                        .ctm()
                        .checked_mul(&self.page.page_matrix)?;
                    let image =
                        std::rc::Rc::new(if self.pattern_depth == 0 && self.type3_depth == 0 {
                            PdfImage::try_new_for_render(
                                name.to_string(),
                                &xobject,
                                self.ctx,
                                &image_matrix,
                            )?
                        } else {
                            PdfImage::try_new(name.to_string(), &xobject, self.ctx)?
                        });
                    let bytes = image.retained_sample_bytes()?;
                    let source_dimension = |key| -> PdfResult<Option<u32>> {
                        xobject
                            .get_attr(key)
                            .map(|value| self.ctx.resolve(value))
                            .transpose()
                            .map(|value| value.and_then(|value| value.as_u32()))
                    };
                    let (width, height) = image.pixel_dimensions();
                    // Only full-size images enter this name-only cache. Reduced
                    // images depend on each draw's transform, not just its name.
                    let full_precision = self.ctx.render_image_dpi().is_none()
                        || (source_dimension("Width")? == Some(width)
                            && source_dimension("Height")? == Some(height));
                    if full_precision
                        && self.image_cache.len() < self.ctx.limits().max_objects
                        && bytes
                            <= self
                                .ctx
                                .limits()
                                .max_decoded_bytes
                                .saturating_sub(self.image_cache_bytes)
                    {
                        self.image_cache.try_reserve(1).map_err(|_| {
                            PdfError::ResourceLimit("image cache allocation".into())
                        })?;
                        self.image_cache.insert(name.to_owned(), image.clone());
                        self.image_cache_bytes += bytes;
                    }
                    image
                };
                if (self.type3_uncoloured || self.pattern_uncoloured) && !pdfimage.is_mask() {
                    return Err(PdfError::FontError(
                        "d1 glyph image must be a stencil".into(),
                    ));
                }
                let matrix = self.current_state.ctm();
                let image_to_user = Matrix::new(1.0, 0.0, 0.0, -1.0, 0.0, 1.0);
                let matrix = image_to_user.checked_mul(matrix)?;
                let image_matrix = matrix.checked_mul(&self.page.page_matrix)?;
                let imageobject = ImageObject::new(
                    image_matrix,
                    pdfimage,
                    self.current_state.graphic_state.clone(),
                )
                .with_source(
                    op.byte_offset()
                        .filter(|_| self.form_depth == 0 && self.type3_depth == 0)
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
                if depth + self.type3_depth > 128 {
                    return Err(PdfError::ResourceLimit(
                        "Form implementation depth (128)".into(),
                    ));
                }
                let id = self.resource_entry("XObject", name)?.as_object_id();
                if id.is_some_and(|id| self.active_forms.contains(&id)) {
                    return Err(PdfError::PageResourceError("Cyclic Form invocation".into()));
                }
                let form = crate::page::pdf_form::PdfForm::try_new(&xobject, self.ctx)?;
                let group = form
                    .group
                    .as_ref()
                    .map(|group| super::transparency::GroupParameters::parse(group, self.ctx))
                    .transpose()?;
                if group.is_some() && self.type3_depth > 0 {
                    return Err(PdfError::UnsupportedFeature(
                        "Type3 transparency group".into(),
                    ));
                }
                let gray_only = group.as_ref().is_some_and(|g| g.gray_only);
                let mut state = if group.is_some() {
                    form.group_invocation_state(
                        &self.current_state,
                        &self.page.page_matrix,
                        &limits,
                    )?
                } else {
                    form.invocation_state(&self.current_state, &self.page.page_matrix, &limits)?
                };
                let group = group.map(|group| {
                    let general = &self.current_state.graphic_state.general_state;
                    let paint = super::transparency::GroupPaint {
                        blending_profile: group.blending_profile,
                        rendering_intent: general.rendering_intent,
                        isolated: group.isolated,
                        knockout: group.knockout,
                        alpha: general.fill_alpha,
                        blend_mode: general.blend_mode,
                        clip: state.graphic_state.clip_path.clone(),
                        soft_mask: general.soft_mask.clone(),
                    };
                    state.graphic_state.general_state.fill_alpha = 1.;
                    state.graphic_state.general_state.stroke_alpha = 1.;
                    state.graphic_state.general_state.blend_mode =
                        super::general_state::BlendMode::Normal;
                    state.graphic_state.general_state.soft_mask = None;
                    paint
                });
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
                child.language = self.language.clone();
                child.language_initialized = true;
                child.execution_budget = budget;
                child.semantic_capture = self.semantic_capture.clone();
                child.optional_content = self.optional_content.clone();
                child.optional_visible = self.optional_visible;
                child.form_depth = depth;
                child.type3_depth = self.type3_depth;
                child.type3_uncoloured = self.type3_uncoloured;
                child.type3_budget = self.type3_budget.clone();
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
                child.initial_ctm = child.current_state.ctm.clone();
                child.pattern_depth = self.pattern_depth;
                child.pattern_uncoloured = self.pattern_uncoloured;
                let grouped = group.is_some();
                if let Some(group) = group {
                    device.begin_group(group)?;
                }
                let run = if gray_only {
                    child.run_content(&mut crate::device::gray_group::GrayGroupDevice(device))
                } else {
                    child.run_content(device)
                };
                let result = run.and_then(|()| {
                    crate::filter::check_output_size(
                        self.diagnostics.len(),
                        child.diagnostics.len(),
                        limits.max_content_diagnostics,
                        "page content diagnostics",
                    )?;
                    crate::filter::check_output_size(
                        self.diagnostic_bytes,
                        child.diagnostic_bytes,
                        limits.max_content_diagnostic_bytes,
                        "page diagnostic bytes",
                    )?;
                    self.diagnostics
                        .try_reserve(child.diagnostics.len())
                        .map_err(|_| {
                            PdfError::ResourceLimit("Form diagnostic allocation".into())
                        })?;
                    self.diagnostic_bytes += child.diagnostic_bytes;
                    self.diagnostics.append(&mut child.diagnostics);
                    Ok(())
                });
                let cleanup = if grouped {
                    device.end_group(result.is_ok())
                } else {
                    Ok(())
                };
                result?;
                cleanup?;
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

#[cfg(test)]
mod resource_scope_tests {
    #[test]
    fn marked_content_points_do_not_open_scopes_or_change_paint() {
        use crate::{
            document::PdfDocument, page::page_builder::PageBuilder, parser::syntax::ParseMode,
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(10., 10.))
            .unwrap();
        for source in [
            "/App MP /App << /ActualText (not a replacement) >> DP 0 0 1 1 re f",
            "/Outer BMC /App MP /App << /MCID 0 >> DP 0 0 1 1 re f EMC",
        ] {
            doc.replace_page_contents(0, source.as_bytes().to_vec())
                .unwrap();
            let reopened =
                PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            let list = reopened.get_page(0).unwrap().display_list().unwrap();
            assert_eq!(list.objects().len(), 1);
            assert!(list.diagnostics().is_empty());
        }
        for source in ["MP", "(tag) MP", "/Tag <<>> MP", "/Tag DP", "/Tag 1 DP"] {
            doc.replace_page_contents(0, source.as_bytes().to_vec())
                .unwrap();
            assert!(doc.get_page(0).unwrap().display_list().is_err(), "{source}");
        }
    }

    #[test]
    fn printer_functions_are_retained_and_restore_without_affecting_rgb() {
        use crate::{
            document::PdfDocument,
            io::stream_reader::StreamReader,
            objects::pdf_stream::PdfStream,
            page::{page_builder::PageBuilder, page_object::PageObject},
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        let parse = |s: &str| {
            SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
                .get_object()
                .unwrap()
                .into_dict()
                .unwrap()
        };
        let doc = PdfDocument::new().unwrap();
        let bg=doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(parse("<< /FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 >>"),vec![0,0]).unwrap())).unwrap();
        let ucr=doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(parse("<< /FunctionType 0 /Domain [0 1] /Range [-1 1] /Size [2] /BitsPerSample 8 >>"),vec![128,128]).unwrap())).unwrap();
        let resources = parse(&format!(
            "<< /ExtGState << /G << /BG {} 0 R /UCR {} 0 R >> /Reset << /BG /UnusableLegacy /BG2 /Default /UCR2 /Default >> >> >>",
            bg.obj_num(),
            ucr.obj_num()
        ));
        doc.add_page_with_builder(0, PageBuilder::new().size(10., 10.).resources(resources))
            .unwrap();
        doc.replace_page_contents(
            0,
            b"q /G gs 0 0 1 1 re f Q 0 0 1 1 re f /G gs /Reset gs 0 0 1 1 re f".to_vec(),
        )
        .unwrap();
        let source = doc.save_to_bytes().unwrap();
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let doc = PdfDocument::from_bytes_with_mode(source.clone(), mode).unwrap();
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            assert!(list.diagnostics().is_empty());
            let PageObject::Path(first) = &list.objects()[0] else {
                panic!()
            };
            let state = &first.graphic_state().general_state;
            assert_eq!(
                state
                    .black_generation
                    .as_ref()
                    .unwrap()
                    .eval(&[0.5])
                    .unwrap(),
                vec![0.]
            );
            assert!(
                (state
                    .undercolor_removal
                    .as_ref()
                    .unwrap()
                    .eval(&[0.5])
                    .unwrap()[0]
                    - 1. / 255.)
                    .abs()
                    < 1e-6
            );
            for object in &list.objects()[1..] {
                let PageObject::Path(path) = object else {
                    panic!()
                };
                assert!(
                    path.graphic_state()
                        .general_state
                        .black_generation
                        .is_none()
                );
                assert!(
                    path.graphic_state()
                        .general_state
                        .undercolor_removal
                        .is_none()
                );
            }
        }
    }

    #[test]
    fn extgstate_name_metadata_preserves_alpha_and_font_binding() {
        use crate::{
            document::PdfDocument,
            io::stream_reader::StreamReader,
            page::{page_builder::PageBuilder, page_object::PageObject},
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        for label in ["/GS1", "(producer label)"] {
            let resources = SyntaxParser::new(StreamReader::try_new(Cursor::new(
                format!("<< /Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> /ExtGState << /G << /Name {label} /ca 0.5 /CA 0.25 >> >> >>").into_bytes(),
            )).unwrap()).get_object().unwrap().into_dict().unwrap();
            let doc = PdfDocument::new().unwrap();
            doc.add_page_with_builder(0, PageBuilder::new().size(30., 30.).resources(resources))
                .unwrap();
            doc.replace_page_contents(
                0,
                b"q /G gs BT /F 10 Tf (A) Tj ET 0 0 10 10 re f Q 10 10 10 10 re f".to_vec(),
            )
            .unwrap();
            let bytes = doc.save_to_bytes().unwrap();
            for mode in [ParseMode::Strict, ParseMode::Compatible] {
                let reopened = PdfDocument::from_bytes_with_mode(bytes.clone(), mode).unwrap();
                let list = reopened.get_page(0).unwrap().display_list().unwrap();
                assert!(list.diagnostics().is_empty());
                assert_eq!(list.objects().len(), 3);
                let PageObject::Text(text) = &list.objects()[0] else {
                    panic!("missing text")
                };
                assert!(!text.items.is_empty());
                assert_eq!(text.state.general_state.fill_alpha, 0.5);
                assert_eq!(text.state.general_state.stroke_alpha, 0.25);
                let PageObject::Path(path) = &list.objects()[1] else {
                    panic!("missing path")
                };
                assert_eq!(path.graphic_state().general_state.fill_alpha, 0.5);
                let PageObject::Path(restored) = &list.objects()[2] else {
                    panic!("missing restored path")
                };
                assert_eq!(restored.graphic_state().general_state.fill_alpha, 1.0);
            }
        }
    }

    #[test]
    fn print_device_state_is_typed_retained_and_restored() {
        use crate::{
            document::{Document, resource_manager::ResourceKind},
            io::stream_reader::StreamReader,
            objects::pdf_stream::PdfStream,
            page::page_object::PageObject,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        let parse = |s: &str| {
            SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
                .get_object()
                .unwrap()
                .into_dict()
                .unwrap()
        };
        for state in [
            "<< /OP true /OPM 1 /SA true /SM 0.02 /AIS false /TK true >>",
            "<< /OP true /op false /OPM 1 /SA true /SM 0.02 >>",
            "<< /OP true /OPM 1 /SA true /SM 0.02 /BG2 /Default /UCR2 /Default /HT /Default /TR /UnusedLegacyTransfer /TR2 /Identity >>",
        ] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            let dict = parse(&format!(
                "<< /Subtype /Form /BBox [0 0 10 10] /Resources << /ExtGState << /G {state} >> >> >>"
            ));
            let id = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(
                        dict,
                        b"q /G gs 0 0 1 1 re f Q 1 1 1 1 re f".to_vec(),
                    )
                    .unwrap(),
                ))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, id)
                .unwrap();
            doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
                .unwrap();
            let doc =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            let list = doc.get_page(0).unwrap().display_list().unwrap();
            assert!(list.diagnostics().is_empty());
            let PageObject::Path(first) = &list.objects()[0] else {
                panic!()
            };
            let general = &first.graphic_state().general_state;
            assert!(general.stroke_overprint);
            assert_eq!(general.fill_overprint, !state.contains("/op false"));
            assert_eq!(general.overprint_mode, 1);
            assert!(general.stroke_adjustment);
            assert_eq!(general.smoothness, 0.02);
            let PageObject::Path(second) = &list.objects()[1] else {
                panic!()
            };
            assert!(!second.graphic_state().general_state.stroke_overprint);
            assert!(!second.graphic_state().general_state.stroke_adjustment);
        }
        for state in [
            "<< /OP 1 >>",
            "<< /OPM 2 >>",
            "<< /OPM 0.5 >>",
            "<< /SM -1 >>",
            "<< /SA /true >>",
            "<< /AIS true >>",
            "<< /TK false >>",
            "<< /Unknown true >>",
        ] {
            let doc = Document::new().unwrap();
            doc.add_page(0).unwrap();
            let dict = parse(&format!(
                "<< /Subtype /Form /BBox [0 0 10 10] /Resources << /ExtGState << /G {state} >> >> >>"
            ));
            let id = doc
                .try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(dict, b"/G gs".to_vec()).unwrap(),
                ))
                .unwrap();
            let name = doc
                .resources()
                .register_existing(0, ResourceKind::Form, id)
                .unwrap();
            doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
                .unwrap();
            let doc =
                Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                    .unwrap();
            assert!(doc.get_page(0).unwrap().display_list().is_err(), "{state}");
        }
    }

    #[test]
    fn named_application_marked_metadata_preserves_paint_order_text() {
        use crate::{
            document::{Document, resource_manager::ResourceKind},
            io::stream_reader::StreamReader,
            objects::pdf_stream::PdfStream,
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        let dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(b"<< /Subtype /Form /BBox [0 0 10 10] /Resources << /Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> /Properties << /Meta << /Metadata 999 0 R >> /Text << /ActualText (replacement) >> >> >> >>".to_vec())).unwrap()).get_object().unwrap().into_dict().unwrap();
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        // Metadata references must not be loaded by a glyph-painting reader.
        let id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(
                    dict,
                    b"/PlacedGraphic /Meta BDC /Span /Text BDC BT /F 10 Tf (A) Tj ET EMC EMC"
                        .to_vec(),
                )
                .unwrap(),
            ))
            .unwrap();
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, id)
            .unwrap();
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())
            .unwrap();
        let doc = Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
            .unwrap();
        let report = doc
            .get_page(0)
            .unwrap()
            .extract_characters_paint_order_report()
            .unwrap();
        assert!(report.diagnostics.is_empty());
        assert_eq!(report.characters.len(), 1);
        assert_eq!(report.characters[0].unicode.as_deref(), Some("A"));
    }

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

#[cfg(test)]
mod font_reuse_tests {
    use crate::{
        document::{Document, resource_manager::StandardFont},
        page::page_object::PageObject,
    };
    #[test]
    fn repeated_selection_and_saved_states_share_font_but_edits_build_fresh_snapshot() {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{name} 12 Tf (A) Tj q /{name} 14 Tf (B) Tj Q (C) Tj ET").into_bytes(),
        )
        .unwrap();
        let first = doc.get_page(0).unwrap().display_list().unwrap();
        let fonts: Vec<_> = first
            .objects()
            .iter()
            .filter_map(|v| match v {
                PageObject::Text(v) => v.state.text_state.font.as_ref(),
                _ => None,
            })
            .collect();
        assert_eq!(fonts.len(), 3);
        assert!(std::rc::Rc::ptr_eq(fonts[0], fonts[1]));
        assert!(std::rc::Rc::ptr_eq(fonts[0], fonts[2]));
        let other = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        doc.replace_page_contents(0, format!("BT /{other} 12 Tf (D) Tj ET").into_bytes())
            .unwrap();
        let second = doc.get_page(0).unwrap().display_list().unwrap();
        let PageObject::Text(text) = &second.objects()[0] else {
            panic!("text")
        };
        assert_eq!(
            text.state.text_state.font.as_ref().unwrap().name(),
            "Helvetica"
        );
        assert_eq!(fonts[0].name(), "Courier");
        assert!(!std::rc::Rc::ptr_eq(
            fonts[0],
            text.state.text_state.font.as_ref().unwrap()
        ));
    }
}

#[cfg(test)]
mod font_cache_revision_tests {
    use super::*;
    use crate::{
        document::{Document, resource_manager::StandardFont},
        io::stream_reader::StreamReader,
        objects::{pdf_name::PdfName, pdf_number::PdfNumber},
        parser::syntax::SyntaxParser,
    };
    #[test]
    fn replacing_resource_invalidates_an_existing_interpreters_font_cache() {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .register_standard_font(0, StandardFont::Courier)
            .unwrap();
        let page = doc.get_page(0).unwrap();
        let resources = page.resource().unwrap();
        let id = resources
            .get("Font")
            .unwrap()
            .get_attr(&name)
            .unwrap()
            .as_object_id()
            .unwrap();
        let parser = ContentParser::new(SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap(),
        ));
        let mut interpreter = Interpreter::new(&page, doc.ctx(), parser);
        let selection = || {
            Operator::new(
                "Tf".into(),
                vec![
                    PdfObject::PdfName(PdfName::new(name.clone())),
                    PdfObject::PdfNumber(PdfNumber::new(12.)),
                ],
            )
        };
        interpreter.handle_set_font(selection()).unwrap();
        let original = interpreter
            .current_state
            .graphic_state
            .text_state
            .font
            .clone()
            .unwrap();
        let replacement = SyntaxParser::new(
            StreamReader::try_new(std::io::Cursor::new(
                b"<< /Type /Font /Subtype /Type1 /BaseFont /Helvetica >>".to_vec(),
            ))
            .unwrap(),
        )
        .get_object()
        .unwrap();
        doc.update_object(id, replacement).unwrap();
        interpreter.handle_set_font(selection()).unwrap();
        let updated = interpreter
            .current_state
            .graphic_state
            .text_state
            .font
            .as_ref()
            .unwrap();
        assert_eq!(original.name(), "Courier");
        assert_eq!(updated.name(), "Helvetica");
        assert!(!std::rc::Rc::ptr_eq(&original, updated));
    }
}

#[cfg(test)]
mod image_cache_tests {
    use super::*;
    use crate::{document::Document, objects::pdf_name::PdfName, parser::options::ParseOptions};
    #[test]
    fn repeated_images_share_pixels_under_budget_and_resource_edits_invalidate_cache() {
        let mut options = ParseOptions::default();
        options.limits.max_decoded_bytes = 16 * 1024;
        let doc = Document::new_with_options(options).unwrap();
        doc.add_page(0).unwrap();
        let name = doc
            .resources()
            .embed_rgb_image(0, 32, 32, vec![128; 32 * 32 * 3])
            .unwrap();
        let page = doc.get_page(0).unwrap();
        let resources = page.resource().unwrap();
        let id = resources
            .get("XObject")
            .unwrap()
            .get_attr(&name)
            .unwrap()
            .as_object_id()
            .unwrap();
        let parser = ContentParser::new(crate::parser::syntax::SyntaxParser::new(
            crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new()))
                .unwrap(),
        ));
        let mut interpreter = Interpreter::new(&page, doc.ctx(), parser);
        let mut trace = TraceDevice::for_display_list(options.limits);
        let op = || {
            Operator::new(
                "Do".into(),
                vec![PdfObject::PdfName(PdfName::new(name.clone()))],
            )
        };
        for _ in 0..22 {
            interpreter.handle_do(op(), &mut trace).unwrap();
        }
        assert_eq!(trace.objects().len(), 22);
        let first = match &trace.objects()[0] {
            super::super::page_object::PageObject::Image(v) => v.image.clone(),
            _ => panic!(),
        };
        for draw in trace.objects() {
            let super::super::page_object::PageObject::Image(image) = draw else {
                panic!()
            };
            assert!(std::rc::Rc::ptr_eq(&first, &image.image));
        }
        let list = trace.into_display_list();
        assert!(list.collection_usage().0 < 16 * 1024);
        let updated = doc
            .ctx()
            .resolve(&PdfObject::PdfReference(
                crate::objects::pdf_reference::PdfReference::from_id(id),
            ))
            .unwrap()
            .into_stream()
            .unwrap();
        let updated = crate::objects::pdf_stream::PdfStream::from_decoded_data(
            updated.dict().clone(),
            vec![0; 32 * 32 * 3],
        )
        .unwrap();
        doc.update_object(id, PdfObject::PdfStream(updated))
            .unwrap();
        let mut trace = TraceDevice::for_display_list(options.limits);
        interpreter.handle_do(op(), &mut trace).unwrap();
        let super::super::page_object::PageObject::Image(image) = &trace.objects()[0] else {
            panic!()
        };
        assert!(!std::rc::Rc::ptr_eq(&first, &image.image));
        assert_eq!(first.rgba_buffer().unwrap()[0], 128);
        assert_eq!(image.image.rgba_buffer().unwrap()[0], 0);
    }
}
