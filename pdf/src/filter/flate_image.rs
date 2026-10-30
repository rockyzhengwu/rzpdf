//! Streaming 8-bit image samples. No full-source RGB/RGBA allocation.
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
    parser::options::Limits,
};
use flate2::{Decompress, FlushDecompress, Status};
use std::io;

#[derive(Clone, Copy, Debug)]
pub(crate) struct Predictor8 {
    value: u32,
}
impl Predictor8 {
    /// Other predictor layouts retain the ordinary bounded decode path.
    pub fn from_parameters(
        params: Option<&PdfDict>,
        width: u32,
        components: usize,
    ) -> PdfResult<Option<Self>> {
        let integer = |key: &str, default| -> PdfResult<u32> {
            params
                .and_then(|p| p.get(key))
                .map(|value| {
                    value
                        .as_number()
                        .ok_or_else(|| {
                            PdfError::FilterError(format!(
                                "Flate image predictor {key} must be an integer"
                            ))
                        })?
                        .as_u32_checked()
                })
                .unwrap_or(Ok(default))
        };
        let value = integer("Predictor", 1)?;
        if value == 1 {
            return Ok(Some(Self { value }));
        }
        if !matches!(value, 2 | 10..=15) {
            return Err(PdfError::FilterError(
                "unsupported Flate image predictor".into(),
            ));
        }
        let columns = integer("Columns", 1)?;
        let colors = integer("Colors", 1)?;
        let bits = integer("BitsPerComponent", 8)?;
        if columns == 0 || colors == 0 || !matches!(bits, 1 | 2 | 4 | 8 | 16) {
            return Err(PdfError::FilterError(
                "invalid Flate image predictor dimensions".into(),
            ));
        }
        Ok(
            (columns == width && colors as usize == components && bits == 8)
                .then_some(Self { value }),
        )
    }
    fn png(self) -> bool {
        self.value >= 10
    }
    fn restore(
        self,
        row: &mut [u8],
        previous: &[u8],
        components: usize,
        filter: u8,
    ) -> PdfResult<()> {
        if self.value == 1 {
            return Ok(());
        }
        let filter = if self.value == 2 { 1 } else { filter };
        if filter > 4 {
            return Err(PdfError::FilterError("unknown PNG image row filter".into()));
        }
        for index in 0..row.len() {
            let left = if index >= components {
                row[index - components]
            } else {
                0
            };
            let up = previous.get(index).copied().unwrap_or(0);
            let upper_left = if index >= components {
                previous.get(index - components).copied().unwrap_or(0)
            } else {
                0
            };
            let predicted = match filter {
                0 => 0,
                1 => left,
                2 => up,
                3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                _ => super::predictor::paeth_predictor(left, up, upper_left),
            };
            row[index] = row[index].wrapping_add(predicted);
        }
        Ok(())
    }
}

struct InflateRows<'a> {
    input: &'a [u8],
    decoder: Decompress,
    ended: bool,
}
impl<'a> InflateRows<'a> {
    fn new(input: &'a [u8]) -> Self {
        Self {
            input,
            decoder: Decompress::new(true),
            ended: false,
        }
    }
    fn read(&mut self, output: &mut [u8]) -> PdfResult<usize> {
        if self.ended {
            return Ok(0);
        }
        let input_before = self.decoder.total_in();
        let output_before = self.decoder.total_out();
        let status = self
            .decoder
            .decompress(
                &self.input[input_before as usize..],
                output,
                FlushDecompress::None,
            )
            .map_err(|e| PdfError::FilterError(format!("Flate image decompression: {e}")))?;
        self.ended = status == Status::StreamEnd;
        let bytes = (self.decoder.total_out() - output_before) as usize;
        if !self.ended && bytes == 0 && self.decoder.total_in() == input_before {
            return Err(PdfError::FilterError(
                "truncated Flate image zlib stream".into(),
            ));
        }
        Ok(bytes)
    }
    fn exact(&mut self, mut output: &mut [u8]) -> PdfResult<()> {
        while !output.is_empty() {
            let bytes = self.read(output)?;
            if bytes == 0 && self.ended {
                return Err(PdfError::FilterError("truncated Flate image row".into()));
            }
            output = &mut output[bytes..];
        }
        Ok(())
    }
    fn finish(&mut self, compatible_tail: bool, maximum: usize) -> PdfResult<bool> {
        // Drain through StreamEnd, including the Adler checksum, before any
        // preview can escape. Match the legacy bounded image EOL recovery.
        let mut tail = [0u8; 3];
        let mut length = 0;
        loop {
            let bytes = self.read(&mut tail[length..])?;
            length += bytes;
            if self.decoder.total_out() > maximum as u64 {
                return Err(limit(
                    "image_source_bytes",
                    maximum as u64,
                    self.decoder.total_out(),
                ));
            }
            if length == tail.len() {
                return Err(PdfError::FilterError(
                    "excess decoded Flate image samples".into(),
                ));
            }
            if self.ended {
                break;
            }
        }
        if length == 0 {
            return Ok(false);
        }
        if compatible_tail && matches!(&tail[..length], b"\r" | b"\n" | b"\r\n") {
            return Ok(true);
        }
        Err(PdfError::FilterError(
            "excess decoded Flate image samples".into(),
        ))
    }
}
fn limit(reason: &'static str, maximum: u64, actual: u64) -> PdfError {
    PdfError::ResourceLimitExceeded {
        reason,
        stage: "image_decode",
        limit: maximum,
        actual,
    }
}
fn zeros_u64(pool: &OperationBudget, slots: usize) -> PdfResult<(Vec<u64>, MemoryLease)> {
    let size = slots
        .checked_mul(8)
        .ok_or_else(|| PdfError::ResourceLimit("image sampling accumulators".into()))?;
    let mut lease = pool.reserve(size)?;
    let mut data = Vec::new();
    data.try_reserve_exact(slots)
        .map_err(|_| PdfError::ResourceLimit("image sampling allocation".into()))?;
    lease.resize(data.capacity() * 8)?;
    data.resize(slots, 0);
    Ok((data, lease))
}

pub(crate) struct FlatePreview {
    pub samples: BudgetedBytes,
    pub recovered_tail: bool,
}

#[derive(Clone, Copy)]
pub(crate) struct FlateImageRequest {
    pub source: (u32, u32),
    pub target: (u32, u32),
    pub components: usize,
    pub predictor: Predictor8,
    pub decode: Option<[f64; 8]>,
    pub compatible: bool,
    pub limits: Limits,
}

/// Forward-only source rows shared by opaque and masked preview decoders.
/// Predictor history always retains original samples, before Decode/Matte.
pub(crate) struct FlateRows<'a> {
    zlib: InflateRows<'a>,
    request: FlateImageRequest,
    row: BudgetedBytes,
    previous: BudgetedBytes,
    count: u32,
}
impl<'a> FlateRows<'a> {
    pub fn new(
        input: &'a [u8],
        request: FlateImageRequest,
        pool: &OperationBudget,
    ) -> PdfResult<Self> {
        let (row_bytes, history_bytes) = Self::storage_bytes(request)?;
        Ok(Self {
            zlib: InflateRows::new(input),
            request,
            row: BudgetedBytes::zeroed(pool, row_bytes)?,
            previous: BudgetedBytes::zeroed(pool, history_bytes)?,
            count: 0,
        })
    }
    pub fn storage_bytes(request: FlateImageRequest) -> PdfResult<(usize, usize)> {
        let pixels = u64::from(request.source.0) * u64::from(request.source.1);
        if pixels == 0 || !matches!(request.components, 1 | 3) {
            return Err(PdfError::ObjectError(
                "invalid Flate source dimensions/components".into(),
            ));
        }
        if pixels > super::MAX_TARGET_SOURCE_IMAGE_PIXELS {
            return Err(limit(
                "image_source_pixels",
                super::MAX_TARGET_SOURCE_IMAGE_PIXELS,
                pixels,
            ));
        }
        let row_bytes = (request.source.0 as usize)
            .checked_mul(request.components)
            .ok_or_else(|| PdfError::ResourceLimit("Flate row size".into()))?;
        let inflated = row_bytes
            .checked_add(usize::from(request.predictor.png()))
            .and_then(|n| n.checked_mul(request.source.1 as usize))
            .ok_or_else(|| PdfError::ResourceLimit("Flate source size".into()))?;
        if inflated > request.limits.max_decoded_bytes {
            return Err(limit(
                "image_source_bytes",
                request.limits.max_decoded_bytes as u64,
                inflated as u64,
            ));
        }
        Ok((
            row_bytes,
            if request.predictor.png() {
                row_bytes
            } else {
                0
            },
        ))
    }
    pub fn next(&mut self) -> PdfResult<()> {
        if self.count >= self.request.source.1 {
            return Err(PdfError::FilterError("too many Flate row reads".into()));
        }
        if self.count > 0 && self.request.predictor.png() {
            std::mem::swap(&mut self.row, &mut self.previous);
        }
        let mut filter = [0];
        if self.request.predictor.png() {
            self.zlib.exact(&mut filter)?;
        }
        self.zlib.exact(self.row.as_mut_slice())?;
        self.request.predictor.restore(
            self.row.as_mut_slice(),
            self.previous.as_slice(),
            self.request.components,
            filter[0],
        )?;
        self.count += 1;
        Ok(())
    }
    pub fn samples(&self) -> &[u8] {
        self.row.as_slice()
    }
    pub fn finish(&mut self) -> PdfResult<bool> {
        if self.count != self.request.source.1 {
            return Err(PdfError::FilterError(
                "incomplete Flate row traversal".into(),
            ));
        }
        self.zlib.finish(
            self.request.compatible && self.request.predictor.value == 1,
            self.request.limits.max_decoded_bytes,
        )
    }
}

/// Area-average in integer source/destination coordinates. One reconstructed
/// row, one previous PNG row, two output-row accumulators and the final target
/// samples are owned. Decoder-private fixed state is excluded from the pool.
pub(crate) fn decode_target(
    input: &[u8],
    request: FlateImageRequest,
    pool: &OperationBudget,
) -> PdfResult<FlatePreview> {
    let FlateImageRequest {
        source,
        target,
        components,
        predictor,
        decode,
        compatible: _,
        limits,
    } = request;
    let (sw, sh, tw, th) = (
        source.0 as usize,
        source.1 as usize,
        target.0 as usize,
        target.1 as usize,
    );
    if sw == 0
        || sh == 0
        || tw == 0
        || th == 0
        || tw > sw
        || th > sh
        || !matches!(components, 1 | 3)
    {
        return Err(PdfError::ObjectError(
            "invalid Flate preview dimensions/components".into(),
        ));
    }
    let source_pixels = u64::from(source.0) * u64::from(source.1);
    if source_pixels > super::MAX_TARGET_SOURCE_IMAGE_PIXELS {
        return Err(limit(
            "image_source_pixels",
            super::MAX_TARGET_SOURCE_IMAGE_PIXELS,
            source_pixels,
        ));
    }
    let pixels = u64::from(target.0) * u64::from(target.1);
    if pixels > limits.max_image_pixels {
        return Err(limit("image_pixels", limits.max_image_pixels, pixels));
    }
    if pixels * 4 > limits.max_decoded_bytes as u64 {
        return Err(limit(
            "decoded_bytes",
            limits.max_decoded_bytes as u64,
            pixels * 4,
        ));
    }
    let row_bytes = sw
        .checked_mul(components)
        .ok_or_else(|| PdfError::ResourceLimit("Flate image row size".into()))?;
    let inflated = row_bytes
        .checked_add(usize::from(predictor.png()))
        .and_then(|n| n.checked_mul(sh))
        .ok_or_else(|| PdfError::ResourceLimit("Flate image source byte size".into()))?;
    if inflated > limits.max_decoded_bytes {
        return Err(limit(
            "image_source_bytes",
            limits.max_decoded_bytes as u64,
            inflated as u64,
        ));
    }
    let slots = tw
        .checked_mul(components)
        .ok_or_else(|| PdfError::ResourceLimit("image target row size".into()))?;
    let needed = slots
        .checked_mul(th)
        .and_then(|n| n.checked_add(row_bytes))
        .and_then(|n| n.checked_add(if predictor.png() { row_bytes } else { 0 }))
        .and_then(|n| {
            slots
                .checked_mul(16)
                .and_then(|scratch| n.checked_add(scratch))
        })
        .and_then(|n| n.checked_add(pool.live_bytes()))
        .ok_or_else(|| PdfError::ResourceLimit("image operation size overflow".into()))?;
    if needed > pool.maximum_bytes() {
        return Err(limit(
            "image_operation_bytes",
            pool.maximum_bytes() as u64,
            needed as u64,
        ));
    }
    let mut output = BudgetedBytes::zeroed(
        pool,
        slots
            .checked_mul(th)
            .ok_or_else(|| PdfError::ResourceLimit("image target samples".into()))?,
    )?;
    let mut rows = FlateRows::new(input, request, pool)?;
    let (mut horizontal, _horizontal_lease) = zeros_u64(pool, slots)?;
    let (mut accumulated, _accumulated_lease) = zeros_u64(pool, slots)?;
    let mut table = [[0u8; 256]; 3];
    for (component, values) in table.iter_mut().enumerate().take(components) {
        for (sample, value) in values.iter_mut().enumerate() {
            *value = if let Some(endpoints) = decode {
                ((endpoints[component * 2]
                    + sample as f64 / 255.
                        * (endpoints[component * 2 + 1] - endpoints[component * 2]))
                    .clamp(0., 1.)
                    * 255.)
                    .round() as u8
            } else {
                sample as u8
            };
        }
    }
    let mut output_y = 0usize;
    for y in 0..sh {
        rows.next()?;
        horizontal.fill(0);
        for x in 0..tw {
            let start = x as u64 * sw as u64;
            let end = (x as u64 + 1) * sw as u64;
            for sx in start / tw as u64..end.div_ceil(tw as u64) {
                let weight = end.min((sx + 1) * tw as u64) - start.max(sx * tw as u64);
                for c in 0..components {
                    horizontal[x * components + c] +=
                        u64::from(table[c][rows.samples()[sx as usize * components + c] as usize])
                            * weight;
                }
            }
        }
        let mut cursor = y as u64 * th as u64;
        let end = (y as u64 + 1) * th as u64;
        while cursor < end {
            let boundary = (output_y as u64 + 1) * sh as u64;
            let stop = end.min(boundary);
            let weight = stop - cursor;
            for (sum, value) in accumulated.iter_mut().zip(&horizontal) {
                *sum += value * weight;
            }
            cursor = stop;
            if stop == boundary {
                let destination =
                    &mut output.as_mut_slice()[output_y * slots..(output_y + 1) * slots];
                for (sample, sum) in destination.iter_mut().zip(&mut accumulated) {
                    *sample = ((*sum + source_pixels / 2) / source_pixels) as u8;
                    *sum = 0;
                }
                output_y += 1;
            }
        }
    }
    if output_y != th {
        return Err(PdfError::IOError(
            io::Error::other("image sampling coverage").to_string(),
        ));
    }
    let recovered_tail = rows.finish()?;
    Ok(FlatePreview {
        samples: output,
        recovered_tail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    fn compress(bytes: &[u8]) -> Vec<u8> {
        let mut encoder =
            flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
        encoder.write_all(bytes).unwrap();
        encoder.finish().unwrap()
    }
    fn request(
        source: (u32, u32),
        target: (u32, u32),
        components: usize,
        value: u32,
    ) -> FlateImageRequest {
        FlateImageRequest {
            source,
            target,
            components,
            predictor: Predictor8 { value },
            decode: None,
            compatible: false,
            limits: Limits::default(),
        }
    }
    fn predicted(
        samples: &[u8],
        width: usize,
        components: usize,
        predictor: u32,
        filter: u8,
    ) -> Vec<u8> {
        let stride = width * components;
        let mut output = Vec::new();
        for (y, row) in samples.chunks_exact(stride).enumerate() {
            if predictor >= 10 {
                output.push(filter);
            }
            for (x, &sample) in row.iter().enumerate() {
                let left = if x >= components {
                    row[x - components]
                } else {
                    0
                };
                let up = if y > 0 {
                    samples[(y - 1) * stride + x]
                } else {
                    0
                };
                let upper_left = if y > 0 && x >= components {
                    samples[(y - 1) * stride + x - components]
                } else {
                    0
                };
                let prediction = match if predictor == 2 {
                    1
                } else if predictor == 1 {
                    0
                } else {
                    filter
                } {
                    0 => 0,
                    1 => left,
                    2 => up,
                    3 => ((u16::from(left) + u16::from(up)) / 2) as u8,
                    _ => super::super::predictor::paeth_predictor(left, up, upper_left),
                };
                output.push(sample.wrapping_sub(prediction));
            }
        }
        output
    }
    // Independent floating-point rectangle-overlap oracle, rather than the
    // decoder's integer scanline traversal and accumulators.
    fn oracle(
        samples: &[u8],
        source: (u32, u32),
        target: (u32, u32),
        components: usize,
    ) -> Vec<u8> {
        let mut out = Vec::new();
        for dy in 0..target.1 {
            for dx in 0..target.0 {
                for c in 0..components {
                    let (left, right) = (
                        dx as f64 * source.0 as f64 / target.0 as f64,
                        (dx + 1) as f64 * source.0 as f64 / target.0 as f64,
                    );
                    let (top, bottom) = (
                        dy as f64 * source.1 as f64 / target.1 as f64,
                        (dy + 1) as f64 * source.1 as f64 / target.1 as f64,
                    );
                    let mut sum = 0.;
                    for y in top.floor() as u32..bottom.ceil() as u32 {
                        for x in left.floor() as u32..right.ceil() as u32 {
                            let area = (right.min(x as f64 + 1.) - left.max(x as f64))
                                * (bottom.min(y as f64 + 1.) - top.max(y as f64));
                            sum += samples
                                [(y as usize * source.0 as usize + x as usize) * components + c]
                                as f64
                                * area;
                        }
                    }
                    out.push((sum / ((right - left) * (bottom - top))).round() as u8);
                }
            }
        }
        out
    }
    #[test]
    fn area_sampling_and_all_supported_predictors_match_independent_oracle() {
        for components in [1, 3] {
            let samples: Vec<_> = (0..7 * 5 * components)
                .map(|x| ((x * 31 + x / 7 * 11) % 256) as u8)
                .collect();
            for predictor in [1, 2, 10, 11, 12, 13, 14, 15] {
                for filter in if predictor >= 10 { 0..=4 } else { 0..=0 } {
                    let encoded = compress(&predicted(&samples, 7, components, predictor, filter));
                    for target in [(1, 1), (3, 2), (6, 4), (7, 5)] {
                        let pool = OperationBudget::new(100_000);
                        let result = decode_target(
                            &encoded,
                            request((7, 5), target, components, predictor),
                            &pool,
                        )
                        .unwrap();
                        let expected = oracle(&samples, (7, 5), target, components);
                        assert!(
                            result
                                .samples
                                .as_slice()
                                .iter()
                                .zip(expected)
                                .all(|(a, b)| a.abs_diff(b) <= 1)
                        );
                        assert_eq!(pool.live_bytes(), result.samples.capacity());
                        drop(result);
                        assert_eq!(pool.live_bytes(), 0);
                    }
                }
            }
        }
    }
    #[test]
    fn sample_decode_happens_before_averaging_without_changing_predictor_history() {
        let source = [0, 100, 200, 255, 30, 70, 130, 230];
        let encoded = compress(&predicted(&source, 4, 1, 15, 2));
        let mut request = request((4, 2), (2, 1), 1, 15);
        request.decode = Some([1., 0., 0., 1., 0., 1., 0., 1.]);
        let result = decode_target(&encoded, request, &OperationBudget::new(4096)).unwrap();
        let inverse: Vec<_> = source.iter().map(|v| 255 - v).collect();
        assert_eq!(
            result.samples.as_slice(),
            oracle(&inverse, (4, 2), (2, 1), 1)
        );
    }
    #[test]
    fn large_image_peak_is_target_samples_plus_rows_and_accumulators() {
        let source = vec![121; 1916 * 2665 * 3];
        let encoded = compress(&source);
        drop(source);
        let pool = OperationBudget::new(64 * 1024 * 1024);
        let preview =
            decode_target(&encoded, request((1916, 2665), (595, 842), 3, 1), &pool).unwrap();
        assert!(preview.samples.as_slice().iter().all(|v| *v == 121));
        assert!(pool.peak_bytes() < 2 * 1024 * 1024, "{}", pool.peak_bytes());
        let peak = pool.peak_bytes();
        drop(preview);
        assert_eq!(pool.live_bytes(), 0);
        let small = OperationBudget::new(peak - 1);
        assert!(matches!(
            decode_target(&encoded, request((1916, 2665), (595, 842), 3, 1), &small),
            Err(PdfError::ResourceLimitExceeded {
                reason: "image_operation_bytes",
                ..
            })
        ));
        assert_eq!(small.live_bytes(), 0);
    }
    #[test]
    fn truncation_checksum_excess_samples_and_invalid_png_filter_never_publish() {
        let encoded = compress(&[1, 2, 3, 4]);
        let mut checksum = encoded.clone();
        *checksum.last_mut().unwrap() ^= 1;
        let invalid = [
            encoded[..encoded.len() - 1].to_vec(),
            encoded[..encoded.len() - 4].to_vec(),
            checksum,
            compress(&[1, 2, 3]),
            compress(&[1, 2, 3, 4, 5]),
        ];
        for bytes in invalid {
            let pool = OperationBudget::new(4096);
            assert!(decode_target(&bytes, request((2, 2), (1, 1), 1, 1), &pool).is_err());
            assert_eq!(pool.live_bytes(), 0);
        }
        let pool = OperationBudget::new(4096);
        assert!(
            decode_target(
                &compress(&[5, 1, 2, 0, 3, 4]),
                request((2, 2), (1, 1), 1, 15),
                &pool
            )
            .is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
        let result = decode_target(&encoded, request((2, 2), (1, 1), 1, 1), &pool).unwrap();
        assert_eq!(result.samples.as_slice(), [3]);
    }
    #[test]
    fn compatible_eol_recovery_is_bounded_and_all_budgets_are_independent() {
        let bytes = compress(&[1, 2, 3, 4, 13, 10]);
        let pool = OperationBudget::new(4096);
        assert!(decode_target(&bytes, request((2, 2), (1, 1), 1, 1), &pool).is_err());
        let mut compatible = request((2, 2), (1, 1), 1, 1);
        compatible.compatible = true;
        assert!(
            decode_target(&bytes, compatible, &pool)
                .unwrap()
                .recovered_tail
        );
        for (source, target, components, pixels, decoded, reason) in [
            (
                (4001, 4000),
                (1, 1),
                1,
                4_000_000,
                16 * 1024 * 1024,
                "image_source_pixels",
            ),
            (
                (3000, 2000),
                (3000, 2000),
                3,
                4_000_000,
                32 * 1024 * 1024,
                "image_pixels",
            ),
            ((2, 2), (2, 2), 1, 4, 15, "decoded_bytes"),
            (
                (3000, 2000),
                (1, 1),
                3,
                4_000_000,
                16 * 1024 * 1024,
                "image_source_bytes",
            ),
        ] {
            let mut spec = request(source, target, components, 1);
            spec.limits.max_image_pixels = pixels;
            spec.limits.max_decoded_bytes = decoded;
            assert!(
                matches!(decode_target(&[],spec,&pool),Err(PdfError::ResourceLimitExceeded{reason:r,..})if r==reason)
            );
            assert_eq!(pool.live_bytes(), 0);
        }
    }
}
