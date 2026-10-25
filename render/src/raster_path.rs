//! Viewport-independent curve flattening in the global raster plane.
//! Rational Bernstein convex hulls bound the chord error; pixel-grid rounding
//! adds at most sqrt(2)/512, keeping total error below 1/32 device pixel.
use super::*;
use skia_safe::path::{Iter, Verb};
const ERROR: f64 = 1. / 64.;
const GRID: f64 = 256.;
fn work(left: &mut usize) -> PdfResult<()> {
    *left = left
        .checked_sub(1)
        .ok_or_else(|| PdfError::ResourceLimit("raster curve work budget".into()))?;
    Ok(())
}
fn project(p: [f64; 3]) -> PdfResult<[f64; 2]> {
    if !p.iter().all(|v| v.is_finite()) || p[2] <= 0. {
        return Err(PdfError::PageOperatorError("invalid raster conic".into()));
    }
    Ok([p[0] / p[2], p[1] / p[2]])
}
fn point(p: [f64; 2]) -> (f32, f32) {
    (
        ((p[0] * GRID).round() / GRID) as f32,
        ((p[1] * GRID).round() / GRID) as f32,
    )
}
fn chord_distance_squared(p: [f64; 2], a: [f64; 2], b: [f64; 2]) -> f64 {
    let dx = b[0] - a[0];
    let dy = b[1] - a[1];
    let length = dx * dx + dy * dy;
    let t = if length == 0. {
        0.
    } else {
        ((p[0] - a[0]) * dx + (p[1] - a[1]) * dy) / length
    }
    .clamp(0., 1.);
    let x = p[0] - a[0] - t * dx;
    let y = p[1] - a[1] - t * dy;
    x * x + y * y
}
fn segment(
    builder: &mut PathBuilder,
    controls: [[f64; 3]; 4],
    count: usize,
    depth: u8,
    left: &mut usize,
) -> PdfResult<()> {
    work(left)?;
    let a = project(controls[0])?;
    let b = project(controls[count - 1])?;
    let mut flat = true;
    for p in &controls[1..count - 1] {
        if chord_distance_squared(project(*p)?, a, b) > ERROR * ERROR {
            flat = false;
            break;
        }
    }
    if flat {
        builder.line_to(point(b));
        return Ok(());
    }
    if depth == 24 {
        return Err(PdfError::ResourceLimit(
            "raster curve subdivision depth".into(),
        ));
    }
    let mut triangle = controls;
    let mut first = [[0.; 3]; 4];
    let mut last = first;
    first[0] = triangle[0];
    last[count - 1] = triangle[count - 1];
    for level in 1..count {
        for i in 0..count - level {
            let next = triangle[i + 1];
            for (value, next) in triangle[i].iter_mut().zip(next) {
                *value = (*value + next) * 0.5;
            }
        }
        first[level] = triangle[0];
        last[count - 1 - level] = triangle[count - 1 - level];
    }
    segment(builder, first, count, depth + 1, left)?;
    segment(builder, last, count, depth + 1, left)
}
pub(super) fn has_curves(path: &skia_safe::Path) -> bool {
    path.verbs().iter().any(|v| {
        matches!(
            v,
            skia_safe::PathVerb::Quad | skia_safe::PathVerb::Conic | skia_safe::PathVerb::Cubic
        )
    })
}
pub(super) fn normalized(path: &skia_safe::Path, left: &mut usize) -> PdfResult<skia_safe::Path> {
    let mut builder = PathBuilder::new();
    builder.set_fill_type(path.fill_type());
    let mut iter = Iter::new(path, false);
    while let Some((verb, points)) = iter.next() {
        work(left)?;
        match verb {
            Verb::Move => {
                builder.move_to(point([f64::from(points[0].x), f64::from(points[0].y)]));
            }
            Verb::Line => {
                builder.line_to(point([f64::from(points[1].x), f64::from(points[1].y)]));
            }
            Verb::Quad | Verb::Cubic | Verb::Conic => {
                let mut controls = [[0.; 3]; 4];
                for (i, p) in points.iter().enumerate() {
                    controls[i] = [f64::from(p.x), f64::from(p.y), 1.];
                }
                if verb == Verb::Conic {
                    let weight = f64::from(iter.conic_weight().unwrap_or(1.));
                    for value in &mut controls[1] {
                        *value *= weight;
                    }
                }
                segment(&mut builder, controls, points.len(), 0, left)?;
            }
            Verb::Close => {
                builder.close();
            }
            Verb::Done => break,
        }
    }
    Ok(builder.snapshot())
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn cubic_global_chords_meet_independent_sample_error_and_budget() {
        let mut p = PathBuilder::new();
        p.move_to((10000., 10000.));
        p.cubic_to((10000., 10100.), (10100., 10100.), (10100., 10000.));
        let path = p.snapshot();
        let poly = normalized(&path, &mut 10000).unwrap();
        let points = poly.points();
        for i in 0..=1000 {
            let t = i as f64 / 1000.;
            let u = 1. - t;
            let sample = [
                10000. + 100. * (3. * u * t * t + t * t * t),
                10000. + 300. * (u * u * t + u * t * t),
            ];
            let d = points
                .windows(2)
                .map(|s| {
                    chord_distance_squared(
                        sample,
                        [f64::from(s[0].x), f64::from(s[0].y)],
                        [f64::from(s[1].x), f64::from(s[1].y)],
                    )
                })
                .fold(f64::INFINITY, f64::min);
            assert!(d.sqrt() < 1. / 32.);
        }
        assert!(matches!(
            normalized(&path, &mut 1),
            Err(PdfError::ResourceLimit(_))
        ));
    }
}
