use crate::error::{PdfError, PdfResult};

pub fn run_length_decode(input: &[u8]) -> PdfResult<Vec<u8>> {
    let mut output = Vec::new();
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
                output.extend_from_slice(&input[i..i + count]);
                i += count;
            }
            129..=255 => {
                let count = 257 - (length_byte as usize);
                if i >= input.len() {
                    return Err(PdfError::FilterError(
                        "RunLengthDecode repeat run is missing its byte".to_string(),
                    ));
                }
                output.extend(std::iter::repeat(input[i]).take(count));
                i += 1;
            }
            128 => {
                break;
            }
        }
    }

    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::run_length_decode;

    #[test]
    fn test_run_length_decode_rejects_truncated_literal() {
        assert!(run_length_decode(&[2, b'A', b'B']).is_err());
    }
}
