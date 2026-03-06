use crate::error::{PdfError, PdfResult};

use super::value::{ColorRgb, ColorValue};

#[derive(Debug, Clone, Default)]
pub struct DeviceRgb {}

impl DeviceRgb {
    pub fn new() -> Self {
        DeviceRgb {}
    }

    pub fn default_value(&self) -> ColorValue {
        ColorValue::new(vec![0.0, 0.0, 0.0])
    }

    pub fn number_of_components(&self) -> usize {
        3
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        if value.value_size() != 3 {
            return Err(PdfError::ColorError(
                "DeviceRgb need 3 element value".to_string(),
            ));
        }
        let r = value.values()[0];
        let g = value.values()[1];
        let b = value.values()[2];
        Ok(ColorRgb::new(r, g, b))
    }
}
