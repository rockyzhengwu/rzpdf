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

/// Independent contours are culled as a set, never composited separately:
/// winding, holes and overlapping alpha must remain one coverage operation.
pub(super) struct Contours {
    paths: Vec<skia_safe::Path>,
    fill: PathFillType,
    grid: Option<ContourGrid>,
}
struct ContourGrid {
    cells: Vec<Vec<usize>>,
    columns: usize,
    first_x: i32,
    first_y: i32,
    cell: i32,
}
impl Contours {
    pub(super) fn new(
        path: &skia_safe::Path,
        budget: &mut usize,
        max_bytes: usize,
    ) -> PdfResult<Self> {
        let count = path
            .verbs()
            .iter()
            .filter(|v| **v == skia_safe::PathVerb::Move)
            .count();
        if count <= 1 || path.points().len() < 256 || path.is_inverse_fill_type() {
            return Ok(Self {
                paths: vec![path.clone()],
                fill: path.fill_type(),
                grid: None,
            });
        }
        if count
            .checked_mul(256)
            .and_then(|n| n.checked_add(path.points().len().saturating_mul(16)))
            .is_none_or(|n| n > max_bytes / 4)
        {
            return Err(PdfError::ResourceLimit("canonical contour storage".into()));
        }
        let mut paths = Vec::new();
        paths
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("canonical contour allocation".into()))?;
        let mut builder = PathBuilder::new();
        for (verb, points) in Iter::new(path, false) {
            work(budget)?;
            match verb {
                Verb::Move => {
                    if !builder.is_empty() {
                        paths.push(builder.detach());
                    }
                    builder.move_to(points[0]);
                }
                Verb::Line => {
                    builder.line_to(points[1]);
                }
                Verb::Close => {
                    builder.close();
                }
                Verb::Done => break,
                _ => {
                    return Err(PdfError::PageOperatorError(
                        "canonical contours require normalized lines".into(),
                    ));
                }
            }
        }
        if !builder.is_empty() {
            paths.push(builder.detach());
        }
        Ok(Self {
            paths,
            fill: path.fill_type(),
            grid: None,
        })
    }

    pub(super) fn reserve_grid_work(
        &mut self,
        bounds: (i32, i32, i32, i32),
        cell: i32,
        halo: i32,
        budget: &mut usize,
        max_bytes: usize,
    ) -> PdfResult<()> {
        let (left, top, right, bottom) = bounds;
        let first_x = left.div_euclid(cell) * cell;
        let first_y = top.div_euclid(cell) * cell;
        let columns = ((right - 1).div_euclid(cell) - left.div_euclid(cell) + 1) as usize;
        let rows = ((bottom - 1).div_euclid(cell) - top.div_euclid(cell) + 1) as usize;
        let count = columns
            .checked_mul(rows)
            .ok_or_else(|| PdfError::ResourceLimit("canonical contour grid".into()))?;
        if self.paths.len() == 1 {
            let cost = count
                .checked_mul(self.paths[0].points().len().max(1))
                .ok_or_else(|| PdfError::ResourceLimit("canonical curve coverage work".into()))?;
            *budget = budget
                .checked_sub(cost)
                .ok_or_else(|| PdfError::ResourceLimit("canonical curve coverage work".into()))?;
            return Ok(());
        }
        let mut storage = count
            .checked_mul(std::mem::size_of::<Vec<usize>>())
            .filter(|n| *n <= max_bytes / 4)
            .ok_or_else(|| PdfError::ResourceLimit("canonical contour grid bytes".into()))?;
        let mut cells = Vec::new();
        cells
            .try_reserve_exact(count)
            .map_err(|_| PdfError::ResourceLimit("canonical contour grid allocation".into()))?;
        cells.resize_with(count, Vec::<usize>::new);
        // Bin once in source order. Per-cell scans over every contour are
        // quadratic for sparse pages and can themselves exhaust browser work.
        for (index, path) in self.paths.iter().enumerate() {
            work(budget)?;
            let b = path.bounds();
            let x0 = (((b.left - halo as f32 - 1.).floor() as i32).div_euclid(cell)
                - first_x.div_euclid(cell))
            .max(0);
            let y0 = (((b.top - halo as f32 - 1.).floor() as i32).div_euclid(cell)
                - first_y.div_euclid(cell))
            .max(0);
            let x1 = (((b.right + halo as f32 + 1.).ceil() as i32).div_euclid(cell)
                - first_x.div_euclid(cell))
            .min(columns as i32 - 1);
            let y1 = (((b.bottom + halo as f32 + 1.).ceil() as i32).div_euclid(cell)
                - first_y.div_euclid(cell))
            .min(rows as i32 - 1);
            for y in y0..=y1 {
                for x in x0..=x1 {
                    *budget = budget
                        .checked_sub(path.points().len().max(1) + 2)
                        .ok_or_else(|| {
                            PdfError::ResourceLimit("canonical curve coverage work".into())
                        })?;
                    let bin = &mut cells[y as usize * columns + x as usize];
                    if bin.len() == bin.capacity() {
                        let extra = bin.capacity().max(4);
                        let bytes =
                            extra
                                .checked_mul(std::mem::size_of::<usize>())
                                .ok_or_else(|| {
                                    PdfError::ResourceLimit("canonical contour index bytes".into())
                                })?;
                        storage = storage
                            .checked_add(bytes)
                            .filter(|n| *n <= max_bytes / 4)
                            .ok_or_else(|| {
                                PdfError::ResourceLimit("canonical contour index bytes".into())
                            })?;
                        bin.try_reserve_exact(extra).map_err(|_| {
                            PdfError::ResourceLimit("canonical contour index allocation".into())
                        })?;
                    }
                    bin.push(index);
                }
            }
        }
        self.grid = Some(ContourGrid {
            cells,
            columns,
            first_x,
            first_y,
            cell,
        });
        Ok(())
    }
    pub(super) fn for_cell(&self, x: i32, y: i32) -> skia_safe::Path {
        if self.paths.len() == 1 {
            return self.paths[0].clone();
        }
        let grid = self.grid.as_ref().expect("coverage grid preflight");
        let column = (x - grid.first_x) / grid.cell;
        let row = (y - grid.first_y) / grid.cell;
        let mut builder = PathBuilder::new();
        builder.set_fill_type(self.fill);
        for index in &grid.cells[row as usize * grid.columns + column as usize] {
            builder.add_path(&self.paths[*index]);
        }
        builder.detach()
    }
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn contour_culling_keeps_holes_and_overlaps_in_one_coverage_mask() {
        for fill in [PathFillType::Winding, PathFillType::EvenOdd] {
            let mut builder = PathBuilder::new();
            builder.set_fill_type(fill);
            for y in 0..8 {
                for x in 0..8 {
                    let x = x as f32 * 24. + 0.25;
                    let y = y as f32 * 24. + 0.5;
                    builder.add_rect(skia_safe::Rect::from_xywh(x, y, 35., 35.), None, None);
                    builder.add_rect(
                        skia_safe::Rect::from_xywh(x + 4., y + 4., 12., 12.),
                        None,
                        None,
                    );
                }
            }
            let path = builder.detach();
            let mut budget = 1_000_000;
            let mut contours = Contours::new(&path, &mut budget, 4_000_000).unwrap();
            assert!(contours.paths.len() > 1);
            assert!(
                contours
                    .reserve_grid_work((0, 0, 256, 256), 64, 2, &mut 1, 4_000_000)
                    .is_err()
            );
            assert!(Contours::new(&path, &mut budget, 1).is_err());
            contours
                .reserve_grid_work((0, 0, 256, 256), 64, 2, &mut budget, 4_000_000)
                .unwrap();
            for y in [0, 64, 128] {
                for x in [0, 64, 128] {
                    let local = contours.for_cell(x, y);
                    let mut images = Vec::new();
                    for outline in [&path, &local] {
                        let mut mask =
                            surfaces::raster(&skia_safe::ImageInfo::new_a8((68, 68)), None, None)
                                .unwrap();
                        mask.canvas().clear(Color::TRANSPARENT);
                        mask.canvas().translate((2. - x as f32, 2. - y as f32));
                        let mut paint = Paint::default();
                        paint.set_color(Color::WHITE).set_anti_alias(true);
                        mask.canvas().draw_path(outline, &paint);
                        images.push(mask.peek_pixels().unwrap().bytes().unwrap().to_vec());
                    }
                    assert_eq!(images[0], images[1], "{fill:?} cell {x} {y}");
                }
            }
        }
    }
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
