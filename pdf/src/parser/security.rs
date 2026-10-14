use crate::error::{PdfError, PdfResult};
use crate::objects::pdf_dict::PdfDict;

#[derive(Debug, Default)]
pub struct CryptFilter {
    cfm: Option<String>,
    length: u32,
}

impl CryptFilter {
    pub fn new(cfm: String, length: u32) -> Self {
        Self {
            cfm: Some(cfm),
            length,
        }
    }
    pub fn try_new(cf: &PdfDict) -> PdfResult<Self> {
        let mut filter = CryptFilter::default();
        if let Some(cfm) = cf.get("CFM") {
            let cfm = cfm
                .as_name()
                .ok_or_else(|| PdfError::ObjectError("CryptFilter CFM must be a name".into()))?
                .name();
            if !matches!(cfm, "None" | "V2" | "AESV2" | "AESV3") {
                return Err(PdfError::UnsupportedFeature(format!(
                    "CryptFilter method {cfm}"
                )));
            }
            filter.cfm = Some(cfm.to_string());
        }
        if let Some(length) = cf.get("Length") {
            filter.length = length
                .as_number()
                .ok_or_else(|| {
                    PdfError::ObjectError("CryptFilter Length must be an integer".into())
                })?
                .as_u32_checked()?;
        }
        Ok(filter)
    }
}

#[cfg(test)]
mod safety_tests {
    use super::*;
    use crate::{io::stream_reader::StreamReader, parser::syntax::SyntaxParser};
    use std::io::Cursor;

    #[test]
    fn crypt_filter_fields_are_checked_without_panics() {
        let parse = |source: &str| {
            let obj = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap();
            CryptFilter::try_new(obj.as_dict().unwrap())
        };
        for source in [
            "<< /CFM 1 >>",
            "<< /CFM /Unknown >>",
            "<< /Length /bad >>",
            "<< /Length -1 >>",
            "<< /Length 1.5 >>",
            "<< /Length 4294967296 >>",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
        let filter = parse("<< /CFM /AESV3 /Length 32 >>").unwrap();
        assert_eq!(filter.cfm.as_deref(), Some("AESV3"));
        assert_eq!(filter.length, 32);
    }
}

pub use crate::security::SecurityHandler;
