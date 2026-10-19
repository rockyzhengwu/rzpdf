//! Unhinted default-instance point expansion, preserving fractional transforms.
use super::{
    composite_glyf::{ComponentArguments, GlyphComponent},
    loca::GlyphLocations,
};
use crate::{
    error::{PdfError, PdfResult},
    parser::options::Limits,
};

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ExpandedPoint {
    pub position: [f64; 2],
    pub on_curve: bool,
}

#[derive(Debug, Default, PartialEq)]
pub struct ExpandedGlyph {
    pub points: Vec<ExpandedPoint>,
    pub contour_end_points: Vec<usize>,
}

fn charge(bytes: &mut usize, count: usize, size: usize, limits: &Limits) -> PdfResult<()> {
    *bytes = count
        .checked_mul(size)
        .and_then(|n| bytes.checked_add(n))
        .ok_or_else(|| PdfError::ResourceLimit("Expanded glyph budget overflow".into()))?;
    if *bytes > limits.max_decoded_bytes {
        return Err(PdfError::ResourceLimit(
            "Expanded glyph cumulative storage".into(),
        ));
    }
    Ok(())
}

impl ExpandedGlyph {
    /// Closed quadratic contours in design units; no rounding or flattening.
    pub fn outline_commands(
        &self,
        limits: &Limits,
    ) -> PdfResult<Vec<super::glyf::GlyphOutlineCommand>> {
        use super::glyf::GlyphOutlineCommand as Command;
        let mut previous = None;
        for &end in &self.contour_end_points {
            if end >= self.points.len() || previous.is_some_and(|p| p >= end) {
                return Err(PdfError::FontError("Invalid expanded contour range".into()));
            }
            previous = Some(end);
        }
        if previous.map_or(0, |end| end + 1) != self.points.len()
            || self
                .points
                .iter()
                .any(|p| !p.position.iter().all(|v| v.is_finite()))
        {
            return Err(PdfError::FontError(
                "Invalid expanded outline points".into(),
            ));
        }
        let capacity = self
            .contour_end_points
            .len()
            .checked_mul(2)
            .and_then(|extra| self.points.len().checked_add(extra))
            .ok_or_else(|| PdfError::ResourceLimit("Expanded outline size overflow".into()))?;
        if self.points.len() > limits.max_objects
            || self.contour_end_points.len() > limits.max_objects
            || capacity > limits.max_path_segments
        {
            return Err(PdfError::ResourceLimit("Expanded outline commands".into()));
        }
        let mut bytes = 0;
        charge(
            &mut bytes,
            self.points.len(),
            std::mem::size_of::<ExpandedPoint>(),
            limits,
        )?;
        charge(
            &mut bytes,
            self.contour_end_points.len(),
            std::mem::size_of::<usize>(),
            limits,
        )?;
        charge(&mut bytes, capacity, std::mem::size_of::<Command>(), limits)?;
        let mut commands = Vec::new();
        commands
            .try_reserve_exact(capacity)
            .map_err(|_| PdfError::ResourceLimit("Expanded outline allocation".into()))?;
        let midpoint = |a: ExpandedPoint, b: ExpandedPoint| {
            [
                a.position[0] * 0.5 + b.position[0] * 0.5,
                a.position[1] * 0.5 + b.position[1] * 0.5,
            ]
        };
        let mut begin = 0;
        for &end in &self.contour_end_points {
            let contour = &self.points[begin..=end];
            let first = contour[0];
            let last = contour[contour.len() - 1];
            let (start, mut index, stop) = if first.on_curve {
                (first.position, 1, contour.len())
            } else if last.on_curve {
                (last.position, 0, contour.len() - 1)
            } else {
                (midpoint(first, last), 0, contour.len())
            };
            commands.push(Command::Move(start));
            while index < stop {
                let point = contour[index];
                if point.on_curve {
                    commands.push(Command::Line(point.position));
                    index += 1;
                } else if index + 1 == stop {
                    commands.push(Command::Quadratic {
                        control: point.position,
                        end: start,
                    });
                    index += 1;
                } else {
                    let next = contour[index + 1];
                    commands.push(Command::Quadratic {
                        control: point.position,
                        end: if next.on_curve {
                            next.position
                        } else {
                            midpoint(point, next)
                        },
                    });
                    index += if next.on_curve { 2 } else { 1 };
                }
            }
            commands.push(Command::Close);
            begin = end + 1;
        }
        Ok(commands)
    }

    fn append(
        &mut self,
        mut child: Self,
        component: GlyphComponent,
        bytes: &mut usize,
        limits: &Limits,
    ) -> PdfResult<()> {
        let [a, b, c, d] = component.transform;
        let transform = |[x, y]: [f64; 2]| [a * x + c * y, b * x + d * y];
        for point in &mut child.points {
            point.position = transform(point.position);
        }
        let offset = match component.arguments {
            ComponentArguments::Offset([x, y]) => {
                let offset = [f64::from(x), f64::from(y)];
                // With neither offset flag, use the recommended unscaled default.
                if component.flags & 0x800 != 0 {
                    transform(offset)
                } else {
                    offset
                }
            }
            ComponentArguments::Points([parent, child_index]) => {
                let p = self.points.get(usize::from(parent)).ok_or_else(|| {
                    PdfError::UnsupportedFeature(
                        "Composite parent point/phantom point unavailable".into(),
                    )
                })?;
                let q = child.points.get(usize::from(child_index)).ok_or_else(|| {
                    PdfError::UnsupportedFeature(
                        "Composite child point/phantom point unavailable".into(),
                    )
                })?;
                [p.position[0] - q.position[0], p.position[1] - q.position[1]]
            }
        };
        let base = self.points.len();
        let count = base
            .checked_add(child.points.len())
            .ok_or_else(|| PdfError::ResourceLimit("Expanded point count overflow".into()))?;
        let contours = self
            .contour_end_points
            .len()
            .checked_add(child.contour_end_points.len())
            .ok_or_else(|| PdfError::ResourceLimit("Expanded contour count overflow".into()))?;
        if count > limits.max_path_segments
            || count > limits.max_objects
            || contours > limits.max_objects
        {
            return Err(PdfError::ResourceLimit(
                "Expanded glyph points/contours".into(),
            ));
        }
        charge(
            bytes,
            child.points.len(),
            std::mem::size_of::<ExpandedPoint>(),
            limits,
        )?;
        charge(
            bytes,
            child.contour_end_points.len(),
            std::mem::size_of::<usize>(),
            limits,
        )?;
        self.points
            .try_reserve(child.points.len())
            .map_err(|_| PdfError::ResourceLimit("Expanded point allocation".into()))?;
        self.contour_end_points
            .try_reserve(child.contour_end_points.len())
            .map_err(|_| PdfError::ResourceLimit("Expanded contour allocation".into()))?;
        for point in &mut child.points {
            point.position[0] += offset[0];
            point.position[1] += offset[1];
            if !point.position.iter().all(|v| v.is_finite()) {
                return Err(PdfError::FontError("Non-finite composite transform".into()));
            }
        }
        for end in child.contour_end_points {
            self.contour_end_points
                .push(base.checked_add(end).ok_or_else(|| {
                    PdfError::ResourceLimit("Expanded contour index overflow".into())
                })?);
        }
        self.points.extend(child.points);
        Ok(())
    }
}

impl GlyphLocations<'_> {
    /// Expand real control points only. No phantom points, grid rounding,
    /// hinting, metrics selection or variable-instance deltas are applied.
    pub fn expanded_glyph(&self, gid: u16, limits: &Limits) -> PdfResult<ExpandedGlyph> {
        self.validate_composite_dependencies(gid, limits)?;
        struct Frame {
            components: Vec<GlyphComponent>,
            next: usize,
            output: ExpandedGlyph,
        }
        let mut stack: Vec<Frame> = Vec::new();
        let mut pending = Some(gid);
        let mut completed = None;
        let mut bytes = 0;
        loop {
            if let Some(id) = pending.take() {
                let data = self.glyph_data(id)?;
                charge(&mut bytes, data.len(), 1, limits)?;
                if self.glyph_header(id)?.is_some_and(|h| h.is_composite()) {
                    let components = self.composite_glyph(id, limits)?.unwrap().components;
                    charge(
                        &mut bytes,
                        components.len(),
                        std::mem::size_of::<GlyphComponent>(),
                        limits,
                    )?;
                    charge(&mut bytes, 1, std::mem::size_of::<Frame>(), limits)?;
                    stack
                        .try_reserve(1)
                        .map_err(|_| PdfError::ResourceLimit("Expanded stack allocation".into()))?;
                    stack.push(Frame {
                        components,
                        next: 0,
                        output: ExpandedGlyph::default(),
                    });
                } else {
                    let mut output = ExpandedGlyph::default();
                    if let Some(simple) = self.simple_glyph(id, limits)? {
                        charge(
                            &mut bytes,
                            simple.points.len(),
                            std::mem::size_of::<super::glyf::GlyphPoint>()
                                + std::mem::size_of::<ExpandedPoint>()
                                + 1,
                            limits,
                        )?;
                        charge(
                            &mut bytes,
                            simple.contour_end_points.len(),
                            std::mem::size_of::<u16>() + std::mem::size_of::<usize>(),
                            limits,
                        )?;
                        output
                            .points
                            .try_reserve(simple.points.len())
                            .map_err(|_| {
                                PdfError::ResourceLimit("Expanded leaf allocation".into())
                            })?;
                        output
                            .contour_end_points
                            .try_reserve(simple.contour_end_points.len())
                            .map_err(|_| {
                                PdfError::ResourceLimit("Expanded leaf contour allocation".into())
                            })?;
                        output
                            .points
                            .extend(simple.points.into_iter().map(|p| ExpandedPoint {
                                position: [f64::from(p.x), f64::from(p.y)],
                                on_curve: p.on_curve,
                            }));
                        output
                            .contour_end_points
                            .extend(simple.contour_end_points.into_iter().map(usize::from));
                    }
                    completed = Some(output);
                }
            }
            if let Some(child) = completed.take() {
                if let Some(parent) = stack.last_mut() {
                    parent.output.append(
                        child,
                        parent.components[parent.next - 1],
                        &mut bytes,
                        limits,
                    )?;
                } else {
                    return Ok(child);
                }
            }
            let frame = stack
                .last_mut()
                .ok_or_else(|| PdfError::FontError("Missing composite expansion frame".into()))?;
            if let Some(component) = frame.components.get(frame.next) {
                pending = Some(component.glyph_id);
                frame.next += 1;
            } else {
                completed = Some(stack.pop().unwrap().output);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn expanded_quadratics_keep_fractional_midpoints_and_validate_public_data() {
        use super::super::glyf::GlyphOutlineCommand as C;
        let mut glyph = ExpandedGlyph {
            points: vec![
                ExpandedPoint {
                    position: [0.25, 0.5],
                    on_curve: false,
                },
                ExpandedPoint {
                    position: [2.75, 4.5],
                    on_curve: false,
                },
            ],
            contour_end_points: vec![1],
        };
        let limits = Limits::default();
        assert_eq!(
            glyph.outline_commands(&limits).unwrap(),
            [
                C::Move([1.5, 2.5]),
                C::Quadratic {
                    control: [0.25, 0.5],
                    end: [1.5, 2.5]
                },
                C::Quadratic {
                    control: [2.75, 4.5],
                    end: [1.5, 2.5]
                },
                C::Close,
            ]
        );
        assert!(
            glyph
                .outline_commands(&Limits {
                    max_path_segments: 3,
                    ..limits
                })
                .is_err()
        );
        assert!(
            glyph
                .outline_commands(&Limits {
                    max_decoded_bytes: 1,
                    ..limits
                })
                .is_err()
        );
        glyph.contour_end_points = vec![0];
        assert!(glyph.outline_commands(&limits).is_err());
        glyph.contour_end_points = vec![1, 1];
        assert!(glyph.outline_commands(&limits).is_err());
        glyph.contour_end_points = vec![1];
        glyph.points[0].position[0] = f64::NAN;
        assert!(glyph.outline_commands(&limits).is_err());
        assert!(
            ExpandedGlyph::default()
                .outline_commands(&limits)
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn nested_records_expand_in_order_and_bound_work() {
        let header = |contours: i16| {
            [contours, 0, 0, 20, 20]
                .into_iter()
                .flat_map(i16::to_be_bytes)
                .collect::<Vec<_>>()
        };
        let mut root = header(-1);
        for word in [43_u16, 1, 3, 5, 8192, 1, 2, 1, 0] {
            root.extend(word.to_be_bytes());
        }
        let mut nested = header(-1);
        for word in [3_u16, 2, 2, 4] {
            nested.extend(word.to_be_bytes());
        }
        let mut leaf = header(1);
        leaf.extend([0, 2, 0, 0, 1, 1, 1]); // contour end, instruction length, flags
        for delta in [0_i16, 10, -10, 0, 0, 10] {
            leaf.extend(delta.to_be_bytes());
        }
        let mut glyf = Vec::new();
        let mut loca = 0_u32.to_be_bytes().to_vec();
        for glyph in [&root, &nested, &leaf, &vec![]] {
            glyf.extend(glyph);
            loca.extend((glyf.len() as u32).to_be_bytes());
        }
        let locations = GlyphLocations::try_new(&loca, &glyf, 4, 1).unwrap();
        let limits = Limits::default();
        let output = locations.expanded_glyph(0, &limits).unwrap();
        assert_eq!(
            output.points.iter().map(|p| p.position).collect::<Vec<_>>(),
            [
                [4., 7.],
                [9., 7.],
                [4., 12.],
                [9., 7.],
                [19., 7.],
                [9., 17.]
            ]
        );
        assert!(output.points.iter().all(|p| p.on_curve));
        assert_eq!(output.contour_end_points, [2, 5]);
        let commands = output.outline_commands(&limits).unwrap();
        assert_eq!(commands.len(), 8);
        assert_eq!(
            super::super::glyf::outline_bounds(&commands, &limits).unwrap(),
            Some([4., 7., 19., 17.])
        );
        assert_eq!(
            locations.expanded_glyph(3, &limits).unwrap(),
            ExpandedGlyph::default()
        );
        for bounded in [
            Limits {
                max_depth: 2,
                ..limits
            },
            Limits {
                max_path_segments: 5,
                ..limits
            },
            Limits {
                max_decoded_bytes: 300,
                ..limits
            },
        ] {
            assert!(matches!(
                locations.expanded_glyph(0, &bounded),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        // Change the second component's parent point, then create a nested cycle.
        glyf[24..26].copy_from_slice(&99_u16.to_be_bytes());
        assert!(
            GlyphLocations::try_new(&loca, &glyf, 4, 1)
                .unwrap()
                .expanded_glyph(0, &limits)
                .is_err()
        );
        let nested_offset = root.len();
        glyf[nested_offset + 12..nested_offset + 14].copy_from_slice(&0_u16.to_be_bytes());
        assert!(matches!(
            GlyphLocations::try_new(&loca, &glyf, 4, 1)
                .unwrap()
                .expanded_glyph(0, &limits),
            Err(PdfError::FontError(_))
        ));
    }
    fn child() -> ExpandedGlyph {
        ExpandedGlyph {
            points: vec![ExpandedPoint {
                position: [2., 4.],
                on_curve: false,
            }],
            contour_end_points: vec![0],
        }
    }
    #[test]
    fn transforms_offsets_and_point_alignment_preserve_fractional_points() {
        let limits = Limits::default();
        let component = GlyphComponent {
            flags: 2,
            glyph_id: 0,
            arguments: ComponentArguments::Offset([3, 5]),
            transform: [0.5, 0.25, -0.5, 1.],
        };
        let mut output = ExpandedGlyph::default();
        output.append(child(), component, &mut 0, &limits).unwrap();
        assert_eq!(output.points[0].position, [2., 9.5]);
        assert!(!output.points[0].on_curve);
        output
            .append(
                child(),
                GlyphComponent {
                    arguments: ComponentArguments::Points([0, 0]),
                    ..component
                },
                &mut 0,
                &limits,
            )
            .unwrap();
        assert_eq!(output.points[1].position, output.points[0].position);
        assert_eq!(output.contour_end_points, [0, 1]);
        let mut scaled = ExpandedGlyph::default();
        scaled
            .append(
                child(),
                GlyphComponent {
                    flags: 0x802,
                    ..component
                },
                &mut 0,
                &limits,
            )
            .unwrap();
        assert_eq!(scaled.points[0].position, [-2., 10.25]);
        assert!(
            output
                .append(
                    child(),
                    GlyphComponent {
                        arguments: ComponentArguments::Points([99, 0]),
                        ..component
                    },
                    &mut 0,
                    &limits
                )
                .is_err()
        );
        assert!(
            output
                .append(
                    child(),
                    component,
                    &mut 0,
                    &Limits {
                        max_path_segments: 2,
                        ..limits
                    }
                )
                .is_err()
        );
    }
}
