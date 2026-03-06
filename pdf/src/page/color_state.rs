use crate::color::{colorspace::ColorSpace, device_gray::DeviceGray, value::ColorValue};

#[derive(Debug, Clone)]
pub struct ColorState {
    pub stroke_color_space: ColorSpace,
    pub stroke_color_value: ColorValue,
    pub fill_color_space: ColorSpace,
    pub fill_color_value: ColorValue,
}

impl Default for ColorState {
    fn default() -> Self {
        ColorState {
            stroke_color_space: ColorSpace::DeviceGray(DeviceGray::new()),
            stroke_color_value: ColorValue::new(vec![0.0]),
            fill_color_space: ColorSpace::DeviceGray(DeviceGray::new()),
            fill_color_value: ColorValue::new(vec![0.0]),
        }
    }
}
