//! JPEG 2000 baseline decoding. Scratch reservations are not an RSS sandbox.
use crate::{
    budget::{BudgetedBytes, OperationBudget},
    error::{PdfError, PdfResult},
    filter::FilterContext,
    objects::pdf_dict::PdfDict,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum JpxColorSpace {
    Gray,
    Rgb,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JpxInfo {
    pub width: u32,
    pub height: u32,
    pub components: usize,
    pub bits_per_component: u8,
    pub color_space: JpxColorSpace,
}
fn error(s: &str) -> PdfError {
    PdfError::FilterError(format!("JPXDecode: {s}"))
}
fn unsupported(s: &str) -> PdfError {
    PdfError::UnsupportedFeature(format!("JPXDecode {s}"))
}
fn limit(s: &str) -> PdfError {
    PdfError::ResourceLimit(format!("JPXDecode {s}"))
}
fn u16be(b: &[u8], p: usize) -> PdfResult<u16> {
    Ok(u16::from_be_bytes(
        b.get(p..p + 2)
            .ok_or_else(|| error("truncated header"))?
            .try_into()
            .unwrap(),
    ))
}
fn u32be(b: &[u8], p: usize) -> PdfResult<u32> {
    Ok(u32::from_be_bytes(
        b.get(p..p + 4)
            .ok_or_else(|| error("truncated header"))?
            .try_into()
            .unwrap(),
    ))
}

fn codestream(data: &[u8]) -> PdfResult<&[u8]> {
    if data.starts_with(&[255, 79]) {
        return Ok(data);
    }
    if !data.starts_with(b"\0\0\0\x0cjP  \r\n\x87\n") {
        return Err(error("invalid JP2 signature"));
    }
    let mut p = 0;
    let mut stream = None;
    while p < data.len() {
        let n = u32be(data, p)? as usize;
        let tag = data
            .get(p + 4..p + 8)
            .ok_or_else(|| error("truncated box"))?;
        if n == 1 {
            return Err(unsupported("extended-length boxes"));
        }
        let n = if n == 0 { data.len() - p } else { n };
        if n < 8 {
            return Err(error("invalid box length"));
        }
        let end = p
            .checked_add(n)
            .filter(|e| *e <= data.len())
            .ok_or_else(|| error("truncated box"))?;
        if tag == b"jp2c" {
            if stream.is_some() {
                return Err(unsupported("multiple codestreams"));
            }
            stream = Some(&data[p + 8..end]);
        }
        if tag == b"jp2h" {
            let mut c = p + 8;
            while c < end {
                let length = u32be(data, c)? as usize;
                if length < 8 || c.checked_add(length).is_none_or(|v| v > end) {
                    return Err(error("invalid header box"));
                }
                let kind = &data[c + 4..c + 8];
                if matches!(kind, b"pclr" | b"cmap" | b"cdef") {
                    return Err(unsupported("palette/component mapping/alpha boxes"));
                }
                if kind == b"colr" && data.get(c + 8) != Some(&1) {
                    return Err(unsupported("ICC colour specification"));
                }
                c += length;
            }
        }
        p = end;
    }
    stream.ok_or_else(|| error("missing codestream"))
}

/// Inspect dimensions and coding envelope before backend header/pixel allocation.
pub fn inspect(data: &[u8], context: FilterContext<'_>) -> PdfResult<JpxInfo> {
    if data.len() > context.max_decoded_bytes {
        return Err(limit("encoded input bytes"));
    }
    let stream = codestream(data)?;
    if !stream.starts_with(&[255, 79, 255, 81]) {
        return Err(error("SOC/SIZ header missing"));
    }
    let length = u16be(stream, 4)? as usize;
    let components = u16be(stream, 40)? as usize;
    if !matches!(components, 1 | 3) {
        return Err(unsupported("only Gray/RGB components are supported"));
    }
    if length != 38 + 3 * components {
        return Err(error("SIZ length mismatch"));
    }
    let width = u32be(stream, 8)?
        .checked_sub(u32be(stream, 16)?)
        .filter(|n| *n > 0)
        .ok_or_else(|| error("invalid width"))?;
    let height = u32be(stream, 12)?
        .checked_sub(u32be(stream, 20)?)
        .filter(|n| *n > 0)
        .ok_or_else(|| error("invalid height"))?;
    if u32be(stream, 16)? != 0
        || u32be(stream, 20)? != 0
        || u32be(stream, 32)? != 0
        || u32be(stream, 36)? != 0
        || u32be(stream, 24)? != width
        || u32be(stream, 28)? != height
    {
        return Err(unsupported("nonzero origins or multiple tiles"));
    }
    if u64::from(width) * u64::from(height) > context.max_image_pixels {
        return Err(limit("pixel count"));
    }
    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(components))
        .filter(|n| *n <= context.max_decoded_bytes)
        .ok_or_else(|| limit("sample bytes"))?;
    for i in 0..components {
        if stream.get(42 + 3 * i..45 + 3 * i) != Some(&[7, 1, 1]) {
            return Err(unsupported("signed, non-8-bit or subsampled components"));
        }
    }
    // Packet data is skipped by Psot, not searched for marker bytes.
    let mut p = 2;
    let mut tile_end = None;
    let mut saw_cod = false;
    let mut parts = 0;
    let mut headers = 0;
    loop {
        let marker = u16be(stream, p)?;
        p += 2;
        if marker == 0xffd9 {
            if p != stream.len() {
                return Err(error("trailing codestream data"));
            }
            break;
        }
        if marker == 0xff93 {
            let end = tile_end
                .take()
                .filter(|end| *end >= p)
                .ok_or_else(|| error("invalid tile data boundary"))?;
            p = end;
            continue;
        }
        headers += 1;
        if headers > 4096 {
            return Err(limit("marker count"));
        }
        let length = u16be(stream, p)? as usize;
        if length < 2 {
            return Err(error("invalid marker length"));
        }
        let end = p
            .checked_add(length)
            .filter(|e| *e <= stream.len())
            .ok_or_else(|| error("truncated marker"))?;
        let payload = &stream[p + 2..end];
        match marker {
            0xff90 => {
                parts += 1;
                if parts > 64 {
                    return Err(limit("tile-part count"));
                }
                if length != 10 || u16be(payload, 0)? != 0 {
                    return Err(error("invalid tile-part header"));
                }
                let n = u32be(payload, 2)? as usize;
                if n == 0 {
                    return Err(unsupported("zero-length tile parts"));
                }
                tile_end = Some(
                    (p - 2)
                        .checked_add(n)
                        .filter(|e| *e >= end && *e <= stream.len())
                        .ok_or_else(|| error("invalid tile-part length"))?,
                );
            }
            0xff52 => {
                if payload.len() < 10 {
                    return Err(error("truncated COD"));
                }
                if payload[1] > 4
                    || !(1..=64).contains(&u16be(payload, 2)?)
                    || payload[5] > 8
                    || payload[6] > 4
                    || payload[7] > 4
                {
                    return Err(unsupported(
                        "coding progression/layers/decomposition/code-block envelope",
                    ));
                }
                saw_cod = true;
            }
            0xff53 | 0xff5e | 0xff5f | 0xff60 | 0xff61 => {
                return Err(unsupported(
                    "component overrides, ROI or packed packet headers",
                ));
            }
            0xff51 if p != 4 => return Err(error("duplicate SIZ header")),
            0xff51 | 0xff5c | 0xff5d | 0xff64 | 0xff55 | 0xff57 | 0xff58 => {}
            _ => return Err(unsupported("codestream marker")),
        }
        p = end;
    }
    if !saw_cod || parts == 0 {
        return Err(error("missing coding/tile headers"));
    }
    Ok(JpxInfo {
        width,
        height,
        components,
        bits_per_component: 8,
        color_space: if components == 1 {
            JpxColorSpace::Gray
        } else {
            JpxColorSpace::Rgb
        },
    })
}

pub fn decode_with_budget(
    input: &[u8],
    context: FilterContext<'_>,
    budget: &OperationBudget,
) -> PdfResult<(JpxInfo, BudgetedBytes)> {
    let info = inspect(input, context)?;
    let samples = (info.width as usize) * (info.height as usize) * info.components;
    let scratch = samples
        .checked_mul(128)
        .and_then(|n| input.len().checked_mul(8).and_then(|m| n.checked_add(m)))
        .and_then(|n| n.checked_add(1024 * 1024))
        .ok_or_else(|| limit("scratch envelope overflow"))?;
    let _scratch = budget.reserve(scratch)?;
    let settings = hayro_jpeg2000::DecodeSettings {
        strict: true,
        ..Default::default()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> PdfResult<BudgetedBytes> {
            let image = hayro_jpeg2000::Image::new(input, &settings)
                .map_err(|_| error("invalid image headers"))?;
            if image.width() != info.width
                || image.height() != info.height
                || image.original_bit_depth() != 8
                || image.has_alpha()
            {
                return Err(unsupported("metadata or alpha outside baseline"));
            }
            let matches_color = matches!(
                (image.color_space(), info.color_space),
                (hayro_jpeg2000::ColorSpace::Gray, JpxColorSpace::Gray)
                    | (hayro_jpeg2000::ColorSpace::RGB, JpxColorSpace::Rgb)
            );
            if !matches_color {
                return Err(unsupported("colour specification outside Gray/RGB"));
            }
            let mut lease = budget.reserve(samples)?;
            let mut output = Vec::new();
            output
                .try_reserve_exact(samples)
                .map_err(|_| limit("output allocation"))?;
            lease.resize(output.capacity())?;
            output.resize(samples, 0);
            let mut decoder = hayro_jpeg2000::DecoderContext::default();
            let decoded = image
                .decode(&mut decoder)
                .map_err(|_| error("invalid compressed samples"))?;
            if decoded.components().len() != info.components
                || decoded
                    .components()
                    .iter()
                    .any(|c| c.samples().len() != samples / info.components || c.bit_depth() != 8)
            {
                return Err(error("decoded component shape mismatch"));
            }
            decoded.store_u8_into(&mut output);
            BudgetedBytes::from_reserved_vec(output, lease)
        },
    ));
    Ok((info, result.map_err(|_| error("backend decoder panic"))??))
}

pub fn jpx_decode(
    input: &[u8],
    _params: Option<&PdfDict>,
    context: FilterContext<'_>,
) -> PdfResult<Vec<u8>> {
    let budget = crate::parser::options::Limits::default().operation_budget();
    let (_, bytes) = decode_with_budget(input, context, &budget)?;
    Ok(bytes.into_reserved_parts().0)
}
