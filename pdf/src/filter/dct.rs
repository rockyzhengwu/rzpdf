use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};

pub fn dct_decode(input: &[u8], _params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    let d = mozjpeg::Decompress::new_mem(input)
        .map_err(|e| PdfError::FilterError(format!("DCT create decoder error:{:?}", e)))?;
    let image = d
        .image()
        .map_err(|e| PdfError::FilterError(format!("DCT decoder read decoder error:{:?}", e)))?;

    let res = match image {
        mozjpeg::Format::RGB(mut rgb) => {
            let mut res = Vec::new();
            let pixels: Vec<[u8; 3]> = rgb.read_scanlines().map_err(|e| {
                PdfError::FilterError(format!("DCT RGB scanline decode error:{e:?}"))
            })?;
            for pixel in pixels {
                res.push(pixel[0]);
                res.push(pixel[1]);
                res.push(pixel[2]);
            }
            res
        }
        mozjpeg::Format::Gray(mut g) => {
            let pixels: Vec<u8> = g.read_scanlines().map_err(|e| {
                PdfError::FilterError(format!("DCT Gray scanline decode error:{e:?}"))
            })?;
            pixels
        }
        mozjpeg::Format::CMYK(mut cmyk) => {
            let mut res = Vec::new();
            let pixels: Vec<[u8; 4]> = cmyk.read_scanlines().map_err(|e| {
                PdfError::FilterError(format!("DCT CMYK scanline decode error:{e:?}"))
            })?;
            for pixel in pixels {
                res.push(pixel[0]);
                res.push(pixel[1]);
                res.push(pixel[2]);
                res.push(pixel[3]);
            }
            res
        }
    };

    Ok(res)
}
