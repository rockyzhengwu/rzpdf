//! Simple TrueType glyph point decoding; no hinting or composite expansion.
use super::loca::GlyphHeader;
use crate::{
    error::{PdfError, PdfResult},
    parser::options::Limits,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GlyphPoint {
    pub x: i32,
    pub y: i32,
    pub on_curve: bool,
}

/// Unhinted outline in font design units; quadratic curves remain exact.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GlyphOutlineCommand {
    Move([f64; 2]),
    Line([f64; 2]),
    Quadratic { control: [f64; 2], end: [f64; 2] },
    Close,
}

/// Tight axis-aligned bounds of lines/quadratics, including curve extrema.
/// None denotes no commands, not an invisible glyph. No fill/hinting analysis.
pub fn outline_bounds(
    commands: &[GlyphOutlineCommand],
    limits: &Limits,
) -> PdfResult<Option<[f64; 4]>> {
    if commands.len() > limits.max_path_segments {
        return Err(PdfError::ResourceLimit("Glyph bounds commands".into()));
    }
    let mut bounds: Option<[f64; 4]> = None;
    let mut current: Option<[f64; 2]> = None;
    let mut include = |point: [f64; 2]| -> PdfResult<()> {
        if !point.iter().all(|value| value.is_finite()) {
            return Err(PdfError::FontError("Non-finite outline bounds".into()));
        }
        match &mut bounds {
            Some(b) => {
                b[0] = b[0].min(point[0]);
                b[1] = b[1].min(point[1]);
                b[2] = b[2].max(point[0]);
                b[3] = b[3].max(point[1]);
            }
            None => bounds = Some([point[0], point[1], point[0], point[1]]),
        }
        Ok(())
    };
    for command in commands {
        match *command {
            GlyphOutlineCommand::Move(point) => {
                if current.is_some() {
                    return Err(PdfError::FontError("Unclosed outline contour".into()));
                }
                include(point)?;
                current = Some(point);
            }
            GlyphOutlineCommand::Line(point) => {
                if current.is_none() {
                    return Err(PdfError::FontError("Outline line without move".into()));
                }
                include(point)?;
                current = Some(point);
            }
            GlyphOutlineCommand::Quadratic { control, end } => {
                let start = current
                    .ok_or_else(|| PdfError::FontError("Outline curve without move".into()))?;
                if !control
                    .iter()
                    .chain(end.iter())
                    .all(|value| value.is_finite())
                {
                    return Err(PdfError::FontError("Non-finite outline curve".into()));
                }
                include(end)?;
                for axis in 0..2 {
                    // Scale first so extreme finite public coordinates cannot overflow
                    // the derivative denominator. Default glyph coordinates are small.
                    let scale = start[axis]
                        .abs()
                        .max(control[axis].abs())
                        .max(end[axis].abs());
                    if scale == 0. {
                        continue;
                    }
                    let (a, b, c) = (
                        start[axis] / scale,
                        control[axis] / scale,
                        end[axis] / scale,
                    );
                    let denominator = a - 2. * b + c;
                    if denominator == 0. {
                        continue;
                    }
                    let t = (a - b) / denominator;
                    if t > 0. && t < 1. {
                        let u = 1. - t;
                        let mut point = [0.; 2];
                        for coordinate in 0..2 {
                            // Convex weights; do not subtract huge endpoints.
                            point[coordinate] = (u * u) * start[coordinate]
                                + (2. * u * t) * control[coordinate]
                                + (t * t) * end[coordinate];
                        }
                        include(point)?;
                    }
                }
                current = Some(end);
            }
            GlyphOutlineCommand::Close => {
                if current.take().is_none() {
                    return Err(PdfError::FontError("Outline close without move".into()));
                }
            }
        }
    }
    if current.is_some() {
        return Err(PdfError::FontError("Unclosed outline contour".into()));
    }
    Ok(bounds)
}

#[derive(Debug)]
pub struct SimpleGlyph<'a> {
    pub header: GlyphHeader,
    pub contour_end_points: Vec<u16>,
    pub points: Vec<GlyphPoint>,
    /// Borrowed, not executed; phantom points and hinting are not applied.
    pub instructions: &'a [u8],
}

impl<'a> SimpleGlyph<'a> {
    /// Build closed contours with implicit midpoints between off-curve points.
    /// No hinting, winding hit test, composite expansion or variable deltas.
    pub fn outline_commands(&self, limits: &Limits) -> PdfResult<Vec<GlyphOutlineCommand>> {
        if self.header.number_of_contours < 0
            || self.header.number_of_contours as usize != self.contour_end_points.len()
        {
            return Err(PdfError::FontError(
                "Inconsistent simple glyph contours".into(),
            ));
        }
        let mut previous = None;
        for &end in &self.contour_end_points {
            if usize::from(end) >= self.points.len() || previous.is_some_and(|p| p >= end) {
                return Err(PdfError::FontError("Invalid outline contour range".into()));
            }
            previous = Some(end);
        }
        if previous.map_or(0, |end| usize::from(end) + 1) != self.points.len() {
            return Err(PdfError::FontError("Outline has unassigned points".into()));
        }
        // At most one line/curve per point, plus move and close per contour.
        let capacity = self
            .contour_end_points
            .len()
            .checked_mul(2)
            .and_then(|extra| extra.checked_add(self.points.len()))
            .ok_or_else(|| PdfError::ResourceLimit("Glyph outline size overflow".into()))?;
        if self.points.len() > limits.max_objects
            || capacity > limits.max_path_segments
            || capacity
                .checked_mul(std::mem::size_of::<GlyphOutlineCommand>())
                .is_none_or(|bytes| bytes > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit(
                "Glyph outline commands/storage".into(),
            ));
        }
        let mut commands = Vec::new();
        commands
            .try_reserve_exact(capacity)
            .map_err(|_| PdfError::ResourceLimit("Glyph outline allocation".into()))?;
        let position = |p: GlyphPoint| [f64::from(p.x), f64::from(p.y)];
        let midpoint = |a: GlyphPoint, b: GlyphPoint| {
            [
                (f64::from(a.x) + f64::from(b.x)) / 2.,
                (f64::from(a.y) + f64::from(b.y)) / 2.,
            ]
        };
        let mut begin = 0;
        for &end in &self.contour_end_points {
            let contour = &self.points[begin..=usize::from(end)];
            let first = contour[0];
            let last = contour[contour.len() - 1];
            let (start, mut index, stop) = if first.on_curve {
                (position(first), 1, contour.len())
            } else if last.on_curve {
                (position(last), 0, contour.len() - 1)
            } else {
                (midpoint(first, last), 0, contour.len())
            };
            commands.push(GlyphOutlineCommand::Move(start));
            while index < stop {
                let point = contour[index];
                if point.on_curve {
                    commands.push(GlyphOutlineCommand::Line(position(point)));
                    index += 1;
                } else if index + 1 == stop {
                    commands.push(GlyphOutlineCommand::Quadratic {
                        control: position(point),
                        end: start,
                    });
                    index += 1;
                } else {
                    let next = contour[index + 1];
                    let endpoint = if next.on_curve {
                        position(next)
                    } else {
                        midpoint(point, next)
                    };
                    commands.push(GlyphOutlineCommand::Quadratic {
                        control: position(point),
                        end: endpoint,
                    });
                    index += if next.on_curve { 2 } else { 1 };
                }
            }
            commands.push(GlyphOutlineCommand::Close);
            begin = usize::from(end) + 1;
        }
        Ok(commands)
    }

    pub fn try_new(data: &'a [u8], limits: &Limits) -> PdfResult<Self> {
        let header = GlyphHeader::from_prefix(data)?;
        if header.is_composite() {
            return Err(PdfError::UnsupportedFeature(
                "Composite glyph point decoding".into(),
            ));
        }
        let contours = header.number_of_contours as usize;
        if data.len() > limits.max_decoded_bytes || contours > limits.max_objects {
            return Err(PdfError::ResourceLimit(
                "Simple glyph input/contours".into(),
            ));
        }
        let mut offset = 10usize;
        let take = |offset: &mut usize, size: usize| -> PdfResult<&'a [u8]> {
            let end = offset
                .checked_add(size)
                .ok_or_else(|| PdfError::FontError("Glyph offset overflow".into()))?;
            let slice = data
                .get(*offset..end)
                .ok_or_else(|| PdfError::FontError("Truncated simple glyph".into()))?;
            *offset = end;
            Ok(slice)
        };
        let mut ends = Vec::new();
        if contours
            .checked_mul(2)
            .is_none_or(|bytes| bytes > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit("Simple glyph contour bytes".into()));
        }
        ends.try_reserve_exact(contours)
            .map_err(|_| PdfError::ResourceLimit("Glyph contours allocation".into()))?;
        for _ in 0..contours {
            let bytes = take(&mut offset, 2)?;
            let end = u16::from_be_bytes([bytes[0], bytes[1]]);
            if ends.last().is_some_and(|previous| *previous >= end) {
                return Err(PdfError::FontError(
                    "Glyph contour ends must increase".into(),
                ));
            }
            ends.push(end);
        }
        let count = ends.last().map_or(0, |end| usize::from(*end) + 1);
        if count > limits.max_path_segments
            || count > limits.max_objects
            || count
                .checked_mul(std::mem::size_of::<GlyphPoint>() + 1)
                .and_then(|bytes| bytes.checked_add(contours * 2))
                .is_none_or(|bytes| bytes > limits.max_decoded_bytes)
        {
            return Err(PdfError::ResourceLimit(
                "Simple glyph points/storage".into(),
            ));
        }
        // A zero-contour glyph can end immediately after its header.
        let instructions = if contours == 0 && offset == data.len() {
            &data[offset..offset]
        } else {
            let bytes = take(&mut offset, 2)?;
            take(
                &mut offset,
                usize::from(u16::from_be_bytes([bytes[0], bytes[1]])),
            )?
        };
        let mut flags = Vec::new();
        flags
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("Glyph flags allocation".into()))?;
        while flags.len() < count {
            let flag = take(&mut offset, 1)?[0];
            if flag & 0x80 != 0 {
                return Err(PdfError::FontError("Reserved glyph flag".into()));
            }
            let repeat = if flag & 8 != 0 {
                usize::from(take(&mut offset, 1)?[0]) + 1
            } else {
                1
            };
            if repeat > count - flags.len() {
                return Err(PdfError::FontError(
                    "Glyph flag repeat exceeds points".into(),
                ));
            }
            flags.extend(std::iter::repeat_n(flag, repeat));
        }
        let mut points = Vec::new();
        points
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("Glyph points allocation".into()))?;
        points.extend(flags.iter().map(|flag| GlyphPoint {
            x: 0,
            y: 0,
            on_curve: flag & 1 != 0,
        }));
        for (short_bit, same_bit, x_axis) in [(2, 16, true), (4, 32, false)] {
            let mut coordinate = 0_i32;
            for (point, flag) in points.iter_mut().zip(&flags) {
                let delta = if flag & short_bit != 0 {
                    let value = i32::from(take(&mut offset, 1)?[0]);
                    if flag & same_bit != 0 { value } else { -value }
                } else if flag & same_bit != 0 {
                    0
                } else {
                    let bytes = take(&mut offset, 2)?;
                    i32::from(i16::from_be_bytes([bytes[0], bytes[1]]))
                };
                coordinate = coordinate
                    .checked_add(delta)
                    .ok_or_else(|| PdfError::FontError("Glyph coordinate overflow".into()))?;
                if x_axis {
                    point.x = coordinate;
                } else {
                    point.y = coordinate;
                }
            }
        }
        Ok(Self {
            header,
            contour_end_points: ends,
            points,
            instructions,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn quadratic_bounds_use_extrema_not_the_control_point_box() {
        use GlyphOutlineCommand::{Close, Line, Move, Quadratic};
        let commands = [
            Move([0., 0.]),
            Quadratic {
                control: [10., 20.],
                end: [20., 0.],
            },
            Close,
        ];
        assert_eq!(
            outline_bounds(&commands, &Limits::default()).unwrap(),
            Some([0., 0., 20., 10.])
        );
        let horizontal = [
            Move([0., 0.]),
            Quadratic {
                control: [20., 10.],
                end: [0., 20.],
            },
            Close,
        ];
        assert_eq!(
            outline_bounds(&horizontal, &Limits::default()).unwrap(),
            Some([0., 0., 10., 20.])
        );
        assert_eq!(outline_bounds(&[], &Limits::default()).unwrap(), None);
        for invalid in [
            vec![Line([0., 0.])],
            vec![Close],
            vec![Move([0., 0.])],
            vec![Move([0., 0.]), Move([1., 1.]), Close],
            vec![
                Move([0., 0.]),
                Quadratic {
                    control: [f64::NAN, 1.],
                    end: [2., 2.],
                },
                Close,
            ],
        ] {
            assert!(outline_bounds(&invalid, &Limits::default()).is_err());
        }
        assert!(
            outline_bounds(
                &commands,
                &Limits {
                    max_path_segments: 2,
                    ..Limits::default()
                }
            )
            .is_err()
        );
        let extreme = [
            Move([1e300, 0.]),
            Quadratic {
                control: [-1e300, 1e300],
                end: [1e300, 0.],
            },
            Close,
        ];
        let bounds = outline_bounds(&extreme, &Limits::default())
            .unwrap()
            .unwrap();
        assert_eq!(bounds, [0., 0., 1e300, 5e299]);
    }
    #[test]
    fn outline_closes_contours_and_inserts_exact_off_curve_midpoints() {
        use GlyphOutlineCommand::{Close, Line, Move, Quadratic};
        let make = |points: &[(i32, i32, bool)]| SimpleGlyph {
            header: GlyphHeader {
                number_of_contours: 1,
                bounds: [0, 0, 100, 100],
            },
            contour_end_points: vec![points.len() as u16 - 1],
            points: points
                .iter()
                .map(|&(x, y, on_curve)| GlyphPoint { x, y, on_curve })
                .collect(),
            instructions: &[],
        };
        for (points, expected) in [
            (
                vec![(0, 0, true), (10, 0, true), (10, 10, true)],
                vec![Move([0., 0.]), Line([10., 0.]), Line([10., 10.]), Close],
            ),
            (
                vec![(0, 0, true), (10, 20, false), (20, 0, true)],
                vec![
                    Move([0., 0.]),
                    Quadratic {
                        control: [10., 20.],
                        end: [20., 0.],
                    },
                    Close,
                ],
            ),
            (
                vec![(0, 0, false), (11, 21, false), (20, 0, true)],
                vec![
                    Move([20., 0.]),
                    Quadratic {
                        control: [0., 0.],
                        end: [5.5, 10.5],
                    },
                    Quadratic {
                        control: [11., 21.],
                        end: [20., 0.],
                    },
                    Close,
                ],
            ),
            (
                vec![(0, 0, false), (11, 21, false)],
                vec![
                    Move([5.5, 10.5]),
                    Quadratic {
                        control: [0., 0.],
                        end: [5.5, 10.5],
                    },
                    Quadratic {
                        control: [11., 21.],
                        end: [5.5, 10.5],
                    },
                    Close,
                ],
            ),
            (
                vec![(0, 0, true), (10, 20, false)],
                vec![
                    Move([0., 0.]),
                    Quadratic {
                        control: [10., 20.],
                        end: [0., 0.],
                    },
                    Close,
                ],
            ),
        ] {
            let glyph = make(&points);
            assert_eq!(
                glyph.outline_commands(&Limits::default()).unwrap(),
                expected
            );
            assert!(
                glyph
                    .outline_commands(&Limits {
                        max_path_segments: 1,
                        ..Limits::default()
                    })
                    .is_err()
            );
            assert!(
                glyph
                    .outline_commands(&Limits {
                        max_decoded_bytes: 1,
                        ..Limits::default()
                    })
                    .is_err()
            );
        }
        let mut glyph = make(&[(0, 0, true), (10, 10, true), (20, 20, true), (30, 30, true)]);
        glyph.header.number_of_contours = 2;
        glyph.contour_end_points = vec![1, 3];
        assert_eq!(
            glyph.outline_commands(&Limits::default()).unwrap(),
            [
                Move([0., 0.]),
                Line([10., 10.]),
                Close,
                Move([20., 20.]),
                Line([30., 30.]),
                Close
            ]
        );
        for ends in [vec![1, 1], vec![1, 4], vec![0, 2]] {
            glyph.contour_end_points = ends;
            assert!(glyph.outline_commands(&Limits::default()).is_err());
        }
        let empty = SimpleGlyph::try_new(&[0; 10], &Limits::default()).unwrap();
        assert!(
            empty
                .outline_commands(&Limits::default())
                .unwrap()
                .is_empty()
        );
    }
    #[test]
    fn repeated_short_flags_decode_points_and_enforce_boundaries() {
        let mut data = vec![
            0, 1, 0, 0, 0, 0, 0, 30, 0, 60, 0, 2, 0, 0, 0x3f, 2, 10, 10, 10, 20, 20, 20,
        ];
        let glyph = SimpleGlyph::try_new(&data, &Limits::default()).unwrap();
        assert_eq!(glyph.contour_end_points, [2]);
        assert_eq!(
            glyph.points,
            [
                GlyphPoint {
                    x: 10,
                    y: 20,
                    on_curve: true
                },
                GlyphPoint {
                    x: 20,
                    y: 40,
                    on_curve: true
                },
                GlyphPoint {
                    x: 30,
                    y: 60,
                    on_curve: true
                }
            ]
        );
        for length in 0..data.len() {
            assert!(SimpleGlyph::try_new(&data[..length], &Limits::default()).is_err());
        }
        assert!(
            SimpleGlyph::try_new(
                &data,
                &Limits {
                    max_path_segments: 2,
                    ..Limits::default()
                }
            )
            .is_err()
        );
        assert!(
            SimpleGlyph::try_new(
                &data,
                &Limits {
                    max_decoded_bytes: 30,
                    ..Limits::default()
                }
            )
            .is_err()
        );
        let loca = [0, 0, 0, 11];
        let locations = super::super::loca::GlyphLocations::try_new(&loca, &data, 1, 0).unwrap();
        assert_eq!(
            locations
                .simple_glyph(0, &Limits::default())
                .unwrap()
                .unwrap()
                .points,
            glyph.points
        );
        data[15] = 3;
        assert!(SimpleGlyph::try_new(&data, &Limits::default()).is_err());
        assert!(
            SimpleGlyph::try_new(&[0; 10], &Limits::default())
                .unwrap()
                .points
                .is_empty()
        );
    }
    #[test]
    fn signed_deltas_unchanged_axes_and_off_curve_points() {
        let data = [
            0, 1, 255, 246, 255, 236, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0x31, 255, 246, 255, 236,
        ];
        let glyph = SimpleGlyph::try_new(&data, &Limits::default()).unwrap();
        assert_eq!(
            glyph.points,
            [
                GlyphPoint {
                    x: -10,
                    y: -20,
                    on_curve: false
                },
                GlyphPoint {
                    x: -10,
                    y: -20,
                    on_curve: true
                }
            ]
        );
        assert!(SimpleGlyph::try_new(&[255; 10], &Limits::default()).is_err());
        let instructions = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 0, 3, 1, 2, 3];
        assert_eq!(
            SimpleGlyph::try_new(&instructions, &Limits::default())
                .unwrap()
                .instructions,
            &[1, 2, 3]
        );
        let bad_ends = [0, 2, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1];
        assert!(SimpleGlyph::try_new(&bad_ends, &Limits::default()).is_err());
    }
}
