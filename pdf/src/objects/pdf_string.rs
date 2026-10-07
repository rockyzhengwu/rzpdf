use crate::error::PdfResult;

#[derive(Debug, PartialEq, Clone)]
pub struct PdfString {
    bytes: Vec<u8>,
    is_hex: bool,
}

impl PdfString {
    pub(crate) fn clone_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let (bytes, lease) = crate::budget::BudgetedBytes::copy_from_slice(budget, &self.bytes)?
            .into_reserved_parts();
        Ok((
            Self {
                bytes,
                is_hex: self.is_hex,
            },
            lease,
        ))
    }

    /// Encode a PDF text string (metadata/bookmarks), not font character codes.
    /// Always uses UTF-16BE with BOM, valid independently of PDFDocEncoding coverage.
    pub fn from_text(text: &str) -> PdfResult<Self> {
        Self::from_text_with_limits(text, &crate::parser::options::Limits::default())
    }

    pub fn from_text_with_limits(
        text: &str,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<Self> {
        let chars = text.chars().count();
        let length = text
            .encode_utf16()
            .count()
            .checked_mul(2)
            .and_then(|n| n.checked_add(2))
            .filter(|&n| n <= limits.max_decoded_bytes && n <= limits.max_token_bytes)
            .filter(|_| chars <= limits.max_text_chars)
            .ok_or_else(|| {
                crate::error::PdfError::ResourceLimit("PDF text string bytes/characters".into())
            })?;
        let mut bytes = Vec::new();
        bytes.try_reserve_exact(length).map_err(|_| {
            crate::error::PdfError::ResourceLimit("PDF text string allocation".into())
        })?;
        bytes.extend_from_slice(&[0xfe, 0xff]);
        for unit in text.encode_utf16() {
            bytes.extend_from_slice(&unit.to_be_bytes());
        }
        Ok(Self::new(bytes, true))
    }

    /// Decode a PDF text string; unlike get_content this is not raw UTF-8.
    pub fn text_content(&self) -> PdfResult<String> {
        self.text_content_with_limits(&crate::parser::options::Limits::default())
    }

    pub fn text_content_with_limits(
        &self,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<String> {
        use crate::error::PdfError;
        if self.bytes.len() > limits.max_token_bytes {
            return Err(PdfError::ResourceLimit(
                "PDF text string input bytes".into(),
            ));
        }
        let mut output = String::new();
        let mut count = 0usize;
        let mut append = |ch: char| -> PdfResult<()> {
            count = count
                .checked_add(1)
                .filter(|&n| n <= limits.max_text_chars)
                .ok_or_else(|| PdfError::ResourceLimit("PDF text string characters".into()))?;
            output
                .len()
                .checked_add(ch.len_utf8())
                .filter(|&n| n <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("PDF text string decoded bytes".into()))?;
            output
                .try_reserve(ch.len_utf8())
                .map_err(|_| PdfError::ResourceLimit("PDF text string allocation".into()))?;
            output.push(ch);
            Ok(())
        };
        if let Some(data) = self.bytes.strip_prefix(&[0xfe, 0xff]) {
            if data.len() % 2 != 0 {
                return Err(PdfError::ObjectError(
                    "odd UTF-16BE text string length".into(),
                ));
            }
            let units = data
                .chunks_exact(2)
                .map(|b| u16::from_be_bytes([b[0], b[1]]));
            for ch in char::decode_utf16(units) {
                append(ch.map_err(|_| {
                    PdfError::ObjectError("invalid UTF-16BE text string surrogate".into())
                })?)?;
            }
        } else if let Some(data) = self.bytes.strip_prefix(&[0xef, 0xbb, 0xbf]) {
            // UTF-8 BOM text strings require PDF 2.0; callers enforce document version.
            let text = std::str::from_utf8(data)
                .map_err(|_| PdfError::ObjectError("invalid UTF-8 text string".into()))?;
            for ch in text.chars() {
                append(ch)?;
            }
        } else {
            for &byte in &self.bytes {
                let code = crate::font::encoding::FontEncoding::PdfDoc
                    .unicode_from_charcode(byte)
                    .filter(|&code| code != 0 || byte == 0)
                    .and_then(char::from_u32)
                    .ok_or_else(|| {
                        PdfError::ObjectError("undefined PDFDocEncoding text string byte".into())
                    })?;
                append(code)?;
            }
        }
        Ok(output)
    }

    pub fn new(bytes: Vec<u8>, is_hex: bool) -> Self {
        Self { bytes, is_hex }
    }

    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    pub fn raw_bytes(&self) -> Vec<u8> {
        self.bytes.clone()
    }

    pub fn get_content(&self) -> PdfResult<String> {
        let text = std::str::from_utf8(&self.bytes).map_err(|e| {
            crate::error::PdfError::ObjectError(format!(
                "PdfString content convert utf8 error: {}",
                e
            ))
        })?;
        let mut output = String::new();
        output.try_reserve_exact(text.len()).map_err(|_| {
            crate::error::PdfError::ResourceLimit("PDF string text allocation".into())
        })?;
        output.push_str(text);
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn pdf_text_strings_encode_unicode_and_decode_supported_encodings() {
        for text in ["", "Café €", "中文🙂", "a\0b"] {
            let value = PdfString::from_text(text).unwrap();
            assert!(value.bytes().starts_with(&[0xfe, 0xff]));
            assert_eq!(value.text_content().unwrap(), text);
        }
        assert_eq!(
            PdfString::from_text("中🙂").unwrap().bytes(),
            &[0xfe, 0xff, 0x4e, 0x2d, 0xd8, 0x3d, 0xde, 0x42]
        );
        assert_eq!(
            PdfString::new(vec![b'A', 0xa0, 0x80], false)
                .text_content()
                .unwrap(),
            "A€•"
        );
        assert_eq!(
            PdfString::new(
                [vec![0xef, 0xbb, 0xbf], "中文🙂".as_bytes().to_vec()].concat(),
                false
            )
            .text_content()
            .unwrap(),
            "中文🙂"
        );
        for bytes in [
            vec![0xfe, 0xff, 0],
            vec![0xfe, 0xff, 0xd8, 0],
            vec![0xfe, 0xff, 0xdc, 0],
            vec![0xef, 0xbb, 0xbf, 0xff],
            vec![0x7f],
            vec![0x9f],
            vec![0xad],
        ] {
            assert!(matches!(
                PdfString::new(bytes, false).text_content(),
                Err(crate::error::PdfError::ObjectError(_))
            ));
        }
    }

    #[test]
    fn pdf_text_string_limits_count_encoded_and_decoded_sizes_separately() {
        use crate::parser::options::Limits;
        let limits = Limits {
            max_text_chars: 1,
            max_token_bytes: 6,
            max_decoded_bytes: 6,
            ..Default::default()
        };
        let value = PdfString::from_text_with_limits("🙂", &limits).unwrap();
        assert_eq!(value.bytes().len(), 6);
        assert_eq!(
            value
                .text_content_with_limits(&Limits {
                    max_decoded_bytes: 4,
                    ..limits
                })
                .unwrap(),
            "🙂"
        );
        assert!(PdfString::from_text_with_limits(
            "🙂",
            &Limits {
                max_token_bytes: 5,
                ..limits
            }
        )
        .is_err());
        assert!(value
            .text_content_with_limits(&Limits {
                max_decoded_bytes: 3,
                ..limits
            })
            .is_err());
        assert!(value
            .text_content_with_limits(&Limits {
                max_text_chars: 0,
                ..limits
            })
            .is_err());
    }

    #[test]
    fn utf8_conversion_preserves_text_and_bounds_invalid_data_errors() {
        for text in ["", "hello", "中文🙂"] {
            let string = PdfString::new(text.as_bytes().to_vec(), false);
            assert_eq!(string.get_content().unwrap(), text);
            assert_eq!(string.bytes(), text.as_bytes());
        }
        let mut payload = vec![b'x'; 100_000];
        payload[42] = 0xff;
        let string = PdfString::new(payload.clone(), true);
        let error = string.get_content().unwrap_err().to_string();
        assert!(error.len() < 160);
        assert!(error.contains("42"));
        assert!(!error.contains("xxxx"));
        assert_eq!(string.bytes(), payload);
        assert!(PdfString::new(vec![0xe2, 0x82], false)
            .get_content()
            .is_err());
    }
}
