use crate::error::{PdfError, PdfResult};

pub fn run_length_decode(input: &[u8]) -> PdfResult<Vec<u8>> {
    run_length_decode_limited(
        input,
        crate::parser::options::Limits::default().max_decoded_bytes,
    )
}

pub fn run_length_decode_limited(input: &[u8], limit: usize) -> PdfResult<Vec<u8>> {
    let length = output_length(input, limit)?;
    decode_planned(input, length)
}

pub fn run_length_decode_with_budget(
    input: &[u8],
    limit: usize,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    let length = output_length(input, limit)?;
    let lease = budget.reserve(length)?;
    crate::budget::BudgetedBytes::from_reserved_vec(decode_planned(input, length)?, lease)
}

fn output_length(input: &[u8], limit: usize) -> PdfResult<usize> {
    let mut length = 0;
    visit_runs(input, |_, count| {
        super::check_output_size(length, count, limit, "RunLengthDecode")?;
        length += count;
        Ok(())
    })?;
    Ok(length)
}

fn decode_planned(input: &[u8], length: usize) -> PdfResult<Vec<u8>> {
    let mut output = Vec::new();
    output
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("RunLengthDecode allocation".into()))?;
    visit_runs(input, |bytes, count| {
        if bytes.len() == count {
            output.extend_from_slice(bytes);
        } else {
            output.resize(output.len() + count, bytes[0]);
        }
        Ok(())
    })?;
    Ok(output)
}

fn visit_runs(input: &[u8], mut emit: impl FnMut(&[u8], usize) -> PdfResult<()>) -> PdfResult<()> {
    let mut i = 0;
    while i < input.len() {
        let length_byte = input[i];
        i += 1;
        match length_byte {
            0..=127 => {
                let count = (length_byte as usize) + 1;
                if i + count > input.len() {
                    return Err(PdfError::FilterError(
                        "RunLengthDecode literal run exceeds input length".to_string(),
                    ));
                }
                emit(&input[i..i + count], count)?;
                i += count;
            }
            129..=255 => {
                let count = 257 - (length_byte as usize);
                if i >= input.len() {
                    return Err(PdfError::FilterError(
                        "RunLengthDecode repeat run is missing its byte".to_string(),
                    ));
                }
                emit(&input[i..i + 1], count)?;
                i += 1;
            }
            128 => {
                break;
            }
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::run_length_decode;

    #[test]
    fn budgeted_mixed_runs_keep_output_charged_and_release_failures() {
        let input = [1, b'A', b'B', 254, b'C', 128, 255];
        let budget = crate::budget::OperationBudget::new(5);
        let neighbor = budget.reserve(1).unwrap();
        assert!(super::run_length_decode_with_budget(&input, 5, &budget).is_err());
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let output = super::run_length_decode_with_budget(&input, 5, &budget).unwrap();
        assert_eq!(output.as_slice(), b"ABCCC");
        assert_eq!(budget.live_bytes(), output.capacity());
        drop(output);
        for (bytes, limit) in [
            (input.as_slice(), 4),
            ([254].as_slice(), 5),
            ([2, b'A'].as_slice(), 5),
        ] {
            assert!(super::run_length_decode_with_budget(bytes, limit, &budget).is_err());
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn test_run_length_decode_rejects_truncated_literal() {
        assert!(run_length_decode(&[2, b'A', b'B']).is_err());
    }
}
