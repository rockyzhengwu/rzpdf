//! Borrowed static OpenType vhea/vmtx metrics; no shaping or substitutions.
use crate::error::{PdfError, PdfResult};

pub struct VerticalMetrics<'a> {
    data: &'a [u8],
    count: u16,
    glyphs: u16,
}

impl<'a> VerticalMetrics<'a> {
    pub fn try_new(vhea: &[u8], vmtx: &'a [u8], glyphs: u16) -> PdfResult<Self> {
        if vhea.len() < 36 {
            return Err(PdfError::FontError("Truncated vhea metrics".into()));
        }
        let version = u32::from_be_bytes([vhea[0], vhea[1], vhea[2], vhea[3]]);
        if !matches!(version, 0x00010000 | 0x00011000) || vhea[24..34].iter().any(|&byte| byte != 0)
        {
            return Err(PdfError::FontError("Invalid vhea metrics header".into()));
        }
        let count = u16::from_be_bytes([vhea[34], vhea[35]]);
        if count == 0 || count > glyphs {
            return Err(PdfError::FontError("Invalid vertical metric count".into()));
        }
        let required = usize::from(count) * 4 + usize::from(glyphs - count) * 2;
        if vmtx.len() < required {
            return Err(PdfError::FontError("Truncated vmtx metrics".into()));
        }
        Ok(Self {
            data: vmtx,
            count,
            glyphs,
        })
    }

    /// Unscaled advance height and signed top sidebearing in font units.
    pub fn metric(&self, gid: u16) -> PdfResult<(u16, i16)> {
        if gid >= self.glyphs {
            return Err(PdfError::FontError(
                "vertical metric GID exceeds glyph count".into(),
            ));
        }
        let advance = usize::from(gid.min(self.count - 1)) * 4;
        let bearing = if gid < self.count {
            usize::from(gid) * 4 + 2
        } else {
            usize::from(self.count) * 4 + usize::from(gid - self.count) * 2
        };
        Ok((
            u16::from_be_bytes([self.data[advance], self.data[advance + 1]]),
            i16::from_be_bytes([self.data[bearing], self.data[bearing + 1]]),
        ))
    }

    /// Static TrueType PDF W2 triple [w1y, v1x, v1y] in 1000-em units.
    /// The caller supplies checked horizontal advance and glyph yMax from
    /// the same source. The vertical origin is yMax + topSideBearing; glyphs
    /// with no outline use yMax=0. No BASE/shaping adjustment is applied.
    pub fn pdf_metric(
        &self,
        gid: u16,
        units_per_em: u16,
        horizontal_advance: u16,
        y_max: i16,
    ) -> PdfResult<[f64; 3]> {
        if !(16..=16384).contains(&units_per_em) {
            return Err(PdfError::FontError("Invalid vertical metric units".into()));
        }
        let (advance, bearing) = self.metric(gid)?;
        let scale = 1000.0 / f64::from(units_per_em);
        // Add in i32: two valid signed 16-bit font values can overflow i16.
        let origin_y = i32::from(y_max) + i32::from(bearing);
        Ok([
            -f64::from(advance) * scale,
            f64::from(horizontal_advance) * scale / 2.0,
            f64::from(origin_y) * scale,
        ])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn trailing_metrics_reuse_advance_but_keep_signed_bearings() {
        let mut header = [0; 36];
        header[..4].copy_from_slice(&0x00011000u32.to_be_bytes());
        header[34..].copy_from_slice(&2u16.to_be_bytes());
        let data = [0x03, 0xe8, 0xff, 0xfe, 0x04, 0x00, 0, 3, 0xff, 0xfb, 0, 7];
        let metrics = VerticalMetrics::try_new(&header, &data, 4).unwrap();
        for (gid, expected) in [
            (0, (1000, -2)),
            (1, (1024, 3)),
            (2, (1024, -5)),
            (3, (1024, 7)),
        ] {
            assert_eq!(metrics.metric(gid).unwrap(), expected);
        }
        assert!(metrics.metric(4).is_err());
        assert_eq!(
            metrics.pdf_metric(2, 1024, 512, 805).unwrap(),
            [-1000.0, 250.0, 781.25]
        );
        assert_eq!(
            metrics.pdf_metric(3, 1024, 512, i16::MAX).unwrap()[2],
            32774.0 * 1000.0 / 1024.0
        );
        assert!(metrics.pdf_metric(0, 0, 512, 0).is_err());
        for end in 0..data.len() {
            assert!(VerticalMetrics::try_new(&header, &data[..end], 4).is_err());
        }
        header[24] = 1;
        assert!(VerticalMetrics::try_new(&header, &data, 4).is_err());
    }

    #[test]
    fn versions_counts_and_header_boundaries_are_explicit() {
        let mut header = [0; 36];
        header[34..].copy_from_slice(&1u16.to_be_bytes());
        for version in [0x00010000u32, 0x00011000] {
            header[..4].copy_from_slice(&version.to_be_bytes());
            assert!(VerticalMetrics::try_new(&header, &[0; 4], 1).is_ok());
        }
        for end in 0..36 {
            assert!(VerticalMetrics::try_new(&header[..end], &[0; 4], 1).is_err());
        }
        for version in [0u32, 0x00010001, 0x00020000] {
            header[..4].copy_from_slice(&version.to_be_bytes());
            assert!(VerticalMetrics::try_new(&header, &[0; 4], 1).is_err());
        }
        header[..4].copy_from_slice(&0x00010000u32.to_be_bytes());
        for count in [0u16, 2, u16::MAX] {
            header[34..].copy_from_slice(&count.to_be_bytes());
            assert!(VerticalMetrics::try_new(&header, &[0; 4], 1).is_err());
        }
        header[34..].copy_from_slice(&1u16.to_be_bytes());
        for offset in 24..34 {
            header[offset] = 1;
            assert!(VerticalMetrics::try_new(&header, &[0; 4], 1).is_err());
            header[offset] = 0;
        }
    }
}
