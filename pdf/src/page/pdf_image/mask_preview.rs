//! Dictionary-only diagnostics and eligibility for joint masked previews.
use super::*;
use crate::{
    error::ImageDecodeDetails,
    filter::flate_image::{FlateImageRequest, Predictor8},
    objects::pdf_stream::PdfStream,
};

fn encoding<R: Read + Seek>(stream: &PdfStream, ctx: &PDFContext<R>) -> PdfResult<&'static str> {
    let filter = stream
        .dict()
        .get("Filter")
        .map(|v| crate::objects::pdf_stream::resolve_filter_setting(v, ctx, false))
        .transpose()?;
    let name = match &filter {
        Some(PdfObject::PdfName(v)) => Some(v.name()),
        Some(PdfObject::PdfArray(v)) if v.len() == 1 => {
            v.get(0).and_then(PdfObject::as_name).map(|v| v.name())
        }
        _ => None,
    };
    Ok(match name {
        Some("Fl" | "FlateDecode") => "flate",
        Some("DCT" | "DCTDecode") => "jpeg",
        Some("JPXDecode") => "jpx",
        Some("JBIG2Decode") => "jbig2",
        Some("CCF" | "CCITTFaxDecode") => "ccitt",
        Some("LZW" | "LZWDecode") => "lzw",
        Some("RL" | "RunLengthDecode") => "run_length",
        None if filter.is_none() || matches!(filter, Some(PdfObject::PdfNull)) => "raw",
        None => "filter_chain",
        _ => "unknown",
    })
}
fn attr<R: Read + Seek>(
    stream: &PdfStream,
    key: &str,
    ctx: &PDFContext<R>,
) -> PdfResult<Option<PdfObject>> {
    stream.dict().get(key).map(|v| ctx.resolve(v)).transpose()
}
fn dimension<R: Read + Seek>(stream: &PdfStream, key: &str, ctx: &PDFContext<R>) -> PdfResult<u32> {
    attr(stream, key, ctx)?
        .as_ref()
        .and_then(PdfObject::as_u32)
        .filter(|v| *v > 0)
        .ok_or_else(|| {
            PdfError::ObjectError(format!("mask image {key} must be a positive integer"))
        })
}

/// Never decode payloads to report why a preview is unavailable. SMask takes
/// precedence over Mask, matching the full-fidelity constructor.
pub(super) fn describe<R: Read + Seek>(
    stream: &PdfStream,
    ctx: &PDFContext<R>,
    details: &mut ImageDecodeDetails,
) -> PdfResult<Option<PdfObject>> {
    if attr(stream, "ImageMask", ctx)?.is_some_and(|v| v.as_bool().is_none_or(|v| v.value())) {
        details.mask_kind = Some("stencil");
        return Ok(None);
    }
    if attr(stream, "SMaskInData", ctx)?.is_some_and(|v| v.as_u32() != Some(0)) {
        details.mask_kind = Some("embedded-alpha");
        return Ok(None);
    }
    let mut selected = None;
    for (key, kind) in [("SMask", "soft"), ("Mask", "explicit")] {
        if let Some(value) = attr(stream, key, ctx)?
            && !matches!(value, PdfObject::PdfNull)
        {
            details.mask_kind = Some(if key == "Mask" && value.as_array().is_some() {
                "color-key"
            } else {
                kind
            });
            selected = Some(value);
            break;
        }
    }
    if let Some(mask) = selected.as_ref().and_then(PdfObject::as_stream) {
        details.mask_width = attr(mask, "Width", ctx)?
            .as_ref()
            .and_then(PdfObject::as_u32);
        details.mask_height = attr(mask, "Height", ctx)?
            .as_ref()
            .and_then(PdfObject::as_u32);
        details.mask_encoding = Some(encoding(mask, ctx)?);
        details.mask_bits_per_component = attr(mask, "BitsPerComponent", ctx)?
            .as_ref()
            .and_then(PdfObject::as_u32);
        details.matte_present =
            attr(mask, "Matte", ctx)?.is_some_and(|v| !matches!(v, PdfObject::PdfNull));
    }
    Ok(selected)
}

fn request<R: Read + Seek>(
    stream: &PdfStream,
    target: (u32, u32),
    is_mask: bool,
    ctx: &PDFContext<R>,
    details: &mut ImageDecodeDetails,
) -> PdfResult<Option<FlateImageRequest>> {
    let mut unsupported = None;
    if encoding(stream, ctx)? != "flate" {
        unsupported = Some("mask_encoding");
    }
    if ["F", "FFilter", "FDecodeParms"]
        .iter()
        .any(|k| stream.dict().contains_key(k))
    {
        unsupported = Some("mask_external_stream");
    }
    if attr(stream, "BitsPerComponent", ctx)?
        .as_ref()
        .and_then(PdfObject::as_u32)
        != Some(8)
    {
        unsupported = Some("mask_component_depth");
    }
    if is_mask {
        for key in ["Mask", "SMask", "SMaskInData"] {
            if attr(stream, key, ctx)?.is_some_and(|v| {
                !matches!(v, PdfObject::PdfNull) && (key != "SMaskInData" || v.as_u32() != Some(0))
            }) {
                unsupported = Some("mask_nested");
            }
        }
        if attr(stream, "ImageMask", ctx)?.is_some_and(|v| v.as_bool().is_none_or(|v| v.value())) {
            unsupported = Some("mask_stencil");
        }
        // The existing full-fidelity image-mask mapping is nearest-neighbor.
        // Do not claim a bilinear source-mask reconstruction path here.
        if attr(stream, "Interpolate", ctx)?.is_some_and(|v| v.as_bool().is_none_or(|v| v.value()))
        {
            unsupported = Some("mask_interpolate");
        }
        if attr(stream, "Subtype", ctx)?
            .as_ref()
            .and_then(PdfObject::as_name)
            .map(|v| v.name())
            != Some("Image")
        {
            return Err(PdfError::ObjectError("SMask Subtype must be Image".into()));
        }
    }
    let space = attr(stream, "ColorSpace", ctx)?;
    let components = match space
        .as_ref()
        .and_then(PdfObject::as_name)
        .map(|v| v.name())
    {
        Some("DeviceGray") => 1,
        Some("G") if !is_mask => 1,
        Some("DeviceRGB" | "RGB") if !is_mask => 3,
        _ => {
            unsupported.get_or_insert("mask_color_space");
            1
        }
    };
    if let Some(reason) = unsupported {
        details.unsupported_reason = Some(reason);
        return Ok(None);
    }
    let source = (
        dimension(stream, "Width", ctx)?,
        dimension(stream, "Height", ctx)?,
    );
    let params = stream
        .dict()
        .get("DecodeParms")
        .map(|v| crate::objects::pdf_stream::resolve_filter_setting(v, ctx, true))
        .transpose()?;
    let dict = match &params {
        None | Some(PdfObject::PdfNull) => None,
        Some(PdfObject::PdfDict(v)) => Some(v),
        Some(PdfObject::PdfArray(v)) if v.len() == 1 => match v.get(0) {
            Some(PdfObject::PdfDict(v)) => Some(v),
            Some(PdfObject::PdfNull) => None,
            _ => {
                return Err(PdfError::FilterError(
                    "invalid mask DecodeParms entry".into(),
                ));
            }
        },
        _ => return Err(PdfError::FilterError("invalid mask DecodeParms".into())),
    };
    let Some(predictor) = Predictor8::from_parameters(dict, source.0, components)? else {
        details.unsupported_reason = Some("mask_predictor_layout");
        return Ok(None);
    };
    Ok(Some(FlateImageRequest {
        source,
        target,
        components,
        predictor,
        decode: image_decode_endpoints(Some(components), stream.dict().get("Decode"), ctx)?,
        compatible: ctx.options().mode == crate::parser::syntax::ParseMode::Compatible,
        limits: ctx.limits(),
    }))
}

impl PdfImage {
    pub(super) fn try_new_soft_preview<R: Read + Seek>(
        name: String,
        object: &PdfObject,
        mask: &PdfObject,
        ctx: &PDFContext<R>,
        plan: super::super::image_decode::ImageDecodePlan,
        details: &mut ImageDecodeDetails,
    ) -> PdfResult<Self> {
        let image = object
            .as_stream()
            .ok_or_else(|| PdfError::ObjectError("image stream required".into()))?;
        let Some(mask_stream) = mask.as_stream() else {
            details.unsupported_reason = Some("mask_invalid");
            return Self::try_new(name, object, ctx);
        };
        let Some(color) = request(image, plan.target, false, ctx, details)? else {
            return Self::try_new(name, object, ctx);
        };
        let Some(alpha) = request(mask_stream, plan.target, true, ctx, details)? else {
            return Self::try_new(name, object, ctx);
        };
        if ctx.render_image_policy()
            != Some(crate::document::prepared_page::ImageDecodePolicy::TargetResolution)
            && super::super::image_decode::ImageDecodePlan::full_fits(color.source, ctx.limits())
            && super::super::image_decode::ImageDecodePlan::full_fits(alpha.source, ctx.limits())
        {
            return Self::try_new(name, object, ctx);
        }
        let space = ColorSpace::try_new(
            image
                .dict()
                .get("ColorSpace")
                .ok_or_else(|| PdfError::ColorError("image ColorSpace is missing".into()))?,
            ctx,
        )?;
        let matte = image_matte(mask, &space, ctx)?;
        details.target_decode_supported = true;
        details.unsupported_reason = None;
        plan.validate(ctx.limits())?;
        let pool = ctx.image_operation_budget();
        // The merged mask grid may cost much more than reading either source
        // (opposing aspect ratios). Charge its conservative cell/sample bound
        // to the same preparation meter, not only to a per-image guard.
        let grid =
            (u64::from(color.source.0) + u64::from(alpha.source.0) + u64::from(plan.target.0))
                .saturating_mul(u64::from(color.source.1) + u64::from(alpha.source.1));
        let units = color
            .source_bytes()?
            .saturating_add(alpha.source_bytes()?)
            .saturating_add(grid.saturating_mul(color.components as u64 + 1));
        ctx.charge_image_decode_work(units)?;
        let raw = image.read_raw_data(ctx, &pool)?;
        let mask_raw = mask_stream.read_raw_data(ctx, &pool)?;
        let decoded = crate::filter::masked_image::decode_soft_mask(
            &raw,
            color,
            &mask_raw,
            alpha,
            matte.as_deref(),
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
                source: color.source,
                dimensions: plan.target,
                samples: decoded.colors,
                decode_applied: true,
                alpha: Some(decoded.alpha),
            }),
        )
    }
}
