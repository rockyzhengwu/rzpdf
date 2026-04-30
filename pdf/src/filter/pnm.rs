use crate::error::{PdfError, PdfResult};

pub fn decode_pnm_bytes(data: &[u8]) -> PdfResult<Vec<u8>> {
    if data.starts_with(b"P5") || data.starts_with(b"P6") {
        decode_binary_pnm(data)
    } else if data.starts_with(b"P7") {
        decode_pam(data)
    } else {
        Err(PdfError::FilterError(
            "unsupported PNM/PAM output from external decoder".to_string(),
        ))
    }
}

fn decode_binary_pnm(data: &[u8]) -> PdfResult<Vec<u8>> {
    let mut cursor = 0;
    let magic = next_token(data, &mut cursor)?.to_vec();
    let _width = next_token(data, &mut cursor)?;
    let _height = next_token(data, &mut cursor)?;
    let _max = next_token(data, &mut cursor)?;
    skip_ws_and_comments(data, &mut cursor);
    let payload = data.get(cursor..).ok_or(PdfError::FilterError(
        "binary PNM payload is missing".to_string(),
    ))?;
    match magic.as_slice() {
        b"P5" | b"P6" => Ok(payload.to_vec()),
        _ => Err(PdfError::FilterError(
            "unsupported binary PNM variant".to_string(),
        )),
    }
}

fn decode_pam(data: &[u8]) -> PdfResult<Vec<u8>> {
    let header_end = data
        .windows(b"ENDHDR\n".len())
        .position(|window| window == b"ENDHDR\n")
        .map(|idx| idx + b"ENDHDR\n".len())
        .or_else(|| {
            data.windows(b"ENDHDR\r\n".len())
                .position(|window| window == b"ENDHDR\r\n")
                .map(|idx| idx + b"ENDHDR\r\n".len())
        })
        .ok_or(PdfError::FilterError(
            "PAM header is missing ENDHDR".to_string(),
        ))?;
    Ok(data[header_end..].to_vec())
}

fn next_token<'a>(data: &'a [u8], cursor: &mut usize) -> PdfResult<&'a [u8]> {
    skip_ws_and_comments(data, cursor);
    let start = *cursor;
    while let Some(byte) = data.get(*cursor) {
        if byte.is_ascii_whitespace() {
            break;
        }
        *cursor += 1;
    }
    if *cursor == start {
        return Err(PdfError::FilterError(
            "unexpected end of PNM header".to_string(),
        ));
    }
    Ok(&data[start..*cursor])
}

fn skip_ws_and_comments(data: &[u8], cursor: &mut usize) {
    loop {
        while let Some(byte) = data.get(*cursor) {
            if !byte.is_ascii_whitespace() {
                break;
            }
            *cursor += 1;
        }
        if data.get(*cursor) == Some(&b'#') {
            while let Some(byte) = data.get(*cursor) {
                *cursor += 1;
                if *byte == b'\n' {
                    break;
                }
            }
            continue;
        }
        break;
    }
}

#[cfg(test)]
mod tests {
    use super::decode_pnm_bytes;

    #[test]
    fn test_decode_pam_payload() {
        let pam = b"P7\nWIDTH 1\nHEIGHT 1\nDEPTH 3\nMAXVAL 255\nTUPLTYPE RGB\nENDHDR\n\x01\x02\x03";
        assert_eq!(decode_pnm_bytes(pam).unwrap(), vec![1, 2, 3]);
    }
}
