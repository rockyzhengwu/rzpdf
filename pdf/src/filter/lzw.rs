use crate::{
    error::{PdfError, PdfResult},
    filter::predictor::apply_predictor,
    objects::pdf_dict::PdfDict,
};

pub fn lzw_decode(input: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    lzw_decode_limited(
        input,
        params,
        crate::parser::options::Limits::default().max_decoded_bytes,
    )
}

pub fn lzw_decode_limited(
    input: &[u8],
    params: Option<&PdfDict>,
    limit: usize,
) -> PdfResult<Vec<u8>> {
    decode_inner(input, params, limit, None, &mut None)
}

/// Output capacity and replacement overlap are charged; fixed dictionary
/// and expansion stack storage (16 KiB) are excluded from heap accounting.
pub fn lzw_decode_with_budget(
    input: &[u8],
    params: Option<&PdfDict>,
    limit: usize,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    let mut lease = Some(budget.reserve(0)?);
    let data = decode_inner(input, params, limit, Some(budget), &mut lease)?;
    crate::budget::BudgetedBytes::from_reserved_vec(data, lease.unwrap())
}

fn decode_inner(
    input: &[u8],
    params: Option<&PdfDict>,
    limit: usize,
    budget: Option<&crate::budget::OperationBudget>,
    lease: &mut Option<crate::budget::MemoryLease>,
) -> PdfResult<Vec<u8>> {
    let early_change = match params.and_then(|dict| dict.get("EarlyChange")) {
        None => 1,
        Some(value) => value.as_i32().ok_or_else(|| {
            PdfError::FilterError("LZWDecode EarlyChange must be an integer".into())
        })?,
    };
    if !(0..=1).contains(&early_change) {
        return Err(PdfError::FilterError(format!(
            "LZWDecode EarlyChange must be 0 or 1, got {early_change}"
        )));
    }

    // Fixed prefix/suffix dictionary avoids thousands of heap entries and
    // per-code clones. Entries below next_code alone are considered valid.
    let mut prefixes = [0u16; 4096];
    let mut suffixes = [0u8; 4096];
    let mut stack = [0u8; 4096];
    let mut result = Vec::new();
    let mut code_size = 9_u8;
    let mut next_code: u16 = 258;
    let mut previous: Option<u16> = None;
    let mut reader = BitReader::new(input);

    while let Some(code) = reader.read_bits(code_size) {
        if code == 256 {
            next_code = 258;
            code_size = 9;
            previous = None;
            continue;
        }
        if code == 257 {
            break;
        }

        let forward = code == next_code;
        if code > next_code || (forward && previous.is_none()) {
            return Err(PdfError::FilterError(format!(
                "LZWDecode encountered unknown code: {code}"
            )));
        }
        let mut entry_code = if forward { previous.unwrap() } else { code };
        let mut length = 0usize;
        while entry_code >= 258 {
            if length >= stack.len() - 1 || entry_code >= next_code {
                return Err(PdfError::FilterError(
                    "LZWDecode invalid prefix chain".into(),
                ));
            }
            stack[length] = suffixes[entry_code as usize];
            length += 1;
            entry_code = prefixes[entry_code as usize];
        }
        if entry_code > 255 {
            return Err(PdfError::FilterError("LZWDecode invalid literal".into()));
        }
        let first = entry_code as u8;
        stack[length] = first;
        length += 1;
        let output_length = length + usize::from(forward);
        super::check_output_size(result.len(), output_length, limit, "LZWDecode")?;
        if let Some(budget) = budget {
            let required = result.len() + output_length;
            if required > result.capacity() {
                let mut new_lease = budget.reserve(required)?;
                let mut replacement = Vec::new();
                replacement
                    .try_reserve_exact(required)
                    .map_err(|_| PdfError::ResourceLimit("LZWDecode output allocation".into()))?;
                new_lease.resize(replacement.capacity())?;
                replacement.extend_from_slice(&result);
                result = replacement;
                *lease = Some(new_lease);
            }
        } else {
            result
                .try_reserve(output_length)
                .map_err(|_| PdfError::ResourceLimit("LZWDecode output allocation".into()))?;
        }
        result.extend(stack[..length].iter().rev().copied());
        if forward {
            result.push(first);
        }

        if let Some(prev) = previous
            && next_code < 4096
        {
            prefixes[next_code as usize] = prev;
            suffixes[next_code as usize] = first;
            next_code += 1;
            while code_size < 12 && next_code + early_change as u16 > (1_u16 << code_size) {
                code_size += 1;
            }
        }
        previous = Some(code);
    }

    apply_predictor(result, params, "LZWDecode")
}

struct BitReader<'a> {
    data: &'a [u8],
    bit_index: usize,
}

impl<'a> BitReader<'a> {
    fn new(data: &'a [u8]) -> Self {
        Self { data, bit_index: 0 }
    }

    fn read_bits(&mut self, count: u8) -> Option<u16> {
        if count == 0 {
            return Some(0);
        }
        let mut value = 0_u16;
        for _ in 0..count {
            let byte = *self.data.get(self.bit_index / 8)?;
            let shift = 7 - (self.bit_index % 8);
            value = (value << 1) | ((byte >> shift) & 1) as u16;
            self.bit_index += 1;
        }
        Some(value)
    }
}

#[cfg(test)]
mod tests {
    use super::lzw_decode;

    #[test]
    fn literal_stream_crosses_all_widths_saturates_table_and_clears() {
        use crate::{
            budget::OperationBudget,
            objects::{PdfObject, pdf_dict::PdfDict, pdf_number::PdfNumber},
        };
        fn write_code(bytes: &mut Vec<u8>, offset: &mut usize, code: u16, width: usize) {
            for bit in (0..width).rev() {
                if *offset / 8 == bytes.len() {
                    bytes.push(0);
                }
                bytes[*offset / 8] |= (((code >> bit) & 1) as u8) << (7 - *offset % 8);
                *offset += 1;
            }
        }
        for early in [0usize, 1] {
            let mut encoded = Vec::new();
            let mut offset = 0;
            write_code(&mut encoded, &mut offset, 256, 9);
            let mut expected = Vec::new();
            for ordinal in 1..=4200 {
                // Explicit literal ordinals, independent of decoder next_code.
                let width = if ordinal < 257 - early {
                    9
                } else if ordinal < 769 - early {
                    10
                } else if ordinal < 1793 - early {
                    11
                } else {
                    12
                };
                let literal = ((ordinal - 1) % 251) as u8;
                write_code(&mut encoded, &mut offset, u16::from(literal), width);
                expected.push(literal);
            }
            write_code(&mut encoded, &mut offset, 4095, 12);
            expected.extend_from_slice(&[((3838 - 1) % 251) as u8, ((3839 - 1) % 251) as u8]);
            write_code(&mut encoded, &mut offset, 256, 12);
            write_code(&mut encoded, &mut offset, 90, 9);
            write_code(&mut encoded, &mut offset, 257, 9);
            expected.push(90);
            let mut params = PdfDict::default();
            params.insert(
                "EarlyChange".into(),
                PdfObject::PdfNumber(PdfNumber::new(early as f32)),
            );
            assert_eq!(
                super::lzw_decode_limited(&encoded, Some(&params), expected.len()).unwrap(),
                expected
            );
            let budget = OperationBudget::new(expected.len() * 2);
            let result =
                super::lzw_decode_with_budget(&encoded, Some(&params), expected.len(), &budget)
                    .unwrap();
            assert_eq!(result.as_slice(), expected);
            assert_eq!(budget.live_bytes(), result.capacity());
            drop(result);
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn budgeted_lzw_tracks_growth_and_dispatch_result_lifetime() {
        use crate::{
            budget::OperationBudget,
            filter::{FilterContext, apply_filter_with_budget},
        };
        let encoded = [0x80, 0x0B, 0x60, 0x50, 0x22, 0x0C, 0x0C, 0x85, 0x01];
        let budget = OperationBudget::new(19);
        let neighbor = budget.reserve(1).unwrap();
        assert!(matches!(
            super::lzw_decode_with_budget(&encoded, None, 10, &budget),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        for name in ["LZW", "LZWDecode"] {
            let result =
                apply_filter_with_budget(name, &encoded, None, FilterContext::default(), &budget)
                    .unwrap();
            assert_eq!(result.as_slice(), [45, 45, 45, 45, 45, 65, 45, 45, 45, 66]);
            assert_eq!(budget.live_bytes(), result.capacity());
            drop(result);
            assert_eq!(budget.live_bytes(), 0);
        }
        assert!(super::lzw_decode_with_budget(&encoded, None, 9, &budget).is_err());
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn prefix_dictionary_handles_forward_codes_and_clear_invalidation() {
        fn pack(codes: &[u16]) -> Vec<u8> {
            let mut bytes = vec![0; (codes.len() * 9).div_ceil(8)];
            for (index, code) in codes.iter().enumerate() {
                for bit in 0..9 {
                    let offset = index * 9 + bit;
                    bytes[offset / 8] |= (((code >> (8 - bit)) & 1) as u8) << (7 - offset % 8);
                }
            }
            bytes
        }
        let input = pack(&[256, 65, 258, 259, 256, 66, 258, 257]);
        assert_eq!(lzw_decode(&input, None).unwrap(), b"AAAAAABBB");
        assert!(matches!(
            super::lzw_decode_limited(&input, None, 8),
            Err(crate::error::PdfError::ResourceLimit(_))
        ));
        for codes in [&[256, 258, 257][..], &[256, 65, 258, 256, 259, 257][..]] {
            assert!(lzw_decode(&pack(codes), None).is_err());
        }
    }

    #[test]
    fn early_change_defaults_only_when_absent() {
        use crate::objects::{PdfObject, pdf_dict::PdfDict, pdf_number::PdfNumber};
        let encoded = [0x80, 0x0B, 0x60, 0x50, 0x22, 0x0C, 0x0C, 0x85, 0x01];
        let expected = lzw_decode(&encoded, None).unwrap();
        for value in ["0", "1"] {
            let mut params = PdfDict::default();
            params.insert(
                "EarlyChange".into(),
                PdfObject::PdfNumber(PdfNumber::new_from_lexeme(value.as_bytes()).unwrap()),
            );
            assert_eq!(lzw_decode(&encoded, Some(&params)).unwrap(), expected);
        }
        for value in ["1.5", "1.0", "-1", "2", "2147483648"] {
            let mut params = PdfDict::default();
            params.insert(
                "EarlyChange".into(),
                PdfObject::PdfNumber(PdfNumber::new_from_lexeme(value.as_bytes()).unwrap()),
            );
            assert!(lzw_decode(&encoded, Some(&params)).is_err(), "{value}");
        }
        let mut params = PdfDict::default();
        params.insert("EarlyChange".into(), PdfObject::PdfNull);
        assert!(lzw_decode(&encoded, Some(&params)).is_err());
    }

    #[test]
    fn test_lzw_decode() {
        let encoded = [0x80, 0x0B, 0x60, 0x50, 0x22, 0x0C, 0x0C, 0x85, 0x01];
        let res = lzw_decode(&encoded, None).unwrap();
        assert_eq!(res, [45, 45, 45, 45, 45, 65, 45, 45, 45, 66]);
    }
}
