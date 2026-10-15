use super::*;
use crate::font::truetype::tables::glyf::GlyphOutlineCommand;
use crate::page::text_state::TextRenderingMode;

impl<R: Read + Seek> Interpreter<'_, R> {
    pub(super) fn accumulate_text_clip(&mut self, text: &mut TextObject) -> PdfResult<()> {
        if !matches!(
            text.state.text_state.text_rendering_mode,
            TextRenderingMode::FillClip
                | TextRenderingMode::StrokeClip
                | TextRenderingMode::FillStrokeClip
                | TextRenderingMode::Clip
        ) {
            return Ok(());
        }
        let limits = self.ctx.limits();
        let mut addition = PdfPath::default();
        let mut total = self.pending_text_clip.as_ref().map_or(0, |path| {
            path.subpaths()
                .iter()
                .map(|p| p.segments().len())
                .sum::<usize>()
        });
        for index in 0..text.items.len() {
            let commands = text.character_outline_display(index, limits)?;
            let count = commands.len();
            self.execution_budget.path_segments(count, &limits)?;
            total = total
                .checked_add(count)
                .filter(|v| {
                    *v <= limits.max_path_segments
                        && v.checked_mul(std::mem::size_of::<Segment>())
                            .is_some_and(|bytes| bytes <= limits.max_decoded_bytes)
                })
                .ok_or_else(|| PdfError::ResourceLimit("accumulated text clip segments".into()))?;
            let point = |p: [f64; 2]| -> PdfResult<Point> {
                let [x, y] = p.map(|v| v as f32);
                if !x.is_finite() || !y.is_finite() {
                    return Err(PdfError::PageOperatorError(
                        "text clip coordinate overflow".into(),
                    ));
                }
                Ok(Point::new(x, y))
            };
            let mut segments = Vec::new();
            let mut current = [0.; 2];
            for command in commands {
                if matches!(command, GlyphOutlineCommand::Move(_)) && !segments.is_empty() {
                    addition.try_add_subpath(SubPath::new(std::mem::take(&mut segments), true))?;
                }
                segments
                    .try_reserve(1)
                    .map_err(|_| PdfError::ResourceLimit("text clip contour allocation".into()))?;
                match command {
                    GlyphOutlineCommand::Move(p) => {
                        segments.push(Segment::MoveTo(point(p)?));
                        current = p;
                    }
                    GlyphOutlineCommand::Line(p) => {
                        segments.push(Segment::LineTo(point(p)?));
                        current = p;
                    }
                    GlyphOutlineCommand::Quadratic { control, end } => {
                        let first = std::array::from_fn(|i| {
                            current[i] + (control[i] - current[i]) * 2. / 3.
                        });
                        let second =
                            std::array::from_fn(|i| end[i] + (control[i] - end[i]) * 2. / 3.);
                        segments.push(Segment::CurveTo(Bezier::new(
                            point(first)?,
                            point(second)?,
                            point(end)?,
                        )));
                        current = end;
                    }
                    GlyphOutlineCommand::Close => segments.push(Segment::Close),
                }
            }
            if !segments.is_empty() {
                addition.try_add_subpath(SubPath::new(segments, true))?;
            }
        }
        let pending = self.pending_text_clip.get_or_insert_with(PdfPath::default);
        for subpath in addition.into_subpaths() {
            pending.try_add_subpath(subpath)?;
        }
        text.mark_clipping_interpreted();
        Ok(())
    }

    pub(super) fn finish_text_clip(&mut self) -> PdfResult<()> {
        if let Some(path) = self.pending_text_clip.take() {
            self.current_state.graphic_state.clip_path.try_add_element(
                crate::page::clip_path::ClipElement::new(
                    path,
                    FillType::Winding,
                    Matrix::identity(),
                ),
                &self.ctx.limits(),
            )?;
        }
        Ok(())
    }
}
