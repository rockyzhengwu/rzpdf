use mozjpeg::ColorSpace;

use crate::color::value::ColorValue;

#[derive(Debug, Clone)]
pub struct PdfColor {
    #[expect(
        dead_code,
        reason = "retain opaque public color record pending API review"
    )]
    value: ColorValue,
    #[expect(
        dead_code,
        reason = "retain opaque public color record pending API review"
    )]
    space: ColorSpace,
}
