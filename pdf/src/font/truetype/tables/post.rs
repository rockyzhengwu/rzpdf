use crate::error::{PdfError, PdfResult};

/// Common post header only, not glyph-name or variable-instance validation.
#[derive(Debug, Clone, Copy)]
pub struct PostMetrics {
    version: u32,
    italic_angle: f64,
    underline_position: i16,
    underline_thickness: i16,
    fixed_pitch: bool,
}

impl PostMetrics {
    pub fn from_header(data: &[u8]) -> PdfResult<Self> {
        if data.len() < 32 {
            return Err(PdfError::FontError("Truncated post header".into()));
        }
        let version = u32::from_be_bytes(data[..4].try_into().unwrap());
        if !matches!(version, 0x00010000 | 0x00020000 | 0x00025000 | 0x00030000) {
            return Err(PdfError::UnsupportedFeature("post table version".into()));
        }
        Ok(Self {
            version,
            italic_angle: f64::from(i32::from_be_bytes(data[4..8].try_into().unwrap())) / 65536.0,
            underline_position: i16::from_be_bytes(data[8..10].try_into().unwrap()),
            underline_thickness: i16::from_be_bytes(data[10..12].try_into().unwrap()),
            fixed_pitch: u32::from_be_bytes(data[12..16].try_into().unwrap()) != 0,
        })
    }
    pub fn version(&self) -> u32 {
        self.version
    }
    pub fn italic_angle(&self) -> f64 {
        self.italic_angle
    }
    pub fn underline_position(&self) -> i16 {
        self.underline_position
    }
    pub fn underline_thickness(&self) -> i16 {
        self.underline_thickness
    }
    pub fn is_fixed_pitch(&self) -> bool {
        self.fixed_pitch
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn signed_fractional_angle_and_nonzero_pitch_are_preserved() {
        let mut data = [0; 32];
        data[..4].copy_from_slice(&0x00030000_u32.to_be_bytes());
        data[4..8].copy_from_slice(&(-819200_i32).to_be_bytes());
        data[8..10].copy_from_slice(&(-150_i16).to_be_bytes());
        data[10..12].copy_from_slice(&50_i16.to_be_bytes());
        data[12..16].copy_from_slice(&2_u32.to_be_bytes());
        let metrics = PostMetrics::from_header(&data).unwrap();
        assert_eq!(metrics.italic_angle(), -12.5);
        assert_eq!(metrics.underline_position(), -150);
        assert_eq!(metrics.underline_thickness(), 50);
        assert!(metrics.is_fixed_pitch());
        for length in 0..32 {
            assert!(PostMetrics::from_header(&data[..length]).is_err());
        }
        for version in [0x00010000_u32, 0x00020000, 0x00025000, 0x00030000] {
            data[..4].copy_from_slice(&version.to_be_bytes());
            assert_eq!(PostMetrics::from_header(&data).unwrap().version(), version);
        }
        data[..4].copy_from_slice(&0x00040000_u32.to_be_bytes());
        assert!(PostMetrics::from_header(&data).is_err());
    }
}
