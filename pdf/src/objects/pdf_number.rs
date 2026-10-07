use crate::{
    error::{PdfError, PdfResult},
    parser::syntax::is_number_token,
};

#[derive(Debug, PartialEq, Clone)]
pub struct PdfNumber {
    lexeme: String,
    value: f32,
    is_integer: bool,
}
impl PdfNumber {
    pub fn new(value: f32) -> Self {
        let lexeme = if value.fract() == 0.0 {
            format!("{value:.0}")
        } else {
            value.to_string()
        };
        Self {
            lexeme,
            value,
            is_integer: value.fract() == 0.0,
        }
    }

    pub fn new_from_lexeme(buf: &[u8]) -> PdfResult<Self> {
        if !is_number_token(buf) {
            return Err(PdfError::ParserError("invalid PDF number token".into()));
        }
        Self::from_owned_lexeme(
            std::str::from_utf8(buf)
                .map_err(|e| PdfError::ParserError(format!("number token is not utf8: {e:?}")))?
                .to_string(),
        )
    }

    pub(crate) fn from_owned_lexeme(lexeme: String) -> PdfResult<Self> {
        let buf = lexeme.as_bytes();
        if !is_number_token(buf) {
            return Err(PdfError::ParserError("invalid PDF number token".into()));
        }
        let value = lexeme
            .parse::<f32>()
            .map_err(|err| PdfError::ParserError(format!("invalid PDF number: {err}")))?;
        Ok(Self {
            value,
            is_integer: !buf.contains(&b'.'),
            lexeme,
        })
    }

    pub fn lexeme(&self) -> &str {
        self.lexeme.as_str()
    }

    pub(crate) fn clone_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let (lexeme, lease) = crate::budget::copy_string(&self.lexeme, budget)?;
        Ok((
            Self {
                lexeme,
                value: self.value,
                is_integer: self.is_integer,
            },
            lease,
        ))
    }

    pub fn is_integer(&self) -> bool {
        self.is_integer
    }

    pub fn as_u64_checked(&self) -> PdfResult<u64> {
        self.parse_unsigned_integer()
    }

    pub fn as_u32_checked(&self) -> PdfResult<u32> {
        let value = self.parse_unsigned_integer()?;
        u32::try_from(value).map_err(|_| {
            PdfError::ObjectError(format!("pdf number out of range for u32: {}", self.lexeme))
        })
    }

    pub fn as_u16_checked(&self) -> PdfResult<u16> {
        let value = self.parse_unsigned_integer()?;
        u16::try_from(value).map_err(|_| {
            PdfError::ObjectError(format!("pdf number out of range for u16: {}", self.lexeme))
        })
    }

    pub fn as_i32_checked(&self) -> PdfResult<i32> {
        if !self.is_integer {
            return Err(PdfError::ObjectError("pdf number is not an integer".into()));
        }
        self.lexeme.parse::<i32>().map_err(|_| {
            PdfError::ObjectError(format!("pdf number outside i32 range: {}", self.lexeme))
        })
    }

    pub fn as_u8_checked(&self) -> PdfResult<u8> {
        u8::try_from(self.parse_unsigned_integer()?).map_err(|_| {
            PdfError::ObjectError(format!("pdf number outside u8 range: {}", self.lexeme))
        })
    }

    fn parse_unsigned_integer(&self) -> PdfResult<u64> {
        if !self.is_integer {
            return Err(PdfError::ObjectError(format!(
                "pdf number is not an integer: {}",
                self.lexeme
            )));
        }
        self.lexeme.parse::<u64>().map_err(|_| {
            PdfError::ObjectError(format!(
                "pdf number is not a valid unsigned integer: {}",
                self.lexeme
            ))
        })
    }

    pub fn get_u64(&self) -> u64 {
        self.as_u64_checked()
            .unwrap_or_else(|_| self.value.round() as u64)
    }

    pub fn get_u32(&self) -> u32 {
        self.as_u32_checked()
            .unwrap_or_else(|_| self.value.round() as u32)
    }
    pub fn get_u16(&self) -> u16 {
        self.as_u16_checked()
            .unwrap_or_else(|_| self.value.round() as u16)
    }

    pub fn get_i8(&self) -> i8 {
        self.value as i8
    }

    pub fn get_i32(&self) -> i32 {
        self.value as i32
    }

    pub fn get_u8(&self) -> u8 {
        self.value as u8
    }

    pub fn value(&self) -> f32 {
        self.value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn lexeme_constructor_validates_grammar_and_handles_long_fraction() {
        for source in [
            "", "+", "-", ".", "1e3", "NaN", "Infinity", "12x", "1.2.3", " 1",
        ] {
            assert!(
                PdfNumber::new_from_lexeme(source.as_bytes()).is_err(),
                "{source}"
            );
        }
        for (source, expected) in [("+.5", 0.5), ("-.125", -0.125), ("1.", 1.0), ("-0", 0.0)] {
            assert_eq!(
                PdfNumber::new_from_lexeme(source.as_bytes())
                    .unwrap()
                    .value(),
                expected
            );
        }
        let source = format!("0.{}", "1".repeat(100));
        let number = PdfNumber::new_from_lexeme(source.as_bytes()).unwrap();
        assert!(number.value().is_finite());
        assert!((number.value() - 1.0 / 9.0).abs() < 1e-7);
        assert_eq!(number.lexeme(), source);
        let integer = PdfNumber::new_from_lexeme(b"18446744073709551615").unwrap();
        assert_eq!(integer.as_u64_checked().unwrap(), u64::MAX);
    }
}
