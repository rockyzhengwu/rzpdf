use crate::error::{PdfError, PdfResult};

pub fn ascii_85_decode(input: &[u8]) -> PdfResult<Vec<u8>> {
    ascii_85_decode_limited(
        input,
        crate::parser::options::Limits::default().max_decoded_bytes,
    )
}

pub fn ascii_85_decode_limited(input: &[u8], limit: usize) -> PdfResult<Vec<u8>> {
    decode_planned(input, output_length(input, limit)?)
}

pub fn ascii_85_decode_with_budget(
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
    visit_groups(input, |bytes| {
        super::check_output_size(length, bytes.len(), limit, "ASCII85Decode")?;
        length += bytes.len();
        Ok(())
    })?;
    Ok(length)
}

fn decode_planned(input: &[u8], length: usize) -> PdfResult<Vec<u8>> {
    let mut result = Vec::new();
    result
        .try_reserve_exact(length)
        .map_err(|_| PdfError::ResourceLimit("ASCII85Decode allocation".into()))?;
    visit_groups(input, |bytes| {
        result.extend_from_slice(bytes);
        Ok(())
    })?;
    Ok(result)
}

fn visit_groups(input: &[u8], mut emit: impl FnMut(&[u8]) -> PdfResult<()>) -> PdfResult<()> {
    let mut buffer = [0u32; 5];
    let mut count = 0;
    let mut input = input.iter().peekable();
    while let Some(c) = input.next() {
        match c {
            b'~' => {
                if input.next() == Some(&b'>') {
                    break;
                } else {
                    return Err(PdfError::FilterError(
                        "Invalid end sequence in ASCII85 stream".to_string(),
                    ));
                }
            }
            b'z' => {
                if count != 0 {
                    return Err(PdfError::FilterError(
                        "`z` cannot appear inside a group".to_string(),
                    ));
                }
                emit(&[0, 0, 0, 0])?;
            }
            0 | 32 | 13 | 12 | 10 | 9 => continue,
            b'!'..=b'u' => {
                buffer[count] = u32::from(*c) - 33;
                count += 1;
            }
            _ => {
                return Err(PdfError::FilterError(format!(
                    "Invalid character in ASCII85 stream: '{}'",
                    c
                )));
            }
        }
        if count == 5 {
            let value = decode_group(&buffer)?;
            emit(&value.to_be_bytes())?;
            count = 0;
        }
    }

    if count != 0 {
        if count == 1 {
            return Err(PdfError::FilterError(
                "ASCII85 final group needs at least two characters".into(),
            ));
        }
        buffer[count..].fill(84);
        let value = decode_group(&buffer)?;
        emit(&value.to_be_bytes()[..count - 1])?;
    }

    Ok(())
}

fn decode_group(group: &[u32]) -> PdfResult<u32> {
    let value = group
        .iter()
        .fold(0_u64, |acc, &byte| acc * 85 + u64::from(byte));
    u32::try_from(value)
        .map_err(|_| PdfError::FilterError("ASCII85 group value exceeds 32 bits".into()))
}

#[cfg(test)]
mod tests {
    use super::ascii_85_decode;

    #[test]
    fn budgeted_groups_account_for_shorthand_and_partial_tail() {
        let budget = crate::budget::OperationBudget::new(5);
        let neighbor = budget.reserve(1).unwrap();
        assert!(super::ascii_85_decode_with_budget(b"z!!~>ignored", 5, &budget).is_err());
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let decoded = super::ascii_85_decode_with_budget(b"z!!~>ignored", 5, &budget).unwrap();
        assert_eq!(decoded.as_slice(), &[0; 5]);
        assert_eq!(budget.live_bytes(), decoded.capacity());
        drop(decoded);
        for input in [
            b"!z".as_slice(),
            b"!~>".as_slice(),
            b"uuuuu".as_slice(),
            b"~x".as_slice(),
        ] {
            assert!(super::ascii_85_decode_with_budget(input, 5, &budget).is_err());
            assert_eq!(budget.live_bytes(), 0);
        }
    }

    #[test]
    fn test_ascii_85_decode() {
        let input = r#"87cURD]j7BEbo80~>"#;
        let result = ascii_85_decode(input.as_bytes()).unwrap();
        assert_eq!(result, b"Hello world!");

        let content = r#"9jqo^BlbD-BleB1DJ+*+F(f,q/0JhKF<GL>Cj@.4Gp$d7F!,L7@<6@)/0JDEF<G%<+EV:2F!,O<DJ+*.@<*K0@<6L(Df-\0Ec5e;DffZ(EZee.Bl.9pF"AGXBPCsi+DGm>@3BB/F*&OCAfu2/AKYi(DIb:@FD,*)+C]U=@3BN#EcYf8ATD3s@q?d$AftVqCh[NqF<G:8+EV:.+Cf>-FD5W8ARlolDIal(DId<j@<?3r@:F%a+D58'ATD4$Bl@l3De:,-DJs`8ARoFb/0JMK@qB4^F!,R<AKZ&-DfTqBG%G>uD.RTpAKYo'+CT/5+Cei#DII?(E,9)oF*2M7/c~>"#;
        let result = ascii_85_decode(content.as_bytes()).unwrap();
        let expected = r#"Man is distinguished, not only by his reason, but by this singular passion from other animals, which is a lust of the mind, that by a perseverance of delight in the continued and indefatigable generation of knowledge, exceeds the short vehemence of any carnal pleasure."#;
        assert_eq!(result, expected.as_bytes())
    }
}
