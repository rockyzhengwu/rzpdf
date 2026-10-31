use super::*;

impl PdfImage {
    pub(super) fn try_new_bilevel_preview<R: Read + Seek>(
        name: String,
        object: &PdfObject,
        ctx: &PDFContext<R>,
        plan: super::super::image_decode::ImageDecodePlan,
        details: &mut crate::error::ImageDecodeDetails,
    ) -> PdfResult<Self> {
        let image = object
            .as_stream()
            .ok_or_else(|| PdfError::ObjectError("bilevel image stream missing".into()))?;
        if ["F", "FFilter", "FDecodeParms"]
            .iter()
            .any(|key| image.dict().contains_key(key))
        {
            return Self::try_new(name, object, ctx);
        }
        let stencil = details.mask_kind == Some("stencil");
        if !stencil {
            let color = image
                .dict()
                .get("ColorSpace")
                .map(|value| ctx.resolve(value))
                .transpose()?;
            if !color
                .as_ref()
                .and_then(PdfObject::as_name)
                .is_some_and(|name| matches!(name.name(), "DeviceGray" | "G"))
            {
                details.unsupported_reason = Some("color_space");
                return Self::try_new(name, object, ctx);
            }
        }
        if !image
            .dict()
            .get("BitsPerComponent")
            .map(|value| ctx.resolve(value))
            .transpose()?
            .as_ref()
            .map_or(stencil, |value| value.as_u32() == Some(1))
        {
            details.unsupported_reason = Some("component_depth");
            return Self::try_new(name, object, ctx);
        }
        let endpoints = image_decode_endpoints(Some(1), image.dict().get("Decode"), ctx)?;
        let decode = endpoints.map_or([0., 1.], |e| [e[0], e[1]]);
        if stencil && !matches!(decode, [0., 1.] | [1., 0.]) {
            return Err(PdfError::ObjectError(
                "stencil image Decode must be [0 1] or [1 0]".into(),
            ));
        }
        let pool = ctx.image_operation_budget();
        let (params_object, _lease) = image
            .dict()
            .get("DecodeParms")
            .map(|value| {
                crate::objects::pdf_stream::resolve_filter_setting_with_budget(
                    value, ctx, true, &pool,
                )
            })
            .transpose()?
            .unwrap_or((PdfObject::PdfNull, pool.reserve(0)?));
        let params = match &params_object {
            PdfObject::PdfNull => None,
            PdfObject::PdfDict(dict) => Some(dict),
            PdfObject::PdfArray(array) if array.len() == 1 => match array.get(0) {
                Some(PdfObject::PdfNull) => None,
                Some(PdfObject::PdfDict(dict)) => Some(dict),
                _ => return Err(PdfError::FilterError("invalid bilevel DecodeParms".into())),
            },
            _ => return Err(PdfError::FilterError("invalid bilevel DecodeParms".into())),
        };
        plan.validate(ctx.limits())?;
        details.target_decode_supported = true;
        details.unsupported_reason = None;

        let raw = image.read_raw_data(ctx, &pool)?;
        let request = crate::filter::bilevel_preview::Request {
            source: plan.source,
            target: plan.target,
            decode,
            limits: ctx.limits(),
        };
        let samples = if details.encoding == "jbig2" {
            let globals = crate::objects::pdf_stream::resolve_jbig2_globals_with_budget(
                Some(&params_object),
                ctx,
                &pool,
            )?;
            let mut sampler = crate::filter::bilevel_preview::Sampler::new(request, &pool)?;
            let packed = crate::filter::jbig2::decode_preview(
                &raw,
                crate::filter::FilterContext {
                    jbig2_globals: globals.as_ref().map(|v| v.as_slice()),
                    max_image_pixels: ctx.limits().max_image_source_bytes as u64,
                    max_decoded_bytes: ctx.limits().max_decoded_bytes,
                    ..Default::default()
                },
                &pool,
                plan.source,
                |work| ctx.charge_image_decode_work(work),
            )?;
            sampler.push_packed(packed.as_slice())?;
            sampler.finish()?
        } else {
            ctx.charge_image_decode_work(u64::from(plan.source.0) * u64::from(plan.source.1))?;
            crate::filter::ccittfax::preview::decode_target(
                &raw,
                params,
                crate::filter::ccittfax::preview::Request {
                    source: plan.source,
                    target: plan.target,
                    decode,
                    limits: ctx.limits(),
                },
                &pool,
            )?
        };
        Self::try_new_with_samples(
            name,
            object,
            ctx,
            Some(DecodedImageSamples {
                source: plan.source,
                dimensions: plan.target,
                samples,
                decode_applied: true,
                alpha: None,
            }),
        )
    }
}
