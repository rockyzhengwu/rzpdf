//! Pattern selection is transactional: publish only a fully interpreted paint.
use super::*;
use crate::color::pattern::{PatternPaint, tiling::TilingPattern};
use std::sync::Arc;

impl<R: Read + Seek> Interpreter<'_, R> {
    pub(super) fn select_pattern(&mut self, op: &Operator, stroke: bool) -> PdfResult<()> {
        let colors = &self.current_state.graphic_state.color_state;
        let ColorSpace::Pattern(space) = (if stroke {
            &colors.stroke_color_space
        } else {
            &colors.fill_color_space
        }) else {
            unreachable!()
        };
        let mut space = space.clone();
        let expected = space
            .base
            .as_ref()
            .map(|base| base.number_of_components())
            .transpose()?
            .unwrap_or(0);
        if op.num_operands() != expected + 1 {
            return Err(PdfError::ColorError(
                "pattern colour component count".into(),
            ));
        }
        let mut values = Vec::new();
        values
            .try_reserve_exact(expected)
            .map_err(|_| PdfError::ResourceLimit("pattern colour allocation".into()))?;
        for index in 0..expected {
            values.push(op.get_as_f32(index)?);
        }
        let name = op.get_as_name(expected)?.name();
        let object = self.named_resource("Pattern", name)?;
        let pattern = crate::color::pattern::Pattern::try_new(&object, self.ctx)?;
        let uncoloured =
            matches!(&pattern, crate::color::pattern::Pattern::Tiling(p) if p.paint_type == 2);
        if uncoloured != space.base.is_some() {
            return Err(PdfError::ColorError(
                "pattern PaintType disagrees with colour space".into(),
            ));
        }
        let value = ColorValue::new(values);
        let paint = match pattern {
            crate::color::pattern::Pattern::Tiling(pattern) => {
                self.compile_tiling_pattern(&object, &pattern, space.base.as_deref(), &value)?
            }
            crate::color::pattern::Pattern::Shading(pattern) => {
                self.compile_shading_pattern(pattern)?
            }
        };
        if !paint.list.diagnostics().is_empty() {
            self.unsupported_operator_reason(
                op,
                Some("pattern cell recovered unsupported operations; inspect pattern diagnostics"),
            )?;
        }
        // Sharing immutable paint snapshots does not promise Send or Sync.
        #[expect(
            clippy::arc_with_non_send_sync,
            reason = "shared public paint snapshots retain thread-affine font resources"
        )]
        let paint = Arc::new(paint);
        space.paint = Some(paint);
        let colors = &mut self.current_state.graphic_state.color_state;
        if stroke {
            colors.stroke_color_space = ColorSpace::Pattern(space);
            colors.stroke_color_value = value;
        } else {
            colors.fill_color_space = ColorSpace::Pattern(space);
            colors.fill_color_value = value;
        }
        Ok(())
    }

    fn compile_shading_pattern(
        &self,
        pattern: crate::color::pattern::shading::ShadingPattern,
    ) -> PdfResult<PatternPaint> {
        let limits = self.ctx.limits();
        let work = self.execution_budget.child(&limits)?;
        work.operation(&limits)?;
        let mut state = crate::page::graphic_state::GraphicState::default();
        // Use the ordinary interpreter validation for supported ExtGState values.
        if let Some(extgs) = pattern.ext_gstate {
            let mut entries = crate::objects::pdf_dict::PdfDict::default();
            entries.insert("G".into(), PdfObject::PdfDict(extgs));
            let mut scope = crate::objects::pdf_dict::PdfDict::default();
            scope.insert("ExtGState".into(), PdfObject::PdfDict(entries));
            let parser = ContentParser::new(crate::parser::syntax::SyntaxParser::with_options(
                crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                    b"/G gs".to_vec(),
                ))?,
                self.ctx.options(),
            ));
            let mut child =
                Interpreter::new(self.page, self.ctx, parser).with_resource_scope(scope);
            child.execution_budget = work;
            child.optional_content = self.optional_content.clone();
            let mut trace = TraceDevice::for_display_list(limits);
            child.run_content(&mut trace)?;
            if !child.diagnostics.is_empty() {
                return Err(PdfError::UnsupportedFeature("pattern ExtGState".into()));
            }
            state = child.current_state.graphic_state;
        }
        let matrix = pattern
            .matrix
            .checked_mul(&self.initial_ctm)?
            .checked_mul(&self.page.page_matrix)?;
        let object = crate::page::shading_object::ShadingObject::new(
            String::new(),
            matrix,
            pattern.shading,
            state,
        )
        .with_pattern_background();
        let mut trace = TraceDevice::for_display_list(limits);
        trace.start_page(self.page.page_width, self.page.page_height)?;
        trace.do_shading(object)?;
        let (retained_bytes, retained_segments) = trace.pattern_storage()?;
        Ok(PatternPaint {
            list: trace.into_display_list(),
            retained_bytes,
            retained_segments,
            tiling: false,
            tile_matrix: None,
        })
    }

    fn compile_tiling_pattern(
        &self,
        object: &PdfObject,
        pattern: &TilingPattern,
        base: Option<&ColorSpace>,
        tint: &ColorValue,
    ) -> PdfResult<PatternPaint> {
        self.compile_tiling_pattern_impl(object, pattern, base, tint, true)
    }

    fn compile_tiling_pattern_impl(
        &self,
        object: &PdfObject,
        pattern: &TilingPattern,
        base: Option<&ColorSpace>,
        tint: &ColorValue,
        allow_compact: bool,
    ) -> PdfResult<PatternPaint> {
        let limits = self.ctx.limits();
        let budget = self.execution_budget.child(&limits)?;
        if self.pattern_depth + self.form_depth + self.type3_depth >= 128 {
            return Err(PdfError::ResourceLimit(
                "pattern implementation depth (128)".into(),
            ));
        }
        let to_user = pattern.matrix.checked_mul(&self.initial_ctm)?;
        let to_display = to_user.checked_mul(&self.page.page_matrix)?;
        // Nonoverlapping cells can be retained once and repeated by the device.
        // Overlapping/negative-step patterns keep the ordered expansion path.
        let compact = allow_compact
            && pattern.xstep > 0.
            && pattern.ystep > 0.
            && pattern.bbox.llx() >= 0.
            && pattern.bbox.lly() >= 0.
            && pattern.bbox.urx() <= pattern.xstep
            && pattern.bbox.ury() <= pattern.ystep;
        let inverse = to_display.checked_inverse()?;
        let corners = [
            [0., 0.],
            [self.page.page_width, 0.],
            [0., self.page.page_height],
            [self.page.page_width, self.page.page_height],
        ];
        let mut lower = [f64::INFINITY; 2];
        let mut upper = [f64::NEG_INFINITY; 2];
        for [x, y] in corners {
            let p = inverse.checked_transform(&Point::new(x, y))?;
            for (axis, v) in [p.x(), p.y()].into_iter().enumerate() {
                lower[axis] = lower[axis].min(f64::from(v));
                upper[axis] = upper[axis].max(f64::from(v));
            }
        }
        let range = |axis: usize, low: f32, high: f32, step: f32| -> PdfResult<(i64, i64)> {
            let a = (lower[axis] - f64::from(high)) / f64::from(step);
            let b = (upper[axis] - f64::from(low)) / f64::from(step);
            let start = a.min(b).floor();
            let end = a.max(b).ceil();
            if !start.is_finite()
                || !end.is_finite()
                || start.abs() > (1_u64 << 40) as f64
                || end.abs() > (1_u64 << 40) as f64
            {
                return Err(PdfError::ResourceLimit("pattern tile index range".into()));
            }
            Ok((start as i64, end as i64))
        };
        let (mut x0, mut x1) = range(0, pattern.bbox.llx(), pattern.bbox.urx(), pattern.xstep)?;
        let (mut y0, mut y1) = range(1, pattern.bbox.lly(), pattern.bbox.ury(), pattern.ystep)?;
        if compact {
            x0 = 0;
            x1 = 0;
            y0 = 0;
            y1 = 0;
        }
        let count = (x1 - x0 + 1)
            .checked_mul(y1 - y0 + 1)
            .filter(|n| *n >= 0 && *n as u64 <= limits.max_objects as u64)
            .ok_or_else(|| PdfError::ResourceLimit("pattern tile count".into()))?;
        // Count even empty cells; otherwise an empty stream bypasses work bounds.
        for _ in 0..count {
            budget.operation(&limits)?;
        }
        let data = object
            .as_stream()
            .ok_or_else(|| PdfError::ColorError("pattern stream missing".into()))?
            .decode_data(self.ctx)?;
        let mut trace = TraceDevice::for_display_list(limits);
        trace.start_page(
            if compact {
                pattern.xstep
            } else {
                self.page.page_width
            },
            if compact {
                pattern.ystep
            } else {
                self.page.page_height
            },
        )?;
        let mut diagnostics = Vec::new();
        for y in y0..=y1 {
            for x in x0..=x1 {
                budget.decoded(data.len(), &limits)?;
                let translate = Matrix::new(
                    1.,
                    0.,
                    0.,
                    1.,
                    (x as f64 * f64::from(pattern.xstep)) as f32,
                    (y as f64 * f64::from(pattern.ystep)) as f32,
                );
                let ctm = if compact {
                    self.page.page_matrix.checked_inverse()?
                } else {
                    translate.checked_mul(&to_user)?
                };
                let parser = ContentParser::new(crate::parser::syntax::SyntaxParser::with_options(
                    crate::io::stream_reader::StreamReader::try_new(std::io::Cursor::new(
                        data.clone(),
                    ))?,
                    self.ctx.options(),
                ));
                let mut child = Interpreter::new(self.page, self.ctx, parser)
                    .with_resource_scope(pattern.resources.clone());
                child.execution_budget = budget.clone();
                child.optional_content = self.optional_content.clone();
                child.pattern_depth = self.pattern_depth + 1;
                child.form_depth = self.form_depth + 1; // Never expose cell paths as page-edit provenance.
                child.type3_depth = self.type3_depth;
                child.pattern_uncoloured = pattern.paint_type == 2;
                child.initial_ctm = ctm.clone();
                child.current_state = self.current_state.clone();
                child.current_state.ctm = ctm.clone();
                child.current_state.graphic_state.clip_path = Default::default();
                child.current_state.graphic_state.general_state = Default::default();
                child.current_state.graphic_state.color_state = Default::default();
                if let Some(base) = base {
                    let colors = &mut child.current_state.graphic_state.color_state;
                    colors.fill_color_space = base.clone();
                    colors.stroke_color_space = base.clone();
                    colors.fill_color_value = tint.clone();
                    colors.stroke_color_value = tint.clone();
                }
                let cell = super::super::pdf_form::PdfForm {
                    bounds: pattern.bbox.clone(),
                    matrix: Matrix::identity(),
                    resources: None,
                    group: None,
                };
                child
                    .current_state
                    .graphic_state
                    .clip_path
                    .try_add_element(
                        cell.bbox_clip_element(&ctm, &self.page.page_matrix, &limits)?,
                        &limits,
                    )?;
                child.run_content(&mut trace)?;
                if diagnostics.is_empty() {
                    diagnostics.extend(child.diagnostics);
                }
                // Cell diagnostics remain attached to its immutable snapshot.
            }
        }
        trace.end_page()?;
        let (retained_bytes, retained_segments) = trace.pattern_storage()?;
        let mut list = trace.into_display_list();
        list.set_diagnostics(diagnostics);
        if compact
            && list.objects().iter().any(|object| {
                use crate::page::{general_state::BlendMode, page_object::PageObject};
                let state = match object {
                    PageObject::Path(v) => &v.graphic_state().general_state,
                    PageObject::Text(v) => &v.state.general_state,
                    PageObject::Image(v) => &v.graphic_state.general_state,
                    PageObject::Shading(v) => &v.graphic_state.general_state,
                    PageObject::GroupBegin(v) => {
                        return !v.isolated || v.blend_mode != BlendMode::Normal;
                    }
                    _ => return false,
                };
                state.blend_mode != BlendMode::Normal
            })
        {
            // Blending inside a cell must see the page's current backdrop.
            // Keep the existing ordered path for this less common case.
            return self.compile_tiling_pattern_impl(object, pattern, base, tint, false);
        }
        Ok(PatternPaint {
            list,
            retained_bytes,
            retained_segments,
            tiling: true,
            tile_matrix: compact.then_some(to_display),
        })
    }
}
