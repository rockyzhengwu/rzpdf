use mozjpeg::ColorSpace;

use crate::color::value::ColorValue;

#[derive(Debug, Clone)]
pub struct PdfColor {
    value: ColorValue,
    space: ColorSpace,
}
