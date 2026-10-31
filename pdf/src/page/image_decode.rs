//! Render-only sizing, independent of codec allocation and source PDF objects.
use crate::{
    error::{PdfError, PdfResult},
    geom::matrix::Matrix,
    parser::options::Limits,
};

#[derive(Clone, Copy, Debug)]
pub(crate) struct ImageDecodePlan {
    pub source: (u32, u32),
    pub target: (u32, u32),
}
impl ImageDecodePlan {
    pub fn for_render(source: (u32, u32), matrix: &Matrix, dpi: f32) -> PdfResult<Self> {
        if source.0 == 0 || source.1 == 0 {
            return Err(PdfError::ObjectError(
                "image dimensions must be positive".into(),
            ));
        }
        let scale = f64::from(dpi) / 72.;
        let width = f64::from(matrix.a()).hypot(f64::from(matrix.b())) * scale;
        let height = f64::from(matrix.c()).hypot(f64::from(matrix.d())) * scale;
        if !width.is_finite() || !height.is_finite() || !dpi.is_finite() || dpi <= 0. {
            return Err(PdfError::ObjectError(
                "image target dimensions nonfinite or invalid".into(),
            ));
        }
        Ok(Self {
            source,
            target: (
                width.ceil().max(1.).min(f64::from(source.0)) as u32,
                height.ceil().max(1.).min(f64::from(source.1)) as u32,
            ),
        })
    }
    pub fn full_fits(source: (u32, u32), limits: Limits) -> bool {
        let pixels = u64::from(source.0) * u64::from(source.1);
        pixels <= limits.max_image_pixels
            && pixels
                .checked_mul(4)
                .is_some_and(|bytes| bytes <= limits.max_decoded_bytes as u64)
    }
    pub fn validate(&self, limits: Limits) -> PdfResult<()> {
        for (reason, actual, limit) in [
            (
                "image_pixels",
                u64::from(self.target.0) * u64::from(self.target.1),
                limits.max_image_pixels,
            ),
            (
                "decoded_bytes",
                (u64::from(self.target.0) * u64::from(self.target.1)).saturating_mul(4),
                limits.max_decoded_bytes as u64,
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
        Ok(())
    }
}
