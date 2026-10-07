use std::io::{Read, Seek};

use crate::error::{PdfError, PdfResult};
use crate::geom::matrix::Matrix;
use crate::geom::rectangle::Rectangle;
use crate::objects::PdfObject;
use crate::pdf_context::PDFContext;

#[derive(Debug, Clone, Default)]
pub struct TilingPattern {
    paint_type: u8,
    tiling_type: u8,
    bbox: Rectangle,
    xstep: f32,
    ystep: f32,
    matrix: Option<Matrix>,
}

impl TilingPattern {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let mut tp = TilingPattern::default();
        if let Some(pt) = obj.get_attr("PaintType") {
            let pt = pt.as_u8().ok_or(PdfError::ColorError(
                "Pattern paint type is not a number".to_string(),
            ))?;
            tp.paint_type = pt;
        }

        if let Some(t) = obj.get_attr("TilingType") {
            let t = t.as_u8().ok_or(PdfError::ColorError(
                "Pattern TilingType is not a number".to_string(),
            ))?;
            tp.tiling_type = t as u8;
        }
        if let Some(bbox) = obj.get_attr("BBox") {
            let rect = Rectangle::try_from(bbox)?;
            tp.bbox = rect;
        }
        if let Some(x) = obj.get_attr("XStep") {
            let xs = x
                .as_f32()
                .filter(|value| value.is_finite() && *value != 0.0)
                .ok_or(PdfError::ColorError(
                    "XStep must be finite and nonzero".into(),
                ))?;
            tp.xstep = xs;
        }
        if let Some(y) = obj.get_attr("YStep") {
            let ys = y
                .as_f32()
                .filter(|value| value.is_finite() && *value != 0.0)
                .ok_or(PdfError::ColorError(
                    "YStep must be finite and nonzero".into(),
                ))?;
            tp.ystep = ys;
        }
        if let Some(matrix) = obj.get_attr("Matrix") {
            tp.matrix = Some(Matrix::try_from(matrix)?);
        }

        if let Some(res) = obj.get_attr("Resources") {
            //println!("{:?}", res);
        }
        let content = obj
            .as_stream()
            .ok_or(PdfError::ColorError(
                "tiling pattern must be a stream".into(),
            ))?
            .decode_data(ctx)?;
        Ok(tp)
    }
}
