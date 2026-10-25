use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};

/// Native IDCT reduction for render-only, sequential JPEG images. Source and
/// output guards are independent; this never constructs full-size RGB pixels.
pub(crate) fn decode_target(
    input: &[u8],
    source: (u32, u32),
    target: (u32, u32),
    context: crate::filter::FilterContext<'_>,
) -> PdfResult<(u32, u32, Vec<u8>)> {
    let action = || -> PdfResult<_> {
        let mut decoder = mozjpeg::Decompress::new_mem(input)
            .map_err(|e| PdfError::FilterError(format!("DCT header: {e}")))?;
        if decoder.size() != (source.0 as usize, source.1 as usize) {
            return Err(PdfError::FilterError(
                "JPEG dimensions disagree with image dictionary".into(),
            ));
        }
        let source_pixels = u64::from(source.0) * u64::from(source.1);
        if source_pixels > 16_000_000 {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "image_source_pixels",
                stage: "image_decode",
                limit: 16_000_000,
                actual: source_pixels,
            });
        }
        let scale = [1u32, 2, 4, 8]
            .into_iter()
            .find(|n| {
                (u64::from(source.0) * u64::from(*n)).div_ceil(8)
                    >= u64::from(target.0.min(source.0))
                    && (u64::from(source.1) * u64::from(*n)).div_ceil(8)
                        >= u64::from(target.1.min(source.1))
            })
            .unwrap_or(8);
        let width = (u64::from(source.0) * u64::from(scale)).div_ceil(8) as u32;
        let height = (u64::from(source.1) * u64::from(scale)).div_ceil(8) as u32;
        let pixels = u64::from(width) * u64::from(height);
        let components = match decoder.color_space() {
            mozjpeg::ColorSpace::JCS_GRAYSCALE => 1,
            mozjpeg::ColorSpace::JCS_CMYK | mozjpeg::ColorSpace::JCS_YCCK => 4,
            _ => 3,
        };
        if pixels > context.max_image_pixels {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "image_pixels",
                stage: "image_decode",
                limit: context.max_image_pixels,
                actual: pixels,
            });
        }
        // RGBA conversion follows decoding; guard its size as well as samples.
        if pixels * 4 > context.max_decoded_bytes as u64 {
            return Err(PdfError::ResourceLimitExceeded {
                reason: "decoded_bytes",
                stage: "image_decode",
                limit: context.max_decoded_bytes as u64,
                actual: pixels * 4,
            });
        }
        decoder.scale(scale as u8);
        let size = pixels as usize * components;
        let mut output = Vec::new();
        output
            .try_reserve_exact(size)
            .map_err(|_| PdfError::ResourceLimit("JPEG reduced output allocation".into()))?;
        output.resize(size, 0);
        macro_rules! read {
            ($image:ident) => {{
                if ($image.width(), $image.height()) != (width as usize, height as usize) {
                    return Err(PdfError::FilterError(
                        "JPEG scaled dimensions disagree".into(),
                    ));
                }
                $image.read_scanlines_into::<u8>(&mut output)
            }};
        }
        let read = match decoder
            .image()
            .map_err(|e| PdfError::FilterError(format!("DCT start: {e}")))?
        {
            mozjpeg::Format::RGB(mut image) => read!(image),
            mozjpeg::Format::Gray(mut image) => read!(image),
            mozjpeg::Format::CMYK(mut image) => read!(image),
        }
        .map_err(|e| PdfError::FilterError(format!("DCT reduced scanlines: {e}")))?;
        if read.len() != size {
            return Err(PdfError::FilterError("DCT reduced output length".into()));
        }
        Ok((width, height, output))
    };
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(action)) {
        Ok(result) => result,
        Err(payload) => {
            if let Some(message) = payload.downcast_ref::<String>()
                && message.starts_with("libjpeg fatal error: ")
            {
                return Err(PdfError::FilterError(message.clone()));
            }
            std::panic::resume_unwind(payload)
        }
    }
}

/// Only sequential SOF0 uses the bounded-scanline reduced path. Progressive
/// coefficient storage can depend on the full source, even with reduced IDCT.
pub(crate) fn sequential(input: &[u8]) -> bool {
    if !input.starts_with(&[0xff, 0xd8]) {
        return false;
    }
    let mut p = 2;
    while input.get(p) == Some(&0xff) {
        while input.get(p) == Some(&0xff) {
            p += 1;
        }
        let Some(&marker) = input.get(p) else {
            return false;
        };
        p += 1;
        if marker == 0xc0 {
            return true;
        }
        if matches!(marker, 0xc1..=0xc3 | 0xc5..=0xc7 | 0xc9..=0xcb | 0xcd..=0xcf | 0xda | 0xd9) {
            return false;
        }
        let Some(bytes) = input.get(p..p + 2) else {
            return false;
        };
        let size = usize::from(u16::from_be_bytes([bytes[0], bytes[1]]));
        if size < 2 {
            return false;
        }
        let Some(next) = p.checked_add(size) else {
            return false;
        };
        p = next;
    }
    false
}

pub fn dct_decode(input: &[u8], _params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    dct_decode_limited(input, crate::filter::FilterContext::default())
}

pub fn dct_decode_limited(
    input: &[u8],
    context: crate::filter::FilterContext<'_>,
) -> PdfResult<Vec<u8>> {
    decode_inner(input, context, None, &mut None)
}

/// Charges the final pixel buffer; native JPEG decoder allocations are excluded.
pub fn dct_decode_with_budget(
    input: &[u8],
    context: crate::filter::FilterContext<'_>,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    let mut lease = Some(budget.reserve(0)?);
    let data = decode_inner(input, context, Some(budget), &mut lease)?;
    crate::budget::BudgetedBytes::from_reserved_vec(data, lease.unwrap())
}

fn decode_inner(
    input: &[u8],
    context: crate::filter::FilterContext<'_>,
    budget: Option<&crate::budget::OperationBudget>,
    lease: &mut Option<crate::budget::MemoryLease>,
) -> PdfResult<Vec<u8>> {
    // mozjpeg deliberately resumes unwinding with this String on libjpeg
    // fatal errors. Translate only that known payload; unrelated panics retain
    // their normal semantics. No global panic hook is installed or changed.
    match std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        decode_native(input, context, budget, lease)
    })) {
        Ok(result) => result,
        Err(payload) => {
            if let Some(message) = payload.downcast_ref::<String>()
                && message.starts_with("libjpeg fatal error: ")
            {
                return Err(PdfError::FilterError(format!(
                    "DCT decode failed: {message}"
                )));
            }
            std::panic::resume_unwind(payload)
        }
    }
}

fn decode_native(
    input: &[u8],
    context: crate::filter::FilterContext<'_>,
    budget: Option<&crate::budget::OperationBudget>,
    lease: &mut Option<crate::budget::MemoryLease>,
) -> PdfResult<Vec<u8>> {
    let d = mozjpeg::Decompress::new_mem(input)
        .map_err(|e| PdfError::FilterError(format!("DCT create decoder error:{:?}", e)))?;
    if let Some(dict) = context.stream_dict {
        for (key, actual) in [("Width", d.width()), ("Height", d.height())] {
            if let Some(value) = dict.get(key)
                && value.as_u32().map(|v| v as usize) != Some(actual)
            {
                return Err(PdfError::FilterError(
                    "JPEG dimensions disagree with image dictionary".into(),
                ));
            }
        }
    }
    let pixels = d
        .width()
        .checked_mul(d.height())
        .ok_or_else(|| PdfError::ResourceLimit("JPEG pixel count overflow".into()))?;
    let components = match d.color_space() {
        mozjpeg::ColorSpace::JCS_GRAYSCALE => 1,
        mozjpeg::ColorSpace::JCS_CMYK | mozjpeg::ColorSpace::JCS_YCCK => 4,
        _ => 3,
    };
    if pixels as u64 > context.max_image_pixels {
        return Err(PdfError::ResourceLimit("JPEG pixel count".into()));
    }
    let bytes = pixels
        .checked_mul(components)
        .ok_or_else(|| PdfError::ResourceLimit("JPEG output size overflow".into()))?;
    super::check_output_size(0, bytes, context.max_decoded_bytes, "DCTDecode")?;
    if let Some(budget) = budget {
        *lease = Some(budget.reserve(bytes)?);
    }
    let mut res = Vec::new();
    res.try_reserve_exact(bytes)
        .map_err(|_| PdfError::ResourceLimit("JPEG output allocation".into()))?;
    if let Some(lease) = lease.as_mut() {
        lease.resize(res.capacity())?;
    }
    res.resize(bytes, 0);
    let image = d
        .image()
        .map_err(|e| PdfError::FilterError(format!("DCT decoder read decoder error:{:?}", e)))?;

    let read = match image {
        mozjpeg::Format::RGB(mut rgb) => rgb.read_scanlines_into::<u8>(&mut res),
        mozjpeg::Format::Gray(mut g) => g.read_scanlines_into::<u8>(&mut res),
        mozjpeg::Format::CMYK(mut cmyk) => cmyk.read_scanlines_into::<u8>(&mut res),
    }
    .map_err(|e| PdfError::FilterError(format!("DCT scanline decode error:{e:?}")))?;
    if read.len() != bytes {
        return Err(PdfError::FilterError("DCT unexpected output length".into()));
    }

    Ok(res)
}

#[cfg(test)]
mod tests {
    #[test]
    fn inconsistent_jpeg_dimensions_reject_before_pixel_allocation() {
        use crate::{
            budget::OperationBudget,
            filter::FilterContext,
            objects::{PdfObject, pdf_dict::PdfDict, pdf_number::PdfNumber},
        };
        let mut encoder = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
        encoder.set_size(2, 2);
        let mut encoder = encoder.start_compress(Vec::new()).unwrap();
        encoder.write_scanlines(&[128; 12]).unwrap();
        let input = encoder.finish().unwrap();
        let mut dict = PdfDict::default();
        dict.insert("Width".into(), PdfObject::PdfNumber(PdfNumber::new(999.)));
        dict.insert("Height".into(), PdfObject::PdfNumber(PdfNumber::new(999.)));
        let pool = OperationBudget::new(1);
        assert!(matches!(
            super::dct_decode_with_budget(
                &input,
                FilterContext {
                    stream_dict: Some(&dict),
                    ..Default::default()
                },
                &pool
            ),
            Err(crate::error::PdfError::FilterError(_))
        ));
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn malformed_jpeg_returns_filter_error_without_leaking_neighbor_budget() {
        use crate::{budget::OperationBudget, error::PdfError, filter::FilterContext};
        let budget = OperationBudget::new(16);
        let neighbor = budget.reserve(3).unwrap();
        for input in [
            b"invalid".as_slice(),
            b"".as_slice(),
            &[0xff, 0xd8, 0xff, 0x00][..],
        ] {
            assert!(matches!(
                super::dct_decode_limited(input, FilterContext::default()),
                Err(PdfError::FilterError(_))
            ));
            assert!(matches!(
                super::dct_decode_with_budget(input, FilterContext::default(), &budget),
                Err(PdfError::FilterError(_))
            ));
            assert_eq!(budget.live_bytes(), 3);
        }
        drop(neighbor);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn budgeted_jpeg_preserves_components_and_releases_capacity() {
        use crate::{
            budget::OperationBudget,
            error::PdfError,
            filter::{FilterContext, apply_filter_with_budget},
        };
        for (space, components) in [
            (mozjpeg::ColorSpace::JCS_RGB, 3),
            (mozjpeg::ColorSpace::JCS_GRAYSCALE, 1),
            (mozjpeg::ColorSpace::JCS_CMYK, 4),
        ] {
            let source = vec![100u8; 4 * components];
            let mut encoder = mozjpeg::Compress::new(space);
            encoder.set_size(2, 2);
            encoder.set_quality(100.0);
            let mut encoder = encoder.start_compress(Vec::new()).unwrap();
            encoder.write_scanlines(&source).unwrap();
            let encoded = encoder.finish().unwrap();
            let budget = OperationBudget::new(source.len());
            let neighbor = budget.reserve(1).unwrap();
            assert!(matches!(
                apply_filter_with_budget("DCT", &encoded, None, FilterContext::default(), &budget),
                Err(PdfError::ResourceLimit(_))
            ));
            assert_eq!(budget.live_bytes(), 1);
            drop(neighbor);
            let result = apply_filter_with_budget(
                "DCTDecode",
                &encoded,
                None,
                FilterContext::default(),
                &budget,
            )
            .unwrap();
            assert_eq!(result.as_slice().len(), source.len());
            assert!(result.as_slice().iter().all(|byte| byte.abs_diff(100) <= 2));
            assert_eq!(budget.live_bytes(), result.capacity());
            drop(result);
            assert_eq!(budget.live_bytes(), 0);
            assert!(
                super::dct_decode_with_budget(
                    &encoded,
                    FilterContext {
                        max_decoded_bytes: source.len() - 1,
                        ..Default::default()
                    },
                    &budget
                )
                .is_err()
            );
            assert_eq!(budget.live_bytes(), 0);
        }
    }
}
