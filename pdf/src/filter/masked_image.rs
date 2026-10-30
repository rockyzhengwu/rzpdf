//! Joint area sampling of two forward-only Flate sample grids.
//! Color is accumulated premultiplied; only target-size straight samples escape.
use super::flate_image::{FlateImageRequest, FlateRows};
use crate::{
    budget::{BudgetedBytes, OperationBudget},
    error::{PdfError, PdfResult},
};

pub(crate) struct MaskedPreview {
    pub colors: BudgetedBytes,
    pub alpha: BudgetedBytes,
    pub recovered_tail: bool,
}

fn decoded(sample: u8, component: usize, request: &FlateImageRequest) -> f64 {
    let value = f64::from(sample) / 255.;
    request.decode.map_or(value, |endpoints| {
        (endpoints[component * 2]
            + value * (endpoints[component * 2 + 1] - endpoints[component * 2]))
            .clamp(0., 1.)
    })
}

/// The mask and color sample cells partition the same unit square. Merge their
/// boundaries with destination cells instead of separately averaging RGB and A.
/// This also preserves coverage when their source dimensions differ.
pub(crate) fn decode_soft_mask(
    color_input: &[u8],
    color: FlateImageRequest,
    mask_input: &[u8],
    mask: FlateImageRequest,
    matte: Option<&[f32]>,
    pool: &OperationBudget,
) -> PdfResult<MaskedPreview> {
    let (sw, sh) = color.source;
    let (mw, mh) = mask.source;
    let (tw, th) = color.target;
    if tw == 0
        || th == 0
        || mask.components != 1
        || !matches!(color.components, 1 | 3)
        || matte.is_some_and(|v| {
            v.len() != color.components
                || v.iter().any(|v| !v.is_finite() || !(0. ..=1.).contains(v))
        })
    {
        return Err(PdfError::ObjectError(
            "invalid masked preview layout".into(),
        ));
    }
    for (reason, actual, limit) in [
        (
            "image_pixels",
            u64::from(tw) * u64::from(th),
            color.limits.max_image_pixels,
        ),
        (
            "decoded_bytes",
            (u64::from(tw) * u64::from(th)).saturating_mul(4),
            color.limits.max_decoded_bytes as u64,
        ),
    ] {
        if actual > limit {
            return Err(PdfError::ResourceLimitExceeded {
                reason,
                stage: "image_decode",
                limit,
                actual,
            });
        }
    }
    // Opposing aspect ratios can make a joint grid much more work than either
    // image alone. Bound that independently of output and source byte budgets.
    let work = (u64::from(sw) + u64::from(mw) + u64::from(tw))
        .saturating_mul(u64::from(sh) + u64::from(mh));
    const MAX_SAMPLING_WORK: u64 = super::MAX_TARGET_SOURCE_IMAGE_PIXELS * 16;
    if work > MAX_SAMPLING_WORK {
        return Err(PdfError::ResourceLimitExceeded {
            reason: "image_sampling_work",
            stage: "image_decode",
            limit: MAX_SAMPLING_WORK,
            actual: work,
        });
    }
    let (cr, ch) = FlateRows::storage_bytes(color)?;
    let (mr, mhistory) = FlateRows::storage_bytes(mask)?;
    let pixels = tw as usize * th as usize;
    let slots = tw as usize * (color.components + 1);
    let needed = [
        cr,
        ch,
        mr,
        mhistory,
        pixels * (color.components + 1),
        slots * 16,
    ]
    .into_iter()
    .try_fold(pool.live_bytes(), |sum, n| sum.checked_add(n))
    .ok_or_else(|| PdfError::ResourceLimit("masked operation size overflow".into()))?;
    if needed > pool.maximum_bytes() {
        return Err(PdfError::ResourceLimitExceeded {
            reason: "image_operation_bytes",
            stage: "image_decode",
            limit: pool.maximum_bytes() as u64,
            actual: needed as u64,
        });
    }
    let mut colors = FlateRows::new(color_input, color, pool)?;
    let mut masks = FlateRows::new(mask_input, mask, pool)?;
    let mut output = BudgetedBytes::zeroed(pool, pixels * color.components)?;
    let mut alpha = BudgetedBytes::zeroed(pool, pixels)?;
    let mut scratch = pool.reserve(slots * 16)?;
    let mut horizontal = Vec::new();
    let mut sums = Vec::new();
    horizontal
        .try_reserve_exact(slots)
        .map_err(|_| PdfError::ResourceLimit("masked sampling allocation".into()))?;
    sums.try_reserve_exact(slots)
        .map_err(|_| PdfError::ResourceLimit("masked sampling allocation".into()))?;
    // Reconcile any allocator rounding with the reservation.
    scratch.resize((horizontal.capacity() + sums.capacity()) * 8)?;
    horizontal.resize(slots, 0f64);
    sums.resize(slots, 0f64);
    colors.next()?;
    masks.next()?;
    let (mut sy, mut my, mut dy) = (0u32, 0u32, 0u32);
    let mut y = 0.;
    let mut dirty = true;
    while dy < th {
        if dirty {
            horizontal.fill(0.);
            let (mut sx, mut mx, mut dx) = (0u32, 0u32, 0u32);
            let mut x = 0.;
            while dx < tw {
                let ce = f64::from(sx + 1) * f64::from(tw) / f64::from(sw);
                let me = f64::from(mx + 1) * f64::from(tw) / f64::from(mw);
                let de = f64::from(dx + 1);
                let end = ce.min(me).min(de);
                let weight = (end - x).max(0.);
                let a = decoded(masks.samples()[mx as usize], 0, &mask);
                let base = dx as usize * (color.components + 1);
                for c in 0..color.components {
                    let value = decoded(
                        colors.samples()[sx as usize * color.components + c],
                        c,
                        &color,
                    );
                    // Undo preblending in the source color domain, then weight
                    // by alpha. Avoid division at transparent/near-zero alpha.
                    let premultiplied = matte.map_or(value * a, |matte| {
                        (value - f64::from(matte[c]) * (1. - a)).clamp(0., a)
                    });
                    horizontal[base + c] += premultiplied * weight;
                }
                horizontal[base + color.components] += a * weight;
                x = end;
                if end == ce {
                    sx += 1;
                }
                if end == me {
                    mx += 1;
                }
                if end == de {
                    dx += 1;
                }
            }
            dirty = false;
        }
        let ce = f64::from(sy + 1) * f64::from(th) / f64::from(sh);
        let me = f64::from(my + 1) * f64::from(th) / f64::from(mh);
        let de = f64::from(dy + 1);
        let end = ce.min(me).min(de);
        let weight = (end - y).max(0.);
        for (sum, value) in sums.iter_mut().zip(&horizontal) {
            *sum += value * weight;
        }
        y = end;
        if end == de {
            for x in 0..tw as usize {
                let base = x * (color.components + 1);
                let a = sums[base + color.components].clamp(0., 1.);
                let pixel = dy as usize * tw as usize + x;
                alpha.as_mut_slice()[pixel] = (a * 255.).round() as u8;
                for c in 0..color.components {
                    output.as_mut_slice()[pixel * color.components + c] = if a > 0. {
                        (sums[base + c] / a * 255.).clamp(0., 255.).round() as u8
                    } else {
                        0
                    };
                }
            }
            sums.fill(0.);
            dy += 1;
        }
        if end == ce {
            sy += 1;
            if sy < sh {
                colors.next()?;
            }
            dirty = true;
        }
        if end == me {
            my += 1;
            if my < mh {
                masks.next()?;
            }
            dirty = true;
        }
    }
    // Neither a complete base image nor a complete target is sufficient: verify
    // both zlib checksums before atomically publishing the pair.
    let recovered_color = colors.finish()?;
    let recovered_mask = masks.finish()?;
    Ok(MaskedPreview {
        colors: output,
        alpha,
        recovered_tail: recovered_color || recovered_mask,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{filter::flate_image::Predictor8, parser::options::Limits};
    use std::io::Write;
    fn compress(bytes: &[u8]) -> Vec<u8> {
        let mut z = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        z.write_all(bytes).unwrap();
        z.finish().unwrap()
    }
    fn request(source: (u32, u32), target: (u32, u32), components: usize) -> FlateImageRequest {
        FlateImageRequest {
            source,
            target,
            components,
            predictor: Predictor8::from_parameters(None, source.0, components)
                .unwrap()
                .unwrap(),
            decode: None,
            compatible: false,
            limits: Limits {
                max_image_pixels: 4_000_000,
                max_decoded_bytes: 16 * 1024 * 1024,
                ..Limits::default()
            },
        }
    }
    // Independent 2D rectangle intersections: deliberately does not use the
    // decoder's merged scanline traversal or separable accumulation.
    fn oracle(
        colors: &[u8],
        c: FlateImageRequest,
        alpha: &[u8],
        m: FlateImageRequest,
        matte: Option<&[f32]>,
    ) -> (Vec<u8>, Vec<u8>) {
        let mut out = Vec::new();
        let mut opacity = Vec::new();
        for dy in 0..c.target.1 {
            for dx in 0..c.target.0 {
                let mut sums = [0.; 3];
                let mut asum = 0.;
                let x0 = f64::from(dx) / f64::from(c.target.0);
                let x1 = f64::from(dx + 1) / f64::from(c.target.0);
                let y0 = f64::from(dy) / f64::from(c.target.1);
                let y1 = f64::from(dy + 1) / f64::from(c.target.1);
                for cy in 0..c.source.1 {
                    for cx in 0..c.source.0 {
                        for my in 0..m.source.1 {
                            for mx in 0..m.source.0 {
                                let left = x0
                                    .max(f64::from(cx) / f64::from(c.source.0))
                                    .max(f64::from(mx) / f64::from(m.source.0));
                                let right = x1
                                    .min(f64::from(cx + 1) / f64::from(c.source.0))
                                    .min(f64::from(mx + 1) / f64::from(m.source.0));
                                let top = y0
                                    .max(f64::from(cy) / f64::from(c.source.1))
                                    .max(f64::from(my) / f64::from(m.source.1));
                                let bottom = y1
                                    .min(f64::from(cy + 1) / f64::from(c.source.1))
                                    .min(f64::from(my + 1) / f64::from(m.source.1));
                                let weight = (right - left).max(0.) * (bottom - top).max(0.);
                                let mask_sample = alpha[(my * m.source.0 + mx) as usize];
                                let a = m.decode.map_or(f64::from(mask_sample) / 255., |e| {
                                    (e[0] + f64::from(mask_sample) / 255. * (e[1] - e[0]))
                                        .clamp(0., 1.)
                                });
                                asum += a * weight;
                                for k in 0..c.components {
                                    let sample =
                                        colors[(cy * c.source.0 + cx) as usize * c.components + k];
                                    let value = c.decode.map_or(f64::from(sample) / 255., |e| {
                                        (e[k * 2]
                                            + f64::from(sample) / 255. * (e[k * 2 + 1] - e[k * 2]))
                                            .clamp(0., 1.)
                                    });
                                    let straight = matte.map_or(value, |v| {
                                        if a > 0. {
                                            ((value - f64::from(v[k])) / a + f64::from(v[k]))
                                                .clamp(0., 1.)
                                        } else {
                                            0.
                                        }
                                    });
                                    sums[k] += straight * a * weight;
                                }
                            }
                        }
                    }
                }
                for v in sums.iter().take(c.components) {
                    out.push(if asum > 0. {
                        (v / asum * 255.).round() as u8
                    } else {
                        0
                    });
                }
                opacity.push((asum / ((x1 - x0) * (y1 - y0)) * 255.).round() as u8);
            }
        }
        (out, opacity)
    }
    #[test]
    fn different_grids_decode_and_matte_match_independent_joint_oracle() {
        for components in [1, 3] {
            for (cs, ms) in [
                ((7, 5), (3, 2)),
                ((3, 2), (7, 5)),
                ((5, 3), (5, 3)),
                ((1, 3), (5, 1)),
            ] {
                let colors: Vec<_> = (0..cs.0 * cs.1 * components as u32)
                    .map(|v| ((v * 71 + 23) % 256) as u8)
                    .collect();
                let alpha: Vec<_> = (0..ms.0 * ms.1).map(|v| ((v * 83) % 256) as u8).collect();
                for target in [(1, 1), (cs.0, cs.1), (9, 7)] {
                    for invert in [false, true] {
                        for matte in [None, Some(&[0.8, 0.3, 1.][..components])] {
                            let mut c = request(cs, target, components);
                            let mut m = request(ms, target, 1);
                            if invert {
                                c.decode = Some([1., 0., 0.2, 0.8, -0.2, 1.2, 0., 1.]);
                                m.decode = Some([1., 0., 0., 1., 0., 1., 0., 1.]);
                            }
                            let pool = OperationBudget::new(1_000_000);
                            let actual = decode_soft_mask(
                                &compress(&colors),
                                c,
                                &compress(&alpha),
                                m,
                                matte,
                                &pool,
                            )
                            .unwrap();
                            let expected = oracle(&colors, c, &alpha, m, matte);
                            for (a, b) in actual.colors.as_slice().iter().zip(&expected.0) {
                                assert!(a.abs_diff(*b) <= 1, "color {a} != {b}");
                            }
                            for (a, b) in actual.alpha.as_slice().iter().zip(&expected.1) {
                                assert!(a.abs_diff(*b) <= 1, "alpha {a} != {b}");
                            }
                            assert_eq!(
                                pool.live_bytes(),
                                actual.colors.capacity() + actual.alpha.capacity()
                            );
                            drop(actual);
                            assert_eq!(pool.live_bytes(), 0);
                        }
                    }
                }
            }
        }
    }
    #[test]
    fn invisible_colors_do_not_bleed_and_matte_is_unblended_before_sampling() {
        let c = request((2, 1), (1, 1), 3);
        let m = request((2, 1), (1, 1), 1);
        let pool = OperationBudget::new(4096);
        let result = decode_soft_mask(
            &compress(&[255, 0, 0, 0, 0, 0]),
            c,
            &compress(&[255, 0]),
            m,
            None,
            &pool,
        )
        .unwrap();
        assert_eq!(result.colors.as_slice(), [255, 0, 0]);
        assert_eq!(result.alpha.as_slice(), [128]);
        let result = decode_soft_mask(
            &compress(&[255, 127, 127, 255, 255, 255]),
            c,
            &compress(&[128, 0]),
            m,
            Some(&[1., 1., 1.]),
            &pool,
        )
        .unwrap();
        assert_eq!(result.colors.as_slice(), [255, 0, 0]);
        assert_eq!(result.alpha.as_slice(), [64]);
        for a in [0, 255] {
            let result =
                decode_soft_mask(&compress(&[70; 6]), c, &compress(&[a; 2]), m, None, &pool)
                    .unwrap();
            assert_eq!(result.alpha.as_slice(), [a]);
            assert_eq!(result.colors.as_slice(), [if a == 0 { 0 } else { 70 }; 3]);
        }
    }
    #[test]
    fn large_pair_is_bounded_and_either_bad_checksum_releases_all_storage() {
        let pool = OperationBudget::new(64 * 1024 * 1024);
        let c = request((2665, 1916), (767, 551), 3);
        let m = request((1333, 958), (767, 551), 1);
        let colors = compress(&vec![180; 2665 * 1916 * 3]);
        let alpha = compress(&vec![128; 1333 * 958]);
        let result = decode_soft_mask(&colors, c, &alpha, m, None, &pool).unwrap();
        assert!(result.colors.as_slice().iter().all(|v| *v == 180));
        assert!(result.alpha.as_slice().iter().all(|v| *v == 128));
        assert!(pool.peak_bytes() < 2 * 1024 * 1024, "{}", pool.peak_bytes());
        drop(result);
        assert_eq!(pool.live_bytes(), 0);
        for corrupt_color in [false, true] {
            let mut bad_color = colors.clone();
            let mut bad_mask = alpha.clone();
            let bad = if corrupt_color {
                &mut bad_color
            } else {
                &mut bad_mask
            };
            *bad.last_mut().unwrap() ^= 1;
            assert!(decode_soft_mask(&bad_color, c, &bad_mask, m, None, &pool).is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
        let result = decode_soft_mask(&colors, c, &alpha, m, None, &pool).unwrap();
        drop(result);
        assert_eq!(pool.live_bytes(), 0);
    }
    #[test]
    fn source_target_work_and_live_storage_limits_are_independent() {
        let c = request((2, 2), (1, 1), 3);
        let m = request((2, 2), (1, 1), 1);
        for (mut color, mut mask, reason) in [
            (c, request((4001, 4000), (1, 1), 1), "image_source_pixels"),
            (request((6000, 1000), (1, 1), 3), m, "image_source_bytes"),
            (c, m, "decoded_bytes"),
            (
                request((16000, 1), (1, 1), 3),
                request((1, 16000), (1, 1), 1),
                "image_sampling_work",
            ),
        ] {
            if reason == "decoded_bytes" {
                color.limits.max_decoded_bytes = 3;
                mask.limits = color.limits;
            }
            let pool = OperationBudget::new(4096);
            assert!(
                matches!(decode_soft_mask(&[],color,&[],mask,None,&pool),Err(PdfError::ResourceLimitExceeded {reason:r,..}) if r==reason),
                "{reason}"
            );
            assert_eq!(pool.live_bytes(), 0);
        }
        let pool = OperationBudget::new(8);
        assert!(
            decode_soft_mask(
                &compress(&[255; 12]),
                c,
                &compress(&[128; 4]),
                m,
                None,
                &pool
            )
            .is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
    }
}
