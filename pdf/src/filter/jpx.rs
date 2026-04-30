use crate::{
    error::{PdfError, PdfResult},
    filter::FilterContext,
    objects::pdf_dict::PdfDict,
};

pub fn jpx_decode(
    _input: &[u8],
    _params: Option<&PdfDict>,
    _context: FilterContext<'_>,
) -> PdfResult<Vec<u8>> {
    Err(PdfError::FilterError(
        "JPXDecode is not implemented without an in-process decoder".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use crate::filter::FilterContext;

    use super::jpx_decode;

    #[test]
    fn test_jpx_decode_reports_unsupported() {
        let err = jpx_decode(b"jp2", None, FilterContext::default()).unwrap_err();
        assert!(err
            .to_string()
            .contains("JPXDecode is not implemented without an in-process decoder"));
    }
}
