use super::*;
use crate::page::marked_text::{Capture, Mark, TextContentLocation};
use std::{cell::RefCell, rc::Rc};

impl<R: Read + Seek> Interpreter<'_, R> {
    pub(super) fn initialize_language(&mut self) -> PdfResult<()> {
        if self.language_initialized {
            return Ok(());
        }
        self.language_initialized = true;
        if !self.ctx.trailer().contains_key("Root") {
            return Ok(());
        }
        let root = self.ctx.get_root()?;
        if let Some(value) = root.get("Lang") {
            let value = self.ctx.resolve(value)?;
            if let Some(text) = value.as_string().filter(|s| s.bytes().len() <= 130) {
                let text = match text.text_content_with_limits(&self.ctx.limits()) {
                    Ok(text) => text,
                    Err(error @ PdfError::ResourceLimit(_)) => return Err(error),
                    Err(_) => return Ok(()),
                };
                self.execution_budget
                    .semantic_bytes(text.len(), &self.ctx.limits())?;
                if crate::font::resolver::valid_language(&text) {
                    self.language = Some(Rc::from(text));
                }
            }
        }
        Ok(())
    }
    pub(super) fn push_language_mark(&mut self, op: &Operator) -> PdfResult<()> {
        if self.language_stack.len() >= self.ctx.limits().max_content_operations {
            return Err(PdfError::ResourceLimit(
                "language marked-content stack".into(),
            ));
        }
        self.execution_budget
            .semantic_bytes(std::mem::size_of::<Option<Rc<str>>>(), &self.ctx.limits())?;
        self.language_stack
            .try_reserve(1)
            .map_err(|_| PdfError::ResourceLimit("language stack allocation".into()))?;
        self.language_stack.push(self.language.clone());
        if op.name() != "BDC" || op.get_as_name(0)?.name() == "OC" {
            return Ok(());
        }
        let value = op.operand(1)?;
        let value = if let Some(name) = value.as_name() {
            self.named_resource("Properties", name.name())?
        } else {
            self.ctx.resolve(value)?
        };
        if let Some(value) = value.as_dict().and_then(|d| d.get("Lang")) {
            let value = self.ctx.resolve(value)?;
            let string = value
                .as_string()
                .ok_or_else(|| PdfError::PageOperatorError("marked language string".into()))?;
            if string.bytes().len() > 130 {
                self.language = None;
                return Ok(());
            }
            let text = string.text_content_with_limits(&self.ctx.limits())?;
            self.language = if crate::font::resolver::valid_language(&text) {
                Some(Rc::from(text))
            } else {
                None
            };
        }
        Ok(())
    }

    /// Painting and paint-order extraction retain glyphs, not logical metadata
    /// replacements. Validate standard properties without rejecting custom tags.
    pub(super) fn validate_selected_properties(&self, op: &Operator) -> PdfResult<()> {
        let tag = op.get_as_name(0)?.name();
        if tag == "OC" && op.name() == "BDC" {
            return Ok(());
        } // membership already evaluated by begin_optional_mark
        let value = op.operand(1)?;
        let value = if let Some(name) = value.as_name() {
            self.named_resource("Properties", name.name())?
        } else {
            self.ctx.resolve(value)?
        };
        let dict = value.as_dict().ok_or_else(|| {
            PdfError::PageOperatorError("marked-content properties must be a dictionary".into())
        })?;
        for (key, value) in dict {
            match key.as_str() {
                "MCID" => {
                    self.ctx
                        .resolve(value)?
                        .as_number()
                        .ok_or_else(|| PdfError::PageOperatorError("MCID integer".into()))?
                        .as_u32_checked()?;
                }
                "ActualText" | "Alt" | "Lang" => {
                    let value = self.ctx.resolve(value)?;
                    let text = value
                        .as_string()
                        .ok_or_else(|| PdfError::PageOperatorError("marked text string".into()))?
                        .text_content_with_limits(&self.ctx.limits())?;
                    self.execution_budget
                        .semantic_bytes(text.len(), &self.ctx.limits())?;
                    self.execution_budget
                        .semantic_chars(text.chars().count(), &self.ctx.limits())?;
                }
                "Type" | "Subtype" | "BBox" | "Attached" if tag == "Artifact" => {}
                // Application metadata has no painting semantics. Keep it in
                // the source program; don't resolve potentially large payloads.
                _ => {}
            }
        }
        Ok(())
    }

    pub(crate) fn capture_marked_text(&mut self, capture: Rc<RefCell<Capture>>) {
        self.semantic_capture = Some(capture);
    }

    pub(super) fn semantic_location(&self) -> PdfResult<TextContentLocation> {
        let page = self.page.source_provenance().page_id.ok_or_else(|| {
            PdfError::DocumentError("marked extraction requires an indirect page".into())
        })?;
        Ok(TextContentLocation {
            page: crate::document::identity::ObjectHandle {
                document: self.ctx.identity(),
                id: page,
            },
            invocations: self.invocation_path.clone(),
            operation_index: self.operation_index,
            operator_byte_offset: self.operation_byte_offset,
        })
    }

    pub(super) fn capture_mark(&mut self, op: &Operator) -> PdfResult<()> {
        let Some(capture) = self.semantic_capture.clone() else {
            return Ok(());
        };
        let context = (self.in_text_object, self.state_stack.len());
        if op.name() == "EMC" {
            if self.semantic_mark_context.pop() != Some(context) {
                return Err(PdfError::PageOperatorError(
                    "EMC crosses a text/graphics-state boundary".into(),
                ));
            }
            return capture.borrow_mut().end();
        }
        let bdc = op.name() == "BDC";
        if op.num_operands() != if bdc { 2 } else { 1 } {
            return Err(PdfError::PageOperatorError(
                "marked extraction BMC/BDC operands".into(),
            ));
        }
        let tag = op.get_as_name(0)?.name().to_string();
        let mut mcid = None;
        let mut actual_text = None;
        if tag == "OC" && self.optional_content.is_none() {
            return Err(PdfError::UnsupportedFeature(
                "marked extraction optional content visibility".into(),
            ));
        }
        if bdc && tag != "OC" {
            let properties = op.operand(1)?;
            let properties = if let Some(name) = properties.as_name() {
                self.named_resource("Properties", name.name())?
            } else {
                self.ctx.resolve(properties)?
            };
            let dict = properties.as_dict().ok_or_else(|| {
                PdfError::PageOperatorError("marked-content properties must be a dictionary".into())
            })?;
            for (key, value) in dict {
                match key.as_str() {
                    "MCID" => {
                        mcid = Some(
                            self.ctx
                                .resolve(value)?
                                .as_number()
                                .ok_or_else(|| PdfError::PageOperatorError("MCID integer".into()))?
                                .as_u32_checked()?,
                        );
                    }
                    "ActualText" | "Lang" | "Alt" => {
                        let value = self.ctx.resolve(value)?;
                        let text = value
                            .as_string()
                            .ok_or_else(|| {
                                PdfError::PageOperatorError(
                                    "marked text property requires a text string".into(),
                                )
                            })?
                            .text_content_with_limits(&self.ctx.limits())?;
                        capture.borrow_mut().metadata(text.len())?;
                        if key == "ActualText" {
                            if !matches!(tag.as_str(), "Span" | "Artifact") {
                                return Err(PdfError::UnsupportedFeature(
                                    "ActualText marked content requires Span or Artifact tag"
                                        .into(),
                                ));
                            }
                            actual_text = Some(text);
                        }
                    }
                    "Type" | "Subtype" | "BBox" | "Attached" if tag == "Artifact" => {}
                    _ => {
                        return Err(PdfError::UnsupportedFeature(format!(
                            "marked extraction property {key}"
                        )));
                    }
                }
            }
        }
        let location = self.semantic_location()?;
        capture.borrow_mut().begin(Mark {
            tag,
            visible: self.optional_visible,
            mcid,
            actual_text,
            location,
        })?;
        self.semantic_mark_context.push(context);
        Ok(())
    }
}
