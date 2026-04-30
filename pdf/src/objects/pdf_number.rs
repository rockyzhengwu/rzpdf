use crate::{
    error::{PdfError, PdfResult},
    parser::parse_utility::real_from_buffer,
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
        let lexeme = std::str::from_utf8(buf)
            .map_err(|e| PdfError::ParserError(format!("number token is not utf8: {e:?}")))?
            .to_string();
        Ok(Self {
            value: real_from_buffer(buf),
            is_integer: !buf.contains(&b'.'),
            lexeme,
        })
    }

    pub fn lexeme(&self) -> &str {
        self.lexeme.as_str()
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
        self.as_u64_checked().unwrap_or_else(|_| self.value.round() as u64)
    }

    pub fn get_u32(&self) -> u32 {
        self.as_u32_checked().unwrap_or_else(|_| self.value.round() as u32)
    }
    pub fn get_u16(&self) -> u16 {
        self.as_u16_checked().unwrap_or_else(|_| self.value.round() as u16)
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
