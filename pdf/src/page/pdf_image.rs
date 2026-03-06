use std::io::{Read, Seek};

use crate::{
    color::colorspace::ColorSpace, error::PdfResult, objects::PdfObject, pdf_context::PDFContext,
};

#[derive(Debug)]
pub struct PdfImage {
    name: String,
    width: f32,
    height: f32,
    colorspace: ColorSpace,
    data: Vec<u8>,
    is_mask: bool,
    is_inline: bool,
}

impl PdfImage {
    pub fn try_new<R: Seek + Read>(
        name: String,
        image: &PdfObject,
        ctx: &PDFContext<R>,
    ) -> PdfResult<Self> {
        let height = image.get_attr("Height").unwrap().as_f32().unwrap();
        let width = image.get_attr("Width").unwrap().as_f32().unwrap();
        let colorspace_obj = image.get_attr("ColorSpace").unwrap();
        let colorspace = ColorSpace::try_new(colorspace_obj, ctx)?;
        let data = image.as_stream().unwrap().decode_data(ctx)?;

        Ok(Self {
            name,
            height,
            width,
            colorspace,
            data,
            is_inline: false,
            is_mask: false,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn width(&self) -> f32 {
        self.width
    }
    pub fn height(&self) -> f32 {
        self.height
    }
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub fn colorspace(&self) -> &ColorSpace {
        &self.colorspace
    }
    pub fn is_mask(&self) -> bool {
        self.is_mask
    }

    pub fn is_inline(&self) -> bool {
        self.is_inline
    }
    pub fn rgba_buffer(&self) -> Vec<u8> {
        let mut rgba = Vec::new();
        match &self.colorspace {
            ColorSpace::DeviceGray(g) => {
                for &g in self.data.iter() {
                    rgba.extend_from_slice(&[g, g, g, 255]);
                }
            }
            ColorSpace::DeviceRgb(_) => {
                for chunk in self.data.chunks_exact(3) {
                    rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                }
            }
            ColorSpace::DeviceCmyk(_) => {
                for chunk in self.data.chunks_exact(4) {
                    let (c, m, y, k) = (
                        chunk[0] as f32 / 255.0,
                        chunk[1] as f32 / 255.0,
                        chunk[2] as f32 / 255.0,
                        chunk[3] as f32 / 255.0,
                    );
                    let r = (255.0 * (1.0 - c) * (1.0 - k)) as u8;
                    let g = (255.0 * (1.0 - m) * (1.0 - k)) as u8;
                    let b = (255.0 * (1.0 - y) * (1.0 - k)) as u8;
                    rgba.extend_from_slice(&[r, g, b, 255]);
                }
            }
            _ => {}
        }
        rgba
    }
}
