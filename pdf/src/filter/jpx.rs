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
    Cmyk,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JpxInfo {
    pub width: u32,
    pub height: u32,
    /// Interleaved sample channels, including a declared trailing opacity channel.
    pub components: usize,
    pub has_alpha: bool,
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

fn codestream(data: &[u8]) -> PdfResult<(&[u8], Option<usize>, Option<&[u8]>, Option<u32>)> {
    if data.starts_with(&[255, 79]) {
        return Ok((data, None, None, None));
    }
    if !data.starts_with(b"\0\0\0\x0cjP  \r\n\x87\n") {
        return Err(error("invalid JP2 signature"));
    }
    let mut p = 0;
    let mut stream = None;
    let mut palette = None;
    let mut mapping = None;
    let mut definitions = None;
    let mut enumerated_color = None;
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
                let body = &data[c + 8..c + length];
                if kind == b"pclr" {
                    if palette.is_some() || body.len() < 3 {
                        return Err(error("invalid palette box"));
                    }
                    let entries = usize::from(u16be(body, 0)?);
                    let columns = usize::from(body[2]);
                    if entries == 0
                        || entries > 256
                        || !matches!(columns, 1 | 3)
                        || body
                            .get(3..3 + columns)
                            .is_none_or(|bits| bits.iter().any(|b| *b != 7))
                    {
                        return Err(unsupported(
                            "palette requires up to 256 unsigned 8-bit Gray/RGB entries",
                        ));
                    }
                    if body.len() != 3 + columns + entries * columns {
                        return Err(error("palette length mismatch"));
                    }
                    palette = Some(columns);
                }
                if kind == b"cmap" {
                    if mapping.is_some() {
                        return Err(error("duplicate component mapping"));
                    }
                    mapping = Some(body);
                }
                if kind == b"cdef" {
                    if definitions.is_some() || body.len() < 2 {
                        return Err(error("invalid channel definitions"));
                    }
                    let count = usize::from(u16be(body, 0)?);
                    if count == 0 || count > 5 || body.len() != 2 + count * 6 {
                        return Err(error("channel definition length/count"));
                    }
                    definitions = Some(body);
                }
                if kind == b"colr" {
                    if body.first() != Some(&1) {
                        return Err(unsupported("ICC colour specification"));
                    }
                    if body.len() != 7 {
                        return Err(error("enumerated color specification length"));
                    }
                    if enumerated_color.is_some() {
                        return Err(unsupported("multiple color specifications"));
                    }
                    enumerated_color = Some(u32be(body, 3)?);
                }
                c += length;
            }
        }
        p = end;
    }
    if let Some(columns) = palette {
        let mapping = mapping.ok_or_else(|| error("palette requires component mapping"))?;
        if mapping.len() != columns * 4
            || mapping
                .chunks_exact(4)
                .enumerate()
                .any(|(index, m)| m != [0, 0, 1, index as u8])
        {
            return Err(unsupported("nonsequential palette component mapping"));
        }
    } else if mapping.is_some() {
        return Err(unsupported("component mapping without palette"));
    }
    Ok((
        stream.ok_or_else(|| error("missing codestream"))?,
        palette,
        definitions,
        enumerated_color,
    ))
}

/// Inspect dimensions and coding envelope before backend header/pixel allocation.
pub fn inspect(data: &[u8], context: FilterContext<'_>) -> PdfResult<JpxInfo> {
    if data.len() > context.max_decoded_bytes {
        return Err(limit("encoded input bytes"));
    }
    let (stream, palette, definitions, enumerated_color) = codestream(data)?;
    if !stream.starts_with(&[255, 79, 255, 81]) {
        return Err(error("SOC/SIZ header missing"));
    }
    let length = u16be(stream, 4)? as usize;
    let components = u16be(stream, 40)? as usize;
    if !(1..=5).contains(&components) {
        return Err(unsupported("at most five image components"));
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
        // The final tile is clipped to the image bounds. A larger nominal
        // tile still describes one tile, and its padding is not decoded.
        || u32be(stream, 24)? < width
        || u32be(stream, 28)? < height
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
    let bits_per_component = stream.get(42).copied().unwrap_or(255).wrapping_add(1);
    let indexed_bits = context.jpx_preserve_indices
        && components == 1
        && matches!(bits_per_component, 1 | 2 | 4 | 8);
    for i in 0..components {
        if stream.get(42 + 3 * i..45 + 3 * i) != Some(&[bits_per_component.wrapping_sub(1), 1, 1])
            || (!indexed_bits && bits_per_component != 8)
        {
            return Err(unsupported("signed, non-8-bit or subsampled components"));
        }
    }
    // Packet data is skipped by Psot, not searched for marker bytes.
    let mut p = 2;
    let mut tile_end = None;
    let mut saw_cod = false;
    let mut coding = None;
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
                if saw_cod {
                    return Err(unsupported("tile coding overrides"));
                }
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
                let precinct_bytes = if payload[0] & 1 != 0 {
                    usize::from(payload[5]) + 1
                } else {
                    0
                };
                if payload.len() != 10 + precinct_bytes {
                    return Err(error("COD precinct length mismatch"));
                }
                saw_cod = true;
                coding = Some(payload);
            }
            0xff53 => {
                // An identical component override does not change the work
                // estimated from COD. Component indices fit in one byte in
                // this decoder's bounded component envelope.
                let cod = coding.ok_or_else(|| error("COC before COD"))?;
                if payload.len() != cod.len() - 3
                    || usize::from(payload[0]) >= components
                    || payload[1] != cod[0]
                    || payload[2..] != cod[5..]
                {
                    return Err(unsupported("non-equivalent component coding override"));
                }
            }
            0xff5e | 0xff5f | 0xff60 | 0xff61 => {
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
    if palette.is_some() && components != 1 {
        return Err(unsupported(
            "palette requires one codestream index component",
        ));
    }
    let preserve_palette = context.jpx_preserve_indices && palette.is_some();
    let components = if preserve_palette {
        components
    } else {
        palette.unwrap_or(components)
    };
    let mut has_alpha = false;
    if let Some(definitions) = definitions.filter(|_| !preserve_palette) {
        let count = usize::from(u16be(definitions, 0)?);
        if count != components {
            return Err(error("channel definitions disagree with sample count"));
        }
        for index in 0..count {
            let p = 2 + index * 6;
            let channel = usize::from(u16be(definitions, p)?);
            let kind = u16be(definitions, p + 2)?;
            let association = u16be(definitions, p + 4)?;
            if channel != index {
                return Err(unsupported("nonsequential channel definitions"));
            }
            if index + 1 == count && kind == 1 && association == 0 {
                has_alpha = true;
            } else if kind != 0 || association != index as u16 + 1 {
                return Err(unsupported("channel association or premultiplied opacity"));
            }
        }
    }
    let color_components = components - usize::from(has_alpha);
    if let Some(enumerated) = enumerated_color.filter(|_| !preserve_palette) {
        let expected = match enumerated {
            17 => 1,
            16 | 18 => 3,
            12 => 4,
            _ => return Err(unsupported("enumerated color specification")),
        };
        if color_components != expected {
            return Err(error("color specification disagrees with sample channels"));
        }
    }

    let color_space = match color_components {
        1 => JpxColorSpace::Gray,
        3 => JpxColorSpace::Rgb,
        4 => JpxColorSpace::Cmyk,
        _ => return Err(unsupported("image color component count")),
    };

    (width as usize)
        .checked_mul(height as usize)
        .and_then(|n| n.checked_mul(components))
        .filter(|n| *n <= context.max_decoded_bytes)
        .ok_or_else(|| limit("expanded palette sample bytes"))?;
    Ok(JpxInfo {
        width,
        height,
        components,
        has_alpha,
        bits_per_component,
        color_space,
    })
}

// The backend keeps coefficient/sample planes and code-block metadata, not
// 128 bytes for every output sample. Account separately for small code-blocks
// and decomposition boundaries so ordinary multi-megapixel RGB scans fit.
fn scratch_envelope(input: &[u8], info: JpxInfo) -> PdfResult<usize> {
    let (stream, _, _, _) = codestream(input)?;
    let mut p = 2;
    let mut blocks = None;
    while p + 4 <= stream.len() {
        let marker = u16be(stream, p)?;
        let length = u16be(stream, p + 2)? as usize;
        if marker == 0xff52 {
            let payload = stream
                .get(p + 4..p + 2 + length)
                .ok_or_else(|| error("truncated COD"))?;
            let levels = *payload.get(5).ok_or_else(|| error("truncated COD"))?;
            let mut bw = 1usize << (payload[6] + 2);
            let mut bh = 1usize << (payload[7] + 2);
            if payload[0] & 1 != 0 {
                for byte in &payload[10..] {
                    bw = bw.min(1usize << (byte & 15).saturating_sub(1));
                    bh = bh.min(1usize << (byte >> 4).saturating_sub(1));
                }
            }
            let (mut w, mut h) = (info.width as usize, info.height as usize);
            let mut count = 0usize;
            for _ in 0..levels {
                w = w.div_ceil(2);
                h = h.div_ceil(2);
                // One boundary block per axis covers odd high-band origins.
                count = count
                    .checked_add(
                        (w.div_ceil(bw) + 1)
                            .checked_mul(h.div_ceil(bh) + 1)
                            .and_then(|n| n.checked_mul(3))
                            .ok_or_else(|| limit("code-block count"))?,
                    )
                    .ok_or_else(|| limit("code-block count"))?;
            }
            count = count
                .checked_add(
                    (w.div_ceil(bw) + 1)
                        .checked_mul(h.div_ceil(bh) + 1)
                        .ok_or_else(|| limit("code-block count"))?,
                )
                .ok_or_else(|| limit("code-block count"))?;
            // Layers hold per-code-block packet metadata. Its size grows
            // with layers even when the image sample count stays fixed.
            blocks = Some(
                count
                    .checked_mul(2048 + usize::from(u16be(payload, 2)?) * 128)
                    .ok_or_else(|| limit("code-block metadata"))?,
            );
            break;
        }
        if length < 2 {
            return Err(error("invalid marker length"));
        }
        p = p
            .checked_add(2 + length)
            .ok_or_else(|| limit("header offset"))?;
    }
    let samples = (info.width as usize)
        .checked_mul(info.height as usize)
        .and_then(|n| n.checked_mul(info.components))
        .ok_or_else(|| limit("sample size"))?;
    samples
        .checked_mul(32)
        .and_then(|n| blocks?.checked_mul(info.components)?.checked_add(n))
        .and_then(|n| input.len().checked_mul(8)?.checked_add(n))
        .and_then(|n| n.checked_add(1024 * 1024))
        .ok_or_else(|| limit("scratch envelope overflow"))
}

pub fn decode_with_budget(
    input: &[u8],
    context: FilterContext<'_>,
    budget: &OperationBudget,
) -> PdfResult<(JpxInfo, BudgetedBytes)> {
    let info = inspect(input, context)?;
    let samples = (info.width as usize) * (info.height as usize) * info.components;
    let scratch = scratch_envelope(input, info)?;
    let _scratch = budget.reserve(scratch)?;
    let settings = hayro_jpeg2000::DecodeSettings {
        strict: true,
        resolve_palette_indices: !context.jpx_preserve_indices,
        ..Default::default()
    };
    let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(
        || -> PdfResult<BudgetedBytes> {
            let image = hayro_jpeg2000::Image::new(input, &settings)
                .map_err(|_| error("invalid image headers"))?;
            if image.width() != info.width
                || image.height() != info.height
                || image.original_bit_depth() != info.bits_per_component
                || image.has_alpha() != info.has_alpha
            {
                return Err(unsupported("metadata or alpha outside baseline"));
            }
            let matches_color = matches!(
                (image.color_space(), info.color_space),
                (hayro_jpeg2000::ColorSpace::Gray, JpxColorSpace::Gray)
                    | (hayro_jpeg2000::ColorSpace::RGB, JpxColorSpace::Rgb)
                    | (hayro_jpeg2000::ColorSpace::CMYK, JpxColorSpace::Cmyk)
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
                || decoded.components().iter().any(|c| {
                    c.samples().len() != samples / info.components
                        || c.bit_depth() != info.bits_per_component
                })
            {
                return Err(error("decoded component shape mismatch"));
            }
            if context.jpx_preserve_indices {
                if info.components != 1 {
                    return Err(unsupported("Indexed requires one sample channel"));
                }
                let maximum = ((1u32 << info.bits_per_component) - 1) as f32;
                for (target, sample) in output.iter_mut().zip(decoded.components()[0].samples()) {
                    *target = sample.round().clamp(0., maximum) as u8;
                }
            } else {
                decoded.store_u8_into(&mut output);
            }
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

#[cfg(test)]
mod scratch_tests {
    use super::*;
    #[test]
    fn workspace_tracks_codeblock_size_and_layers_instead_of_flat_sample_multiplier() {
        let bytes = include_bytes!("../../tests/fixtures/v06/rgb-raw.j2k");
        let info = inspect(bytes, FilterContext::default()).unwrap();
        let base = scratch_envelope(bytes, info).unwrap();
        let marker = bytes.windows(2).position(|b| b == [0xff, 0x52]).unwrap();
        let mut fine = bytes.to_vec();
        fine[marker + 4 + 6] = 0;
        fine[marker + 4 + 7] = 0;
        assert!(scratch_envelope(&fine, info).unwrap() > base);
        let mut layered = bytes.to_vec();
        layered[marker + 4 + 2..marker + 4 + 4].copy_from_slice(&64u16.to_be_bytes());
        assert!(scratch_envelope(&layered, info).unwrap() > base);
        let mut large = info;
        large.width = 1527;
        large.height = 1276;
        let workspace = scratch_envelope(bytes, large).unwrap();
        assert!(workspace < 512 * 1024 * 1024);
        let pool = OperationBudget::new(1024);
        assert!(matches!(
            decode_with_budget(bytes, FilterContext::default(), &pool),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), 0);
    }
}
