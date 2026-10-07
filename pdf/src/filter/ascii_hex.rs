use crate::error::{PdfError, PdfResult};

pub fn ascii_hex_decode(input: &[u8]) -> PdfResult<Vec<u8>> {
    ascii_hex_decode_limited(
        input,
        crate::parser::options::Limits::default().max_decoded_bytes,
    )
}

pub fn ascii_hex_decode_limited(input: &[u8], limit: usize) -> PdfResult<Vec<u8>> {
    let length = output_length(input, limit)?;
    decode_planned(input, length)
}

pub fn ascii_hex_decode_with_budget(
    input: &[u8],
    limit: usize,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    let length = output_length(input, limit)?;
    let lease = budget.reserve(length)?;
    let bytes = decode_planned(input, length)?;
    crate::budget::BudgetedBytes::from_reserved_vec(bytes, lease)
}

fn output_length(input: &[u8], limit: usize) -> PdfResult<usize> {
    let mut length = 0;
    visit_bytes(input, |_| {
        super::check_output_size(length, 1, limit, "ASCIIHexDecode")?;
        length += 1;
        Ok(())
    })?;
    Ok(length)
}

fn decode_planned(input: &[u8], length: usize) -> PdfResult<Vec<u8>> {
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("ASCIIHexDecode allocation".into()))?;
    visit_bytes(input, |byte| {
        bytes.push(byte);
        Ok(())
    })?;
    Ok(bytes)
}

fn visit_bytes(input: &[u8], mut emit: impl FnMut(u8) -> PdfResult<()>) -> PdfResult<()> {
    let mut high = None;
    for &c in input {
        let nibble = match c {
            b'>' => break,
            0 | 32 | 13 | 12 | 10 | 9 => continue,
            b'0'..=b'9' => c - b'0',
            b'a'..=b'f' => c - b'a' + 10,
            b'A'..=b'F' => c - b'A' + 10,
            _ => {
                return Err(PdfError::FilterError(format!(
                    "Invalid character in ASCIIHexDecode stream: '{}'",
                    c
                )));
            }
        };
        if let Some(first) = high.take() {
            emit(first << 4 | nibble)?;
        } else {
            high = Some(nibble);
        }
    }
    if let Some(first) = high {
        emit(first << 4)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {

    use super::ascii_hex_decode;

    #[test]
    fn budgeted_hex_reserves_exact_output_and_retains_ownership() {
        let budget = crate::budget::OperationBudget::new(3);
        let neighbor = crate::budget::BudgetedBytes::copy_from_slice(&budget, b"x").unwrap();
        assert!(super::ascii_hex_decode_with_budget(b"61 62 F>ignored", 3, &budget).is_err());
        assert_eq!(budget.live_bytes(), neighbor.capacity());
        drop(neighbor);
        let bytes = super::ascii_hex_decode_with_budget(b"61 62 F>ignored", 3, &budget).unwrap();
        assert_eq!(bytes.as_slice(), &[b'a', b'b', 0xf0]);
        assert_eq!(budget.live_bytes(), bytes.capacity());
        drop(bytes);
        assert_eq!(budget.live_bytes(), 0);
        for (input, limit) in [(b"6162F".as_slice(), 2), (b"61g".as_slice(), 3)] {
            assert!(super::ascii_hex_decode_with_budget(input, limit, &budget).is_err());
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn test_ascii_hex_decode() {
        let raw = r#"48656c6c6f20776f726c64>"#;
        let result = ascii_hex_decode(raw.as_bytes()).unwrap();
        assert_eq!(result, b"Hello world");
    }
}
