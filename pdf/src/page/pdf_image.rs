use std::io::{Read, Seek};

use crate::{
    color::colorspace::ColorSpace,
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone)]
pub struct PdfImage {
    name: String,
    width: u32,
    height: u32,
    colorspace: ColorSpace,
    data: Vec<u8>,
    is_mask: bool,
    is_inline: bool,
    pixel_count: usize,
    max_rgba_bytes: usize,
    soft_mask: Option<Vec<u8>>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::device_rgb::DeviceRgb;

    #[test]
    fn packed_samples_reset_at_rows_and_decode_before_quantization() {
        assert_eq!(
            unpack_image_samples(&[0xbf, 0x5f], 3, 2, 1, 1, None, 6).unwrap(),
            [255, 0, 255, 0, 255, 0]
        );
        assert_eq!(
            unpack_image_samples(&[0x1b], 4, 1, 1, 2, None, 4).unwrap(),
            [0, 85, 170, 255]
        );
        assert_eq!(
            unpack_image_samples(&[0x08, 0xff], 3, 1, 1, 4, None, 3).unwrap(),
            [0, 136, 255]
        );
        assert_eq!(
            unpack_image_samples(&[0, 0, 0x80, 0, 0xff, 0xff], 3, 1, 1, 16, None, 3).unwrap(),
            [0, 128, 255]
        );
        let mut endpoints = [0.0; 8];
        endpoints[0] = 1.0;
        assert_eq!(
            unpack_image_samples(&[0x80, 0], 1, 1, 1, 16, Some(endpoints), 1).unwrap(),
            [127]
        );
        endpoints[0] = 0.0;
        endpoints[1] = 0.5;
        // Original 16-bit precision gives 64, whereas quantizing to 8 bits first gives 65.
        assert_eq!(
            unpack_image_samples(&[0x81, 0], 1, 1, 1, 16, Some(endpoints), 1).unwrap(),
            [64]
        );
        assert!(matches!(
            unpack_image_samples(&[0], 1, 1, 1, 16, None, 1),
            Err(PdfError::ObjectError(_))
        ));
        assert!(matches!(
            unpack_image_samples(&[0], 4, 1, 1, 2, None, 3),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn decode_arrays_support_default_inversion_and_clamped_ranges() {
        use crate::{
            objects::{pdf_array::PdfArray, pdf_dict::PdfDict, pdf_number::PdfNumber},
            parser::options::ParseOptions,
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (values, expected) in [
            (vec![0.0, 1.0], vec![0, 128, 255]),
            (vec![1.0, 0.0], vec![255, 127, 0]),
            (vec![-1.0, 2.0], vec![0, 129, 255]),
            (vec![0.5, 0.5], vec![128, 128, 128]),
        ] {
            let mut array = PdfArray::default();
            for value in values {
                array
                    .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(value)))
                    .unwrap();
            }
            let mut bytes = vec![0, 128, 255];
            apply_image_decode(&mut bytes, Some(1), Some(&PdfObject::PdfArray(array)), &ctx)
                .unwrap();
            assert_eq!(bytes, expected);
        }
        let bad = PdfObject::PdfArray(PdfArray::default());
        assert!(apply_image_decode(&mut [128], Some(1), Some(&bad), &ctx).is_err());
        assert!(
            apply_image_decode(
                &mut [128],
                Some(1),
                Some(&PdfObject::PdfNumber(
                    crate::objects::pdf_number::PdfNumber::new(1.0)
                )),
                &ctx
            )
            .is_err()
        );
    }

    #[test]
    fn soft_mask_parser_rejects_invalid_fields_lengths_and_nested_masks() {
        use crate::{
            objects::{
                pdf_dict::PdfDict, pdf_name::PdfName, pdf_number::PdfNumber, pdf_stream::PdfStream,
            },
            parser::options::ParseOptions,
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        let mut mask = PdfDict::default();
        for (key, value) in [("Subtype", "Image"), ("ColorSpace", "DeviceGray")] {
            mask.try_insert(key.into(), PdfObject::PdfName(PdfName::new(value.into())))
                .unwrap();
        }
        for (key, value) in [("Width", 1), ("Height", 1), ("BitsPerComponent", 8)] {
            mask.try_insert(
                key.into(),
                PdfObject::PdfNumber(PdfNumber::new(value as f32)),
            )
            .unwrap();
        }
        let mut base = mask.clone();
        base.try_insert(
            "ColorSpace".into(),
            PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
        )
        .unwrap();
        let image_with_mask = |dict: PdfDict, alpha: Vec<u8>| {
            let mut image = base.clone();
            image
                .try_insert(
                    "SMask".into(),
                    PdfObject::PdfStream(PdfStream::new(dict, alpha)),
                )
                .unwrap();
            PdfObject::PdfStream(PdfStream::new(image, vec![10, 20, 30]))
        };
        let valid = image_with_mask(mask.clone(), vec![128]);
        for (bits, bytes, expected) in [
            (1, vec![0xff], 255),
            (2, vec![0x80], 170),
            (4, vec![0x80], 136),
            (16, vec![0x80, 0], 128),
        ] {
            let mut packed = mask.clone();
            packed
                .try_insert(
                    "BitsPerComponent".into(),
                    PdfObject::PdfNumber(PdfNumber::new(bits as f32)),
                )
                .unwrap();
            assert_eq!(
                PdfImage::try_new("test".into(), &image_with_mask(packed, bytes), &ctx)
                    .unwrap()
                    .rgba_buffer()
                    .unwrap(),
                [10, 20, 30, expected]
            );
        }
        assert_eq!(
            PdfImage::try_new("test".into(), &valid, &ctx)
                .unwrap()
                .rgba_buffer()
                .unwrap(),
            vec![10, 20, 30, 128]
        );
        let mut inverted = mask.clone();
        let mut decode = crate::objects::pdf_array::PdfArray::default();
        decode
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(1.0)))
            .unwrap();
        decode
            .try_add_obj(PdfObject::PdfNumber(PdfNumber::new(0.0)))
            .unwrap();
        inverted
            .try_insert("Decode".into(), PdfObject::PdfArray(decode))
            .unwrap();
        inverted
            .try_insert(
                "ImageMask".into(),
                PdfObject::PdfBool(crate::objects::pdf_bool::PdfBool::new(b"false")),
            )
            .unwrap();
        assert_eq!(
            PdfImage::try_new("test".into(), &image_with_mask(inverted, vec![128]), &ctx)
                .unwrap()
                .rgba_buffer()
                .unwrap(),
            vec![10, 20, 30, 127]
        );
        for key in [
            "Width",
            "Height",
            "BitsPerComponent",
            "Subtype",
            "ColorSpace",
        ] {
            let mut missing = mask.clone();
            missing.remove(key);
            assert!(matches!(
                PdfImage::try_new("test".into(), &image_with_mask(missing, vec![128]), &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
        for (key, value) in [
            ("Width", PdfObject::PdfNumber(PdfNumber::new(2.0))),
            ("Height", PdfObject::PdfNumber(PdfNumber::new(0.5))),
            (
                "BitsPerComponent",
                PdfObject::PdfNumber(PdfNumber::new(16.0)),
            ),
            (
                "ColorSpace",
                PdfObject::PdfName(PdfName::new("DeviceRGB".into())),
            ),
            ("Subtype", PdfObject::PdfName(PdfName::new("Wrong".into()))),
            ("SMask", valid.clone()),
            ("Matte", PdfObject::PdfNumber(PdfNumber::new(1.0))),
            ("Decode", PdfObject::PdfNumber(PdfNumber::new(1.0))),
        ] {
            let mut invalid = mask.clone();
            invalid.try_insert(key.into(), value).unwrap();
            assert!(
                PdfImage::try_new("test".into(), &image_with_mask(invalid, vec![128]), &ctx)
                    .is_err(),
                "{key}"
            );
        }
        for alpha in [vec![], vec![0, 255]] {
            assert!(matches!(
                PdfImage::try_new("test".into(), &image_with_mask(mask.clone(), alpha), &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
    }

    #[test]
    fn rgba_conversion_checks_data_length_and_allocation_budget() {
        let mut image = PdfImage {
            name: "test".into(),
            width: 1,
            height: 1,
            colorspace: ColorSpace::DeviceRgb(DeviceRgb::new()),
            data: vec![10, 20, 30],
            is_mask: false,
            is_inline: false,
            pixel_count: 1,
            max_rgba_bytes: 4,
            soft_mask: None,
        };
        assert_eq!(image.rgba_buffer().unwrap(), vec![10, 20, 30, 255]);
        image.soft_mask = Some(vec![128]);
        assert_eq!(image.rgba_buffer().unwrap(), vec![10, 20, 30, 128]);
        image.soft_mask = Some(vec![]);
        assert!(image.rgba_buffer().is_err());
        image.soft_mask = None;
        image.max_rgba_bytes = 3;
        assert!(matches!(
            image.rgba_buffer(),
            Err(PdfError::ResourceLimit(_))
        ));
        image.max_rgba_bytes = 4;
        image.data.pop();
        assert!(image.rgba_buffer().is_err());
        image.data.extend_from_slice(&[30, 40]);
        assert!(image.rgba_buffer().is_err());
    }
}

impl PdfImage {
    pub fn try_new<R: Seek + Read>(
        name: String,
        image: &PdfObject,
        ctx: &PDFContext<R>,
    ) -> PdfResult<Self> {
        let resolved = ctx.resolve(image)?;
        let image = &resolved;
        let stream = image
            .as_stream()
            .ok_or_else(|| PdfError::ObjectError("image must be a stream".into()))?;
        if let Some(subtype) = image.get_attr("Subtype") {
            let subtype = ctx.resolve(subtype)?;
            if subtype.as_name().map(|name| name.bytes()) != Some(b"Image".as_slice()) {
                return Err(PdfError::ObjectError("image Subtype must be /Image".into()));
            }
        }
        if let Some(mask) = image.get_attr("ImageMask") {
            let mask = ctx.resolve(mask)?;
            let is_mask = mask
                .as_bool()
                .ok_or_else(|| PdfError::ObjectError("image ImageMask must be boolean".into()))?
                .value();
            if is_mask {
                return Err(PdfError::UnsupportedFeature("stencil image masks".into()));
            }
        }
        for key in ["Mask"] {
            if let Some(value) = image.get_attr(key) {
                let value = ctx.resolve(value)?;
                if !matches!(value, PdfObject::PdfNull) {
                    return Err(PdfError::UnsupportedFeature(format!("image {key}")));
                }
            }
        }
        let dimension = |key: &str| -> PdfResult<u32> {
            let object = ctx.resolve(
                image
                    .get_attr(key)
                    .ok_or_else(|| PdfError::ObjectError(format!("image {key} is missing")))?,
            )?;
            let value = object
                .as_number()
                .ok_or_else(|| PdfError::ObjectError(format!("image {key} must be an integer")))?
                .as_u32_checked()?;
            if value == 0 {
                return Err(PdfError::ObjectError(format!(
                    "image {key} must be positive"
                )));
            }
            Ok(value)
        };
        let height = dimension("Height")?;
        let width = dimension("Width")?;
        if u64::from(width) * u64::from(height) > ctx.limits().max_image_pixels {
            return Err(PdfError::ResourceLimit("image pixel count".into()));
        }
        let pixel_count = usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| PdfError::ResourceLimit("image pixel count overflow".into()))?;
        pixel_count
            .checked_mul(4)
            .filter(|bytes| *bytes <= ctx.limits().max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("image RGBA byte budget".into()))?;
        let bits = if let Some(bits) = image.get_attr("BitsPerComponent") {
            let bits = ctx.resolve(bits)?;
            let bits = bits
                .as_u32()
                .filter(|bits| matches!(bits, 1 | 2 | 4 | 8 | 16))
                .ok_or_else(|| {
                    PdfError::ObjectError(
                        "image BitsPerComponent must be an integer 1, 2, 4, 8 or 16".into(),
                    )
                })?;
            bits as usize
        } else {
            8
        };
        let colorspace_obj = image
            .get_attr("ColorSpace")
            .ok_or_else(|| PdfError::ColorError("image ColorSpace is missing".into()))?;
        let colorspace = ColorSpace::try_new(colorspace_obj, ctx)?;
        let mut data = stream.decode_data(ctx)?;
        let components = match &colorspace {
            ColorSpace::DeviceGray(_) => Some(1),
            ColorSpace::DeviceRgb(_) => Some(3),
            ColorSpace::DeviceCmyk(_) => Some(4),
            _ => None,
        };
        if bits == 8 {
            apply_image_decode(&mut data, components, image.get_attr("Decode"), ctx)?;
        } else {
            let components = components.ok_or_else(|| {
                PdfError::UnsupportedFeature("packed image samples for this color space".into())
            })?;
            let endpoints =
                image_decode_endpoints(Some(components), image.get_attr("Decode"), ctx)?;
            data = unpack_image_samples(
                &data,
                width as usize,
                height as usize,
                components,
                bits,
                endpoints,
                ctx.limits().max_decoded_bytes,
            )?;
        }
        let soft_mask = match image.get_attr("SMask") {
            None | Some(PdfObject::PdfNull) => None,
            Some(value) => {
                let mask = ctx.resolve(value)?;
                if matches!(mask, PdfObject::PdfNull) {
                    None
                } else {
                    let mask = mask.as_stream().ok_or_else(|| {
                        PdfError::ObjectError("SMask must be an image stream".into())
                    })?;
                    for (key, expected) in [("Subtype", "Image"), ("ColorSpace", "DeviceGray")] {
                        let value = mask.dict().get(key).ok_or_else(|| {
                            PdfError::ObjectError("SMask image field missing".into())
                        })?;
                        let value = ctx.resolve(value)?;
                        if value.as_name().map(|name| name.name()) != Some(expected) {
                            return Err(PdfError::UnsupportedFeature(
                                "SMask requires Image/DeviceGray".into(),
                            ));
                        }
                    }
                    for (key, expected) in [("Width", width), ("Height", height)] {
                        let value = mask.dict().get(key).ok_or_else(|| {
                            PdfError::ObjectError("SMask dimensions/components missing".into())
                        })?;
                        let value = ctx.resolve(value)?;
                        let actual = value
                            .as_number()
                            .ok_or_else(|| {
                                PdfError::ObjectError(
                                    "SMask dimensions/components must be integers".into(),
                                )
                            })?
                            .as_u32_checked()?;
                        if actual != expected {
                            return Err(PdfError::UnsupportedFeature(
                                "SMask requires matching dimensions".into(),
                            ));
                        }
                    }
                    let mask_bits =
                        ctx.resolve(mask.dict().get("BitsPerComponent").ok_or_else(|| {
                            PdfError::ObjectError("SMask BitsPerComponent missing".into())
                        })?)?;
                    let mask_bits = mask_bits
                        .as_u32()
                        .filter(|bits| matches!(bits, 1 | 2 | 4 | 8 | 16))
                        .ok_or_else(|| {
                            PdfError::ObjectError("invalid SMask BitsPerComponent".into())
                        })? as usize;
                    for key in ["Mask", "SMask", "Matte"] {
                        if let Some(value) = mask.dict().get(key) {
                            if !matches!(ctx.resolve(value)?, PdfObject::PdfNull) {
                                return Err(PdfError::UnsupportedFeature(format!("SMask {key}")));
                            }
                        }
                    }
                    if let Some(value) = mask.dict().get("ImageMask") {
                        let value = ctx.resolve(value)?;
                        let flag = value
                            .as_bool()
                            .ok_or_else(|| {
                                PdfError::ObjectError("SMask ImageMask must be boolean".into())
                            })?
                            .value();
                        if flag {
                            return Err(PdfError::UnsupportedFeature("SMask stencil mask".into()));
                        }
                    }
                    let mut alpha = mask.decode_data(ctx)?;
                    if mask_bits == 8 {
                        apply_image_decode(&mut alpha, Some(1), mask.dict().get("Decode"), ctx)?;
                    } else {
                        let endpoints =
                            image_decode_endpoints(Some(1), mask.dict().get("Decode"), ctx)?;
                        alpha = unpack_image_samples(
                            &alpha,
                            width as usize,
                            height as usize,
                            1,
                            mask_bits,
                            endpoints,
                            ctx.limits().max_decoded_bytes,
                        )?;
                    }
                    if alpha.len() != pixel_count {
                        return Err(PdfError::ObjectError(
                            "SMask data length does not match dimensions".into(),
                        ));
                    }
                    Some(alpha)
                }
            }
        };

        Ok(Self {
            name,
            height,
            width,
            colorspace,
            data,
            is_inline: false,
            is_mask: false,
            pixel_count,
            max_rgba_bytes: ctx.limits().max_decoded_bytes,
            soft_mask,
        })
    }
    pub fn name(&self) -> &str {
        &self.name
    }
    pub fn width(&self) -> f32 {
        self.width as f32
    }
    pub fn height(&self) -> f32 {
        self.height as f32
    }
    pub fn pixel_dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub(crate) fn retained_sample_bytes(&self) -> PdfResult<usize> {
        self.data
            .len()
            .checked_add(self.soft_mask.as_ref().map_or(0, Vec::len))
            .ok_or_else(|| PdfError::ResourceLimit("image retained samples".into()))
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
    pub fn rgba_buffer(&self) -> PdfResult<Vec<u8>> {
        let components = match &self.colorspace {
            ColorSpace::DeviceGray(_) => 1,
            ColorSpace::DeviceRgb(_) => 3,
            ColorSpace::DeviceCmyk(_) => 4,
            _ => {
                return Err(PdfError::UnsupportedFeature(
                    "image RGBA conversion for this color space".into(),
                ));
            }
        };
        let input_size = self
            .pixel_count
            .checked_mul(components)
            .ok_or_else(|| PdfError::ResourceLimit("image data size overflow".into()))?;
        if self.data.len() != input_size {
            return Err(PdfError::ObjectError(
                "image data length does not match dimensions/components".into(),
            ));
        }
        let output_size = self
            .pixel_count
            .checked_mul(4)
            .filter(|n| *n <= self.max_rgba_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("image RGBA byte budget".into()))?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(output_size)
            .map_err(|_| PdfError::ResourceLimit("unable to allocate image RGBA buffer".into()))?;
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
        if let Some(alpha) = &self.soft_mask {
            if alpha.len() != self.pixel_count {
                return Err(PdfError::ObjectError("SMask alpha length mismatch".into()));
            }
            for (pixel, &alpha) in rgba.chunks_exact_mut(4).zip(alpha) {
                pixel[3] = alpha;
            }
        }
        Ok(rgba)
    }
}

fn apply_image_decode<R: Read + Seek>(
    data: &mut [u8],
    components: Option<usize>,
    decode: Option<&PdfObject>,
    ctx: &PDFContext<R>,
) -> PdfResult<()> {
    let Some(endpoints) = image_decode_endpoints(components, decode, ctx)? else {
        return Ok(());
    };
    let components = components.unwrap();
    for (index, sample) in data.iter_mut().enumerate() {
        let component = index % components;
        let low = endpoints[component * 2];
        let high = endpoints[component * 2 + 1];
        *sample = ((low + f64::from(*sample) / 255.0 * (high - low)).clamp(0.0, 1.0) * 255.0)
            .round() as u8;
    }
    Ok(())
}

fn image_decode_endpoints<R: Read + Seek>(
    components: Option<usize>,
    decode: Option<&PdfObject>,
    ctx: &PDFContext<R>,
) -> PdfResult<Option<[f64; 8]>> {
    let Some(decode) = decode else {
        return Ok(None);
    };
    let decode = ctx.resolve(decode)?;
    if matches!(decode, PdfObject::PdfNull) {
        return Ok(None);
    }
    let components = components
        .ok_or_else(|| PdfError::UnsupportedFeature("image Decode for this color space".into()))?;
    let array = decode
        .as_array()
        .ok_or_else(|| PdfError::ObjectError("image Decode must be an array".into()))?;
    if array.len() != components * 2 {
        return Err(PdfError::ObjectError(
            "image Decode component count mismatch".into(),
        ));
    }
    let mut endpoints = [0.0f64; 8];
    for (index, value) in array.into_iter().enumerate() {
        let value = ctx.resolve(value)?;
        endpoints[index] = f64::from(
            value
                .as_f32()
                .filter(|value| value.is_finite())
                .ok_or_else(|| {
                    PdfError::ObjectError("image Decode requires finite numbers".into())
                })?,
        );
    }
    Ok(Some(endpoints))
}

fn unpack_image_samples(
    source: &[u8],
    width: usize,
    height: usize,
    components: usize,
    bits: usize,
    endpoints: Option<[f64; 8]>,
    limit: usize,
) -> PdfResult<Vec<u8>> {
    let overflow = || PdfError::ResourceLimit("image sample size overflow".into());
    let row_samples = width.checked_mul(components).ok_or_else(overflow)?;
    let row_bytes = row_samples
        .checked_mul(bits)
        .and_then(|n| n.checked_add(7))
        .ok_or_else(overflow)?
        / 8;
    if source.len() != row_bytes.checked_mul(height).ok_or_else(overflow)? {
        return Err(PdfError::ObjectError(
            "packed image sample length mismatch".into(),
        ));
    }
    let size = row_samples
        .checked_mul(height)
        .filter(|n| *n <= limit)
        .ok_or_else(overflow)?;
    let mut output = Vec::new();
    output.try_reserve_exact(size).map_err(|_| overflow())?;
    let maximum = ((1u32 << bits) - 1) as f64;
    for row in source.chunks_exact(row_bytes) {
        for index in 0..row_samples {
            let offset = index * bits;
            let sample = if bits == 16 {
                u16::from_be_bytes([row[offset / 8], row[offset / 8 + 1]]) as u32
            } else {
                (u32::from(row[offset / 8]) >> (8 - bits - offset % 8)) & ((1 << bits) - 1)
            };
            let (low, high) = endpoints.map_or((0.0, 1.0), |e| {
                let component = index % components;
                (e[component * 2], e[component * 2 + 1])
            });
            output.push(
                ((low + sample as f64 / maximum * (high - low)).clamp(0.0, 1.0) * 255.0).round()
                    as u8,
            );
        }
    }
    Ok(output)
}
