//! Bounded forward patch rasterization for display. This approximates geometry
//! in device pixels; it does not replace the certified point-sampling API.
use super::*;
use crate::color::conversion::ColorConversionContext;
use crate::color::shading_patch::{interpolate_corners, subdivide_tensor};

#[derive(Debug, Clone, Copy)]
pub struct PatchRasterOptions {
    pub width: u32,
    pub height: u32,
    /// PDF shading coordinates to output pixels, in PDF matrix order.
    pub matrix: [f64; 6],
    /// Scratch allowance in addition to the caller-owned output raster.
    pub max_scratch_bytes: usize,
    pub paint_background: bool,
}
#[derive(Clone, Copy)]
struct Cell {
    controls: [[[f64; 2]; 4]; 4],
    uv: [f64; 4],
    depth: u8,
}
fn work(remaining: &mut usize) -> PdfResult<()> {
    *remaining = remaining
        .checked_sub(1)
        .ok_or_else(|| PdfError::ResourceLimit("patch raster work".into()))?;
    Ok(())
}
impl Shading {
    /// Rasterize a patch mesh with at most 1/8 pixel geometric flatness error
    /// per accepted cell. Fold hits use greatest v, then u; later patches win.
    /// Scratch/work/depth limits are explicit errors. On error the caller must
    /// discard output; no approximate coverage is substituted in color_at_point.
    pub fn rasterize_patch_into(
        &self,
        options: PatchRasterOptions,
        context: &ColorConversionContext,
        output: &mut [u8],
    ) -> PdfResult<()> {
        self.rasterize_patch_region_into(options, [0, 0], context, output)
    }
    /// Rasterize local output pixels at an integer origin in the global device
    /// plane. Geometry and barycentric arithmetic retain their global phase.
    pub fn rasterize_patch_region_into(
        &self,
        options: PatchRasterOptions,
        origin: [u32; 2],
        context: &ColorConversionContext,
        output: &mut [u8],
    ) -> PdfResult<()> {
        let [ox, oy] = origin.map(f64::from);
        let mesh = self
            .patch_mesh
            .as_ref()
            .ok_or_else(|| invalid("patch raster requires patch geometry"))?;
        if mesh.components == 0 || mesh.patches.is_empty() {
            return Err(invalid("empty patch raster geometry"));
        }
        let [a, b, c, d, e, f] = options.matrix;
        let determinant = a * d - b * c;
        if !options.matrix.iter().all(|v| v.is_finite())
            || determinant == 0.
            || options.width == 0
            || options.height == 0
        {
            return Err(invalid("patch raster transform or dimensions"));
        }
        let pixels = (options.width as usize)
            .checked_mul(options.height as usize)
            .filter(|n| n.checked_mul(4) == Some(output.len()))
            .ok_or_else(|| invalid("patch raster output shape"))?;
        const STACK: usize = 37; // Four-way depth-first traversal, maximum depth 12.
        let scratch = pixels
            .checked_mul(std::mem::size_of::<[f64; 2]>())
            .and_then(|n| n.checked_add(STACK * std::mem::size_of::<Cell>()))
            .and_then(|n| {
                mesh.components
                    .checked_mul(std::mem::size_of::<f32>())
                    .and_then(|m| n.checked_add(m))
            })
            .filter(|n| {
                *n <= options.max_scratch_bytes
                    && *n
                        <= self
                            .patch_byte_limit
                            .saturating_sub(self.retained_payload_bytes)
            })
            .ok_or_else(|| PdfError::ResourceLimit("patch raster scratch bytes".into()))?;
        let mut roots = Vec::new();
        roots
            .try_reserve_exact(pixels)
            .map_err(|_| PdfError::ResourceLimit("patch raster roots".into()))?;
        roots.resize(pixels, [-1.; 2]);
        let mut stack = Vec::new();
        stack
            .try_reserve_exact(STACK)
            .map_err(|_| PdfError::ResourceLimit("patch raster stack".into()))?;
        let mut values = Vec::new();
        values
            .try_reserve_exact(mesh.components)
            .map_err(|_| PdfError::ResourceLimit("patch raster colors".into()))?;
        values.resize(mesh.components, 0.);
        if roots.capacity() * std::mem::size_of::<[f64; 2]>()
            + stack.capacity() * std::mem::size_of::<Cell>()
            + values.capacity() * 4
            > scratch
        {
            return Err(PdfError::ResourceLimit(
                "patch raster scratch capacity".into(),
            ));
        }
        let mut color = ColorValue::new(values);
        let mut remaining = self.patch_work_limit;
        let in_bbox = |index: usize| {
            let x = (index % options.width as usize) as f64 + ox + 0.5 - e;
            let y = (index / options.width as usize) as f64 + oy + 0.5 - f;
            let px = (d * x - c * y) / determinant;
            let py = (-b * x + a * y) / determinant;
            self.bbox.is_none_or(|q| {
                px >= q[0] as f64 && py >= q[1] as f64 && px <= q[2] as f64 && py <= q[3] as f64
            })
        };
        output.fill(0);
        if options.paint_background
            && let Some(color) = &self.background
        {
            let rgb = self.colorspace.rgb_with_context(color, context)?;
            for (index, pixel) in output.chunks_exact_mut(4).enumerate() {
                if in_bbox(index) {
                    pixel.copy_from_slice(&[rgb.r(), rgb.g(), rgb.b(), 255]);
                }
            }
        }
        for patch in &mesh.patches {
            // Charge linear buffer initialization/scanning in 16-pixel blocks,
            // independently of hit count; offscreen meshes cannot evade work limits.
            let scans = pixels
                .div_ceil(16)
                .checked_mul(2)
                .ok_or_else(|| PdfError::ResourceLimit("patch raster scans".into()))?;
            remaining = remaining
                .checked_sub(scans)
                .ok_or_else(|| PdfError::ResourceLimit("patch raster scan work".into()))?;
            let common_v = patch.geometry.proves_common_v();
            roots.fill([-1.; 2]);
            let controls = patch
                .geometry
                .tensor_controls()
                .ok_or_else(|| invalid("patch raster controls"))?
                .map(|row| row.map(|[x, y]| [a * x + c * y + e, b * x + d * y + f]));
            if !controls.iter().flatten().flatten().all(|v| v.is_finite()) {
                return Err(invalid("patch device controls overflow"));
            }
            stack.push(Cell {
                controls,
                uv: [0., 0., 1., 1.],
                depth: 0,
            });
            while let Some(cell) = stack.pop() {
                work(&mut remaining)?;
                let mut bounds = [
                    f64::INFINITY,
                    f64::INFINITY,
                    f64::NEG_INFINITY,
                    f64::NEG_INFINITY,
                ];
                for [x, y] in cell.controls.iter().flatten() {
                    bounds[0] = bounds[0].min(*x);
                    bounds[1] = bounds[1].min(*y);
                    bounds[2] = bounds[2].max(*x);
                    bounds[3] = bounds[3].max(*y);
                }
                if bounds[2] < ox + 0.5
                    || bounds[3] < oy + 0.5
                    || bounds[0] > ox + options.width as f64 - 0.5
                    || bounds[1] > oy + options.height as f64 - 0.5
                {
                    continue;
                }
                let corners = [
                    cell.controls[0][0],
                    cell.controls[3][0],
                    cell.controls[3][3],
                    cell.controls[0][3],
                ];
                // Bernstein convex-hull bound for deviation from the bilinear
                // corners, plus the bilinear surface's triangle-diagonal warp.
                let mut flatness = 0f64;
                for j in 0..4 {
                    for i in 0..4 {
                        let u = i as f64 / 3.;
                        let v = j as f64 / 3.;
                        let weights = [(1. - u) * (1. - v), (1. - u) * v, u * v, u * (1. - v)];
                        let linear: [f64; 2] = std::array::from_fn(|axis| {
                            corners.iter().zip(weights).map(|(p, w)| p[axis] * w).sum()
                        });
                        flatness = flatness.max(
                            (cell.controls[j][i][0] - linear[0])
                                .hypot(cell.controls[j][i][1] - linear[1]),
                        );
                    }
                }
                let warp = (corners[0][0] - corners[1][0] + corners[2][0] - corners[3][0])
                    .hypot(corners[0][1] - corners[1][1] + corners[2][1] - corners[3][1])
                    * 0.25;
                if flatness + warp > 0.125 {
                    if cell.depth == 12 {
                        return Err(PdfError::ResourceLimit(
                            "patch raster subdivision depth".into(),
                        ));
                    }
                    let children = subdivide_tensor(cell.controls)
                        .ok_or_else(|| invalid("patch subdivision overflow"))?;
                    let [u0, v0, u1, v1] = cell.uv;
                    let um = (u0 + u1) * 0.5;
                    let vm = (v0 + v1) * 0.5;
                    let uv = [
                        [u0, v0, um, vm],
                        [um, v0, u1, vm],
                        [u0, vm, um, v1],
                        [um, vm, u1, v1],
                    ];
                    for index in (0..4).rev() {
                        stack.push(Cell {
                            controls: children[index],
                            uv: uv[index],
                            depth: cell.depth + 1,
                        });
                    }
                    continue;
                }
                let [u0, v0, u1, v1] = cell.uv;
                let uv = [[u0, v0], [u0, v1], [u1, v1], [u1, v0]];
                for indices in [[0, 1, 2], [0, 2, 3]] {
                    let p = indices.map(|i| corners[i]);
                    let parameters = indices.map(|i| uv[i]);
                    let cross = |x: [f64; 2], y: [f64; 2]| x[0] * y[1] - x[1] * y[0];
                    let sub = |x: [f64; 2], y: [f64; 2]| [x[0] - y[0], x[1] - y[1]];
                    let area = cross(sub(p[1], p[0]), sub(p[2], p[0]));
                    if !area.is_finite() {
                        return Err(invalid("patch raster triangle overflow"));
                    }
                    if area == 0. {
                        continue;
                    }
                    let minx = ((p.iter().map(|v| v[0]).fold(f64::INFINITY, f64::min) - 0.5).ceil()
                        - ox)
                        .max(0.) as u32;
                    let miny = ((p.iter().map(|v| v[1]).fold(f64::INFINITY, f64::min) - 0.5).ceil()
                        - oy)
                        .max(0.) as u32;
                    let maxx = ((p.iter().map(|v| v[0]).fold(f64::NEG_INFINITY, f64::max) - 0.5)
                        .floor()
                        - ox)
                        .min(options.width as f64 - 1.) as u32;
                    let maxy = ((p.iter().map(|v| v[1]).fold(f64::NEG_INFINITY, f64::max) - 0.5)
                        .floor()
                        - oy)
                        .min(options.height as f64 - 1.) as u32;
                    for y in miny..=maxy {
                        for x in minx..=maxx {
                            work(&mut remaining)?;
                            let q = [x as f64 + ox + 0.5, y as f64 + oy + 0.5];
                            let w = [
                                cross(sub(p[1], q), sub(p[2], q)) / area,
                                cross(sub(p[2], q), sub(p[0], q)) / area,
                                cross(sub(p[0], q), sub(p[1], q)) / area,
                            ];
                            if w.iter().any(|v| !v.is_finite()) {
                                return Err(invalid("patch raster triangle weights"));
                            }
                            if w.iter().any(|v| *v < -1e-12 || *v > 1. + 1e-12) {
                                continue;
                            }
                            let sample: [f64; 2] = std::array::from_fn(|axis| {
                                parameters
                                    .iter()
                                    .zip(w)
                                    .map(|(uv, w)| uv[axis] * w)
                                    .sum::<f64>()
                                    .clamp(0., 1.)
                            });
                            let index = y as usize * options.width as usize + x as usize;
                            let old = roots[index];
                            if old[0] < 0.
                                || (common_v && sample[0] > old[0])
                                || (!common_v
                                    && (sample[1] > old[1]
                                        || (sample[1] == old[1] && sample[0] > old[0])))
                            {
                                roots[index] = sample;
                            }
                        }
                    }
                }
            }
            for (index, [u, v]) in roots.iter().copied().enumerate() {
                if u < 0. || !in_bbox(index) {
                    continue;
                }
                work(&mut remaining)?;
                let colors = std::array::from_fn::<_, 4, _>(|i| {
                    &patch.values[i * mesh.components..(i + 1) * mesh.components]
                });
                interpolate_corners(colors, u, v, color.values_mut())
                    .ok_or_else(|| invalid("patch raster interpolation"))?;
                let rgb = if self.functions.is_empty() {
                    self.colorspace.rgb_with_context(&color, context)?
                } else {
                    self.evaluate_color(color.values(), context)?
                };
                output[index * 4..index * 4 + 4].copy_from_slice(&[rgb.r(), rgb.g(), rgb.b(), 255]);
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::Document, io::stream_reader::StreamReader, objects::pdf_stream::PdfStream,
        parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;
    fn shading(function: bool) -> Shading {
        let doc = Document::new().unwrap();
        let source = format!(
            "<< /ShadingType 6 /ColorSpace /DeviceGray /BitsPerFlag 8 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 3 0 3 0 1] {} >>",
            if function {
                "/Function << /FunctionType 2 /Domain [0 1] /N 2 >>"
            } else {
                ""
            }
        );
        let dict =
            SyntaxParser::new(StreamReader::try_new(Cursor::new(source.into_bytes())).unwrap())
                .get_object()
                .unwrap()
                .into_dict()
                .unwrap();
        let points = [
            [0, 0],
            [0, 85],
            [0, 170],
            [0, 255],
            [85, 255],
            [170, 255],
            [255, 255],
            [255, 170],
            [255, 85],
            [255, 0],
            [170, 0],
            [85, 0],
        ];
        let mut data = vec![0];
        for p in points {
            data.extend(p);
        }
        data.extend([0, 0, 255, 255]);
        Shading::try_new(&PdfObject::PdfStream(PdfStream::new(dict, data)), doc.ctx()).unwrap()
    }
    fn options() -> PatchRasterOptions {
        PatchRasterOptions {
            width: 12,
            height: 12,
            matrix: [4., 0., 0., 4., 0., 0.],
            max_scratch_bytes: 65536,
            paint_background: false,
        }
    }
    #[test]
    fn region_raster_uses_global_barycentric_coordinates_and_bounded_scratch() {
        for function in [false, true] {
            for folded in [false, true] {
                let mut shading = shading(function);
                if folded {
                    shading.patch_mesh.as_mut().unwrap().patches[0].geometry =
                        crate::color::shading_patch::CoonsPatch::try_new([
                            [3., 0.],
                            [3., 1.],
                            [3., 2.],
                            [3., 3.],
                            [-1., 3.],
                            [-1., 3.],
                            [3., 3.],
                            [3., 2.],
                            [3., 1.],
                            [3., 0.],
                            [-1., 0.],
                            [-1., 0.],
                        ])
                        .unwrap();
                }
                let options = PatchRasterOptions {
                    width: 43,
                    height: 37,
                    matrix: [13.3, 0.15, -0.2, 9.7, 2.35, 1.4],
                    max_scratch_bytes: 128 * 1024,
                    paint_background: false,
                };
                let mut full = vec![0; 43 * 37 * 4];
                shading
                    .rasterize_patch_into(options, &Default::default(), &mut full)
                    .unwrap();
                for y in (0..37).step_by(7) {
                    for x in (0..43).step_by(10) {
                        let w = 10.min(43 - x);
                        let h = 7.min(37 - y);
                        let region = PatchRasterOptions {
                            width: w,
                            height: h,
                            ..options
                        };
                        let mut tile = vec![0; (w * h * 4) as usize];
                        shading
                            .rasterize_patch_region_into(
                                region,
                                [x, y],
                                &Default::default(),
                                &mut tile,
                            )
                            .unwrap();
                        for row in 0..h as usize {
                            assert_eq!(
                                &tile[row * w as usize * 4..(row + 1) * w as usize * 4],
                                &full[((y as usize + row) * 43 + x as usize) * 4
                                    ..((y as usize + row) * 43 + x as usize + w as usize) * 4]
                            );
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn forward_raster_preserves_parameter_functions_and_later_patch_precedence() {
        for function in [false, true] {
            let mut shading = shading(function);
            let mut output = vec![0; 12 * 12 * 4];
            shading
                .rasterize_patch_into(options(), &Default::default(), &mut output)
                .unwrap();
            for y in 0..12 {
                for x in 0..12 {
                    let expected = shading
                        .color_at_point((x as f64 + 0.5) / 4., (y as f64 + 0.5) / 4.)
                        .unwrap()
                        .unwrap()
                        .r();
                    let pixel = &output[(y * 12 + x) * 4..(y * 12 + x + 1) * 4];
                    assert!(pixel[0].abs_diff(expected) <= 1);
                    assert_eq!(pixel[0], pixel[1]);
                    assert_eq!(pixel[1], pixel[2]);
                    assert_eq!(pixel[3], 255);
                }
            }
            let mut later = shading.patch_mesh.as_ref().unwrap().patches[0].clone();
            later.values.fill(0.25);
            shading.patch_mesh.as_mut().unwrap().patches.push(later);
            shading
                .rasterize_patch_into(options(), &Default::default(), &mut output)
                .unwrap();
            let expected = if function { 16u8 } else { 64u8 };
            assert!(
                output
                    .chunks_exact(4)
                    .all(|p| p[0].abs_diff(expected) <= 1 && p[3] == 255)
            );
        }
    }
    #[test]
    fn pattern_background_obeys_bbox_when_patch_is_offscreen() {
        let mut shading = shading(false);
        shading.patch_mesh.as_mut().unwrap().patches[0].geometry =
            crate::color::shading_patch::CoonsPatch::try_new([[100., 100.]; 12]).unwrap();
        shading.background = Some(ColorValue::new(vec![0.75]));
        shading.bbox = Some([0., 0., 1.5, 3.]);
        for paint in [false, true] {
            let mut options = options();
            options.paint_background = paint;
            let mut output = vec![0; 12 * 12 * 4];
            shading
                .rasterize_patch_into(options, &Default::default(), &mut output)
                .unwrap();
            for y in 0..12 {
                for x in 0..12 {
                    let pixel = &output[(y * 12 + x) * 4..(y * 12 + x + 1) * 4];
                    if paint && x < 6 {
                        assert_eq!(pixel, [191, 191, 191, 255]);
                    } else {
                        assert_eq!(pixel, [0, 0, 0, 0]);
                    }
                }
            }
        }
    }

    #[test]
    fn folded_geometry_selects_greatest_u_at_common_v_and_budgets_fail_closed() {
        let mut shading = shading(false);
        shading.patch_mesh.as_mut().unwrap().patches[0].geometry =
            crate::color::shading_patch::CoonsPatch::try_new([
                [3., 0.],
                [3., 1.],
                [3., 2.],
                [3., 3.],
                [-1., 3.],
                [-1., 3.],
                [3., 3.],
                [3., 2.],
                [3., 1.],
                [3., 0.],
                [-1., 0.],
                [-1., 0.],
            ])
            .unwrap();
        let mut output = vec![0; 12 * 12 * 4];
        shading
            .rasterize_patch_into(options(), &Default::default(), &mut output)
            .unwrap();
        for y in 0..12 {
            for x in 0..12 {
                // x=3*(2u-1)^2, y=3v; select the larger of the two u roots.
                let expected = ((1. + ((x as f64 + 0.5) / 12.).sqrt()) * 0.5 * 255.).round() as u8;
                let pixel = &output[(y * 12 + x) * 4..(y * 12 + x + 1) * 4];
                assert!(
                    pixel[0].abs_diff(expected) <= 2,
                    "x={x}: {pixel:?} vs {expected}"
                );
                assert_eq!(pixel[3], 255);
            }
        }
        let mut limited = options();
        limited.max_scratch_bytes = 1;
        assert!(matches!(
            shading.rasterize_patch_into(limited, &Default::default(), &mut output),
            Err(PdfError::ResourceLimit(_))
        ));
        shading.patch_work_limit = 0;
        assert!(matches!(
            shading.rasterize_patch_into(options(), &Default::default(), &mut output),
            Err(PdfError::ResourceLimit(_))
        ));
        let mut invalid = options();
        invalid.matrix[0] = f64::INFINITY;
        assert!(
            shading
                .rasterize_patch_into(invalid, &Default::default(), &mut output)
                .is_err()
        );
    }
}
