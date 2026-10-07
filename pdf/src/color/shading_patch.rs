//! Parameter-space geometry for PDF Type6 patches. Not yet wired into Shading.

#[derive(Debug, Clone)]
pub(crate) struct CoonsPatch {
    // PDF stream order: corners are points 0, 3, 6, 9.
    points: [[f64; 2]; 12],
}

fn unit(value: f64) -> bool {
    value.is_finite() && (0.0..=1.0).contains(&value)
}

fn cubic(points: [[f64; 2]; 4], t: f64) -> [f64; 2] {
    // de Casteljau interpolation also preserves endpoints exactly.
    let mix = |a: [f64; 2], b: [f64; 2]| [a[0] * (1.0 - t) + b[0] * t, a[1] * (1.0 - t) + b[1] * t];
    let a = mix(points[0], points[1]);
    let b = mix(points[1], points[2]);
    let c = mix(points[2], points[3]);
    mix(mix(a, b), mix(b, c))
}

/// Derivative of the cubic Bernstein curve.
fn cubic_derivative(points: [[f64; 2]; 4], t: f64) -> [f64; 2] {
    let differences: [[f64; 2]; 3] = std::array::from_fn(|i| {
        std::array::from_fn(|axis| 3.0 * (points[i + 1][axis] - points[i][axis]))
    });
    std::array::from_fn(|axis| {
        (1.0 - t) * (1.0 - t) * differences[0][axis]
            + 2.0 * t * (1.0 - t) * differences[1][axis]
            + t * t * differences[2][axis]
    })
}

/// Weights in PDF corner order, for components/parameters before conversion.
pub(crate) fn corner_weights(u: f64, v: f64) -> Option<[f64; 4]> {
    (unit(u) && unit(v)).then_some([(1.0 - u) * (1.0 - v), (1.0 - u) * v, u * v, u * (1.0 - v)])
}

/// PDF corner components or function parameters, before RGB/function mapping.
/// Validation precedes writes, so failure preserves the caller's buffer.
pub(crate) fn interpolate_corners(
    corners: [&[f32]; 4],
    u: f64,
    v: f64,
    output: &mut [f32],
) -> Option<()> {
    let weights = corner_weights(u, v)?;
    if output.is_empty()
        || corners
            .iter()
            .any(|c| c.len() != output.len() || c.iter().any(|v| !v.is_finite()))
    {
        return None;
    }
    for (index, value) in output.iter_mut().enumerate() {
        *value = corners
            .iter()
            .zip(weights)
            .map(|(corner, weight)| f64::from(corner[index]) * weight)
            .sum::<f64>() as f32;
    }
    Some(())
}

impl CoonsPatch {
    /// Nonzero PDF edge flags inherit four points and two corner colours.
    pub(crate) fn shared_edge(&self, flag: u8) -> Option<([[f64; 2]; 4], [usize; 2])> {
        let start = match flag {
            1 => 3,
            2 => 6,
            3 => 9,
            _ => return None,
        };
        Some((
            std::array::from_fn(|i| self.points[(start + i) % 12]),
            [usize::from(flag), (usize::from(flag) + 1) % 4],
        ))
    }

    pub(crate) fn continue_from(&self, flag: u8, tail: [[f64; 2]; 8]) -> Option<Self> {
        let (edge, _) = self.shared_edge(flag)?;
        let mut points = [[0.0; 2]; 12];
        points[..4].copy_from_slice(&edge);
        points[4..].copy_from_slice(&tail);
        Self::try_new(points)
    }

    pub(crate) fn try_new(points: [[f64; 2]; 12]) -> Option<Self> {
        points
            .iter()
            .flatten()
            .all(|v| v.is_finite())
            .then_some(Self { points })
    }

    pub(crate) fn point(&self, u: f64, v: f64) -> Option<[f64; 2]> {
        let weights = corner_weights(u, v)?;
        let p = &self.points;
        let bottom = cubic([p[0], p[11], p[10], p[9]], u);
        let top = cubic([p[3], p[4], p[5], p[6]], u);
        let left = cubic([p[0], p[1], p[2], p[3]], v);
        let right = cubic([p[9], p[8], p[7], p[6]], v);
        let corners = [p[0], p[3], p[6], p[9]];
        let result = std::array::from_fn(|axis| {
            let bilinear: f64 = corners.iter().zip(weights).map(|(p, w)| p[axis] * w).sum();
            (1.0 - v) * bottom[axis] + v * top[axis] + (1.0 - u) * left[axis] + u * right[axis]
                - bilinear
        });
        result.iter().all(|v| v.is_finite()).then_some(result)
    }

    /// Analytic derivatives in u/v order; not an inverse or fold resolver.
    pub(crate) fn derivatives(&self, u: f64, v: f64) -> Option<[[f64; 2]; 2]> {
        if !unit(u) || !unit(v) {
            return None;
        }
        let p = &self.points;
        let c1 = [p[0], p[11], p[10], p[9]];
        let c2 = [p[3], p[4], p[5], p[6]];
        let d1 = [p[0], p[1], p[2], p[3]];
        let d2 = [p[9], p[8], p[7], p[6]];
        let bottom = cubic(c1, u);
        let top = cubic(c2, u);
        let left = cubic(d1, v);
        let right = cubic(d2, v);
        let bottom_derivative = cubic_derivative(c1, u);
        let top_derivative = cubic_derivative(c2, u);
        let left_derivative = cubic_derivative(d1, v);
        let right_derivative = cubic_derivative(d2, v);
        let du = std::array::from_fn(|axis| {
            (1.0 - v) * bottom_derivative[axis] + v * top_derivative[axis] + right[axis]
                - left[axis]
                - ((1.0 - v) * (p[9][axis] - p[0][axis]) + v * (p[6][axis] - p[3][axis]))
        });
        let dv = std::array::from_fn(|axis| {
            top[axis] - bottom[axis]
                + (1.0 - u) * left_derivative[axis]
                + u * right_derivative[axis]
                - ((1.0 - u) * (p[3][axis] - p[0][axis]) + u * (p[6][axis] - p[9][axis]))
        });
        let result = [du, dv];
        result
            .iter()
            .flatten()
            .all(|v| v.is_finite())
            .then_some(result)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn corner_components_and_parameters_interpolate_before_conversion() {
        let corners = [
            [1.0, 0.0, 0.0],
            [0.0, 1.0, 0.0],
            [0.0, 0.0, 1.0],
            [1.0, 1.0, 1.0],
        ];
        let references = corners.each_ref().map(|c| c.as_slice());
        let mut rgb = [-1.0; 3];
        assert_eq!(
            interpolate_corners(references, 0.25, 0.75, &mut rgb),
            Some(())
        );
        assert_eq!(rgb, [0.25, 0.625, 0.25]);
        let parameters = [[0.0], [1.0], [0.0], [1.0]];
        let mut t = [-1.0];
        assert!(
            interpolate_corners(
                parameters.each_ref().map(|c| c.as_slice()),
                0.25,
                0.75,
                &mut t
            )
            .is_some()
        );
        assert_eq!(t, [0.625]);
        assert_eq!(t[0] * t[0], 0.390625);
        assert!(interpolate_corners(references, -0.1, 0.75, &mut rgb).is_none());
        assert_eq!(rgb, [0.25, 0.625, 0.25]);
        assert!(interpolate_corners(references, 0.25, 0.75, &mut t).is_none());
        assert_eq!(t, [0.625]);
        let mut invalid = corners;
        invalid[2][1] = f32::NAN;
        assert!(
            interpolate_corners(
                invalid.each_ref().map(|c| c.as_slice()),
                0.25,
                0.75,
                &mut rgb
            )
            .is_none()
        );
        assert_eq!(rgb, [0.25, 0.625, 0.25]);
    }
    #[test]
    fn folded_surface_retains_multiple_parameter_preimages() {
        let mut p = rectangle();
        // Identical horizontal curves x(u)=12u-27u^2+18u^3 fold twice.
        p[11][0] = 4.0;
        p[10][0] = -1.0;
        p[4][0] = 4.0;
        p[5][0] = -1.0;
        let patch = CoonsPatch::try_new(p).unwrap();
        let offset = 3.0_f64.sqrt() / 6.0;
        for u in [0.5 - offset, 0.5, 0.5 + offset] {
            close(patch.point(u, 0.5).unwrap(), [1.5, 1.5]);
        }
        assert!(patch.derivatives(0.5, 0.5).unwrap()[0][0] < 0.0);
        assert!(patch.derivatives(0.5 - offset, 0.5).unwrap()[0][0] > 0.0);
    }

    #[test]
    fn curved_interior_is_not_corner_bilinear_geometry() {
        let mut p = rectangle();
        p[4][1] = 6.0;
        p[5][1] = 6.0;
        let patch = CoonsPatch::try_new(p).unwrap();
        close(patch.point(0.5, 0.5).unwrap(), [1.5, 2.625]);
        assert_ne!(patch.point(0.5, 0.5).unwrap(), [1.5, 1.5]);
    }

    #[test]
    fn analytic_derivatives_match_rectangle_and_curved_finite_differences() {
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        assert_eq!(
            patch.derivatives(0.25, 0.75),
            Some([[3.0, 0.0], [0.0, 3.0]])
        );
        assert!(patch.derivatives(f64::NAN, 0.5).is_none());
        let mut p = rectangle();
        p[1][0] = -1.0;
        p[4][1] = 4.0;
        p[7][0] = 4.0;
        p[10][1] = -1.0;
        let curved = CoonsPatch::try_new(p).unwrap();
        let epsilon = 1e-5;
        for (u, v) in [(0.25, 0.25), (0.5, 0.5), (0.75, 0.25)] {
            let analytic = curved.derivatives(u, v).unwrap();
            for (direction, minus, plus) in [
                (
                    0,
                    curved.point(u - epsilon, v).unwrap(),
                    curved.point(u + epsilon, v).unwrap(),
                ),
                (
                    1,
                    curved.point(u, v - epsilon).unwrap(),
                    curved.point(u, v + epsilon).unwrap(),
                ),
            ] {
                for axis in 0..2 {
                    let numerical = (plus[axis] - minus[axis]) / (2.0 * epsilon);
                    assert!((numerical - analytic[direction][axis]).abs() < 1e-8);
                }
            }
        }
    }
    #[test]
    fn shared_edge_flags_preserve_controls_corners_and_curve_direction() {
        let mut points = rectangle();
        points[4][1] = 4.0;
        points[7][0] = 4.0;
        points[10][1] = -1.0;
        let patch = CoonsPatch::try_new(points).unwrap();
        for flag in 1..=3 {
            let (edge, colors) = patch.shared_edge(flag).unwrap();
            let start = usize::from(flag) * 3;
            for i in 0..4 {
                assert_eq!(edge[i], points[(start + i) % 12]);
            }
            assert_eq!(colors, [usize::from(flag), (usize::from(flag) + 1) % 4]);
            let next = patch
                .continue_from(flag, rectangle()[4..].try_into().unwrap())
                .unwrap();
            for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
                let previous = match flag {
                    1 => patch.point(t, 1.0),
                    2 => patch.point(1.0, 1.0 - t),
                    _ => patch.point(1.0 - t, 0.0),
                }
                .unwrap();
                close(next.point(0.0, t).unwrap(), previous);
            }
        }
        assert!(patch.shared_edge(0).is_none());
        assert!(patch.shared_edge(4).is_none());
        let mut tail: [[f64; 2]; 8] = rectangle()[4..].try_into().unwrap();
        tail[0][0] = f64::NAN;
        assert!(patch.continue_from(1, tail).is_none());
    }
    fn rectangle() -> [[f64; 2]; 12] {
        [
            [0.0, 0.0],
            [0.0, 1.0],
            [0.0, 2.0],
            [0.0, 3.0],
            [1.0, 3.0],
            [2.0, 3.0],
            [3.0, 3.0],
            [3.0, 2.0],
            [3.0, 1.0],
            [3.0, 0.0],
            [2.0, 0.0],
            [1.0, 0.0],
        ]
    }
    fn close(a: [f64; 2], b: [f64; 2]) {
        for axis in 0..2 {
            assert!((a[axis] - b[axis]).abs() < 1e-12, "{a:?} != {b:?}");
        }
    }
    #[test]
    fn rectangle_and_corner_order() {
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        close(patch.point(0.25, 0.75).unwrap(), [0.75, 2.25]);
        for (u, v, index) in [(0.0, 0.0, 0), (0.0, 1.0, 1), (1.0, 1.0, 2), (1.0, 0.0, 3)] {
            let weights = corner_weights(u, v).unwrap();
            assert_eq!(weights[index], 1.0);
            assert_eq!(weights.iter().sum::<f64>(), 1.0);
        }
        assert_eq!(
            corner_weights(0.25, 0.75),
            Some([0.1875, 0.5625, 0.1875, 0.0625])
        );
    }
    #[test]
    fn curved_boundaries_are_interpolated_in_stream_order() {
        let mut p = rectangle();
        p[1][0] = -1.0;
        p[4][1] = 4.0;
        p[7][0] = 4.0;
        p[10][1] = -1.0;
        let patch = CoonsPatch::try_new(p).unwrap();
        for t in [0.0, 0.25, 0.5, 0.75, 1.0] {
            close(
                patch.point(0.0, t).unwrap(),
                cubic([p[0], p[1], p[2], p[3]], t),
            );
            close(
                patch.point(t, 1.0).unwrap(),
                cubic([p[3], p[4], p[5], p[6]], t),
            );
            close(
                patch.point(1.0, t).unwrap(),
                cubic([p[9], p[8], p[7], p[6]], t),
            );
            close(
                patch.point(t, 0.0).unwrap(),
                cubic([p[0], p[11], p[10], p[9]], t),
            );
        }
    }
    #[test]
    fn rejects_nonfinite_controls_and_outside_parameters() {
        let mut bad = rectangle();
        bad[5][0] = f64::INFINITY;
        assert!(CoonsPatch::try_new(bad).is_none());
        let patch = CoonsPatch::try_new(rectangle()).unwrap();
        for (u, v) in [(-0.1, 0.5), (0.5, 1.1), (f64::NAN, 0.5)] {
            assert!(patch.point(u, v).is_none());
        }
    }
}
