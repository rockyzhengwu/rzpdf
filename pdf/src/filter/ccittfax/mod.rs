pub(crate) mod preview;
use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};
use fax::Color;
use fax::decoder::{DecodeStatus, Group3Decoder, decode_g4, pels};

// Production decoding uses the bounded fax backend below. The small legacy
// bit-reader remains only as a unit-test reference.
#[cfg(test)]
mod bitreader;

pub struct Param {
    k: i32,
    columns: u16,
    rows: u16,
    end_of_line: bool,
    encoded_byte_align: bool,
    end_of_block: bool,
    black_is_1: bool,
    damaged_rows_before_error: u16,
}
impl Default for Param {
    fn default() -> Self {
        Param {
            k: 0,
            columns: 1728,
            rows: 0,
            end_of_line: false,
            end_of_block: true,
            encoded_byte_align: false,
            black_is_1: false,
            damaged_rows_before_error: 0,
        }
    }
}
impl Param {
    pub fn try_new(dict: &PdfDict) -> PdfResult<Self> {
        let mut param = Param::default();
        if let Some(k) = dict.get("K") {
            let number = k.as_number().ok_or(PdfError::FilterError(
                "CCITTFaxDecode K must be a number".to_string(),
            ))?;
            if !number.is_integer() {
                return Err(PdfError::FilterError(
                    "CCITTFaxDecode K must be an integer".into(),
                ));
            }
            param.k = number.lexeme().parse::<i32>().map_err(|_| {
                PdfError::FilterError("CCITTFaxDecode K outside signed integer range".into())
            })?;
        }
        if let Some(cl) = dict.get("Columns") {
            param.columns = cl.as_u16().ok_or(PdfError::FilterError(
                "CCITTFaxDecode Columns must be a number".to_string(),
            ))?;
        }
        if let Some(rw) = dict.get("Rows") {
            param.rows = rw.as_u16().ok_or(PdfError::FilterError(
                "CCITTFaxDecode Rows must be a number".to_string(),
            ))?;
        }
        if let Some(black) = dict.get("BlackIs1") {
            param.black_is_1 = black
                .as_bool()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode BlackIs1 must be a boolean".to_string(),
                ))?
                .value();
        }
        if let Some(eol) = dict.get("EndOfLine") {
            param.end_of_line = eol
                .as_bool()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode EndOfLine must be a boolean".to_string(),
                ))?
                .value();
        }
        if let Some(eob) = dict.get("EndOfBlock") {
            param.end_of_block = eob
                .as_bool()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode EndOfBlock must be a boolean".to_string(),
                ))?
                .value();
        }
        if let Some(aligned) = dict.get("EncodedByteAlign") {
            param.encoded_byte_align = aligned
                .as_bool()
                .ok_or(PdfError::FilterError(
                    "CCITTFaxDecode EncodedByteAlign must be a boolean".to_string(),
                ))?
                .value();
        }
        if let Some(rows) = dict.get("DamagedRowsBeforeError") {
            param.damaged_rows_before_error = rows.as_u16().ok_or(PdfError::FilterError(
                "CCITTFaxDecode DamagedRowsBeforeError must be a number".to_string(),
            ))?;
        }
        Ok(param)
    }
}

pub fn ccittfax_decode(buf: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    ccittfax_decode_limited(
        buf,
        params,
        crate::parser::options::Limits::default().max_decoded_bytes,
    )
}

pub fn ccittfax_decode_limited(
    buf: &[u8],
    params: Option<&PdfDict>,
    limit: usize,
) -> PdfResult<Vec<u8>> {
    decode_inner(buf, params, limit, None, &mut None, None)
}

/// Output capacity is charged; fax decoder transition buffers are excluded.
pub fn ccittfax_decode_with_budget(
    buf: &[u8],
    params: Option<&PdfDict>,
    limit: usize,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    let mut lease = Some(budget.reserve(0)?);
    let data = decode_inner(buf, params, limit, Some(budget), &mut lease, None)?;
    crate::budget::BudgetedBytes::from_reserved_vec(data, lease.unwrap())
}

pub(crate) fn decode_context(
    buf: &[u8],
    params: Option<&PdfDict>,
    context: super::FilterContext<'_>,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<(Vec<u8>, Option<crate::budget::MemoryLease>)> {
    let mut lease = budget.map(|pool| pool.reserve(0)).transpose()?;
    let bytes = decode_inner(
        buf,
        params,
        context.max_decoded_bytes,
        budget,
        &mut lease,
        Some(context),
    )?;
    Ok((bytes, lease))
}

fn decode_inner(
    buf: &[u8],
    params: Option<&PdfDict>,
    limit: usize,
    budget: Option<&crate::budget::OperationBudget>,
    lease: &mut Option<crate::budget::MemoryLease>,
    context: Option<super::FilterContext<'_>>,
) -> PdfResult<Vec<u8>> {
    let mut p = match params {
        Some(d) => Param::try_new(d)?,
        None => Param::default(),
    };
    let known_height = context
        .and_then(|c| c.stream_dict)
        .and_then(|d| d.get("Height"))
        .and_then(|v| v.as_u16());
    if p.rows == 0 {
        p.rows = known_height.unwrap_or(0);
    }
    let width = p.columns;
    if width == 0 {
        return Err(PdfError::FilterError(
            "CCITT Columns must be positive".into(),
        ));
    }
    super::check_output_size(0, usize::from(width), limit, "CCITTFaxDecode")?;
    if p.rows > 0 {
        super::check_output_size(
            0,
            usize::from(width) * usize::from(p.rows),
            limit,
            "CCITTFaxDecode",
        )?;
    }
    let height = if p.rows > 0 { Some(p.rows) } else { None };
    let mut res = Vec::new();
    let exceeded = std::cell::Cell::new(false);
    let mut append_error = None;
    if p.k < 0 {
        let consumed = std::cell::Cell::new(0usize);
        let decoded = decode_g4(
            buf.iter()
                .copied()
                .inspect(|_| consumed.set(consumed.get() + 1))
                .take_while(|_| !exceeded.get()),
            width,
            height,
            |line| {
                if super::check_output_size(res.len(), usize::from(width), limit, "CCITTFaxDecode")
                    .is_err()
                {
                    exceeded.set(true);
                } else {
                    if let Err(error) =
                        append_line(&mut res, line, p.columns, p.black_is_1, budget, lease)
                    {
                        append_error = Some(error);
                        exceeded.set(true);
                    }
                }
            },
        );
        if let Some(error) = append_error {
            return Err(error);
        }
        if exceeded.get() {
            return Err(PdfError::ResourceLimit("CCITT output size".into()));
        }
        if decoded.is_none() {
            let actual = res.len() / usize::from(width);
            let missing = usize::from(p.rows).saturating_sub(actual);
            // Recover only physical EOF and a small tail of a known image.
            // Invalid codes before EOF and substantial damage stay fatal.
            let recover = context.is_some_and(|c| c.compatible)
                && known_height == Some(p.rows)
                && consumed.get() == buf.len()
                && actual > 0
                && (1..=4).contains(&missing)
                && missing * 100 <= usize::from(p.rows);
            if !recover {
                return Err(PdfError::FilterError(
                    "CCITTFaxDecode Group 4 decoding failed".into(),
                ));
            }
            let additional = missing * usize::from(width);
            super::check_output_size(res.len(), additional, limit, "CCITTFaxDecode")?;
            for _ in 0..missing {
                append_line(&mut res, &[], width, p.black_is_1, budget, lease)?;
            }
            if let Some(flag) = context.and_then(|c| c.recovered) {
                flag.set(true);
            }
        }
        return Ok(res);
    }
    if p.k == 0 {
        let decoded = decode_g3_complete(
            buf.iter().copied().take_while(|_| !exceeded.get()),
            |line| {
                if super::check_output_size(res.len(), usize::from(width), limit, "CCITTFaxDecode")
                    .is_err()
                {
                    exceeded.set(true);
                } else {
                    if let Err(error) =
                        append_line(&mut res, line, p.columns, p.black_is_1, budget, lease)
                    {
                        append_error = Some(error);
                        exceeded.set(true);
                    }
                }
            },
        );
        if let Some(error) = append_error {
            return Err(error);
        }
        if exceeded.get() {
            return Err(PdfError::ResourceLimit("CCITT output size".into()));
        }
        decoded.ok_or(PdfError::FilterError(
            "CCITTFaxDecode Group 3 1D decoding failed".to_string(),
        ))?;
        if let Some(expected_rows) = height {
            let actual_rows = res.len() / p.columns as usize;
            if actual_rows != expected_rows as usize {
                return Err(PdfError::FilterError(format!(
                    "CCITTFaxDecode decoded {actual_rows} rows but expected {expected_rows}"
                )));
            }
        }
        return Ok(res);
    }

    Err(PdfError::FilterError(format!(
        "CCITTFaxDecode mixed Group 3 2D mode (K={}) is not implemented",
        p.k
    )))
}

// The dependency's convenience decoder stops before delivering a nonempty
// final row when its terminating EOL also starts the return-to-control marker.
fn decode_g3_complete(input: impl Iterator<Item = u8>, mut row: impl FnMut(&[u16])) -> Option<()> {
    let mut decoder = Group3Decoder::new(input.map(Ok::<_, std::convert::Infallible>)).ok()?;
    loop {
        let status = decoder.advance().ok()?;
        if !decoder.transitions().is_empty() {
            row(decoder.transitions());
        }
        if status == DecodeStatus::End {
            return Some(());
        }
    }
}

fn append_line(
    output: &mut Vec<u8>,
    line: &[u16],
    width: u16,
    black_is_1: bool,
    budget: Option<&crate::budget::OperationBudget>,
    lease: &mut Option<crate::budget::MemoryLease>,
) -> PdfResult<()> {
    let required = output
        .len()
        .checked_add(usize::from(width))
        .ok_or_else(|| PdfError::ResourceLimit("CCITT output length".into()))?;
    if let Some(budget) = budget {
        if required > output.capacity() {
            let mut new_lease = budget.reserve(required)?;
            let mut replacement = Vec::new();
            replacement
                .try_reserve_exact(required)
                .map_err(|_| PdfError::ResourceLimit("CCITT output allocation".into()))?;
            new_lease.resize(replacement.capacity())?;
            replacement.extend_from_slice(output);
            *output = replacement;
            *lease = Some(new_lease);
        }
    } else {
        output
            .try_reserve(usize::from(width))
            .map_err(|_| PdfError::ResourceLimit("CCITT output allocation".into()))?;
    }
    output.extend(pels(line, width).map(|color| match (color, black_is_1) {
        (Color::White, false) => 255,
        (Color::Black, false) => 0,
        (Color::White, true) => 0,
        (Color::Black, true) => 255,
    }));
    Ok(())
}

#[cfg(test)]
mod tests {
    use crate::objects::{PdfObject, pdf_bool::PdfBool, pdf_dict::PdfDict, pdf_number::PdfNumber};

    use super::ccittfax_decode;

    #[test]
    fn group3_delivers_final_row_before_rtc_and_charges_growth() {
        let eol = "000000000001";
        let bits = format!("{eol}10011{eol}10011{}", eol.repeat(6));
        let mut encoded = vec![0u8; bits.len().div_ceil(8)];
        for (offset, bit) in bits.bytes().enumerate() {
            encoded[offset / 8] |= (bit - b'0') << (7 - offset % 8);
        }
        let mut params = PdfDict::default();
        params.insert("Columns".into(), PdfObject::PdfNumber(PdfNumber::new(8.0)));
        params.insert("Rows".into(), PdfObject::PdfNumber(PdfNumber::new(2.0)));
        assert_eq!(
            ccittfax_decode(&encoded, Some(&params)).unwrap(),
            vec![255; 16]
        );
        let budget = crate::budget::OperationBudget::new(24);
        let neighbor = budget.reserve(1).unwrap();
        assert!(super::ccittfax_decode_with_budget(&encoded, Some(&params), 16, &budget).is_err());
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let result =
            super::ccittfax_decode_with_budget(&encoded, Some(&params), 16, &budget).unwrap();
        assert_eq!(result.as_slice(), [255; 16]);
        assert_eq!(budget.live_bytes(), result.capacity());
        drop(result);
        assert_eq!(budget.live_bytes(), 0);
        params.insert("Rows".into(), PdfObject::PdfNumber(PdfNumber::new(3.0)));
        assert!(matches!(
            super::ccittfax_decode_with_budget(&encoded, Some(&params), 24, &budget),
            Err(crate::error::PdfError::FilterError(_))
        ));
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn k_parameter_is_not_truncated_to_a_signed_byte() {
        let mut dict = PdfDict::default();
        dict.insert(
            "K".into(),
            PdfObject::PdfNumber(PdfNumber::new_from_lexeme(b"255").unwrap()),
        );
        assert_eq!(super::Param::try_new(&dict).unwrap().k, 255);
        for value in [b"1.5".as_slice(), b"4294967295"] {
            dict.insert(
                "K".into(),
                PdfObject::PdfNumber(PdfNumber::new_from_lexeme(value).unwrap()),
            );
            assert!(super::Param::try_new(&dict).is_err());
        }
    }

    #[test]
    fn test_decode_g4() {
        let width = 17u16;
        let rows = 4u16;
        let mut encoder = fax::encoder::Encoder::new(fax::VecWriter::new());
        let mut expected = Vec::new();
        for row in 0..rows {
            let pixels: Vec<_> = (0..width)
                .map(|column| {
                    if (row + column / 3) % 2 == 0 {
                        fax::Color::White
                    } else {
                        fax::Color::Black
                    }
                })
                .collect();
            expected.extend(
                pixels
                    .iter()
                    .map(|color| if *color == fax::Color::White { 255 } else { 0 }),
            );
            encoder.encode_line(pixels.into_iter(), width).unwrap();
        }
        let buffer = encoder.finish().unwrap().finish();
        let mut pd = PdfDict::default();
        pd.insert(
            "Columns".to_string(),
            PdfObject::PdfNumber(PdfNumber::new(f32::from(width))),
        );
        pd.insert(
            "Rows".into(),
            PdfObject::PdfNumber(PdfNumber::new(f32::from(rows))),
        );
        pd.insert("K".to_string(), PdfObject::PdfNumber(PdfNumber::new(-1.0)));
        let images = ccittfax_decode(&buffer, Some(&pd)).unwrap();
        assert_eq!(images, expected);
        let peak = usize::from(width) * (usize::from(rows) * 2 - 1);
        let budget = crate::budget::OperationBudget::new(peak);
        pd.insert(
            "Rows".into(),
            PdfObject::PdfNumber(PdfNumber::new(f32::from(rows + 1))),
        );
        let mut padded = expected.clone();
        padded.extend(std::iter::repeat_n(255, usize::from(width)));
        assert_eq!(ccittfax_decode(&buffer, Some(&pd)).unwrap(), padded);
        let padded_budget = crate::budget::OperationBudget::new(
            usize::from(width) * (usize::from(rows + 1) * 2 - 1),
        );
        let padded_result = super::ccittfax_decode_with_budget(
            &buffer,
            Some(&pd),
            usize::from(width) * usize::from(rows + 1),
            &padded_budget,
        )
        .unwrap();
        assert_eq!(padded_result.as_slice(), padded);
        assert_eq!(padded_budget.live_bytes(), padded_result.capacity());
        drop(padded_result);
        assert_eq!(padded_budget.live_bytes(), 0);
        assert_eq!(budget.live_bytes(), 0);
        pd.insert(
            "Rows".into(),
            PdfObject::PdfNumber(PdfNumber::new(f32::from(rows))),
        );
        let neighbor = budget.reserve(1).unwrap();
        assert!(matches!(
            super::ccittfax_decode_with_budget(&buffer, Some(&pd), expected.len(), &budget),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        for name in ["CCF", "CCITTFaxDecode"] {
            let decoded = crate::filter::apply_filter_with_budget(
                name,
                &buffer,
                Some(&pd),
                crate::filter::FilterContext::default(),
                &budget,
            )
            .unwrap();
            assert_eq!(decoded.as_slice(), expected);
            assert_eq!(budget.live_bytes(), decoded.capacity());
            drop(decoded);
            assert_eq!(budget.live_bytes(), 0);
        }
        assert!(
            super::ccittfax_decode_with_budget(&buffer, Some(&pd), expected.len() - 1, &budget)
                .is_err()
        );
        assert_eq!(budget.live_bytes(), 0);
        pd.insert("BlackIs1".into(), PdfObject::PdfBool(PdfBool::new(b"true")));
        assert_eq!(
            ccittfax_decode(&buffer, Some(&pd)).unwrap(),
            expected.iter().map(|value| 255 - value).collect::<Vec<_>>()
        );
    }
}

#[cfg(test)]
mod tail_recovery_tests {
    use super::*;
    use crate::objects::{PdfObject, pdf_number::PdfNumber};
    #[test]
    fn only_short_physical_eof_tail_of_known_image_is_recovered() {
        let mut params = PdfDict::default();
        params.insert("K".into(), PdfObject::PdfNumber(PdfNumber::new(-1.)));
        params.insert("Columns".into(), PdfObject::PdfNumber(PdfNumber::new(8.)));
        let mut image = PdfDict::default();
        image.insert("Height".into(), PdfObject::PdfNumber(PdfNumber::new(1022.)));
        let recovered = std::cell::Cell::new(false);
        let context = super::super::FilterContext {
            stream_dict: Some(&image),
            compatible: true,
            recovered: Some(&recovered),
            ..Default::default()
        };
        // Group 4 V(0) repeats the preceding white row, one bit per row.
        let input = [255; 128];
        assert!(ccittfax_decode_limited(&input, Some(&params), 8176).is_err());
        let (bytes, _) = decode_context(&input, Some(&params), context, None).unwrap();
        assert_eq!(bytes, [255; 8176]);
        assert!(recovered.get());
        assert!(decode_context(&input[..127], Some(&params), context, None).is_err());
        assert!(
            decode_context(
                &input,
                Some(&params),
                super::super::FilterContext {
                    compatible: false,
                    ..context
                },
                None
            )
            .is_err()
        );
        assert!(
            decode_context(
                &input,
                Some(&params),
                super::super::FilterContext {
                    max_decoded_bytes: 8175,
                    ..context
                },
                None
            )
            .is_err()
        );
        let pool = crate::budget::OperationBudget::new(32768);
        let (bytes, lease) = decode_context(&input, Some(&params), context, Some(&pool)).unwrap();
        assert_eq!(pool.live_bytes(), bytes.capacity());
        drop(bytes);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
    }
}

/// Determine Group 4 framing from decoded row/EOB boundaries, never EI bytes.
/// A counting sink avoids allocating a bitmap during inline-image framing.
pub(crate) fn encoded_length(
    input: &[u8],
    params: Option<&PdfDict>,
    width: u32,
    height: u32,
    limits: crate::parser::options::Limits,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<usize> {
    let p = params.map(Param::try_new).transpose()?.unwrap_or_default();
    if p.k >= 0 {
        return Err(PdfError::UnsupportedFeature(
            "direct inline CCITT Group 3 framing".into(),
        ));
    }
    if width == 0
        || height == 0
        || u32::from(p.columns) != width
        || (p.rows != 0 && u32::from(p.rows) != height)
    {
        return Err(PdfError::FilterError(
            "inline CCITT dimensions disagree with parameters".into(),
        ));
    }
    if u64::from(width) * u64::from(height) > limits.max_image_pixels {
        return Err(PdfError::ResourceLimit("inline CCITT pixels".into()));
    }
    let scratch = (width as usize)
        .checked_mul(64)
        .and_then(|n| n.checked_add(4096))
        .filter(|n| *n <= limits.max_decoded_bytes)
        .ok_or_else(|| PdfError::ResourceLimit("inline CCITT framing scratch".into()))?;
    let _lease = budget.reserve(scratch)?;
    struct Counter {
        width: u32,
        pixels: u32,
        rows: u32,
        invalid: bool,
    }
    impl hayro_ccitt::Decoder for Counter {
        fn push_pixels(&mut self, _white: bool, count: u32) {
            self.pixels = self.pixels.saturating_add(count);
            self.invalid |= self.pixels > self.width;
        }
        fn next_line(&mut self) {
            self.invalid |= self.pixels != self.width;
            self.rows = self.rows.saturating_add(1);
            self.pixels = 0;
        }
    }
    let mut counter = Counter {
        width,
        pixels: 0,
        rows: 0,
        invalid: false,
    };
    let rows = height
        .checked_add(u32::from(p.end_of_block))
        .ok_or_else(|| PdfError::ResourceLimit("inline CCITT row count".into()))?;
    let mut decoder = hayro_ccitt::DecoderContext::new(hayro_ccitt::DecodeSettings {
        columns: width,
        rows,
        end_of_block: p.end_of_block,
        end_of_line: p.end_of_line,
        rows_are_byte_aligned: p.encoded_byte_align,
        encoding: hayro_ccitt::EncodingMode::Group4,
        invert_black: p.black_is_1,
    });
    let consumed = hayro_ccitt::decode(input, &mut counter, &mut decoder).map_err(|_| {
        PdfError::FilterError("inline CCITT incomplete or invalid coded rows".into())
    })?;
    if counter.invalid || counter.rows != height || consumed == 0 || consumed > input.len() {
        return Err(PdfError::FilterError(
            "inline CCITT row count mismatch".into(),
        ));
    }
    Ok(consumed)
}
