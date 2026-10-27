//! Cover the continuous display extent with whole pixels, without first
//! rounding DPI/72 to f32. Geometry transforms retain their backend precision.
use pdf::error::{PdfError, PdfResult};

/// Return integral f64 extents so callers can apply their own pixel, byte and
/// side limits before converting to an integer allocation size.
pub fn raster_dimensions(width: f32, height: f32, xdpi: f32, ydpi: f32) -> PdfResult<(f64, f64)> {
    if [width, height, xdpi, ydpi]
        .iter()
        .any(|v| !v.is_finite() || *v <= 0.)
    {
        return Err(PdfError::DocumentError(
            "invalid render page dimensions or DPI".into(),
        ));
    }
    Ok((
        (f64::from(width) * f64::from(xdpi) / 72.).ceil(),
        (f64::from(height) * f64::from(ydpi) / 72.).ceil(),
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn covers_fractional_edges_without_extra_pixels_at_common_dpi() {
        assert_eq!(
            raster_dimensions(612.283, 790.866, 72., 72.).unwrap(),
            (613., 791.)
        );
        assert_eq!(
            raster_dimensions(600., 792., 96., 96.).unwrap(),
            (800., 1056.)
        );
        assert_eq!(raster_dimensions(0.1, 0.2, 72., 72.).unwrap(), (1., 1.));
        for dpi in [72., 96., 120., 144., 288., 576., 1152.] {
            for size in [1., 36., 72., 600., 612., 1000., 1000.125] {
                let (w, h) = raster_dimensions(size, size, dpi, dpi).unwrap();
                let exact = f64::from(size) * f64::from(dpi) / 72.;
                assert!(w >= exact && w - exact < 1.);
                assert_eq!(w, h);
            }
        }
        for value in [0., -1., f32::NAN, f32::INFINITY] {
            assert!(raster_dimensions(value, 1., 72., 72.).is_err());
            assert!(raster_dimensions(1., 1., value, 72.).is_err());
        }
    }
}
