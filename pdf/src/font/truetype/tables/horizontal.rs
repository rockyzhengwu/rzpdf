use crate::error::{PdfError, PdfResult};

#[derive(Debug, Clone)]
pub struct HorizontalMetrics {
    units_per_em: u16,
    glyph_count: u16,
    advances: Vec<u16>,
    bounds: [i16; 4],
    ascent: i16,
    descent: i16,
}

impl HorizontalMetrics {
    pub fn try_new(head: &[u8], hhea: &[u8], hmtx: &[u8], glyph_count: u16) -> PdfResult<Self> {
        if head.len() < 54 || hhea.len() < 36 {
            return Err(PdfError::FontError("Truncated head/hhea metrics".into()));
        }
        let u16_at =
            |data: &[u8], offset: usize| u16::from_be_bytes([data[offset], data[offset + 1]]);
        let i16_at =
            |data: &[u8], offset: usize| i16::from_be_bytes([data[offset], data[offset + 1]]);
        if head[..4] != [0, 1, 0, 0]
            || head[12..16] != [0x5f, 0x0f, 0x3c, 0xf5]
            || hhea[..4] != [0, 1, 0, 0]
            || hhea[24..34].iter().any(|byte| *byte != 0)
        {
            return Err(PdfError::FontError(
                "Invalid head/hhea metrics header".into(),
            ));
        }
        let units_per_em = u16_at(head, 18);
        let count = u16_at(hhea, 34);
        if !(16..=16384).contains(&units_per_em) || count == 0 || count > glyph_count {
            return Err(PdfError::FontError(
                "Invalid font units or horizontal metric count".into(),
            ));
        }
        let required = usize::from(count) * 4 + usize::from(glyph_count - count) * 2;
        if hmtx.len() < required {
            return Err(PdfError::FontError("Truncated hmtx metrics".into()));
        }
        let bounds = [
            i16_at(head, 36),
            i16_at(head, 38),
            i16_at(head, 40),
            i16_at(head, 42),
        ];
        if bounds[0] > bounds[2] || bounds[1] > bounds[3] {
            return Err(PdfError::FontError("Invalid head bounding box".into()));
        }
        let mut advances = Vec::new();
        advances
            .try_reserve_exact(usize::from(count))
            .map_err(|_| PdfError::ResourceLimit("horizontal metrics allocation".into()))?;
        for index in 0..usize::from(count) {
            advances.push(u16_at(hmtx, index * 4));
        }
        Ok(Self {
            units_per_em,
            glyph_count,
            advances,
            bounds,
            ascent: i16_at(hhea, 4),
            descent: i16_at(hhea, 6),
        })
    }
    pub fn units_per_em(&self) -> u16 {
        self.units_per_em
    }
    pub fn bounds(&self) -> [i16; 4] {
        self.bounds
    }
    pub fn ascent(&self) -> i16 {
        self.ascent
    }
    pub fn descent(&self) -> i16 {
        self.descent
    }
    pub fn advance_width(&self, gid: u16) -> PdfResult<u16> {
        if gid >= self.glyph_count {
            return Err(PdfError::FontError("metric GID exceeds glyph count".into()));
        }
        Ok(self.advances[usize::from(gid).min(self.advances.len() - 1)])
    }
    /// Scaled horizontal advance; not shaped text or a vertical metric.
    pub fn advance_width_1000(&self, gid: u16) -> PdfResult<f64> {
        Ok(f64::from(self.advance_width(gid)?) * 1000.0 / f64::from(self.units_per_em))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn tables() -> (Vec<u8>, Vec<u8>, Vec<u8>) {
        let mut head = vec![0; 54];
        head[..4].copy_from_slice(&[0, 1, 0, 0]);
        head[12..16].copy_from_slice(&[0x5f, 0x0f, 0x3c, 0xf5]);
        head[18..20].copy_from_slice(&1024u16.to_be_bytes());
        head[38..40].copy_from_slice(&(-200i16).to_be_bytes());
        head[40..42].copy_from_slice(&1000i16.to_be_bytes());
        head[42..44].copy_from_slice(&800i16.to_be_bytes());
        let mut hhea = vec![0; 36];
        hhea[..4].copy_from_slice(&[0, 1, 0, 0]);
        hhea[4..6].copy_from_slice(&800i16.to_be_bytes());
        hhea[6..8].copy_from_slice(&(-200i16).to_be_bytes());
        hhea[34..].copy_from_slice(&2u16.to_be_bytes());
        let hmtx = [500u16, 0, 1000, 0, 0, 0]
            .into_iter()
            .flat_map(u16::to_be_bytes)
            .collect();
        (head, hhea, hmtx)
    }

    #[test]
    fn trailing_glyphs_reuse_last_advance_and_scaling_retains_fraction() {
        let (head, hhea, hmtx) = tables();
        let metrics = HorizontalMetrics::try_new(&head, &hhea, &hmtx, 4).unwrap();
        assert_eq!(metrics.advance_width(0).unwrap(), 500);
        for gid in 1..4 {
            assert_eq!(metrics.advance_width(gid).unwrap(), 1000);
        }
        assert_eq!(metrics.advance_width_1000(0).unwrap(), 488.28125);
        assert_eq!(metrics.bounds(), [0, -200, 1000, 800]);
        assert_eq!((metrics.ascent(), metrics.descent()), (800, -200));
        assert!(metrics.advance_width(4).is_err());
    }

    #[test]
    fn metric_lengths_counts_and_units_are_checked() {
        let (mut head, mut hhea, hmtx) = tables();
        for end in 0..hmtx.len() {
            assert!(HorizontalMetrics::try_new(&head, &hhea, &hmtx[..end], 4).is_err());
        }
        assert!(HorizontalMetrics::try_new(&head[..53], &hhea, &hmtx, 4).is_err());
        assert!(HorizontalMetrics::try_new(&head, &hhea[..35], &hmtx, 4).is_err());
        for count in [0u16, 5] {
            hhea[34..36].copy_from_slice(&count.to_be_bytes());
            assert!(HorizontalMetrics::try_new(&head, &hhea, &hmtx, 4).is_err());
        }
        hhea[34..36].copy_from_slice(&2u16.to_be_bytes());
        for units in [0u16, 15, 16385] {
            head[18..20].copy_from_slice(&units.to_be_bytes());
            assert!(HorizontalMetrics::try_new(&head, &hhea, &hmtx, 4).is_err());
        }
    }
}
