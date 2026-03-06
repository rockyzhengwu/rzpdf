use super::value::{ColorRgb, ColorValue};
use crate::error::PdfResult;

#[derive(Debug, Clone, Default)]
pub struct DeviceGray {}

impl DeviceGray {
    pub fn new() -> Self {
        DeviceGray {}
    }
    pub fn default_value(&self) -> ColorValue {
        ColorValue::new(vec![0.0])
    }

    pub fn number_of_components(&self) -> usize {
        1
    }
    pub fn rgb(&self, value: &ColorValue) -> PdfResult<ColorRgb> {
        let g = value.values()[0];
        Ok(ColorRgb::new(g, g, g))
    }
}
