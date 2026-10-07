//! Nonzero-winding geometry for closed unhinted quadratic glyph outlines.
use super::glyf::{GlyphOutlineCommand, outline_bounds};
use crate::{
    error::{PdfError, PdfResult},
    parser::options::Limits,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutlineLocation {
    Outside,
    Inside,
    Boundary,
}

/// Classify filled geometry, independent of text rendering mode/visibility.
/// Boundary uses 64*f64::EPSILON in globally normalized coordinates; results
/// within floating-point resolution are not exact-arithmetic predicates.
/// Quadratics are split at y extrema and intersected without flattening.
pub fn outline_location(
    commands: &[GlyphOutlineCommand],
    point: [f64; 2],
    limits: &Limits,
) -> PdfResult<OutlineLocation> {
    if !point.iter().all(|v| v.is_finite()) {
        return Err(PdfError::FontError("Non-finite outline query".into()));
    }
    // Validate the whole sequence before any early geometric return.
    let Some(bounds) = outline_bounds(commands, limits)? else {
        return Ok(OutlineLocation::Outside);
    };
    if point[0] < bounds[0] || point[0] > bounds[2] || point[1] < bounds[1] || point[1] > bounds[3]
    {
        return Ok(OutlineLocation::Outside);
    }
    let mut scale = point[0].abs().max(point[1].abs());
    for command in commands {
        match command {
            GlyphOutlineCommand::Move(p) | GlyphOutlineCommand::Line(p) => {
                scale = scale.max(p[0].abs()).max(p[1].abs());
            }
            GlyphOutlineCommand::Quadratic { control, end } => {
                for value in control.iter().chain(end.iter()) {
                    scale = scale.max(value.abs());
                }
            }
            GlyphOutlineCommand::Close => {}
        }
    }
    if scale == 0. {
        return Ok(OutlineLocation::Boundary);
    }
    let normalize = |p: [f64; 2]| [p[0] / scale, p[1] / scale];
    let point = normalize(point);
    let epsilon = 64. * f64::EPSILON;
    let mut winding = 0_i64;
    let mut current = [0.; 2];
    let mut start = [0.; 2];
    for command in commands {
        let boundary = match *command {
            GlyphOutlineCommand::Move(p) => {
                current = normalize(p);
                start = current;
                false
            }
            GlyphOutlineCommand::Line(p) => {
                let end = normalize(p);
                let hit = line(current, end, point, epsilon, &mut winding);
                current = end;
                hit
            }
            GlyphOutlineCommand::Quadratic { control, end } => {
                let end = normalize(end);
                let hit = quadratic(
                    current,
                    normalize(control),
                    end,
                    point,
                    epsilon,
                    &mut winding,
                );
                current = end;
                hit
            }
            GlyphOutlineCommand::Close => {
                let hit = line(current, start, point, epsilon, &mut winding);
                current = start;
                hit
            }
        };
        if boundary {
            return Ok(OutlineLocation::Boundary);
        }
    }
    Ok(if winding == 0 {
        OutlineLocation::Outside
    } else {
        OutlineLocation::Inside
    })
}

fn line(a: [f64; 2], b: [f64; 2], p: [f64; 2], epsilon: f64, winding: &mut i64) -> bool {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx.hypot(dy);
    if length == 0. {
        return (p[0] - a[0]).hypot(p[1] - a[1]) <= epsilon;
    }
    let cross = (p[0] - a[0]) * dy - (p[1] - a[1]) * dx;
    if cross.abs() <= epsilon * length
        && p[0] >= a[0].min(b[0]) - epsilon
        && p[0] <= a[0].max(b[0]) + epsilon
        && p[1] >= a[1].min(b[1]) - epsilon
        && p[1] <= a[1].max(b[1]) + epsilon
    {
        return true;
    }
    if (a[1] <= p[1] && p[1] < b[1]) || (b[1] <= p[1] && p[1] < a[1]) {
        let x = a[0] + (p[1] - a[1]) / dy * dx;
        if x > p[0] {
            *winding += if b[1] > a[1] { 1 } else { -1 };
        }
    }
    false
}

fn evaluate(a: [f64; 2], b: [f64; 2], c: [f64; 2], t: f64) -> [f64; 2] {
    let u = 1. - t;
    [
        u * u * a[0] + 2. * u * t * b[0] + t * t * c[0],
        u * u * a[1] + 2. * u * t * b[1] + t * t * c[1],
    ]
}

fn quadratic(
    a: [f64; 2],
    b: [f64; 2],
    c: [f64; 2],
    p: [f64; 2],
    epsilon: f64,
    winding: &mut i64,
) -> bool {
    let denominator = a[1] - 2. * b[1] + c[1];
    let vertex = if denominator == 0. {
        0.
    } else {
        (a[1] - b[1]) / denominator
    };
    let intervals = if vertex > 0. && vertex < 1. {
        [(0., vertex), (vertex, 1.)]
    } else {
        [(0., 1.), (1., 1.)]
    };
    for (low, high) in intervals {
        if low == high {
            continue;
        }
        let from = evaluate(a, b, c, low);
        let to = evaluate(a, b, c, high);
        if (p[0] - from[0]).hypot(p[1] - from[1]) <= epsilon
            || (p[0] - to[0]).hypot(p[1] - to[1]) <= epsilon
        {
            return true;
        }
        if from[1] == to[1] {
            if (p[1] - from[1]).abs() <= epsilon {
                let mut minimum = from[0].min(to[0]);
                let mut maximum = from[0].max(to[0]);
                let d = a[0] - 2. * b[0] + c[0];
                if d != 0. {
                    let t = (a[0] - b[0]) / d;
                    if t > low && t < high {
                        let x = evaluate(a, b, c, t)[0];
                        minimum = minimum.min(x);
                        maximum = maximum.max(x);
                    }
                }
                if p[0] >= minimum - epsilon && p[0] <= maximum + epsilon {
                    return true;
                }
            }
            continue;
        }
        if p[1] < from[1].min(to[1]) || p[1] > from[1].max(to[1]) {
            continue;
        }
        let upward = to[1] > from[1];
        let (mut left, mut right) = (low, high);
        for _ in 0..64 {
            let middle = (left + right) / 2.;
            if (evaluate(a, b, c, middle)[1] < p[1]) == upward {
                left = middle;
            } else {
                right = middle;
            }
        }
        let intersection = evaluate(a, b, c, (left + right) / 2.);
        if (intersection[0] - p[0]).abs() <= epsilon {
            return true;
        }
        // Half-open y ranges avoid counting shared vertices/tangencies twice.
        if p[1] < from[1].max(to[1]) && intersection[0] > p[0] {
            *winding += if upward { 1 } else { -1 };
        }
    }
    false
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn curve_tangencies_and_scale_reflection_do_not_change_winding_membership() {
        for scale in [1e-200, 1., 1e200] {
            for sign in [-1., 1.] {
                let arch = [
                    Move([0., 0.]),
                    Quadratic {
                        control: [10. * scale * sign, 20. * scale],
                        end: [20. * scale * sign, 0.],
                    },
                    Close,
                ];
                assert_eq!(
                    outline_location(&arch, [10. * scale * sign, 5. * scale], &Limits::default())
                        .unwrap(),
                    OutlineLocation::Inside
                );
                assert_eq!(
                    outline_location(&arch, [0., 10. * scale], &Limits::default()).unwrap(),
                    OutlineLocation::Outside
                );
                assert_eq!(
                    outline_location(&arch, [10. * scale * sign, 10. * scale], &Limits::default())
                        .unwrap(),
                    OutlineLocation::Boundary
                );
            }
        }
        let horizontal = [
            Move([0., 0.]),
            Quadratic {
                control: [20., 0.],
                end: [0., 0.],
            },
            Close,
        ];
        assert_eq!(
            outline_location(&horizontal, [5., 0.], &Limits::default()).unwrap(),
            OutlineLocation::Boundary
        );
        let overlapping = [
            Move([0., 0.]),
            Line([10., 0.]),
            Line([10., 10.]),
            Line([0., 10.]),
            Close,
            Move([3., 3.]),
            Line([7., 3.]),
            Line([7., 7.]),
            Line([3., 7.]),
            Close,
        ];
        assert_eq!(
            outline_location(&overlapping, [5., 5.], &Limits::default()).unwrap(),
            OutlineLocation::Inside
        );
    }
    use GlyphOutlineCommand::{Close, Line, Move, Quadratic};
    #[test]
    fn winding_handles_curves_holes_boundaries_and_invalid_inputs() {
        let arch = [
            Move([0., 0.]),
            Quadratic {
                control: [10., 20.],
                end: [20., 0.],
            },
            Close,
        ];
        for (point, expected) in [
            ([10., 5.], OutlineLocation::Inside),
            ([10., 11.], OutlineLocation::Outside),
            ([10., 10.], OutlineLocation::Boundary),
            ([10., 0.], OutlineLocation::Boundary),
            ([1., 8.], OutlineLocation::Outside),
        ] {
            assert_eq!(
                outline_location(&arch, point, &Limits::default()).unwrap(),
                expected
            );
        }
        let hole = [
            Move([0., 0.]),
            Line([10., 0.]),
            Line([10., 10.]),
            Line([0., 10.]),
            Close,
            Move([3., 3.]),
            Line([3., 7.]),
            Line([7., 7.]),
            Line([7., 3.]),
            Close,
        ];
        assert_eq!(
            outline_location(&hole, [1., 5.], &Limits::default()).unwrap(),
            OutlineLocation::Inside
        );
        assert_eq!(
            outline_location(&hole, [5., 5.], &Limits::default()).unwrap(),
            OutlineLocation::Outside
        );
        assert!(outline_location(&arch, [f64::NAN, 0.], &Limits::default()).is_err());
        assert!(outline_location(&[Line([0., 0.])], [0., 0.], &Limits::default()).is_err());
        assert!(
            outline_location(
                &arch,
                [10., 5.],
                &Limits {
                    max_path_segments: 2,
                    ..Limits::default()
                }
            )
            .is_err()
        );
        assert_eq!(
            outline_location(&[], [0., 0.], &Limits::default()).unwrap(),
            OutlineLocation::Outside
        );
    }
}
