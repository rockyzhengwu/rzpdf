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
    data: ImageSamples,
    source_dimensions: (u32, u32),
    is_mask: bool,
    is_inline: bool,
    interpolate: bool,
    pixel_count: usize,
    max_rgba_bytes: usize,
    soft_mask: Option<ImageSoftMask>,
    hard_mask_alpha: Option<ImageHardMask>,
    // Non-device and Matte-preblended samples retain packed precision until
    // unblending/colour conversion.
    color_sample_decode: Option<(usize, Vec<f64>)>,
    rendering_intent: Option<crate::color::conversion::RenderingIntent>,
}

#[derive(Debug, Clone)]
enum ImageSamples {
    Owned(Vec<u8>),
    Budgeted(std::rc::Rc<crate::budget::BudgetedBytes>),
}
impl std::ops::Deref for ImageSamples {
    type Target = [u8];
    fn deref(&self) -> &[u8] {
        match self {
            Self::Owned(bytes) => bytes,
            Self::Budgeted(bytes) => bytes.as_slice(),
        }
    }
}
impl ImageSamples {
    fn as_slice(&self) -> &[u8] {
        self
    }
}
struct DecodedImageSamples {
    source: (u32, u32),
    dimensions: (u32, u32),
    samples: crate::budget::BudgetedBytes,
    decode_applied: bool,
}

#[derive(Debug, Clone)]
struct ImageHardMask {
    width: u32,
    height: u32,
    alpha: Vec<u8>,
}

impl ImageHardMask {
    fn alpha_at(&self, x: u32, y: u32, width: u32, height: u32) -> u8 {
        let sx =
            (((u128::from(x) * 2 + 1) * u128::from(self.width)) / (u128::from(width) * 2)) as usize;
        let sy = (((u128::from(y) * 2 + 1) * u128::from(self.height)) / (u128::from(height) * 2))
            as usize;
        self.alpha[sy * self.width as usize + sx]
    }
}

#[derive(Debug, Clone)]
struct ImageSoftMask {
    width: u32,
    height: u32,
    // Keep packed precision: rounding 16-bit alpha before unblending can
    // turn small nonzero alpha into zero and amplify colour errors.
    data: Vec<u8>,
    bits: usize,
    decode: [f64; 2],
    matte: Option<Vec<f32>>,
}

impl ImageSoftMask {
    fn alpha(&self, pixel: usize, width: usize, height: usize) -> f64 {
        let x = pixel % width;
        let y = pixel / width;
        let sx = if width == self.width as usize {
            x
        } else {
            ((x as u128 * 2 + 1) * u128::from(self.width) / (width as u128 * 2)) as usize
        };
        let sy = if height == self.height as usize {
            y
        } else {
            ((y as u128 * 2 + 1) * u128::from(self.height) / (height as u128 * 2)) as usize
        };
        let pixel = sy * self.width as usize + sx;
        let width = self.width as usize;
        let row_bytes = (width * self.bits).div_ceil(8);
        let row_start = pixel / width * row_bytes;
        let offset = pixel % width * self.bits;
        let sample = if self.bits == 16 {
            u32::from(u16::from_be_bytes([
                self.data[row_start + offset / 8],
                self.data[row_start + offset / 8 + 1],
            ]))
        } else {
            (u32::from(self.data[row_start + offset / 8]) >> (8 - self.bits - offset % 8))
                & ((1 << self.bits) - 1)
        };
        (self.decode[0]
            + f64::from(sample) / f64::from((1u32 << self.bits) - 1)
                * (self.decode[1] - self.decode[0]))
            .clamp(0.0, 1.0)
    }
}

impl PdfImage {
    /// Target-resolution decoding is exclusively an interpreter render policy.
    /// Unsupported codecs/masks use the unchanged, bounded full-resolution path.
    pub(crate) fn try_new_for_render<R: Read + Seek>(
        name: String,
        object: &PdfObject,
        ctx: &PDFContext<R>,
        matrix: &crate::geom::matrix::Matrix,
        device_color_override: impl FnOnce() -> PdfResult<bool>,
    ) -> PdfResult<Self> {
        // Full-fidelity extraction/display-list APIs keep their existing error
        // shape; contextual diagnostics belong to render-time decoding.
        if ctx.render_image_dpi().is_none() {
            return Self::try_new(name, object, ctx);
        }
        let mut details = crate::error::ImageDecodeDetails {
            encoding: "unknown",
            width: 0,
            height: 0,
            target_width: None,
            target_height: None,
            target_decode_supported: false,
            unsupported_reason: None,
            jpeg_process: None,
        };
        Self::try_new_for_render_inner(
            name,
            object,
            ctx,
            matrix,
            device_color_override,
            &mut details,
        )
        .map_err(|source| PdfError::ImageDecode {
            details: Box::new(details),
            source: Box::new(source),
        })
    }

    fn try_new_for_render_inner<R: Read + Seek>(
        name: String,
        object: &PdfObject,
        ctx: &PDFContext<R>,
        matrix: &crate::geom::matrix::Matrix,
        device_color_override: impl FnOnce() -> PdfResult<bool>,
        details: &mut crate::error::ImageDecodeDetails,
    ) -> PdfResult<Self> {
        use super::image_decode::ImageDecodePlan;
        let Some(dpi) = ctx.render_image_dpi() else {
            return Self::try_new(name, object, ctx);
        };
        let resolved = ctx.resolve(object)?;
        let Some(stream) = resolved.as_stream() else {
            return Self::try_new(name, object, ctx);
        };
        let pool = ctx.limits().operation_budget();
        let (filter, _filter_lease) = match stream.dict().get("Filter") {
            Some(value) => crate::objects::pdf_stream::resolve_filter_setting_with_budget(
                value, ctx, false, &pool,
            )?,
            None => (PdfObject::PdfNull, pool.reserve(0)?),
        };
        let filter_name = match &filter {
            PdfObject::PdfName(name) => Some(name.name()),
            PdfObject::PdfArray(array) if array.len() == 1 => {
                array.get(0).and_then(PdfObject::as_name).map(|v| v.name())
            }
            _ => None,
        };
        details.encoding = match filter_name {
            Some("DCT" | "DCTDecode") => "jpeg",
            Some("Fl" | "FlateDecode") => "flate",
            Some("JPXDecode") => "jpx",
            Some("JBIG2Decode") => "jbig2",
            Some("CCF" | "CCITTFaxDecode") => "ccitt",
            None if matches!(filter, PdfObject::PdfNull) => "raw",
            Some("LZW" | "LZWDecode") => "lzw",
            Some("RL" | "RunLengthDecode") => "run_length",
            None if matches!(filter, PdfObject::PdfArray(_)) => "filter_chain",
            _ => "unknown",
        };
        let dimension = |key| -> PdfResult<u32> {
            ctx.resolve(
                stream
                    .dict()
                    .get(key)
                    .ok_or_else(|| PdfError::ObjectError("image dimension missing".into()))?,
            )?
            .as_number()
            .ok_or_else(|| PdfError::ObjectError("image dimension must be integer".into()))?
            .as_u32_checked()
        };
        let source = (dimension("Width")?, dimension("Height")?);
        details.width = source.0;
        details.height = source.1;
        let plan = ImageDecodePlan::for_render(source, matrix, dpi)?;
        details.target_width = Some(plan.target.0);
        details.target_height = Some(plan.target.1);
        if !matches!(details.encoding, "jpeg" | "flate") {
            details.unsupported_reason = Some("encoding");
            return Self::try_new(name, object, ctx);
        }
        for key in ["Mask", "SMask"] {
            if let Some(value) = stream.dict().get(key)
                && !matches!(ctx.resolve(value)?, PdfObject::PdfNull)
            {
                details.unsupported_reason = Some("mask");
                return Self::try_new(name, object, ctx);
            }
        }
        if stream
            .dict()
            .get("ImageMask")
            .map(|v| ctx.resolve(v))
            .transpose()?
            .is_some_and(|v| v.as_bool().is_none_or(|flag| flag.value()))
        {
            details.unsupported_reason = Some("mask");
            return Self::try_new(name, object, ctx);
        }
        if stream
            .dict()
            .get("SMaskInData")
            .map(|v| ctx.resolve(v))
            .transpose()?
            .is_some_and(|v| v.as_u32() != Some(0))
        {
            details.unsupported_reason = Some("mask");
            return Self::try_new(name, object, ctx);
        }
        if ["F", "FFilter", "FDecodeParms"]
            .iter()
            .any(|key| stream.dict().contains_key(key))
        {
            details.unsupported_reason = Some("external_stream");
            return Self::try_new(name, object, ctx);
        }
        let bits = stream
            .dict()
            .get("BitsPerComponent")
            .map(|v| ctx.resolve(v))
            .transpose()?;
        if bits.as_ref().map_or(Some(8), PdfObject::as_u32) != Some(8) {
            details.unsupported_reason = Some("component_depth");
            return Self::try_new(name, object, ctx);
        }
        let (params, _params_lease) = match stream.dict().get("DecodeParms") {
            Some(value) => crate::objects::pdf_stream::resolve_filter_setting_with_budget(
                value, ctx, true, &pool,
            )?,
            None => (PdfObject::PdfNull, pool.reserve(0)?),
        };
        let params = match &params {
            PdfObject::PdfNull => None,
            PdfObject::PdfDict(dict) => Some(dict),
            PdfObject::PdfArray(array) if array.len() == 1 => match array.get(0) {
                Some(PdfObject::PdfNull) => None,
                Some(PdfObject::PdfDict(dict)) => Some(dict),
                _ => {
                    return Err(PdfError::FilterError(
                        "invalid image DecodeParms entry".into(),
                    ));
                }
            },
            _ => return Err(PdfError::FilterError("invalid image DecodeParms".into())),
        };
        let full_fits = ImageDecodePlan::full_fits(source, ctx.limits());
        let target_policy = ctx.render_image_policy()
            == Some(crate::document::prepared_page::ImageDecodePolicy::TargetResolution);
        if full_fits && !target_policy {
            return Self::try_new(name, object, ctx);
        }
        if details.encoding == "jpeg" {
            if params.is_some() {
                details.unsupported_reason = Some("decode_parameters");
                return Self::try_new(name, object, ctx);
            }
            let raw = stream.read_raw_data(ctx, &pool)?;
            details.jpeg_process = crate::filter::dct::process(&raw);
            if details.jpeg_process != Some("sequential") {
                details.unsupported_reason = Some("jpeg_process");
                return Self::try_new(name, object, ctx);
            }
            details.target_decode_supported = true;
            // Keep the existing JPEG IDCT sizes and full-fidelity fallback.
            if full_fits
                && (u64::from(source.0) * u64::from(source.1)
                    > crate::filter::dct::MAX_TARGET_SOURCE_PIXELS
                    || plan.target.0 > source.0.div_ceil(2)
                    || plan.target.1 > source.1.div_ceil(2))
            {
                return Self::try_new(name, object, ctx);
            }
            let (width, height, samples) = crate::filter::dct::decode_target(
                &raw,
                source,
                plan.target,
                crate::filter::FilterContext {
                    max_image_pixels: ctx.limits().max_image_pixels,
                    max_decoded_bytes: ctx.limits().max_decoded_bytes,
                    ..Default::default()
                },
                &pool,
            )?;
            return Self::try_new_with_samples(
                name,
                object,
                ctx,
                Some(DecodedImageSamples {
                    source,
                    dimensions: (width, height),
                    samples,
                    decode_applied: false,
                }),
            );
        }
        if full_fits
            && u64::from(source.0) * u64::from(source.1)
                > crate::filter::MAX_TARGET_SOURCE_IMAGE_PIXELS
        {
            return Self::try_new(name, object, ctx);
        }
        if device_color_override()? {
            details.unsupported_reason = Some("device_color_override");
            return Self::try_new(name, object, ctx);
        }
        let color = stream
            .dict()
            .get("ColorSpace")
            .map(|v| ctx.resolve(v))
            .transpose()?;
        let components = match color
            .as_ref()
            .and_then(PdfObject::as_name)
            .map(|v| v.name())
        {
            Some("DeviceRGB" | "RGB") => 3,
            Some("DeviceGray" | "G") => 1,
            _ => {
                details.unsupported_reason = Some("color_space");
                return Self::try_new(name, object, ctx);
            }
        };
        let Some(predictor) =
            crate::filter::flate_image::Predictor8::from_parameters(params, source.0, components)?
        else {
            details.unsupported_reason = Some("predictor_layout");
            return Self::try_new(name, object, ctx);
        };
        details.target_decode_supported = true;
        plan.validate(ctx.limits())?;
        let decode = image_decode_endpoints(Some(components), stream.dict().get("Decode"), ctx)?;
        let raw = stream.read_raw_data(ctx, &pool)?;
        let decoded = crate::filter::flate_image::decode_target(
            &raw,
            crate::filter::flate_image::FlateImageRequest {
                source: plan.source,
                target: plan.target,
                components,
                predictor,
                decode,
                compatible: ctx.options().mode == crate::parser::syntax::ParseMode::Compatible,
                limits: ctx.limits(),
            },
            &pool,
        )?;
        if decoded.recovered_tail {
            ctx.record_structure_recovery(
                crate::parser::diagnostic::StructureReason::ImageTrailingBytes,
            )?;
        }
        Self::try_new_with_samples(
            name,
            object,
            ctx,
            Some(DecodedImageSamples {
                source,
                dimensions: plan.target,
                samples: decoded.samples,
                decode_applied: true,
            }),
        )
    }
    pub(crate) fn mark_inline(mut self) -> Self {
        self.is_inline = true;
        self
    }
    pub fn try_new<R: Seek + Read>(
        name: String,
        image: &PdfObject,
        ctx: &PDFContext<R>,
    ) -> PdfResult<Self> {
        Self::try_new_with_samples(name, image, ctx, None)
    }
    fn try_new_with_samples<R: Seek + Read>(
        name: String,
        image: &PdfObject,
        ctx: &PDFContext<R>,
        supplied: Option<DecodedImageSamples>,
    ) -> PdfResult<Self> {
        let resolved = ctx.resolve(image)?;
        let image = &resolved;
        let stream = image
            .as_stream()
            .ok_or_else(|| PdfError::ObjectError("image must be a stream".into()))?;
        let filter = stream
            .dict()
            .get("Filter")
            .map(|f| crate::objects::pdf_stream::resolve_filter_setting(f, ctx, false))
            .transpose()?;
        let is_jpx = match filter.as_ref() {
            Some(PdfObject::PdfName(name)) => name.name() == "JPXDecode",
            Some(PdfObject::PdfArray(array)) => array
                .into_iter()
                .any(|f| f.as_name().is_some_and(|n| n.name() == "JPXDecode")),
            _ => false,
        };
        let is_jbig2 = filter.as_ref().is_some_and(|filter| match filter {
            PdfObject::PdfName(name) => name.name() == "JBIG2Decode",
            PdfObject::PdfArray(array) => array
                .into_iter()
                .any(|f| f.as_name().is_some_and(|n| n.name() == "JBIG2Decode")),
            _ => false,
        });
        let is_ccitt = filter.as_ref().is_some_and(|filter| match filter {
            PdfObject::PdfName(name) => matches!(name.name(), "CCF" | "CCITTFaxDecode"),
            PdfObject::PdfArray(array) => array
                .into_iter()
                .last()
                .and_then(PdfObject::as_name)
                .is_some_and(|name| matches!(name.name(), "CCF" | "CCITTFaxDecode")),
            _ => false,
        });
        let jpx_preserve_indices = if is_jpx {
            image
                .get_attr("ColorSpace")
                .map(|value| ColorSpace::try_new(value, ctx))
                .transpose()?
                .is_some_and(|color| matches!(color, ColorSpace::Indexed(_)))
        } else {
            false
        };
        let jpx = if is_jpx {
            let encoded = stream.encoded_jpx_payload(ctx)?;
            Some(crate::filter::jpx::inspect(
                &encoded,
                crate::filter::FilterContext {
                    jpx_preserve_indices,
                    max_decoded_bytes: ctx.limits().max_decoded_bytes,
                    max_image_pixels: ctx.limits().max_image_pixels,
                    ..Default::default()
                },
            )?)
        } else {
            None
        };
        if let Some(info) = jpx {
            let number = |key| {
                image
                    .get_attr(key)
                    .map(|v| ctx.resolve_owned(v))
                    .transpose()
                    .map(|v| v.and_then(|v| v.as_u32()))
            };
            let alpha_mode = number("SMaskInData")?.unwrap_or(0);
            if image.get_attr("SMaskInData").is_some() && number("SMaskInData")?.is_none() {
                return Err(PdfError::ObjectError(
                    "JPX SMaskInData must be an integer".into(),
                ));
            }
            if alpha_mode > 1 {
                return Err(PdfError::UnsupportedFeature(
                    "JPX premultiplied SMaskInData".into(),
                ));
            }
            if alpha_mode == 1 && !info.has_alpha {
                return Err(PdfError::ObjectError(
                    "JPX SMaskInData requires an opacity channel".into(),
                ));
            }
            if alpha_mode == 1 {
                for key in ["Mask", "SMask"] {
                    if let Some(value) = image.get_attr(key)
                        && !matches!(ctx.resolve(value)?, PdfObject::PdfNull)
                    {
                        return Err(PdfError::UnsupportedFeature(
                            "JPX embedded and explicit masks combined".into(),
                        ));
                    }
                }
            }
            if number("Width")? != Some(info.width) || number("Height")? != Some(info.height) {
                return Err(PdfError::ObjectError(
                    "JPX dimensions disagree with image dictionary".into(),
                ));
            }
        }
        if let Some(subtype) = image.get_attr("Subtype") {
            let subtype = ctx.resolve(subtype)?;
            if subtype.as_name().map(|name| name.bytes()) != Some(b"Image".as_slice()) {
                return Err(PdfError::ObjectError("image Subtype must be /Image".into()));
            }
        }
        let interpolate = image
            .get_attr("Interpolate")
            .map(|value| ctx.resolve(value))
            .transpose()?
            .map(|value| {
                value.as_bool().map(|v| v.value()).ok_or_else(|| {
                    PdfError::ObjectError("image Interpolate must be boolean".into())
                })
            })
            .transpose()?
            .unwrap_or(false);
        let is_mask = if let Some(mask) = image.get_attr("ImageMask") {
            let mask = ctx.resolve(mask)?;

            mask.as_bool()
                .ok_or_else(|| PdfError::ObjectError("image ImageMask must be boolean".into()))?
                .value()
        } else {
            false
        };
        if is_mask && jpx.is_some() {
            return Err(PdfError::UnsupportedFeature(
                "JPX stencil image outside baseline".into(),
            ));
        }
        if is_jbig2 && (!is_mask || image.get_attr("BitsPerComponent").is_some()) {
            let bits = image
                .get_attr("BitsPerComponent")
                .map(|v| ctx.resolve_owned(v))
                .transpose()?;
            if bits.as_ref().and_then(PdfObject::as_u32) != Some(1) {
                return Err(PdfError::ObjectError(
                    "JBIG2 images require 1-bit samples".into(),
                ));
            }
        }
        if is_mask {
            for key in ["ColorSpace", "SMask", "Mask"] {
                if let Some(value) = image.get_attr(key)
                    && !matches!(ctx.resolve(value)?, PdfObject::PdfNull)
                {
                    return Err(PdfError::ObjectError(format!(
                        "stencil image must not specify {key}"
                    )));
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
        let source_dimensions = (dimension("Width")?, dimension("Height")?);
        let (width, height) = supplied
            .as_ref()
            .map_or(source_dimensions, |v| v.dimensions);
        if supplied
            .as_ref()
            .is_some_and(|v| v.source != source_dimensions)
        {
            return Err(PdfError::ObjectError(
                "decoded image source dimensions disagree".into(),
            ));
        }
        if u64::from(width) * u64::from(height) > ctx.limits().max_image_pixels {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "image_pixels",
                stage: "image_decode",
                limit: ctx.limits().max_image_pixels,
                actual: u64::from(width) * u64::from(height),
            });
        }
        let pixel_count = usize::try_from(u64::from(width) * u64::from(height))
            .map_err(|_| PdfError::ResourceLimit("image pixel count overflow".into()))?;
        pixel_count
            .checked_mul(4)
            .filter(|bytes| *bytes <= ctx.limits().max_decoded_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("image RGBA byte budget".into()))?;
        let bits = if jpx.is_some() {
            8
        } else if let Some(bits) = image.get_attr("BitsPerComponent") {
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
            if is_mask { 1 } else { 8 }
        };
        if is_mask && bits != 1 {
            return Err(PdfError::ObjectError(
                "stencil image BitsPerComponent must be 1".into(),
            ));
        }
        let stencil_decode = if is_mask {
            let endpoints = image_decode_endpoints(Some(1), image.get_attr("Decode"), ctx)?;
            if endpoints.is_some_and(|e| !matches!((e[0], e[1]), (0.0, 1.0) | (1.0, 0.0))) {
                return Err(PdfError::ObjectError(
                    "stencil image Decode must be [0 1] or [1 0]".into(),
                ));
            }
            endpoints
        } else {
            None
        };
        let colorspace = if is_mask {
            // Internal sample carrier only: a stencil has no PDF ColorSpace.
            ColorSpace::DeviceGray(crate::color::device_gray::DeviceGray::new())
        } else {
            if let Some(colorspace_obj) = image.get_attr("ColorSpace") {
                let color = ColorSpace::try_new(colorspace_obj, ctx)?;
                if let Some(info) = jpx
                    && color.number_of_components()?
                        != info.components - usize::from(info.has_alpha)
                {
                    return Err(PdfError::ObjectError(
                        "JPX dictionary color component count mismatch".into(),
                    ));
                }
                color
            } else if let Some(info) = jpx {
                match info.color_space {
                    crate::filter::jpx::JpxColorSpace::Gray => {
                        ColorSpace::DeviceGray(crate::color::device_gray::DeviceGray::new())
                    }
                    crate::filter::jpx::JpxColorSpace::Rgb => {
                        ColorSpace::DeviceRgb(crate::color::device_rgb::DeviceRgb::new())
                    }
                    crate::filter::jpx::JpxColorSpace::Cmyk => {
                        ColorSpace::DeviceCmyk(crate::color::device_cmyk::DeviceCmyk::new())
                    }
                }
            } else {
                return Err(PdfError::ColorError("image ColorSpace is missing".into()));
            }
        };
        let indexed = matches!(colorspace, ColorSpace::Indexed(_));
        if is_jbig2 && !matches!(colorspace, ColorSpace::DeviceGray(_)) {
            return Err(PdfError::UnsupportedFeature(
                "JBIG2 colour space outside Gray/stencil baseline".into(),
            ));
        }
        let indexed_decode = if indexed && jpx.is_none() {
            image_decode_endpoints(Some(1), image.get_attr("Decode"), ctx)?
        } else {
            None
        };
        let resolved_soft_mask = image
            .get_attr("SMask")
            .map(|mask| ctx.resolve(mask))
            .transpose()?;
        let matte = resolved_soft_mask
            .as_ref()
            .map(|mask| image_matte(mask, &colorspace, ctx))
            .transpose()?
            .flatten();
        let color_sample_decode = match &colorspace {
            ColorSpace::DeviceGray(_)
            | ColorSpace::DeviceRgb(_)
            | ColorSpace::DeviceCmyk(_)
            | ColorSpace::CalGray(_)
            | ColorSpace::CalRgb(_)
            | ColorSpace::Lab(_)
            | ColorSpace::IccBased(_)
            | ColorSpace::Separation(_)
            | ColorSpace::DeviceN(_)
                if matte.is_some()
                    || !matches!(
                        &colorspace,
                        ColorSpace::DeviceGray(_)
                            | ColorSpace::DeviceRgb(_)
                            | ColorSpace::DeviceCmyk(_)
                    ) =>
            {
                let count = colorspace.number_of_components()?;
                let slots = count
                    .checked_mul(2)
                    .filter(|n| {
                        n.checked_mul(std::mem::size_of::<f64>())
                            .is_some_and(|bytes| bytes <= ctx.limits().max_decoded_bytes)
                    })
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("image Decode endpoint budget".into())
                    })?;
                let mut defaults = Vec::new();
                defaults.try_reserve_exact(slots).map_err(|_| {
                    PdfError::ResourceLimit("image Decode endpoint allocation".into())
                })?;
                defaults.resize(slots, 0.0);
                for pair in defaults.as_chunks_mut::<2>().0 {
                    pair[1] = 1.0;
                }
                match &colorspace {
                    ColorSpace::Lab(space) => {
                        for (target, value) in defaults.iter_mut().zip(space.component_ranges()) {
                            *target = f64::from(value);
                        }
                    }
                    ColorSpace::IccBased(space) => {
                        for (target, value) in defaults.iter_mut().zip(space.range()) {
                            *target = f64::from(*value);
                        }
                    }
                    _ => {}
                }
                if jpx.is_none()
                    && let Some(decode) = image.get_attr("Decode")
                {
                    let decode = ctx.resolve(decode)?;
                    if !matches!(decode, PdfObject::PdfNull) {
                        let array = decode
                            .as_array()
                            .filter(|array| array.len() == slots)
                            .ok_or_else(|| {
                                PdfError::ObjectError(
                                    "image Decode component count mismatch".into(),
                                )
                            })?;
                        for (target, value) in defaults.iter_mut().zip(array) {
                            *target = f64::from(
                                ctx.resolve(value)?
                                    .as_f32()
                                    .filter(|value| value.is_finite())
                                    .ok_or_else(|| {
                                        PdfError::ObjectError(
                                            "image Decode requires finite numbers".into(),
                                        )
                                    })?,
                            );
                        }
                    }
                }
                Some((bits, defaults))
            }
            _ => None,
        };
        let (mut data, sample_lease, decode_applied) = if let Some(supplied) = supplied {
            let (data, lease) = supplied.samples.into_reserved_parts();
            (data, Some(lease), supplied.decode_applied)
        } else {
            let data = if is_jpx {
                stream.decode_jpx_image_data(ctx, jpx_preserve_indices)?
            } else {
                stream.decode_data(ctx)?
            };
            (data, None, false)
        };
        let embedded_alpha = if let Some(info) = jpx.filter(|info| info.has_alpha) {
            if data.len()
                != pixel_count
                    .checked_mul(info.components)
                    .ok_or_else(|| PdfError::ResourceLimit("JPX channel bytes".into()))?
            {
                return Err(PdfError::ObjectError(
                    "JPX interleaved channel length mismatch".into(),
                ));
            }
            let use_alpha = image
                .get_attr("SMaskInData")
                .map(|v| ctx.resolve_owned(v))
                .transpose()?
                .and_then(|v| v.as_u32())
                == Some(1);
            let colors = info.components - 1;
            let color_bytes = pixel_count
                .checked_mul(colors)
                .ok_or_else(|| PdfError::ResourceLimit("JPX color bytes".into()))?;
            let alpha_bytes = if use_alpha { pixel_count } else { 0 };
            if data
                .capacity()
                .checked_add(color_bytes)
                .and_then(|n| n.checked_add(alpha_bytes))
                .is_none_or(|n| n > ctx.limits().max_decoded_bytes)
            {
                return Err(PdfError::ResourceLimit(
                    "JPX channel separation bytes".into(),
                ));
            }
            let mut pixels = Vec::new();
            pixels
                .try_reserve_exact(color_bytes)
                .map_err(|_| PdfError::ResourceLimit("JPX color allocation".into()))?;
            let mut alpha = Vec::new();
            alpha
                .try_reserve_exact(alpha_bytes)
                .map_err(|_| PdfError::ResourceLimit("JPX opacity allocation".into()))?;
            for sample in data.chunks_exact(info.components) {
                pixels.extend_from_slice(&sample[..colors]);
                if use_alpha {
                    alpha.push(sample[colors]);
                }
            }
            data = pixels;
            use_alpha.then_some(ImageSoftMask {
                width,
                height,
                data: alpha,
                bits: 8,
                decode: [0., 1.],
                matte: None,
            })
        } else {
            None
        };

        if is_ccitt {
            // The legacy fax filter returns one 0/255 byte per sample. Adapt
            // that contract to PDF's row-padded 1-bit samples before applying
            // Decode or stencil semantics; do not unpack the expanded bytes.
            if bits != 1 || colorspace.number_of_components()? != 1 {
                return Err(PdfError::ObjectError(
                    "CCITT images require one-component 1-bit samples".into(),
                ));
            }
            if data.len() != pixel_count || data.iter().any(|value| !matches!(*value, 0 | 255)) {
                return Err(PdfError::ObjectError(
                    "CCITT decoded sample length/value mismatch".into(),
                ));
            }
            let row_bytes = (width as usize).div_ceil(8);
            for row in 0..height as usize {
                for byte in 0..row_bytes {
                    let mut packed = 0;
                    for bit in 0..8 {
                        let x = byte * 8 + bit;
                        if x < width as usize && data[row * width as usize + x] != 0 {
                            packed |= 1 << (7 - bit);
                        }
                    }
                    data[row * row_bytes + byte] = packed;
                }
            }
            data.truncate(row_bytes * height as usize);
        }
        let components = match &colorspace {
            ColorSpace::DeviceGray(_) => Some(1),
            ColorSpace::DeviceRgb(_) => Some(3),
            ColorSpace::DeviceCmyk(_) => Some(4),
            _ => None,
        };
        // Some producers count the two stream-separating whitespace bytes in
        // an image's decoded samples. Recover only this bounded surplus;
        // never pad missing samples or discard arbitrary extra image data.
        let packed_row = (width as usize)
            .checked_mul(colorspace.number_of_components()?)
            .and_then(|n| n.checked_mul(bits))
            .and_then(|n| n.checked_add(7))
            .map(|n| n / 8)
            .ok_or_else(|| PdfError::ResourceLimit("image packed row bytes".into()))?;
        let expected = packed_row
            .checked_mul(height as usize)
            .ok_or_else(|| PdfError::ResourceLimit("image packed bytes".into()))?;
        if !is_mask
            && (filter
                .as_ref()
                .is_none_or(|filter| matches!(filter, PdfObject::PdfNull))
                || filter.as_ref().is_some_and(|filter| match filter {
                    PdfObject::PdfName(name) => {
                        matches!(name.name(), "FlateDecode" | "Fl" | "LZWDecode" | "LZW")
                    }
                    PdfObject::PdfArray(filters) => filters
                        .into_iter()
                        .last()
                        .and_then(PdfObject::as_name)
                        .is_some_and(|name| {
                            matches!(name.name(), "FlateDecode" | "Fl" | "LZWDecode" | "LZW")
                        }),
                    _ => false,
                }))
            && data.len() > expected
            && matches!(&data[expected..], b"\r\n" | b"\n" | b"\r")
            && ctx.options().mode == crate::parser::syntax::ParseMode::Compatible
        {
            data.truncate(expected);
            ctx.record_structure_recovery(
                crate::parser::diagnostic::StructureReason::ImageTrailingBytes,
            )?;
        }
        // SMask takes precedence over Mask. Colour keys compare encoded samples
        // before Decode, colour conversion, or indexed palette lookup.
        let hard_mask_alpha = if resolved_soft_mask
            .as_ref()
            .is_some_and(|value| !matches!(value, PdfObject::PdfNull))
        {
            None
        } else if let Some(value) = image.get_attr("Mask") {
            let value = ctx.resolve(value)?;
            if matches!(value, PdfObject::PdfNull) {
                None
            } else {
                Some(image_mask_alpha(
                    &value,
                    &data,
                    width,
                    height,
                    bits,
                    colorspace.number_of_components()?,
                    ctx,
                )?)
            }
        } else {
            None
        };
        if color_sample_decode.is_some() {
            let count = colorspace.number_of_components()?;
            let row_bytes = (width as usize)
                .checked_mul(count)
                .and_then(|n| n.checked_mul(bits))
                .and_then(|n| n.checked_add(7))
                .map(|n| n / 8)
                .ok_or_else(|| PdfError::ResourceLimit("image sample size overflow".into()))?;
            if row_bytes.checked_mul(height as usize) != Some(data.len()) {
                return Err(PdfError::ObjectError(
                    "packed image sample length mismatch".into(),
                ));
            }
        } else if indexed {
            data = unpack_image_samples_inner(
                &data,
                width as usize,
                height as usize,
                1,
                bits,
                indexed_decode,
                ctx.limits().max_decoded_bytes,
                true,
            )?;
        } else if bits == 8 {
            // Decode is ignored for non-stencil JPX images (ISO image dictionary).
            if jpx.is_none() && !decode_applied {
                apply_image_decode(&mut data, components, image.get_attr("Decode"), ctx)?;
            }
        } else {
            let components = components.ok_or_else(|| {
                PdfError::UnsupportedFeature("packed image samples for this color space".into())
            })?;
            let endpoints = if is_mask {
                stencil_decode
            } else {
                image_decode_endpoints(Some(components), image.get_attr("Decode"), ctx)?
            };
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
        let soft_mask = match resolved_soft_mask.as_ref() {
            None | Some(PdfObject::PdfNull) => embedded_alpha,
            Some(mask) => {
                let mask = mask
                    .as_stream()
                    .ok_or_else(|| PdfError::ObjectError("SMask must be an image stream".into()))?;
                for (key, expected) in [("Subtype", "Image"), ("ColorSpace", "DeviceGray")] {
                    let value = mask
                        .dict()
                        .get(key)
                        .ok_or_else(|| PdfError::ObjectError("SMask image field missing".into()))?;
                    let value = ctx.resolve(value)?;
                    if value.as_name().map(|name| name.name()) != Some(expected) {
                        return Err(PdfError::UnsupportedFeature(
                            "SMask requires Image/DeviceGray".into(),
                        ));
                    }
                }
                let dimension = |key: &str| -> PdfResult<u32> {
                    let value = ctx.resolve(mask.dict().get(key).ok_or_else(|| {
                        PdfError::ObjectError("SMask dimension missing".into())
                    })?)?;
                    value.as_u32().filter(|n| *n > 0).ok_or_else(|| {
                        PdfError::ObjectError("SMask dimension must be a positive integer".into())
                    })
                };
                let mask_width = dimension("Width")?;
                let mask_height = dimension("Height")?;
                if u64::from(mask_width) * u64::from(mask_height) > ctx.limits().max_image_pixels {
                    return Err(PdfError::ResourceLimit("SMask pixel limit".into()));
                }
                let mask_bits =
                    ctx.resolve(mask.dict().get("BitsPerComponent").ok_or_else(|| {
                        PdfError::ObjectError("SMask BitsPerComponent missing".into())
                    })?)?;
                let mask_bits = mask_bits
                    .as_u32()
                    .filter(|bits| matches!(bits, 1 | 2 | 4 | 8 | 16))
                    .ok_or_else(|| PdfError::ObjectError("invalid SMask BitsPerComponent".into()))?
                    as usize;
                for key in ["Mask", "SMask"] {
                    if let Some(value) = mask.dict().get(key)
                        && !matches!(ctx.resolve(value)?, PdfObject::PdfNull)
                    {
                        return Err(PdfError::UnsupportedFeature(format!("SMask {key}")));
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
                let endpoints = image_decode_endpoints(Some(1), mask.dict().get("Decode"), ctx)?;
                let row_bytes = (mask_width as usize)
                    .checked_mul(mask_bits)
                    .and_then(|n| n.checked_add(7))
                    .map(|n| n / 8)
                    .ok_or_else(|| PdfError::ResourceLimit("SMask sample size".into()))?;
                let expected = row_bytes
                    .checked_mul(mask_height as usize)
                    .ok_or_else(|| PdfError::ResourceLimit("SMask sample bytes".into()))?;
                if alpha.len() > expected
                    && matches!(&alpha[expected..], b"\r\n" | b"\n" | b"\r")
                    && ctx.options().mode == crate::parser::syntax::ParseMode::Compatible
                {
                    alpha.truncate(expected);
                    ctx.record_structure_recovery(
                        crate::parser::diagnostic::StructureReason::ImageTrailingBytes,
                    )?;
                }
                if row_bytes.checked_mul(mask_height as usize) != Some(alpha.len()) {
                    return Err(PdfError::ObjectError(
                        "SMask data length does not match dimensions".into(),
                    ));
                }
                Some(ImageSoftMask {
                    width: mask_width,
                    height: mask_height,
                    data: alpha,
                    bits: mask_bits,
                    decode: endpoints.map_or([0.0, 1.0], |v| [v[0], v[1]]),
                    matte,
                })
            }
        };

        let rendering_intent = image
            .get_attr("Intent")
            .map(|value| {
                let value = ctx.resolve(value)?;
                let name = value
                    .as_name()
                    .ok_or_else(|| PdfError::ObjectError("image Intent must be a name".into()))?
                    .name();
                crate::color::conversion::RenderingIntent::from_pdf_name(name)
                    .ok_or_else(|| PdfError::UnsupportedFeature("image rendering Intent".into()))
            })
            .transpose()?;
        Ok(Self {
            name,
            height,
            width,
            colorspace,
            data: if let Some(lease) = sample_lease {
                ImageSamples::Budgeted(std::rc::Rc::new(
                    crate::budget::BudgetedBytes::from_reserved_vec(data, lease)?,
                ))
            } else {
                ImageSamples::Owned(data)
            },
            source_dimensions,
            is_inline: false,
            is_mask,
            interpolate,
            pixel_count,
            max_rgba_bytes: ctx.limits().max_decoded_bytes,
            soft_mask,
            hard_mask_alpha,
            color_sample_decode,
            rendering_intent,
        })
    }
    pub fn has_explicit_mask(&self) -> bool {
        self.hard_mask_alpha.is_some()
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
    /// Original PDF image dimensions, including for a render-only preview.
    pub fn source_pixel_dimensions(&self) -> (u32, u32) {
        self.source_dimensions
    }
    pub fn data(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub(crate) fn retained_sample_bytes(&self) -> PdfResult<usize> {
        let bytes = self
            .data
            .len()
            .checked_add(self.soft_mask.as_ref().map_or(0, |mask| mask.data.len()))
            .and_then(|n| {
                n.checked_add(
                    self.hard_mask_alpha
                        .as_ref()
                        .map_or(0, |mask| mask.alpha.len()),
                )
            })
            .and_then(|n| {
                self.soft_mask
                    .as_ref()
                    .and_then(|mask| mask.matte.as_ref())
                    .map_or(Some(n), |matte| {
                        matte
                            .capacity()
                            .checked_mul(std::mem::size_of::<f32>())
                            .and_then(|bytes| n.checked_add(bytes))
                    })
            })
            .and_then(|n| {
                self.color_sample_decode
                    .as_ref()
                    .map_or(Some(n), |(_, endpoints)| {
                        endpoints
                            .capacity()
                            .checked_mul(std::mem::size_of::<f64>())
                            .and_then(|bytes| n.checked_add(bytes))
                    })
            })
            .ok_or_else(|| PdfError::ResourceLimit("image retained samples".into()))?;
        bytes
            .checked_add(self.colorspace.retained_profile_bytes()?)
            .ok_or_else(|| PdfError::ResourceLimit("image retained profile".into()))
    }

    pub fn colorspace(&self) -> &ColorSpace {
        &self.colorspace
    }
    pub fn interpolate(&self) -> bool {
        self.interpolate
    }

    pub fn is_mask(&self) -> bool {
        self.is_mask
    }

    pub fn is_inline(&self) -> bool {
        self.is_inline
    }
    pub fn rgba_buffer(&self) -> PdfResult<Vec<u8>> {
        let mut context = crate::color::conversion::ColorConversionContext::default();
        if let Some(intent) = self.rendering_intent {
            context.rendering_intent = intent;
        }
        self.rgba_buffer_inner(None, &context, self.width, self.height)
    }
    pub fn rendering_intent(&self) -> Option<crate::color::conversion::RenderingIntent> {
        self.rendering_intent
    }
    pub fn rgba_buffer_with_context(
        &self,
        context: &crate::color::conversion::ColorConversionContext,
    ) -> PdfResult<Vec<u8>> {
        self.rgba_buffer_inner(None, context, self.width, self.height)
    }

    /// Rasterize an image mask at its own resolution when it is finer than
    /// the base image. Extraction APIs retain the original image dimensions.
    pub fn rendering_rgba_with_context(
        &self,
        context: &crate::color::conversion::ColorConversionContext,
    ) -> PdfResult<(u32, u32, Vec<u8>)> {
        let mut width = self.width;
        let mut height = self.height;
        if let Some(mask) = &self.hard_mask_alpha {
            width = width.max(mask.width);
            height = height.max(mask.height);
        }
        if let Some(mask) = &self.soft_mask {
            width = width.max(mask.width);
            height = height.max(mask.height);
        }
        let pixels = self.rgba_buffer_inner(None, context, width, height)?;
        Ok((width, height, pixels))
    }

    /// Apply the current nonstroking color to a stencil's transparent coverage.
    pub fn rgba_buffer_with_stencil_color(
        &self,
        color: &crate::color::value::ColorRgb,
    ) -> PdfResult<Vec<u8>> {
        if !self.is_mask
            || [color.r, color.g, color.b]
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(PdfError::ColorError(
                "stencil color requires a stencil and finite RGB components in [0,1]".into(),
            ));
        }
        self.rgba_buffer_inner(
            Some(color),
            &crate::color::conversion::ColorConversionContext::default(),
            self.width,
            self.height,
        )
    }

    fn rgba_buffer_inner(
        &self,
        stencil_color: Option<&crate::color::value::ColorRgb>,
        context: &crate::color::conversion::ColorConversionContext,
        width: u32,
        height: u32,
    ) -> PdfResult<Vec<u8>> {
        if self.is_mask && stencil_color.is_none() {
            return Err(PdfError::ColorError(
                "stencil RGBA conversion requires the current nonstroking color".into(),
            ));
        }
        let components = match &self.colorspace {
            ColorSpace::DeviceGray(_) => 1,
            ColorSpace::DeviceRgb(_) => 3,
            ColorSpace::DeviceCmyk(_) => 4,
            ColorSpace::Indexed(_) => 1,
            ColorSpace::CalGray(_)
            | ColorSpace::CalRgb(_)
            | ColorSpace::Lab(_)
            | ColorSpace::IccBased(_)
            | ColorSpace::Separation(_)
            | ColorSpace::DeviceN(_) => self.colorspace.number_of_components()?,
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
        if self.color_sample_decode.is_none() && self.data.len() != input_size {
            return Err(PdfError::ObjectError(
                "image data length does not match dimensions/components".into(),
            ));
        }
        if let Some(mask) = &self.soft_mask {
            let row_bytes = (mask.width as usize)
                .checked_mul(mask.bits)
                .and_then(|n| n.checked_add(7))
                .map(|n| n / 8)
                .ok_or_else(|| PdfError::ResourceLimit("SMask sample size".into()))?;
            if row_bytes.checked_mul(mask.height as usize) != Some(mask.data.len()) {
                return Err(PdfError::ObjectError("SMask alpha length mismatch".into()));
            }
        }
        let pixel_count = (width as usize)
            .checked_mul(height as usize)
            .ok_or_else(|| PdfError::ResourceLimit("image render pixel count".into()))?;
        let source_index = |index: usize| {
            if (width, height) == (self.width, self.height) {
                return index;
            }
            let x = index % width as usize;
            let y = index / width as usize;
            let sx =
                ((x as u128 * 2 + 1) * u128::from(self.width) / (u128::from(width) * 2)) as usize;
            let sy =
                ((y as u128 * 2 + 1) * u128::from(self.height) / (u128::from(height) * 2)) as usize;
            sy * self.width as usize + sx
        };
        let output_size = pixel_count
            .checked_mul(4)
            .filter(|n| *n <= self.max_rgba_bytes)
            .ok_or_else(|| PdfError::ResourceLimit("image RGBA byte budget".into()))?;
        let mut rgba = Vec::new();
        rgba.try_reserve_exact(output_size)
            .map_err(|_| PdfError::ResourceLimit("unable to allocate image RGBA buffer".into()))?;
        // Common scan path: preserve the ICC transform, Decode and Range, but
        // amortize transform-cache locking and dispatch over small stack batches.
        // Matte and resampled masks retain the general high-precision path.
        if let (ColorSpace::IccBased(space), Some((8, endpoints))) =
            (&self.colorspace, &self.color_sample_decode)
            && self.soft_mask.is_none()
            && self.hard_mask_alpha.is_none()
            && (width, height) == (self.width, self.height)
            && context.icc_fallback == crate::color::conversion::IccFallback::Reject
            && let Some(profile) = space.profile()
        {
            if self.data.len() != input_size {
                return Err(PdfError::ObjectError(
                    "packed image sample length mismatch".into(),
                ));
            }
            let mut source = [0f32; 128 * 4];
            let mut target = [0f32; 128 * 3];
            // Decode/Range depends on the component and byte, not the pixel.
            // Keep exactly the scalar f64 -> f32 rounding with bounded tables.
            let mut decoded = [[0f32; 256]; 4];
            for (c, table) in decoded.iter_mut().enumerate().take(components) {
                for (sample, value) in table.iter_mut().enumerate() {
                    let v = (endpoints[c * 2]
                        + sample as f64 / 255. * (endpoints[c * 2 + 1] - endpoints[c * 2]))
                        as f32;
                    *value = v.clamp(space.range()[c * 2], space.range()[c * 2 + 1]);
                }
            }
            for data in self.data.chunks(128 * components) {
                for (input, output) in data
                    .chunks_exact(components)
                    .zip(source[..data.len()].chunks_exact_mut(components))
                {
                    for (c, (&sample, value)) in input.iter().zip(output).enumerate() {
                        *value = decoded[c][sample as usize];
                    }
                }
                let rgb = &mut target[..data.len() / components * 3];
                profile.convert_samples(&mut source[..data.len()], rgb, context)?;
                for pixel in rgb.as_chunks::<3>().0 {
                    let color = crate::color::value::ColorRgb::new(pixel[0], pixel[1], pixel[2]);
                    rgba.extend_from_slice(&[color.r(), color.g(), color.b(), 255]);
                }
            }
            return Ok(rgba);
        }
        if let Some((bits, endpoints)) = &self.color_sample_decode {
            let bits = *bits;
            let row_samples = (self.width as usize)
                .checked_mul(components)
                .ok_or_else(|| PdfError::ResourceLimit("image sample size overflow".into()))?;
            let row_bytes = row_samples
                .checked_mul(bits)
                .and_then(|n| n.checked_add(7))
                .map(|n| n / 8)
                .ok_or_else(|| PdfError::ResourceLimit("image sample size overflow".into()))?;
            if row_bytes.checked_mul(self.height as usize) != Some(self.data.len()) {
                return Err(PdfError::ObjectError(
                    "packed image sample length mismatch".into(),
                ));
            }
            let mut values = Vec::new();
            values
                .try_reserve_exact(components)
                .map_err(|_| PdfError::ResourceLimit("image color component allocation".into()))?;
            values.resize(components, 0.0);
            let mut color = crate::color::value::ColorValue::new(values);
            let maximum = ((1u32 << bits) - 1) as f64;
            for y in 0..height as usize {
                for x in 0..width as usize {
                    let source = source_index(y * width as usize + x);
                    let row = &self.data[source / self.width as usize * row_bytes..][..row_bytes];
                    let pixel = source % self.width as usize;
                    for component in 0..components {
                        let offset = (pixel * components + component) * bits;
                        let sample = if bits == 16 {
                            u16::from_be_bytes([row[offset / 8], row[offset / 8 + 1]]) as u32
                        } else {
                            (u32::from(row[offset / 8]) >> (8 - bits - offset % 8))
                                & ((1 << bits) - 1)
                        };
                        color.values_mut()[component] = (endpoints[component * 2]
                            + sample as f64 / maximum
                                * (endpoints[component * 2 + 1] - endpoints[component * 2]))
                            as f32;
                    }
                    let pixel_index = rgba.len() / 4;
                    if let Some(mask) = &self.soft_mask
                        && let Some(matte) = &mask.matte
                    {
                        let alpha = mask.alpha(pixel_index, width as usize, height as usize);
                        if alpha == 0.0 {
                            rgba.extend_from_slice(&[0, 0, 0, 0]);
                            continue;
                        }
                        unblend_matte(&mut color, matte, alpha, &self.colorspace);
                    }
                    let rgb = self.colorspace.rgb_with_context(&color, context)?;
                    rgba.extend_from_slice(&[rgb.r(), rgb.g(), rgb.b(), 255]);
                }
            }
        } else {
            match &self.colorspace {
                ColorSpace::DeviceGray(_) => {
                    for index in 0..pixel_count {
                        let g = self.data[source_index(index)];
                        if let Some(color) = stencil_color {
                            rgba.extend_from_slice(&[color.r(), color.g(), color.b(), 255 - g]);
                        } else {
                            rgba.extend_from_slice(&[g, g, g, 255]);
                        }
                    }
                }
                ColorSpace::DeviceRgb(_) => {
                    for index in 0..pixel_count {
                        let offset = source_index(index) * 3;
                        let chunk = &self.data[offset..offset + 3];
                        rgba.extend_from_slice(&[chunk[0], chunk[1], chunk[2], 255]);
                    }
                }
                ColorSpace::DeviceCmyk(_) => {
                    for index in 0..pixel_count {
                        let offset = source_index(index) * 4;
                        let chunk = &self.data[offset..offset + 4];
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
                ColorSpace::Indexed(space) => {
                    // Cache only entries actually encountered: eagerly converting the
                    // entire table could fail on an unused tint-transform input.
                    // Fixed stack storage bounds the cache independently of image size.
                    let mut palette: [Option<[u8; 4]>; 256] = [None; 256];
                    for pixel_index in 0..pixel_count {
                        let index = self.data[source_index(pixel_index)];
                        if let Some(mask) = &self.soft_mask
                            && let Some(matte) = &mask.matte
                        {
                            let alpha = mask.alpha(pixel_index, width as usize, height as usize);
                            if alpha == 0.0 {
                                rgba.extend_from_slice(&[0, 0, 0, 0]);
                                continue;
                            }
                            let mut color =
                                space.base_value(&crate::color::value::ColorValue::new(vec![
                                    index as f32,
                                ]))?;
                            unblend_matte(&mut color, matte, alpha, space.base());
                            let rgb = space.base().rgb_with_context(&color, context)?;
                            rgba.extend_from_slice(&[rgb.r(), rgb.g(), rgb.b(), 255]);
                            continue;
                        }
                        let pixel = match palette[index as usize] {
                            Some(pixel) => pixel,
                            None => {
                                let rgb = space.rgb_with_context(
                                    &crate::color::value::ColorValue::new(vec![index as f32]),
                                    context,
                                )?;
                                let pixel = [rgb.r(), rgb.g(), rgb.b(), 255];
                                palette[index as usize] = Some(pixel);
                                pixel
                            }
                        };
                        rgba.extend_from_slice(&pixel);
                    }
                }
                _ => {}
            }
        }
        if self.color_sample_decode.is_none()
            && stencil_color.is_none()
            && !context.output_profile().is_builtin_srgb()
            && matches!(
                self.colorspace,
                ColorSpace::DeviceGray(_) | ColorSpace::DeviceRgb(_) | ColorSpace::DeviceCmyk(_)
            )
        {
            for pixel in rgba.as_chunks_mut::<4>().0 {
                let color = context.convert_srgb(crate::color::value::ColorRgb::new(
                    f32::from(pixel[0]) / 255.,
                    f32::from(pixel[1]) / 255.,
                    f32::from(pixel[2]) / 255.,
                ))?;
                pixel[0] = color.r();
                pixel[1] = color.g();
                pixel[2] = color.b();
            }
        }
        if let Some(mask) = &self.soft_mask {
            for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                pixel[3] =
                    (mask.alpha(index, width as usize, height as usize) * 255.0).round() as u8;
            }
        }
        if let Some(mask) = &self.hard_mask_alpha {
            for (index, pixel) in rgba.as_chunks_mut::<4>().0.iter_mut().enumerate() {
                let alpha =
                    mask.alpha_at(index as u32 % width, index as u32 / width, width, height);
                pixel[3] = ((u16::from(pixel[3]) * u16::from(alpha) + 127) / 255) as u8;
            }
        }
        Ok(rgba)
    }
}

fn image_mask_alpha<R: Read + Seek>(
    mask: &PdfObject,
    data: &[u8],
    width: u32,
    height: u32,
    bits: usize,
    components: usize,
    ctx: &PDFContext<R>,
) -> PdfResult<ImageHardMask> {
    let count = (width as usize)
        .checked_mul(height as usize)
        .ok_or_else(|| PdfError::ResourceLimit("image Mask pixels".into()))?;
    crate::filter::check_output_size(0, count, ctx.limits().max_decoded_bytes, "image Mask alpha")?;
    let mut alpha = Vec::new();
    alpha
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("image Mask allocation".into()))?;
    if let Some(ranges) = mask.as_array() {
        if ranges.len()
            != components
                .checked_mul(2)
                .ok_or_else(|| PdfError::ResourceLimit("image Mask ranges".into()))?
        {
            return Err(PdfError::ObjectError(
                "colour-key Mask component count mismatch".into(),
            ));
        }
        let mut keys = Vec::new();
        keys.try_reserve_exact(components)
            .map_err(|_| PdfError::ResourceLimit("image Mask keys allocation".into()))?;
        for pair in 0..components {
            let endpoint = |offset| -> PdfResult<u32> {
                ctx.resolve(ranges.get(offset).unwrap())?
                    .as_number()
                    .ok_or_else(|| {
                        PdfError::ObjectError("image Mask keys must be integers".into())
                    })?
                    .as_u32_checked()
            };
            let (low, high) = (endpoint(pair * 2)?, endpoint(pair * 2 + 1)?);
            if low > high || high >= (1u32 << bits) {
                return Err(PdfError::ObjectError(
                    "image Mask range outside sample domain".into(),
                ));
            }
            keys.push((low, high));
        }
        let row_bytes = (width as usize)
            .checked_mul(components)
            .and_then(|n| n.checked_mul(bits))
            .and_then(|n| n.checked_add(7))
            .map(|n| n / 8)
            .ok_or_else(|| PdfError::ResourceLimit("image Mask row bytes".into()))?;
        if row_bytes.checked_mul(height as usize) != Some(data.len()) {
            return Err(PdfError::ObjectError(
                "colour-key Mask sample length mismatch".into(),
            ));
        }
        for row in data.chunks_exact(row_bytes) {
            for x in 0..width as usize {
                let transparent = keys.iter().enumerate().all(|(component, (low, high))| {
                    let offset = (x * components + component) * bits;
                    let value = if bits == 16 {
                        u32::from(u16::from_be_bytes([row[offset / 8], row[offset / 8 + 1]]))
                    } else {
                        (u32::from(row[offset / 8]) >> (8 - bits - offset % 8)) & ((1 << bits) - 1)
                    };
                    (*low..=*high).contains(&value)
                });
                alpha.push(if transparent { 0 } else { 255 });
            }
        }
    } else if let Some(stream) = mask.as_stream() {
        let stencil = stream
            .dict()
            .get("ImageMask")
            .map(|v| ctx.resolve(v))
            .transpose()?;
        if !stencil
            .as_ref()
            .and_then(PdfObject::as_bool)
            .is_some_and(|v| v.value())
        {
            return Err(PdfError::ObjectError(
                "explicit Mask must be an ImageMask stream".into(),
            ));
        }
        // A stencil is forbidden from containing Mask/SMask, bounding recursion.
        let stencil = PdfImage::try_new("Mask".into(), mask, ctx)?;
        if stencil.data.len() != stencil.pixel_count {
            return Err(PdfError::ObjectError(
                "explicit Mask sample length mismatch".into(),
            ));
        }
        let ImageSamples::Owned(mut alpha) = stencil.data else {
            return Err(PdfError::ObjectError(
                "stencil mask requires full-fidelity samples".into(),
            ));
        };
        for value in &mut alpha {
            *value = 255 - *value;
        }
        return Ok(ImageHardMask {
            width: stencil.width,
            height: stencil.height,
            alpha,
        });
    } else {
        return Err(PdfError::ObjectError(
            "image Mask must be an array or stencil stream".into(),
        ));
    }
    Ok(ImageHardMask {
        width,
        height,
        alpha,
    })
}

fn component_range(space: &ColorSpace, component: usize) -> (f64, f64) {
    let range = match space {
        ColorSpace::Lab(space) => {
            let ranges = space.component_ranges();
            (ranges[component * 2], ranges[component * 2 + 1])
        }
        ColorSpace::IccBased(space) => (
            space.range()[component * 2],
            space.range()[component * 2 + 1],
        ),
        _ => (0.0, 1.0),
    };
    (f64::from(range.0), f64::from(range.1))
}

fn image_matte<R: Read + Seek>(
    mask: &PdfObject,
    space: &ColorSpace,
    ctx: &PDFContext<R>,
) -> PdfResult<Option<Vec<f32>>> {
    let Some(matte) = mask.as_stream().and_then(|mask| mask.dict().get("Matte")) else {
        return Ok(None);
    };
    let matte = ctx.resolve(matte)?;
    if matches!(matte, PdfObject::PdfNull) {
        return Ok(None);
    }
    // Indexed palettes contain the preblended base colours, not preblended
    // indices. Consequently Matte describes the base colour components.
    let space = match space {
        ColorSpace::Indexed(space) => space.base(),
        _ => space,
    };
    let count = space.number_of_components()?;
    let values = matte
        .as_array()
        .filter(|array| array.len() == count)
        .ok_or_else(|| PdfError::ObjectError("SMask Matte component count mismatch".into()))?;
    if count
        .checked_mul(std::mem::size_of::<f32>())
        .is_none_or(|bytes| bytes > ctx.limits().max_decoded_bytes)
    {
        return Err(PdfError::ResourceLimit(
            "SMask Matte component budget".into(),
        ));
    }
    let mut color = Vec::new();
    color
        .try_reserve_exact(count)
        .map_err(|_| PdfError::ResourceLimit("SMask Matte allocation".into()))?;
    for (component, value) in values.into_iter().enumerate() {
        let value = ctx
            .resolve(value)?
            .as_f32()
            .filter(|value| value.is_finite())
            .ok_or_else(|| PdfError::ObjectError("SMask Matte requires finite numbers".into()))?;
        let (low, high) = component_range(space, component);
        if !(low..=high).contains(&f64::from(value)) {
            return Err(PdfError::ObjectError(
                "SMask Matte component out of range".into(),
            ));
        }
        color.push(value);
    }
    Ok(Some(color))
}

fn unblend_matte(
    color: &mut crate::color::value::ColorValue,
    matte: &[f32],
    alpha: f64,
    space: &ColorSpace,
) {
    // ISO 32000-1, 11.6.5.3: c' = m + alpha * (c - m).
    // Invert after Decode, before colour conversion (which may be nonlinear).
    for (component, (value, matte)) in color.values_mut().iter_mut().zip(matte).enumerate() {
        let (low, high) = component_range(space, component);
        let premixed = f64::from(*value).clamp(low, high);
        let matte = f64::from(*matte);
        *value = (matte + (premixed - matte) / alpha).clamp(low, high) as f32;
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
    if components > 4 {
        return Err(PdfError::UnsupportedFeature(
            "fixed image Decode supports at most four components".into(),
        ));
    }
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
    unpack_image_samples_inner(
        source, width, height, components, bits, endpoints, limit, false,
    )
}

#[expect(
    clippy::too_many_arguments,
    reason = "PDF component geometry and limits are independent inputs"
)]
fn unpack_image_samples_inner(
    source: &[u8],
    width: usize,
    height: usize,
    components: usize,
    bits: usize,
    endpoints: Option<[f64; 8]>,
    limit: usize,
    indexed: bool,
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
            let (low, high) = endpoints.map_or((0.0, if indexed { maximum } else { 1.0 }), |e| {
                let component = index % components;
                (e[component * 2], e[component * 2 + 1])
            });
            let decoded = low + sample as f64 / maximum * (high - low);
            output.push(if indexed {
                decoded.clamp(0.0, 255.0).round() as u8
            } else {
                (decoded.clamp(0.0, 1.0) * 255.0).round() as u8
            });
        }
    }
    Ok(output)
}

#[cfg(test)]
#[path = "pdf_image_matte_tests.rs"]
mod matte_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::color::device_rgb::DeviceRgb;

    #[test]
    fn nondevice_image_decode_and_retained_budget_boundaries() {
        use crate::{
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::{
                options::{Limits, ParseOptions},
                syntax::SyntaxParser,
            },
        };
        use std::io::Cursor;
        let make_context = |limit| {
            PDFContext::<Cursor<Vec<u8>>>::new_without_source(
                PdfDict::default(),
                ParseOptions {
                    limits: Limits {
                        max_decoded_bytes: limit,
                        ..Default::default()
                    },
                    ..Default::default()
                },
            )
        };
        let image = |decode: &str, bytes: Vec<u8>| {
            let source = format!(
                "<< /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace [/CalGray << /WhitePoint [0.95047 1 1.08883] >>] {decode} >>"
            );
            let dict =
                SyntaxParser::new(StreamReader::try_new(Cursor::new(source.into_bytes())).unwrap())
                    .get_object()
                    .unwrap()
                    .into_dict()
                    .unwrap();
            PdfObject::PdfStream(PdfStream::new(dict, bytes))
        };
        let ctx = make_context(1024);
        for decode in [
            "/Decode [0]",
            "/Decode [0 1 0 1]",
            "/Decode /Bad",
            "/Decode [0 /Bad]",
        ] {
            assert!(
                matches!(
                    PdfImage::try_new("invalid".into(), &image(decode, vec![128]), &ctx),
                    Err(PdfError::ObjectError(_))
                ),
                "{decode}"
            );
        }
        for bytes in [vec![], vec![0, 1]] {
            assert!(matches!(
                PdfImage::try_new("invalid".into(), &image("", bytes), &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
        let mut valid =
            PdfImage::try_new("valid".into(), &image("/Decode null", vec![128]), &ctx).unwrap();
        let endpoint_capacity = valid.color_sample_decode.as_ref().unwrap().1.capacity();
        assert_eq!(
            valid.retained_sample_bytes().unwrap(),
            1 + endpoint_capacity * std::mem::size_of::<f64>()
        );
        assert_eq!(valid.rgba_buffer().unwrap().len(), 4);
        valid.max_rgba_bytes = 3;
        assert!(matches!(
            valid.rgba_buffer(),
            Err(PdfError::ResourceLimit(_))
        ));
        let constrained = make_context(15);
        assert!(
            matches!(PdfImage::try_new("endpoint-budget".into(), &image("", vec![128]), &constrained), Err(PdfError::ResourceLimit(message)) if message == "image Decode endpoint budget")
        );
        let exact = make_context(16);
        assert!(PdfImage::try_new("endpoint-budget".into(), &image("", vec![128]), &exact).is_ok());
    }

    #[test]
    fn calibrated_images_keep_packed_samples_and_lab_decode_ranges() {
        use crate::{
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::{options::ParseOptions, syntax::SyntaxParser},
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (space, bits, bytes, decode, expected_values) in [
            (
                "[/Separation /Spot /DeviceGray << /FunctionType 2 /Domain [0 1] /N 1 >>]",
                16,
                vec![0, 0, 255, 255],
                "/Decode [1 0]",
                vec![vec![1.0], vec![0.0]],
            ),
            (
                "[/Lab << /WhitePoint [0.95047 1 1.08883] /Range [0 0 0 0] >>]",
                8,
                vec![0, 0, 0, 255, 0, 0],
                "",
                vec![vec![0.0, 0.0, 0.0], vec![100.0, 0.0, 0.0]],
            ),
            (
                "[/CalGray << /WhitePoint [0.95047 1 1.08883] >>]",
                16,
                vec![0x80, 0, 255, 255],
                "/Decode [1 0]",
                vec![vec![1.0 - 32768.0 / 65535.0], vec![0.0]],
            ),
            (
                "[/CalRGB << /WhitePoint [0.95047 1 1.08883] >>]",
                2,
                vec![0x1c, 0xe0],
                "",
                vec![vec![0.0, 1.0 / 3.0, 1.0], vec![1.0, 2.0 / 3.0, 0.0]],
            ),
        ] {
            let height = if bits == 2 { 2 } else { 1 };
            let width = if bits == 2 { 1 } else { 2 };
            let source = format!(
                "<< /Width {width} /Height {height} /BitsPerComponent {bits} /ColorSpace {space} {decode} >>"
            );
            let dict =
                SyntaxParser::new(StreamReader::try_new(Cursor::new(source.into_bytes())).unwrap())
                    .get_object()
                    .unwrap()
                    .into_dict()
                    .unwrap();
            let object = PdfObject::PdfStream(PdfStream::new(dict, bytes.clone()));
            let image = PdfImage::try_new("calibrated".into(), &object, &ctx).unwrap();
            assert_eq!(image.data(), bytes);
            let expected = expected_values
                .into_iter()
                .flat_map(|values| {
                    let rgb = image
                        .colorspace
                        .rgb(&crate::color::value::ColorValue::new(values))
                        .unwrap();
                    [rgb.r(), rgb.g(), rgb.b(), 255]
                })
                .collect::<Vec<_>>();
            assert_eq!(image.rgba_buffer().unwrap(), expected, "{space}");
        }
    }

    #[test]
    fn indexed_images_preserve_indices_across_depths_rows_and_decode() {
        use crate::{
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::{options::ParseOptions, syntax::SyntaxParser},
        };
        use std::io::Cursor;
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let palette = [
            [255, 0, 0, 255],
            [0, 255, 0, 255],
            [0, 0, 255, 255],
            [255, 255, 255, 255],
        ];
        for (bits, width, height, bytes, decode, expected) in [
            (1, 3, 2, vec![0x40, 0xa0], "", vec![0, 1, 0, 1, 0, 1]),
            (2, 4, 1, vec![0x1b], "", vec![0, 1, 2, 3]),
            (4, 4, 1, vec![0x01, 0x2f], "", vec![0, 1, 2, 15]),
            (8, 4, 1, vec![0, 1, 2, 255], "", vec![0, 1, 2, 255]),
            // Repeated indices reuse the same converted palette entry across rows.
            (
                8,
                3,
                2,
                vec![2, 2, 255, 2, 255, 255],
                "",
                vec![2, 2, 255, 2, 255, 255],
            ),
            (
                16,
                4,
                1,
                vec![0, 0, 0, 1, 0, 2, 255, 255],
                "",
                vec![0, 1, 2, 255],
            ),
            (
                16,
                4,
                1,
                vec![0, 0, 0x55, 0x55, 0xaa, 0xaa, 255, 255],
                "/Decode [3 0]",
                vec![3, 2, 1, 0],
            ),
            (
                8,
                4,
                1,
                vec![0, 85, 170, 255],
                "/Decode [3 0]",
                vec![3, 2, 1, 0],
            ),
            (2, 4, 1, vec![0x1b], "/Decode [-2 4]", vec![0, 0, 2, 4]),
        ] {
            let dict = parse(&format!("<< /Width {width} /Height {height} /BitsPerComponent {bits} /ColorSpace [/Indexed /DeviceRGB 3 <FF000000FF000000FFFFFFFF>] {decode} >>")).into_dict().unwrap();
            let object = PdfObject::PdfStream(PdfStream::new(dict, bytes));
            let image = PdfImage::try_new("palette".into(), &object, &ctx).unwrap();
            assert_eq!(
                image.data.as_slice(),
                expected,
                "bits={bits}, decode={decode}"
            );
            let expected_rgba = expected
                .iter()
                .flat_map(|index| palette[(*index as usize).min(3)])
                .collect::<Vec<_>>();
            assert_eq!(image.rgba_buffer().unwrap(), expected_rgba);
        }
        for (bits, decode, bytes) in [(2, "/Decode [0]", vec![0]), (2, "", vec![])] {
            let dict = parse(&format!("<< /Width 1 /Height 1 /BitsPerComponent {bits} /ColorSpace [/Indexed /DeviceRGB 0 <FF0000>] {decode} >>")).into_dict().unwrap();
            let object = PdfObject::PdfStream(PdfStream::new(dict, bytes));
            assert!(matches!(
                PdfImage::try_new("palette".into(), &object, &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
    }

    #[test]
    fn stencil_samples_use_current_color_and_reverse_decode() {
        use crate::{
            color::value::ColorRgb,
            io::stream_reader::StreamReader,
            objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
            parser::options::ParseOptions,
            parser::syntax::SyntaxParser,
        };
        use std::io::Cursor;
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (decode, expected) in [
            ("", vec![255, 0, 255, 0, 255, 0]),
            ("/Decode [1 0]", vec![0, 255, 0, 255, 0, 255]),
        ] {
            let dict = parse(&format!(
                "<< /Subtype /Image /Width 3 /Height 2 /ImageMask true {decode} >>"
            ));
            let object = PdfObject::PdfStream(PdfStream::new(
                dict.as_dict().unwrap().clone(),
                vec![0x40, 0xa0],
            ));
            let image = PdfImage::try_new("mask".into(), &object, &ctx).unwrap();
            assert!(image.is_mask());
            assert!(matches!(image.rgba_buffer(), Err(PdfError::ColorError(_))));
            for color in [ColorRgb::new(1.0, 0.0, 0.0), ColorRgb::new(0.0, 1.0, 0.0)] {
                let rgba = image.rgba_buffer_with_stencil_color(&color).unwrap();
                assert_eq!(
                    rgba.as_chunks::<4>()
                        .0
                        .iter()
                        .map(|p| p[3])
                        .collect::<Vec<_>>(),
                    expected
                );
                assert!(
                    rgba.as_chunks::<4>()
                        .0
                        .iter()
                        .all(|p| p[..3] == [color.r(), color.g(), color.b()])
                );
            }
            assert!(
                image
                    .rgba_buffer_with_stencil_color(&ColorRgb::new(f32::NAN, 0.0, 0.0))
                    .is_err()
            );
        }
        for field in [
            "/BitsPerComponent 8",
            "/ColorSpace /DeviceGray",
            "/SMask << >>",
            "/Decode [0 0.5]",
            "/Decode [0]",
            "/ImageMask /true",
        ] {
            let dict = parse(&format!("<< /Width 3 /Height 2 /ImageMask true {field} >>"));
            let object = PdfObject::PdfStream(PdfStream::new(
                dict.as_dict().unwrap().clone(),
                vec![0x40, 0xa0],
            ));
            assert!(
                PdfImage::try_new("mask".into(), &object, &ctx).is_err(),
                "{field}"
            );
        }
        let dict = parse("<< /Width 3 /Height 2 /ImageMask true >>");
        for bytes in [vec![0x40], vec![0x40, 0xa0, 0]] {
            let object =
                PdfObject::PdfStream(PdfStream::new(dict.as_dict().unwrap().clone(), bytes));
            assert!(matches!(
                PdfImage::try_new("mask".into(), &object, &ctx),
                Err(PdfError::ObjectError(_))
            ));
        }
        let mut options = ParseOptions::default();
        options.limits.max_decoded_bytes = 23;
        let limited =
            PDFContext::<Cursor<Vec<u8>>>::new_without_source(PdfDict::default(), options);
        let object = PdfObject::PdfStream(PdfStream::new(
            dict.as_dict().unwrap().clone(),
            vec![0x40, 0xa0],
        ));
        assert!(matches!(
            PdfImage::try_new("mask".into(), &object, &limited),
            Err(PdfError::ResourceLimit(_))
        ));
    }

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
            data: ImageSamples::Owned(vec![10, 20, 30]),
            source_dimensions: (1, 1),
            is_mask: false,
            interpolate: false,
            is_inline: false,
            pixel_count: 1,
            max_rgba_bytes: 4,
            soft_mask: None,
            hard_mask_alpha: None,
            color_sample_decode: None,
            rendering_intent: None,
        };
        assert_eq!(image.rgba_buffer().unwrap(), vec![10, 20, 30, 255]);
        image.soft_mask = Some(ImageSoftMask {
            width: 1,
            height: 1,
            data: vec![128],
            bits: 8,
            decode: [0.0, 1.0],
            matte: None,
        });
        assert_eq!(image.rgba_buffer().unwrap(), vec![10, 20, 30, 128]);
        image.soft_mask.as_mut().unwrap().data.clear();
        assert!(image.rgba_buffer().is_err());
        image.soft_mask = None;
        image.max_rgba_bytes = 3;
        assert!(matches!(
            image.rgba_buffer(),
            Err(PdfError::ResourceLimit(_))
        ));
        image.max_rgba_bytes = 4;
        if let ImageSamples::Owned(data) = &mut image.data {
            data.pop();
        }
        assert!(image.rgba_buffer().is_err());
        if let ImageSamples::Owned(data) = &mut image.data {
            data.extend_from_slice(&[30, 40]);
        }
        assert!(image.rgba_buffer().is_err());
    }
}

#[cfg(test)]
mod image_tail_whitespace_tests {
    use super::*;
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        objects::pdf_stream::PdfStream,
        parser::{
            parser::PdfParser,
            syntax::{ParseMode, SyntaxParser},
        },
    };
    #[test]
    fn compatible_short_whitespace_surplus_is_diagnosed_without_hiding_other_size_errors() {
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let context = PDFContext::try_new(
                PdfParser::new(SyntaxParser::with_mode(
                    StreamReader::try_new(std::io::Cursor::new(Vec::<u8>::new())).unwrap(),
                    mode,
                )),
                CrossRefTable::default(),
            )
            .unwrap();
            let dict=SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(b"<< /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray >>".to_vec())).unwrap()).get_object().unwrap().into_dict().unwrap();
            for filtered in [false, true] {
                let mut dict = dict.clone();
                if filtered {
                    dict.insert(
                        "Filter".into(),
                        PdfObject::PdfName(crate::objects::pdf_name::PdfName::new(
                            "FlateDecode".into(),
                        )),
                    );
                }
                for (bytes, recoverable) in [
                    (vec![128, 13, 10], true),
                    (vec![128, 1, 2], false),
                    (vec![128, 32, 32, 32], false),
                    (vec![], false),
                ] {
                    let bytes = if filtered {
                        use std::io::Write;
                        let mut encoder = flate2::write::ZlibEncoder::new(
                            Vec::new(),
                            flate2::Compression::default(),
                        );
                        encoder.write_all(&bytes).unwrap();
                        encoder.finish().unwrap()
                    } else {
                        bytes
                    };
                    let result = PdfImage::try_new(
                        "test".into(),
                        &PdfObject::PdfStream(PdfStream::new(dict.clone(), bytes)),
                        &context,
                    )
                    .and_then(|image| image.rgba_buffer());
                    if mode == ParseMode::Compatible && recoverable {
                        assert_eq!(result.unwrap(), [128, 128, 128, 255]);
                    } else {
                        assert!(result.is_err());
                    }
                }
            }
            assert_eq!(
                context
                    .structure_diagnostics()
                    .unwrap()
                    .iter()
                    .any(|d| d.reason
                        == crate::parser::diagnostic::StructureReason::ImageTrailingBytes),
                mode == ParseMode::Compatible
            );
        }
    }
}

#[cfg(test)]
mod explicit_mask_tests {
    use super::*;
    use crate::{
        io::stream_reader::StreamReader,
        objects::{pdf_dict::PdfDict, pdf_stream::PdfStream},
        parser::{options::ParseOptions, syntax::SyntaxParser},
    };
    use std::io::Cursor;
    fn object(dict: &str, data: Vec<u8>) -> PdfObject {
        let dict = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(dict.as_bytes().to_vec())).unwrap(),
        )
        .get_object()
        .unwrap()
        .into_dict()
        .unwrap();
        PdfObject::PdfStream(PdfStream::new(dict, data))
    }
    #[test]
    fn batched_icc_image_matches_scalar_with_decode_range_and_tail_batch() {
        use crate::color::{
            conversion::{ColorConversionContext, IccFallback},
            icc_based::IccBased,
            icc_profile::ICCProfile,
        };
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for profile in [ICCProfile::srgb(), ICCProfile::gray_srgb().unwrap()] {
            let n = profile.number_of_components();
            let range = "0.1 0.9 ".repeat(n);
            let space = IccBased::try_new(
                &object(
                    &format!("<< /N {n} /Range [{range}] >>"),
                    profile.to_bytes().unwrap(),
                ),
                &ctx,
            )
            .unwrap();
            let data: Vec<_> = (0..259 * n).map(|i| (i % 256) as u8).collect();
            let mut image = PdfImage::try_new(
                "batch".into(),
                &object(
                    "<< /Width 259 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray >>",
                    vec![0; 259],
                ),
                &ctx,
            )
            .unwrap();
            image.colorspace = ColorSpace::IccBased(space);
            image.data = ImageSamples::Owned(data);
            image.color_sample_decode = Some((8, (0..n).flat_map(|_| [1., 0.]).collect()));
            let context = ColorConversionContext::default();
            let mut scalar = context.clone();
            scalar.icc_fallback = IccFallback::Alternate;
            let expected = image.rgba_buffer_with_context(&scalar).unwrap();
            assert_eq!(image.rgba_buffer_with_context(&context).unwrap(), expected);
            for short in [true, false] {
                let mut invalid = image.clone();
                if short {
                    if let ImageSamples::Owned(data) = &mut invalid.data {
                        data.pop();
                    }
                } else {
                    if let ImageSamples::Owned(data) = &mut invalid.data {
                        data.push(0);
                    }
                }
                assert!(matches!(
                    invalid.rgba_buffer_with_context(&context),
                    Err(PdfError::ObjectError(_))
                ));
            }
            image.max_rgba_bytes = expected.len() - 1;
            assert!(matches!(
                image.rgba_buffer_with_context(&context),
                Err(PdfError::ResourceLimit(_))
            ));
        }
    }
    #[test]
    fn group4_indexed_samples_keep_palette_and_decode_semantics() {
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        let mut encoder = fax::encoder::Encoder::new(fax::VecWriter::new());
        let pixels = [fax::Color::Black, fax::Color::White, fax::Color::Black];
        encoder.encode_line(pixels.into_iter(), 3).unwrap();
        let bytes = encoder.finish().unwrap().finish();
        for (decode, expected) in [
            ("", vec![255, 0, 0, 255, 0, 255, 0, 255, 255, 0, 0, 255]),
            (
                "/Decode [1 0]",
                vec![0, 255, 0, 255, 255, 0, 0, 255, 0, 255, 0, 255],
            ),
        ] {
            let image = object(
                &format!(
                    "<< /Width 3 /Height 1 /BitsPerComponent 1 /ColorSpace [/Indexed /DeviceRGB 1 <FF000000FF00>] /Filter /CCITTFaxDecode /DecodeParms << /K -1 /Columns 3 /Rows 1 >> {decode} >>"
                ),
                bytes.clone(),
            );
            assert_eq!(
                PdfImage::try_new("fax".into(), &image, &ctx)
                    .unwrap()
                    .rgba_buffer()
                    .unwrap(),
                expected
            );
        }
    }

    #[test]
    fn colour_keys_compare_original_samples_before_decode_and_palette() {
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        let rgb = object(
            "<< /Width 2 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB /Mask [10 10 20 20 30 30] /Decode [1 0 1 0 1 0] >>",
            vec![10, 20, 30, 10, 21, 30],
        );
        let rgba = PdfImage::try_new("rgb".into(), &rgb, &ctx)
            .unwrap()
            .rgba_buffer()
            .unwrap();
        assert_eq!(rgba, vec![245, 235, 225, 0, 245, 234, 225, 255]);
        let indexed = object(
            "<< /Width 3 /Height 2 /BitsPerComponent 1 /ColorSpace [/Indexed /DeviceRGB 1 <FF000000FF00>] /Mask [0 0] /Decode [1 0] >>",
            vec![0b01000000, 0b10100000],
        );
        let rgba = PdfImage::try_new("palette".into(), &indexed, &ctx)
            .unwrap()
            .rgba_buffer()
            .unwrap();
        assert_eq!(
            rgba.as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect::<Vec<_>>(),
            vec![0, 255, 0, 255, 0, 255]
        );
        let wide = object(
            "<< /Width 2 /Height 1 /BitsPerComponent 16 /ColorSpace /DeviceGray /Mask [256 256] >>",
            vec![1, 0, 1, 1],
        );
        let rgba = PdfImage::try_new("wide".into(), &wide, &ctx)
            .unwrap()
            .rgba_buffer()
            .unwrap();
        assert_eq!((rgba[3], rgba[7]), (0, 255));
    }
    #[test]
    fn soft_masks_keep_native_grid_packed_precision_and_matte() {
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        let make = |base: &str, colors: Vec<u8>, mask: &str, alpha: Vec<u8>| {
            let mut source = object(base, colors.clone())
                .as_stream()
                .unwrap()
                .dict()
                .clone();
            source
                .try_insert("SMask".into(), object(mask, alpha))
                .unwrap();
            PdfImage::try_new(
                "masked".into(),
                &PdfObject::PdfStream(PdfStream::new(source, colors)),
                &ctx,
            )
        };
        let mut image = make(
            "<< /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB >>", vec![255,0,0],
            "<< /Subtype /Image /Width 3 /Height 2 /BitsPerComponent 1 /ColorSpace /DeviceGray /Decode [1 0] >>", vec![0b01000000, 0b10100000]).unwrap();
        assert_eq!(image.rgba_buffer().unwrap().len(), 4);
        let (w, h, rgba) = image
            .rendering_rgba_with_context(&Default::default())
            .unwrap();
        assert_eq!((w, h), (3, 2));
        assert_eq!(
            rgba.as_chunks::<4>()
                .0
                .iter()
                .map(|p| p[3])
                .collect::<Vec<_>>(),
            [255, 0, 255, 0, 255, 0]
        );
        assert!(
            rgba.as_chunks::<4>()
                .0
                .iter()
                .all(|p| p[..3] == [255, 0, 0])
        );
        image.max_rgba_bytes = 23;
        assert!(matches!(
            image.rendering_rgba_with_context(&Default::default()),
            Err(PdfError::ResourceLimit(_))
        ));
        let image = make(
            "<< /Width 1 /Height 1 /BitsPerComponent 16 /ColorSpace /DeviceGray >>", vec![255,254],
            "<< /Subtype /Image /Width 3 /Height 1 /BitsPerComponent 16 /ColorSpace /DeviceGray /Matte [1] >>", vec![0,0,0,1,255,255]).unwrap();
        let (_, _, rgba) = image
            .rendering_rgba_with_context(&Default::default())
            .unwrap();
        // 1/65535 opacity unblends to black before alpha rounds to zero.
        assert_eq!(rgba, [0, 0, 0, 0, 0, 0, 0, 0, 255, 255, 255, 255]);
        let image = make(
            "<< /Width 3 /Height 2 /BitsPerComponent 1 /ColorSpace [/Indexed /DeviceRGB 1 <FF00000000FF>] >>", vec![0b01000000,0b10100000],
            "<< /Subtype /Image /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray >>", vec![128]).unwrap();
        let (_, _, rgba) = image
            .rendering_rgba_with_context(&Default::default())
            .unwrap();
        assert_eq!(
            rgba,
            [
                255, 0, 0, 128, 0, 0, 255, 128, 255, 0, 0, 128, 0, 0, 255, 128, 255, 0, 0, 128, 0,
                0, 255, 128
            ]
        );
        for dimension in ["0", "-1", "0.5", "1000000"] {
            assert!(make(
                "<< /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray >>", vec![128],
                &format!("<< /Subtype /Image /Width {dimension} /Height 1000000 /BitsPerComponent 8 /ColorSpace /DeviceGray >>"), vec![]).is_err());
        }
    }

    #[test]
    fn rendering_preserves_finer_stencil_and_enforces_combined_byte_budget() {
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (decode, expected) in [
            ("", vec![255, 0, 255, 0, 255, 0, 255, 0]),
            ("/Decode [1 0]", vec![0, 255, 0, 255, 0, 255, 0, 255]),
        ] {
            let mask = object(
                &format!("<< /Width 8 /Height 1 /ImageMask true {decode} >>"),
                vec![0b01010101],
            );
            let mut base = object(
                "<< /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceRGB >>",
                vec![20, 40, 60],
            );
            let mut dict = base.as_stream().unwrap().dict().clone();
            dict.try_insert("Mask".into(), mask).unwrap();
            base = PdfObject::PdfStream(PdfStream::new(dict, vec![20, 40, 60]));
            let mut image = PdfImage::try_new("fine-mask".into(), &base, &ctx).unwrap();
            assert_eq!(image.pixel_dimensions(), (1, 1));
            assert_eq!(image.rgba_buffer().unwrap().len(), 4);
            let (w, h, pixels) = image
                .rendering_rgba_with_context(&Default::default())
                .unwrap();
            assert_eq!((w, h), (8, 1));
            assert_eq!(
                pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| p[3])
                    .collect::<Vec<_>>(),
                expected
            );
            assert!(
                pixels
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .all(|p| p[..3] == [20, 40, 60])
            );
            image.max_rgba_bytes = 31;
            assert!(matches!(
                image.rendering_rgba_with_context(&Default::default()),
                Err(PdfError::ResourceLimit(_))
            ));
        }
    }
    #[test]
    fn explicit_stencil_resamples_and_honours_decode() {
        let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
            PdfDict::default(),
            ParseOptions::default(),
        );
        for (decode, expected) in [
            ("", vec![255, 255, 0, 0]),
            ("/Decode [1 0]", vec![0, 0, 255, 255]),
        ] {
            let mask = object(
                &format!("<< /Width 2 /Height 1 /ImageMask true {decode} >>"),
                vec![0b01000000],
            );
            let base = object(
                "<< /Width 4 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray >>",
                vec![128; 4],
            );
            let mut dict = base.as_stream().unwrap().dict().clone();
            dict.try_insert("Mask".into(), mask).unwrap();
            let base = PdfObject::PdfStream(PdfStream::new(dict, vec![128; 4]));
            let image = PdfImage::try_new("masked".into(), &base, &ctx).unwrap();
            assert!(image.has_explicit_mask());
            assert_eq!(
                image
                    .rgba_buffer()
                    .unwrap()
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| p[3])
                    .collect::<Vec<_>>(),
                expected
            );
        }
        for ranges in ["[1 0]", "[0 256]", "[0]", "[0.5 1]"] {
            let bad = object(
                &format!(
                    "<< /Width 1 /Height 1 /BitsPerComponent 8 /ColorSpace /DeviceGray /Mask {ranges} >>"
                ),
                vec![0],
            );
            assert!(PdfImage::try_new("bad".into(), &bad, &ctx).is_err());
        }
    }
}
