//! Run-based area sampling of bilevel scans. Only target-sized samples and
//! two target rows are owned; decoder transition buffers have a scratch lease.
use super::Param;
use crate::{
    budget::{BudgetedBytes, OperationBudget},
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};

pub(crate) use crate::filter::bilevel_preview::Request;
use crate::filter::bilevel_preview::{Sampler, limit};
impl hayro_ccitt::Decoder for Sampler {
    fn push_pixels(&mut self, white: bool, count: u32) {
        self.push_run(white, count);
    }
    fn next_line(&mut self) {
        self.next_row();
    }
}

pub(crate) fn decode_target(
    input: &[u8],
    params: Option<&PdfDict>,
    request: Request,
    pool: &OperationBudget,
) -> PdfResult<BudgetedBytes> {
    let p = params.map(Param::try_new).transpose()?.unwrap_or_default();
    let (sw, sh) = request.source;
    let (tw, th) = request.target;
    if sw == 0
        || sh == 0
        || tw == 0
        || th == 0
        || tw > sw
        || th > sh
        || u32::from(p.columns) != sw
        || (p.rows != 0 && u32::from(p.rows) != sh)
        || p.damaged_rows_before_error != 0
        || request.decode.iter().any(|v| !v.is_finite())
    {
        return Err(PdfError::FilterError(
            "invalid CCITT preview layout or parameters".into(),
        ));
    }
    let scratch = sw as usize * 64 + 65536;
    let needed = pool.live_bytes().saturating_add(scratch);
    if needed > pool.maximum_bytes() {
        return Err(limit(
            "image_operation_bytes",
            pool.maximum_bytes() as u64,
            needed as u64,
        ));
    }
    let _scratch = pool.reserve(scratch)?;
    let mut sampler = Sampler::new(request, pool)?;
    let mut decoder = hayro_ccitt::DecoderContext::new(hayro_ccitt::DecodeSettings {
        columns: sw,
        rows: sh
            .checked_add(u32::from(p.end_of_block))
            .ok_or_else(|| PdfError::FilterError("CCITT row overflow".into()))?,
        end_of_block: p.end_of_block,
        end_of_line: p.end_of_line,
        rows_are_byte_aligned: p.encoded_byte_align,
        encoding: if p.k < 0 {
            hayro_ccitt::EncodingMode::Group4
        } else if p.k == 0 {
            hayro_ccitt::EncodingMode::Group3_1D
        } else {
            hayro_ccitt::EncodingMode::Group3_2D { k: p.k as u32 }
        },
        invert_black: p.black_is_1,
    });
    hayro_ccitt::decode(input, &mut sampler, &mut decoder)
        .map_err(|_| PdfError::FilterError("invalid or incomplete CCITT preview".into()))?;
    sampler.finish()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::parser::options::Limits;
    #[test]
    fn group3_one_and_two_dimensional_rows_reach_rtc_without_extra_rows() {
        use crate::objects::{PdfObject, pdf_number::PdfNumber};
        let eol = "000000000001";
        for (k, data) in [
            (0, format!("{eol}10011{eol}10011{}", eol.repeat(6))),
            (2, format!("{eol}110011{eol}01{}", eol.repeat(6))),
        ] {
            let mut bytes = vec![0u8; data.len().div_ceil(8)];
            for (index, bit) in data.bytes().enumerate() {
                bytes[index / 8] |= (bit - b'0') << (7 - index % 8);
            }
            let mut params = PdfDict::default();
            for (key, value) in [("K", k), ("Columns", 8), ("Rows", 2)] {
                params
                    .try_insert(
                        key.into(),
                        PdfObject::PdfNumber(PdfNumber::new(value as f32)),
                    )
                    .unwrap();
            }
            let pool = OperationBudget::new(128 * 1024);
            let request = Request {
                source: (8, 2),
                target: (1, 1),
                decode: [0., 1.],
                limits: Limits::default(),
            };
            let output = decode_target(&bytes, Some(&params), request, &pool).unwrap();
            assert_eq!(output.as_slice(), [255]);
            drop(output);
            assert_eq!(pool.live_bytes(), 0);
        }
    }
    #[test]
    fn group4_run_sampling_preserves_fractional_coverage_and_releases_failures() {
        use crate::objects::{PdfObject, pdf_number::PdfNumber};
        let mut encoder = fax::encoder::Encoder::new(fax::VecWriter::new());
        for _ in 0..3500 {
            encoder
                .encode_line(
                    (0..2500).map(|x| {
                        if x < 1250 {
                            fax::Color::Black
                        } else {
                            fax::Color::White
                        }
                    }),
                    2500,
                )
                .unwrap();
        }
        let bytes = encoder.finish().unwrap().finish();
        let mut params = PdfDict::default();
        for (key, value) in [("K", -1), ("Columns", 2500), ("Rows", 3500)] {
            params
                .try_insert(
                    key.into(),
                    PdfObject::PdfNumber(PdfNumber::new(value as f32)),
                )
                .unwrap();
        }
        let pool = OperationBudget::new(2 * 1024 * 1024);
        let spec = || Request {
            source: (2500, 3500),
            target: (1, 1),
            decode: [0., 1.],
            limits: Limits {
                max_image_pixels: 4_000_000,
                max_decoded_bytes: 16 * 1024 * 1024,
                ..Default::default()
            },
        };
        let output = decode_target(&bytes, Some(&params), spec(), &pool).unwrap();
        assert_eq!(output.as_slice(), [128]);
        assert!(pool.peak_bytes() < 256 * 1024);
        drop(output);
        assert_eq!(pool.live_bytes(), 0);
        assert!(decode_target(&bytes[..bytes.len() / 2], Some(&params), spec(), &pool).is_err());
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(
            decode_target(&bytes, Some(&params), spec(), &pool)
                .unwrap()
                .as_slice(),
            [128]
        );
    }
}
