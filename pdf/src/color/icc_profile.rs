//! Bounded ICC input and shared, finite transform cache. Backend allocations
//! are limited by parser/table caps, not charged to an operation-wide lease.
use super::{
    conversion::{ColorConversionContext, RenderingIntent},
    value::ColorRgb,
};
use crate::{
    error::{PdfError, PdfResult},
    parser::options::Limits,
};
use std::sync::{
    Arc, Mutex, OnceLock,
    atomic::{AtomicU64, Ordering},
};
#[cfg(test)]
#[path = "icc_profile_tests.rs"]
mod tests;

#[derive(Clone)]
pub struct ICCProfile {
    inner: Arc<ProfileData>,
}
struct ProfileData {
    id: u64,
    components: usize,
    raw: Vec<u8>,
    profile: moxcms::ColorProfile,
    transforms: Mutex<Vec<CachedTransform>>,
}
struct CachedTransform {
    target: u64,
    intent: RenderingIntent,
    transform: Arc<moxcms::TransformF32Executor>,
}
static NEXT_PROFILE: AtomicU64 = AtomicU64::new(1);
impl std::fmt::Debug for ICCProfile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ICCProfile")
            .field("backend", &"moxcms 0.9.1")
            .field("components", &self.inner.components)
            .field("encoded_bytes", &self.inner.raw.len())
            .finish()
    }
}
impl ICCProfile {
    pub fn srgb() -> Self {
        static SRGB: OnceLock<ICCProfile> = OnceLock::new();
        SRGB.get_or_init(|| Self {
            inner: Arc::new(ProfileData {
                id: 0,
                components: 3,
                raw: Vec::new(),
                profile: moxcms::ColorProfile::new_srgb(),
                transforms: Mutex::new(Vec::new()),
            }),
        })
        .clone()
    }
    /// A generated gray profile using the sRGB transfer curve, for explicitly
    /// chosen nominal gray display inputs (not an implicit print profile).
    pub fn gray_srgb() -> PdfResult<Self> {
        let mut profile = moxcms::ColorProfile::new_gray_with_gamma(1.);
        profile.gray_trc = moxcms::ColorProfile::new_srgb().red_trc;
        let data = profile
            .encode()
            .map_err(|e| PdfError::ColorError(format!("gray ICC encoding: {e:?}")))?;
        Self::try_new(&data)
    }
    pub fn try_new(data: &[u8]) -> PdfResult<Self> {
        Self::try_new_with_limits(data, Limits::default())
    }
    pub fn try_new_with_limits(data: &[u8], limits: Limits) -> PdfResult<Self> {
        let invalid = |message: &str| PdfError::ColorError(format!("ICC profile {message}"));
        let allowance = limits.max_decoded_bytes.min(16 * 1024 * 1024);
        if data.len() > allowance {
            return Err(PdfError::ResourceLimit("ICC profile encoded bytes".into()));
        }
        if data.len() < 132 || &data[36..40] != b"acsp" {
            return Err(invalid("header/signature"));
        }
        let integer = |offset: usize| {
            u32::from_be_bytes([
                data[offset],
                data[offset + 1],
                data[offset + 2],
                data[offset + 3],
            ]) as usize
        };
        let size = integer(0);
        let count = integer(128);
        let end = count
            .checked_mul(12)
            .and_then(|v| v.checked_add(132))
            .ok_or_else(|| invalid("tag table overflow"))?;
        if size < 132 || size > data.len() || end > size {
            return Err(invalid("truncated size or tag table"));
        }
        if count > limits.max_objects {
            return Err(PdfError::ResourceLimit("ICC tag count".into()));
        }
        if !matches!(&data[12..16], b"scnr" | b"mntr" | b"prtr" | b"spac") {
            return Err(PdfError::UnsupportedFeature("ICC profile class".into()));
        }
        if ![2, 4].contains(&data[8]) {
            return Err(PdfError::UnsupportedFeature(
                "ICC profile major version".into(),
            ));
        }
        let mut total = 0usize;
        for index in 0..count {
            let offset = integer(132 + index * 12 + 4);
            let length = integer(132 + index * 12 + 8);
            if offset < end || length < 8 || offset.checked_add(length).is_none_or(|v| v > size) {
                return Err(invalid("tag extent"));
            }
            total = total
                .checked_add(length)
                .filter(|v| *v <= allowance)
                .ok_or_else(|| PdfError::ResourceLimit("ICC cumulative tag payload".into()))?;
        }
        let profile = moxcms::ColorProfile::new_from_slice_with_options(
            &data[..size],
            moxcms::ParsingOptions {
                max_profile_size: allowance.saturating_add(1),
                max_allowed_clut_size: allowance.min(1024 * 1024),
                max_allowed_trc_size: (allowance / 4).min(40_000),
            },
        )
        .map_err(|error| invalid(&format!("parse: {error:?}")))?;
        let components = match profile.color_space {
            moxcms::DataColorSpace::Gray => 1,
            moxcms::DataColorSpace::Rgb => 3,
            moxcms::DataColorSpace::Cmyk => 4,
            _ => {
                return Err(PdfError::UnsupportedFeature(
                    "ICC input colour space other than Gray/RGB/CMYK".into(),
                ));
            }
        };
        let id = NEXT_PROFILE
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| PdfError::ResourceLimit("ICC profile identity".into()))?;
        let mut raw = Vec::new();
        raw.try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("ICC encoded profile allocation".into()))?;
        raw.extend_from_slice(&data[..size]);
        Ok(Self {
            inner: Arc::new(ProfileData {
                id,
                components,
                raw,
                profile,
                transforms: Mutex::new(Vec::new()),
            }),
        })
    }
    pub fn number_of_components(&self) -> usize {
        self.inner.components
    }
    pub(crate) fn retained_source_bytes(&self) -> usize {
        self.inner.raw.capacity()
    }
    pub fn is_builtin_srgb(&self) -> bool {
        self.inner.id == 0
    }
    pub fn to_bytes(&self) -> PdfResult<Vec<u8>> {
        if !self.inner.raw.is_empty() {
            let mut data = Vec::new();
            data.try_reserve_exact(self.inner.raw.len())
                .map_err(|_| PdfError::ResourceLimit("ICC output profile bytes".into()))?;
            data.extend_from_slice(&self.inner.raw);
            return Ok(data);
        }
        self.inner
            .profile
            .encode()
            .map_err(|e| PdfError::ColorError(format!("ICC encoding: {e:?}")))
    }
    pub fn convert(&self, values: &[f32], context: &ColorConversionContext) -> PdfResult<ColorRgb> {
        if values.len() != self.inner.components || values.iter().any(|v| !v.is_finite()) {
            return Err(PdfError::ColorError(
                "ICC input component count or value".into(),
            ));
        }
        let target = context.output_profile();
        if self.inner.id == target.inner.id && self.inner.components == 3 {
            return Ok(ColorRgb::new(
                values[0].clamp(0., 1.),
                values[1].clamp(0., 1.),
                values[2].clamp(0., 1.),
            ));
        }
        let transform = {
            let mut cache = self
                .inner
                .transforms
                .lock()
                .map_err(|_| PdfError::ColorError("ICC transform cache poisoned".into()))?;
            if let Some(hit) = cache
                .iter()
                .find(|v| v.target == target.inner.id && v.intent == context.rendering_intent)
            {
                hit.transform.clone()
            } else {
                let layout = match self.inner.components {
                    1 => moxcms::Layout::Gray,
                    3 => moxcms::Layout::Rgb,
                    4 => moxcms::Layout::Rgba,
                    _ => return Err(PdfError::ColorError("ICC input layout".into())),
                };
                // moxcms selects A2B1 for Absolute but does not apply ICC's
                // media-white scaling. Incorporate it into an RGB matrix target
                // before TRCs, preserving unclipped PCS values and LUT sources.
                let mut absolute_target;
                let destination =
                    if context.rendering_intent == RenderingIntent::AbsoluteColorimetric {
                        if !target.inner.profile.is_matrix_shaper() {
                            return Err(PdfError::UnsupportedFeature(
                                "absolute ICC intent requires a matrix/TRC RGB target".into(),
                            ));
                        }
                        absolute_target = target.inner.profile.clone();
                        let source = self
                            .inner
                            .profile
                            .media_white_point
                            .unwrap_or(self.inner.profile.white_point);
                        let white = target
                            .inner
                            .profile
                            .media_white_point
                            .unwrap_or(target.inner.profile.white_point);
                        let ratios = [white.x / source.x, white.y / source.y, white.z / source.z];
                        if ratios.iter().any(|v| !v.is_finite() || *v <= 0.) {
                            return Err(PdfError::ColorError("ICC absolute media white".into()));
                        }
                        for colorant in [
                            &mut absolute_target.red_colorant,
                            &mut absolute_target.green_colorant,
                            &mut absolute_target.blue_colorant,
                        ] {
                            colorant.x *= ratios[0];
                            colorant.y *= ratios[1];
                            colorant.z *= ratios[2];
                        }
                        &absolute_target
                    } else {
                        &target.inner.profile
                    };
                let transform = self
                    .inner
                    .profile
                    .create_transform_f32(
                        layout,
                        destination,
                        moxcms::Layout::Rgb,
                        moxcms::TransformOptions {
                            rendering_intent: context.rendering_intent.backend(),
                            allow_use_cicp_transfer: false,
                            prefer_fixed_point: false,
                            ..Default::default()
                        },
                    )
                    .map_err(|e| PdfError::UnsupportedFeature(format!("ICC transform: {e:?}")))?;
                if cache.len() == 8 {
                    cache.remove(0);
                }
                cache
                    .try_reserve(1)
                    .map_err(|_| PdfError::ResourceLimit("ICC transform cache".into()))?;
                cache.push(CachedTransform {
                    target: target.inner.id,
                    intent: context.rendering_intent,
                    transform: transform.clone(),
                });
                transform
            }
        };
        let mut source = [0f32; 4];
        for (dst, src) in source.iter_mut().zip(values) {
            *dst = src.clamp(0., 1.);
        }
        let mut rgb = [0f32; 3];
        transform
            .transform(&source[..values.len()], &mut rgb)
            .map_err(|e| PdfError::ColorError(format!("ICC conversion: {e:?}")))?;
        if rgb.iter().any(|v| !v.is_finite()) {
            return Err(PdfError::ColorError("ICC nonfinite output".into()));
        }
        let [r, g, b] = rgb.map(|v| v.clamp(0., 1.));
        Ok(ColorRgb::new(r, g, b))
    }
}
