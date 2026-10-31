//! Area coverage for bilevel image previews. Codec storage remains separate.
use crate::{
    budget::{BudgetedBytes, MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    parser::options::Limits,
};
#[derive(Clone, Copy)]
pub(crate) struct Request {
    pub source: (u32, u32),
    pub target: (u32, u32),
    pub decode: [f64; 2],
    pub limits: Limits,
}
pub(crate) fn limit(reason: &'static str, limit: u64, actual: u64) -> PdfError {
    PdfError::ResourceLimitExceeded {
        reason,
        stage: "image_decode",
        limit,
        actual,
    }
}
pub(crate) struct Sampler {
    source: (u32, u32),
    target: (u32, u32),
    x: u32,
    y: u32,
    output_y: u32,
    horizontal: Vec<u64>,
    accumulated: Vec<u64>,
    output: BudgetedBytes,
    values: [u64; 2],
    invalid: bool,
    _row_leases: [MemoryLease; 2],
}
impl Sampler {
    pub(crate) fn push_run(&mut self, white: bool, count: u32) {
        let end = self.x.saturating_add(count);
        if end > self.source.0 || self.y >= self.source.1 {
            self.invalid = true;
            return;
        }
        let left = u64::from(self.x) * u64::from(self.target.0);
        let right = u64::from(end) * u64::from(self.target.0);
        let stride = u64::from(self.source.0);
        for column in left / stride..right.div_ceil(stride) {
            let weight = right.min((column + 1) * stride) - left.max(column * stride);
            self.horizontal[column as usize] += weight * self.values[usize::from(white)];
        }
        self.x = end;
    }
    pub(crate) fn next_row(&mut self) {
        if self.invalid || self.x != self.source.0 || self.y >= self.source.1 {
            self.invalid = true;
            return;
        }
        let mut cursor = u64::from(self.y) * u64::from(self.target.1);
        let end = (u64::from(self.y) + 1) * u64::from(self.target.1);
        while cursor < end {
            let boundary = (u64::from(self.output_y) + 1) * u64::from(self.source.1);
            let stop = end.min(boundary);
            for (sum, value) in self.accumulated.iter_mut().zip(&self.horizontal) {
                *sum += value * (stop - cursor);
            }
            cursor = stop;
            if stop == boundary {
                let denominator = u64::from(self.source.0) * u64::from(self.source.1);
                let offset = self.output_y as usize * self.target.0 as usize;
                for (index, sum) in self.accumulated.iter_mut().enumerate() {
                    self.output.as_mut_slice()[offset + index] =
                        ((*sum + denominator / 2) / denominator) as u8;
                    *sum = 0;
                }
                self.output_y += 1;
            }
        }
        self.horizontal.fill(0);
        self.x = 0;
        self.y += 1;
    }
}

impl Sampler {
    pub(crate) fn new(request: Request, pool: &OperationBudget) -> PdfResult<Self> {
        let (sw, sh) = request.source;
        let (tw, th) = request.target;
        if sw == 0
            || sh == 0
            || tw == 0
            || th == 0
            || tw > sw
            || th > sh
            || request.decode.iter().any(|v| !v.is_finite())
        {
            return Err(PdfError::FilterError(
                "invalid bilevel preview layout".into(),
            ));
        }
        let source = u64::from(sw) * u64::from(sh);
        let target = u64::from(tw) * u64::from(th);
        for (reason, actual, maximum) in [
            (
                "image_source_bytes",
                source,
                request.limits.max_image_source_bytes as u64,
            ),
            ("image_pixels", target, request.limits.max_image_pixels),
            (
                "decoded_bytes",
                target.saturating_mul(4),
                request.limits.max_decoded_bytes as u64,
            ),
        ] {
            if actual > maximum {
                return Err(limit(reason, maximum, actual));
            }
        }
        let needed = pool
            .live_bytes()
            .saturating_add(target as usize)
            .saturating_add(tw as usize * 16);
        if needed > pool.maximum_bytes() {
            return Err(limit(
                "image_operation_bytes",
                pool.maximum_bytes() as u64,
                needed as u64,
            ));
        }
        let (horizontal, h) = super::flate_image::zeros_u64(pool, tw as usize)?;
        let (accumulated, a) = super::flate_image::zeros_u64(pool, tw as usize)?;
        Ok(Self {
            source: request.source,
            target: request.target,
            x: 0,
            y: 0,
            output_y: 0,
            horizontal,
            accumulated,
            output: BudgetedBytes::zeroed(pool, target as usize)?,
            values: request
                .decode
                .map(|v| (v.clamp(0., 1.) * 255.).round() as u64),
            invalid: false,
            _row_leases: [h, a],
        })
    }
    /// Packed PDF samples use white=1. Ignore row padding rather than sampling it.
    pub(crate) fn push_packed(&mut self, data: &[u8]) -> PdfResult<()> {
        let stride = (self.source.0 as usize).div_ceil(8);
        if stride.checked_mul(self.source.1 as usize) != Some(data.len()) {
            return Err(PdfError::FilterError(
                "bilevel packed row length mismatch".into(),
            ));
        }
        for row in data.chunks_exact(stride) {
            let mut x = 0u32;
            while x < self.source.0 {
                let white = row[x as usize / 8] & (128 >> (x % 8)) != 0;
                let start = x;
                x += 1;
                while x < self.source.0 {
                    if x.is_multiple_of(8)
                        && x + 8 <= self.source.0
                        && row[x as usize / 8] == if white { 255 } else { 0 }
                    {
                        x += 8;
                        continue;
                    }
                    if (row[x as usize / 8] & (128 >> (x % 8)) != 0) != white {
                        break;
                    }
                    x += 1;
                }
                self.push_run(white, x - start);
            }
            self.next_row();
        }
        Ok(())
    }
    pub(crate) fn finish(self) -> PdfResult<BudgetedBytes> {
        if self.invalid || self.x != 0 || self.y != self.source.1 || self.output_y != self.target.1
        {
            return Err(PdfError::FilterError(
                "bilevel preview row count mismatch".into(),
            ));
        }
        Ok(self.output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    fn request(source: (u32, u32), target: (u32, u32)) -> Request {
        Request {
            source,
            target,
            decode: [0., 1.],
            limits: Limits::default(),
        }
    }
    #[test]
    fn area_sampling_retains_thin_strokes_decode_and_ignores_row_padding() {
        let pool = OperationBudget::new(1024);
        for (source, target, packed, expected) in [
            ((8, 8), (1, 1), vec![127; 8], vec![223]),
            ((9, 2), (3, 1), vec![255, 255, 0, 127], vec![128; 3]),
            ((3, 3), (2, 2), vec![224, 160, 224], vec![227; 4]),
        ] {
            let mut sampler = Sampler::new(request(source, target), &pool).unwrap();
            sampler.push_packed(&packed).unwrap();
            let output = sampler.finish().unwrap();
            assert_eq!(output.as_slice(), expected);
            drop(output);
            assert_eq!(pool.live_bytes(), 0);
        }
        let mut spec = request((8, 8), (1, 1));
        spec.decode = [1., 0.];
        let mut sampler = Sampler::new(spec, &pool).unwrap();
        sampler.push_packed(&[127; 8]).unwrap();
        assert_eq!(sampler.finish().unwrap().as_slice(), [32]);
    }
    #[test]
    fn row_and_target_storage_share_one_pool_and_bad_layout_never_publishes() {
        let pool = OperationBudget::new(35);
        assert!(matches!(
            Sampler::new(request((3, 3), (2, 2)), &pool),
            Err(PdfError::ResourceLimitExceeded {
                reason: "image_operation_bytes",
                limit: 35,
                actual: 36,
                ..
            })
        ));
        assert_eq!(pool.live_bytes(), 0);
        let pool = OperationBudget::new(36);
        let mut sampler = Sampler::new(request((3, 3), (2, 2)), &pool).unwrap();
        assert!(sampler.push_packed(&[255]).is_err());
        assert!(sampler.finish().is_err());
        assert_eq!(pool.live_bytes(), 0);
    }
}
