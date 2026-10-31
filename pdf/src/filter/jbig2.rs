//! Embedded JBIG2 with bounded headers and shared Globals. Symbol/text streams
//! use bundled jbig2dec with live allocation callbacks; region-only streams
//! retain the Rust decoder and its preflight scratch envelope.
use crate::{
    budget::{BudgetedBytes, OperationBudget},
    error::{PdfError, PdfResult},
    filter::FilterContext,
    objects::pdf_dict::PdfDict,
};
pub(super) fn error(s: &str) -> PdfError {
    PdfError::FilterError(format!("JBIG2Decode: {s}"))
}
pub(super) fn limit(s: &str) -> PdfError {
    PdfError::ResourceLimit(format!("JBIG2Decode {s}"))
}
fn unsupported(s: &str) -> PdfError {
    PdfError::UnsupportedFeature(format!("JBIG2Decode {s}"))
}
fn u32be(b: &[u8], p: usize) -> PdfResult<u32> {
    Ok(u32::from_be_bytes(
        b.get(p..p + 4)
            .ok_or_else(|| error("truncated segment"))?
            .try_into()
            .unwrap(),
    ))
}
fn area(width: u32, height: u32, context: FilterContext<'_>) -> PdfResult<usize> {
    if width == 0 || height == 0 {
        return Err(error("zero bitmap dimensions"));
    }
    if width > 65535 || height > 65535 {
        return Err(limit("bitmap dimensions"));
    }
    let pixels = u64::from(width) * u64::from(height);
    if pixels > context.max_image_pixels {
        return Err(limit("bitmap pixel count"));
    }
    usize::try_from(pixels).map_err(|_| limit("bitmap size overflow"))
}

struct Envelope {
    page: Option<(u32, u32)>,
    segments: Vec<(u32, u8)>,
    scratch: usize,
    work: usize,
    symbols: bool,
    native_preview: bool,
}
fn preflight(
    data: &[u8],
    globals: bool,
    context: FilterContext<'_>,
    state: &mut Envelope,
) -> PdfResult<()> {
    if data.len() > context.max_decoded_bytes {
        return Err(limit("encoded input bytes"));
    }
    let mut p = 0;
    while p < data.len() {
        if state.segments.len() >= 1024 {
            return Err(limit("segment count"));
        }
        let id = u32be(data, p)?;
        p += 4;
        let flags = *data.get(p).ok_or_else(|| error("truncated flags"))?;
        p += 1;
        let kind = flags & 63;
        let count = *data
            .get(p)
            .ok_or_else(|| error("truncated reference count"))?;
        p += 1;
        let refs = (count >> 5) as usize;
        if refs > 4 {
            return Err(unsupported("extended reference tables"));
        }
        let reference_bytes = if id <= 256 {
            1
        } else if id <= 65536 {
            2
        } else {
            4
        };
        for _ in 0..refs {
            let bytes = data
                .get(p..p + reference_bytes)
                .ok_or_else(|| error("truncated reference"))?;
            let reference = bytes.iter().fold(0u32, |v, b| (v << 8) | u32::from(*b));
            p += reference_bytes;
            if reference >= id || !state.segments.iter().any(|(n, _)| *n == reference) {
                return Err(error("missing or forward segment reference"));
            }
        }
        if state.segments.last().is_some_and(|(n, _)| *n >= id) {
            return Err(unsupported("duplicate or non-sequential segment numbers"));
        }
        let association = if flags & 64 != 0 {
            let n = u32be(data, p)?;
            p += 4;
            n
        } else {
            let n = *data
                .get(p)
                .ok_or_else(|| error("truncated page association"))?;
            p += 1;
            u32::from(n)
        };
        if (globals && association != 0) || (!globals && association != 1 && association != 0) {
            return Err(unsupported("multiple pages or invalid Globals association"));
        }
        let length = u32be(data, p)?;
        p += 4;
        if length == u32::MAX {
            return Err(unsupported("unknown-length segments"));
        }
        let end = p
            .checked_add(length as usize)
            .filter(|n| *n <= data.len())
            .ok_or_else(|| error("truncated segment data"))?;
        let body = &data[p..end];
        p = end;
        let mut bitmap_pixels = 0;
        match kind {
            48 => {
                if globals || association != 1 || state.page.is_some() || body.len() != 19 {
                    return Err(error("invalid page information"));
                }
                let width = u32be(body, 0)?;
                let height = u32be(body, 4)?;
                bitmap_pixels = area(width, height, context)?;
                if body[17] & 128 != 0 {
                    return Err(unsupported("striped pages"));
                }
                state.page = Some((width, height));
            }
            36 | 38 | 39 | 40 | 42 | 43 | 20 | 22 | 23 => {
                let width = u32be(body, 0)?;
                let height = u32be(body, 4)?;
                bitmap_pixels = area(width, height, context)?;
                if body.len() < 18 {
                    return Err(error("truncated region header"));
                }
                if matches!(kind, 20 | 22 | 23) {
                    let grid = area(u32be(body, 18)?, u32be(body, 22)?, context)?;
                    if body.len() < 38 {
                        return Err(error("truncated halftone grid"));
                    }
                    // At most 256 patterns; include grayscale planes, grid index
                    // array and worst 255x255 pattern composition work.
                    bitmap_pixels = bitmap_pixels
                        .checked_add(grid.checked_mul(16).ok_or_else(|| limit("grid scratch"))?)
                        .ok_or_else(|| limit("region scratch"))?;
                    let work = grid
                        .checked_mul(255 * 255)
                        .ok_or_else(|| limit("halftone work"))?;
                    state.work = state
                        .work
                        .checked_add(work)
                        .ok_or_else(|| limit("decode work"))?;
                }
                if globals && matches!(kind, 38 | 39 | 42 | 43 | 22 | 23) {
                    return Err(error("immediate region in Globals"));
                }
            }
            16 => {
                if body.len() < 7 {
                    return Err(error("truncated pattern dictionary"));
                }
                let count = u32be(body, 3)?
                    .checked_add(1)
                    .filter(|n| *n <= 256)
                    .ok_or_else(|| limit("pattern count"))?;
                let width = u32::from(body[1]);
                let height = u32::from(body[2]);
                bitmap_pixels = area(
                    width
                        .checked_mul(count)
                        .ok_or_else(|| limit("collective pattern width"))?,
                    height,
                    context,
                )?;
                bitmap_pixels = bitmap_pixels
                    .checked_add((count as usize) * (height as usize) * 256)
                    .ok_or_else(|| limit("shifted patterns"))?;
            }
            49 | 51 => {
                if globals || !body.is_empty() {
                    return Err(error("invalid end segment"));
                }
            }
            0 => {
                let flags = u16::from_be_bytes(
                    body.get(..2)
                        .ok_or_else(|| error("truncated symbol header"))?
                        .try_into()
                        .unwrap(),
                );
                let mut offset = 2;
                if flags & 1 == 0 {
                    offset += if (flags >> 10) & 3 == 0 { 8 } else { 2 };
                }
                if flags & 2 != 0 && flags & 0x1000 == 0 {
                    offset += 4;
                }
                let exported = u32be(body, offset)?;
                let created = u32be(body, offset + 4)?;
                if u64::from(exported) + u64::from(created)
                    > context.max_image_pixels.min(1_000_000)
                {
                    return Err(limit("symbol count"));
                }
                state.symbols = true;
            }
            4 | 6 | 7 => {
                if globals && kind != 4 {
                    return Err(error("immediate text in Globals"));
                }
                bitmap_pixels = area(u32be(body, 0)?, u32be(body, 4)?, context)?;
                let flags = u16::from_be_bytes(
                    body.get(17..19)
                        .ok_or_else(|| error("truncated text header"))?
                        .try_into()
                        .unwrap(),
                );
                let mut offset = 19;
                if flags & 1 != 0 {
                    offset += 2;
                } else if flags & 2 != 0 && flags & 0x8000 == 0 {
                    offset += 4;
                }
                if u64::from(u32be(body, offset)?) > context.max_image_pixels.min(1_000_000) {
                    return Err(limit("text instance count"));
                }
                state.symbols = true;
            }
            53 => {
                state.symbols = true;
            }
            _ => return Err(unsupported("segment type outside region baseline")),
        }
        if !state.native_preview {
            state.scratch = state
                .scratch
                .checked_add(
                    bitmap_pixels
                        .checked_mul(16)
                        .ok_or_else(|| limit("scratch multiplication"))?,
                )
                .ok_or_else(|| limit("scratch envelope"))?;
        }
        state.work = state
            .work
            .checked_add(bitmap_pixels)
            .ok_or_else(|| limit("work envelope"))?;
        if state.work as u64 > context.max_image_pixels.saturating_mul(16) {
            return Err(limit("cumulative region work"));
        }
        state.segments.push((id, kind));
    }
    Ok(())
}

struct PackedOutput {
    data: Vec<u8>,
    width: usize,
    height: usize,
    x: usize,
    y: usize,
    invalid: bool,
}
impl hayro_jbig2::Decoder for PackedOutput {
    fn push_pixel(&mut self, black: bool) {
        if self.y >= self.height || self.x >= self.width {
            self.invalid = true;
            return;
        }
        if !black {
            self.data[self.y * self.width.div_ceil(8) + self.x / 8] |= 128 >> (self.x % 8);
        }
        self.x += 1;
    }
    fn push_pixel_chunk(&mut self, black: bool, count: u32) {
        let count = (count as usize).saturating_mul(8);
        if count > self.width.saturating_sub(self.x) {
            self.invalid = true;
            return;
        }
        for _ in 0..count {
            self.push_pixel(black);
        }
    }
    fn next_line(&mut self) {
        if self.x != self.width {
            self.invalid = true;
        }
        self.y += 1;
        self.x = 0;
    }
}

pub fn decode_with_budget(
    input: &[u8],
    context: FilterContext<'_>,
    budget: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    decode_inner(input, context, budget, None, |_| Ok(()))
}

/// Decode only a packed source bitmap; native scratch uses live allocation leases.
pub(crate) fn decode_preview(
    input: &[u8],
    context: FilterContext<'_>,
    budget: &OperationBudget,
    source: (u32, u32),
    charge: impl FnOnce(u64) -> PdfResult<()>,
) -> PdfResult<BudgetedBytes> {
    decode_inner(input, context, budget, Some(source), charge)
}
fn decode_inner(
    input: &[u8],
    context: FilterContext<'_>,
    budget: &OperationBudget,
    source: Option<(u32, u32)>,
    charge: impl FnOnce(u64) -> PdfResult<()>,
) -> PdfResult<BudgetedBytes> {
    let mut envelope = Envelope {
        page: None,
        segments: Vec::new(),
        scratch: 1024 * 1024,
        work: 0,
        symbols: false,
        native_preview: source.is_some(),
    };
    let _headers = budget.reserve(1024 * std::mem::size_of::<(u32, u8)>())?;
    envelope
        .segments
        .try_reserve_exact(1024)
        .map_err(|_| limit("segment table allocation"))?;
    if let Some(globals) = context.jbig2_globals {
        preflight(globals, true, context, &mut envelope)?;
    }
    preflight(input, false, context, &mut envelope)?;
    let (width, height) = envelope
        .page
        .ok_or_else(|| error("missing page information"))?;
    if source.is_some_and(|size| size != (width, height)) {
        return Err(error("image dictionary/header mismatch"));
    }
    charge((envelope.work as u64).saturating_add(u64::from(width) * u64::from(height)))?;
    if let Some(dict) = context.stream_dict {
        for (key, expected) in [
            ("Width", width),
            ("Height", height),
            ("BitsPerComponent", 1),
        ] {
            if let Some(value) = dict.get(key)
                && value.as_u32() != Some(expected)
            {
                return Err(error("image dictionary/header mismatch"));
            }
        }
    }
    let length = (width as usize)
        .div_ceil(8)
        .checked_mul(height as usize)
        .filter(|n| *n <= context.max_decoded_bytes)
        .ok_or_else(|| limit("packed output bytes"))?;
    if source.is_some() {
        return super::jbig2_native::decode_preview(
            input,
            context.jbig2_globals,
            width,
            height,
            budget,
        );
    }
    if envelope.symbols {
        return super::jbig2_native::decode(input, context.jbig2_globals, width, height, budget);
    }
    let _scratch = budget.reserve(envelope.scratch)?;
    let mut lease = budget.reserve(length)?;
    let mut data = Vec::new();
    data.try_reserve_exact(length)
        .map_err(|_| limit("output allocation"))?;
    lease.resize(data.capacity())?;
    data.resize(length, 0);
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> PdfResult<BudgetedBytes> {
            let image = hayro_jbig2::Image::new_embedded(input, context.jbig2_globals)
                .map_err(|_| error("invalid embedded headers"))?;
            if image.width() != width || image.height() != height {
                return Err(error("backend dimension mismatch"));
            }
            let mut output = PackedOutput {
                data,
                width: width as usize,
                height: height as usize,
                x: 0,
                y: 0,
                invalid: false,
            };
            image
                .decode(&mut output)
                .map_err(|_| error("invalid compressed region data"))?;
            if output.invalid || output.y != height as usize || output.x != 0 {
                return Err(error("decoded output shape mismatch"));
            }
            BudgetedBytes::from_reserved_vec(output.data, lease)
        },
    ));
    result.map_err(|_| error("backend decoder panic"))?
}

pub fn jbig2_decode(
    input: &[u8],
    _params: Option<&PdfDict>,
    context: FilterContext<'_>,
) -> PdfResult<Vec<u8>> {
    let pool = crate::parser::options::Limits::default().operation_budget();
    Ok(decode_with_budget(input, context, &pool)?
        .into_reserved_parts()
        .0)
}

#[cfg(test)]
mod preview_tests {
    use super::*;
    use crate::filter::bilevel_preview::{Request, Sampler};
    use crate::parser::options::Limits;
    #[test]
    fn packed_native_preview_matches_independent_full_decoder_and_shared_globals() {
        let generic = include_bytes!("../../tests/fixtures/v06/generic-mmr.jb2").as_slice();
        let arithmetic = include_bytes!("../../tests/fixtures/v06/jbig2enc-generic.jb2").as_slice();
        let half = include_bytes!("../../tests/fixtures/v06/halftone-0.jb2").as_slice();
        let globals = include_bytes!("../../tests/fixtures/v06/patterns-global.jb2").as_slice();
        for (input, globals, source) in [
            (generic, None, (32, 32)),
            (arithmetic, None, (32, 32)),
            (half, Some(globals), (32, 32)),
        ] {
            let pool = OperationBudget::new(8 * 1024 * 1024);
            let context = FilterContext {
                jbig2_globals: globals,
                ..Default::default()
            };
            let expected = decode_with_budget(input, context, &pool).unwrap();
            let mut charged = 0;
            let actual = decode_preview(input, context, &pool, source, |work| {
                charged = work;
                Ok(())
            })
            .unwrap();
            assert_eq!(actual.as_slice(), expected.as_slice());
            assert!(charged >= 1024);
            let mut sampler = Sampler::new(
                Request {
                    source,
                    target: (4, 4),
                    decode: [0., 1.],
                    limits: Limits::default(),
                },
                &pool,
            )
            .unwrap();
            sampler.push_packed(actual.as_slice()).unwrap();
            assert_eq!(sampler.finish().unwrap().as_slice().len(), 16);
            drop(actual);
            drop(expected);
            assert_eq!(pool.live_bytes(), 0);
            assert!(decode_preview(input, context, &pool, (31, 32), |_| Ok(())).is_err());
            assert!(
                decode_preview(
                    &input[..input.len() / 2],
                    context,
                    &pool,
                    source,
                    |_| Ok(())
                )
                .is_err()
            );
            assert!(decode_preview(input, context, &pool, source, |_| Err(limit("work"))).is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
    }
}
