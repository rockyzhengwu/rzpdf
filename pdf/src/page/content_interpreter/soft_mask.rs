use super::*;
use crate::page::transparency::{GroupParameters, SoftMaskKind, SoftMaskPaint};
use std::sync::Arc;

impl<R: Read + Seek> Interpreter<'_, R> {
    pub(super) fn compile_soft_mask(
        &self,
        value: &PdfObject,
    ) -> PdfResult<Option<Arc<SoftMaskPaint>>> {
        let value = self.ctx.resolve(value)?;
        if let Some(name) = value.as_name() {
            return if name.name() == "None" {
                Ok(None)
            } else {
                Err(PdfError::PageResourceError(
                    "SMask name must be None".into(),
                ))
            };
        }
        let dict = value.as_dict().ok_or_else(|| {
            PdfError::PageResourceError("SMask must be a dictionary or None".into())
        })?;
        if let Some(kind) = dict.get("Type") {
            if self.ctx.resolve(kind)?.as_name().map(|n| n.name()) != Some("Mask") {
                return Err(PdfError::PageResourceError(
                    "SMask Type must be Mask".into(),
                ));
            }
        }
        let subtype = dict.get("S").map(|v| self.ctx.resolve(v)).transpose()?;
        let kind = match subtype
            .as_ref()
            .and_then(PdfObject::as_name)
            .map(|n| n.name())
        {
            Some("Alpha") => SoftMaskKind::Alpha,
            Some("Luminosity") => SoftMaskKind::Luminosity,
            _ => {
                return Err(PdfError::PageResourceError(
                    "SMask S must be Alpha or Luminosity".into(),
                ));
            }
        };
        let g = dict
            .get("G")
            .ok_or_else(|| PdfError::PageResourceError("SMask G missing".into()))?;
        let id = g.as_object_id();
        if id.is_some_and(|id| self.active_forms.contains(&id)) {
            return Err(PdfError::PageResourceError("cyclic soft mask group".into()));
        }
        let object = self.ctx.resolve(g)?;
        if self.optional_content.is_some() && object.as_stream().is_some_and(|s| s.dict().contains_key("OC")) {
            return Err(PdfError::UnsupportedFeature("optional content on soft-mask G is outside the supported composition subset".into()));
        }
        let form = crate::page::pdf_form::PdfForm::try_new(&object, self.ctx)?;
        let group = form.group.as_ref().ok_or_else(|| {
            PdfError::PageResourceError("SMask G must have a transparency Group".into())
        })?;
        let parameters = GroupParameters::parse(group, self.ctx)?;
        if matches!(kind, SoftMaskKind::Luminosity) && group.get("CS").is_none() {
            return Err(PdfError::UnsupportedFeature(
                "luminosity mask requires explicit DeviceRGB group CS".into(),
            ));
        }
        let mut background = [0.; 3];
        if let Some(value) = dict.get("BC") {
            let value = self.ctx.resolve(value)?;
            let array = value.as_array().filter(|a| a.len() == 3).ok_or_else(|| {
                PdfError::PageResourceError("RGB soft mask BC requires three components".into())
            })?;
            for (component, value) in background.iter_mut().zip(array) {
                *component = self
                    .ctx
                    .resolve(value)?
                    .as_f32()
                    .filter(|v| v.is_finite())
                    .ok_or_else(|| {
                        PdfError::PageResourceError("soft mask BC must be finite".into())
                    })?
                    .clamp(0., 1.);
            }
        }
        let transfer = if let Some(value) = dict.get("TR") {
            let value = self.ctx.resolve(value)?;
            if value
                .as_name()
                .is_some_and(|name| name.name() == "Identity")
            {
                None
            } else {
                let function = crate::function::create_function(&value, self.ctx)?;
                if function.dimensions() != (1, 1) {
                    return Err(PdfError::PageResourceError(
                        "soft mask transfer requires one input/output".into(),
                    ));
                }
                let domain = function.input_domain();
                if domain.len() != 2 || domain[0] > 0. || domain[1] < 1. {
                    return Err(PdfError::PageResourceError(
                        "soft mask transfer Domain must contain [0,1]".into(),
                    ));
                }
                Some(function)
            }
        } else {
            None
        };
        let limits = self.ctx.limits();
        let work = self.execution_budget.child(&limits)?;
        if self.form_depth + self.pattern_depth + self.type3_depth >= 128 {
            return Err(PdfError::ResourceLimit(
                "soft mask implementation depth".into(),
            ));
        }
        let data = object
            .as_stream()
            .ok_or_else(|| PdfError::PageResourceError("soft mask G stream missing".into()))?
            .decode_data(self.ctx)?;
        work.decoded(data.len(), &limits)?;
        let parser = ContentParser::new(crate::parser::syntax::SyntaxParser::with_options(
            crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(data))?,
            self.ctx.options(),
        ));
        let page_resources = self.page.resource()?;
        let mut child = Interpreter::new(self.page, self.ctx, parser)
            .with_resource_scope(form.resource_scope(&page_resources).clone());
        child.form_depth = self.form_depth + 1;
        child.optional_content = self.optional_content.clone();
        child.pattern_depth = self.pattern_depth;
        child.execution_budget = work;
        child.active_forms = self.active_forms.clone();
        if let Some(id) = id {
            child.active_forms.push(id);
        }
        let mut parent = self.current_state.clone();
        parent.graphic_state.clip_path = Default::default();
        let intent = parent.graphic_state.general_state.rendering_intent;
        parent.graphic_state.general_state = Default::default();
        parent.graphic_state.general_state.rendering_intent = intent;
        child.current_state =
            form.group_invocation_state(&parent, &self.page.page_matrix, &limits)?;
        child.initial_ctm = child.current_state.ctm.clone();
        let mut trace = TraceDevice::for_display_list(limits);
        trace.start_page(self.page.page_width, self.page.page_height)?;
        if parameters.knockout {
            trace.begin_group(crate::page::transparency::GroupPaint {
                isolated: true,
                knockout: true,
                alpha: 1.,
                blend_mode: crate::page::general_state::BlendMode::Normal,
                clip: Default::default(),
                soft_mask: None,
            })?;
        }
        child.run_content(&mut trace)?;
        if parameters.knockout {
            trace.end_group(true)?;
        }
        if !child.diagnostics.is_empty() {
            return Err(PdfError::UnsupportedFeature(
                "soft mask group contains unsupported operations".into(),
            ));
        }
        let (mut retained_bytes, retained_segments) = trace.pattern_storage()?;
        if let Some(transfer) = &transfer {
            retained_bytes = retained_bytes
                .checked_add(transfer.retained_payload_bytes(&limits)?)
                .filter(|v| *v <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("soft mask retained payload".into()))?;
        }
        Ok(Some(Arc::new(SoftMaskPaint {
            list: trace.into_display_list(),
            kind,
            background,
            transfer,
            retained_bytes,
            retained_segments,
        })))
    }
}
