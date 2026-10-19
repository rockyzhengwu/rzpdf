use super::value::{ColorRgb, ColorValue};
use crate::error::{PdfError, PdfResult};

#[derive(Debug, Clone, Default)]
pub struct DeviceCmyk {}

impl DeviceCmyk {
    pub fn new() -> Self {
        DeviceCmyk {}
    }
    pub fn default_value(&self) -> ColorValue {
        ColorValue::new(vec![0.0, 0.0, 0.0, 1.0])
    }

    pub fn number_of_components(&self) -> usize {
        4
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        if value.value_size() != 4 || value.values().iter().any(|v| !v.is_finite()) {
            return Err(PdfError::ColorError(
                "DeviceCMYK requires four finite components".to_string(),
            ));
        }
        let c = value.values()[0].clamp(0.0, 1.0);
        let m = value.values()[1].clamp(0.0, 1.0);
        let y = value.values()[2].clamp(0.0, 1.0);
        let k = value.values()[3].clamp(0.0, 1.0);
        //let r = 1.0 - (c + k).min(1.0);
        //let g = 1.0 - (m + k).min(1.0);
        //let b = 1.0 - (y + k).min(1.0);
        let r = (1.0 - c) * (1.0 - k);
        let g = (1.0 - m) * (1.0 - k);
        let b = (1.0 - y) * (1.0 - k);
        Ok(ColorRgb::new(r, g, b))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cmyk_components_are_clipped_before_conversion() {
        let space = DeviceCmyk::new();
        for (input, expected) in [
            ([2.0, -1.0, 0.0, 0.0], [0, 255, 255]),
            ([0.0, 0.0, 0.0, 2.0], [0, 0, 0]),
            ([f32::MAX, -f32::MAX, 0.0, -f32::MAX], [0, 255, 255]),
            ([-1.0, -1.0, -1.0, -1.0], [255, 255, 255]),
        ] {
            let rgb = space.rgb(&ColorValue::new(input.to_vec())).unwrap();
            assert_eq!([rgb.r(), rgb.g(), rgb.b()], expected);
        }
        assert!(space.rgb(&ColorValue::new(vec![f32::NAN; 4])).is_err());
    }
}
