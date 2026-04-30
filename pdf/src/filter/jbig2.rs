use crate::{
    error::{PdfError, PdfResult},
    filter::FilterContext,
    objects::pdf_dict::PdfDict,
};

pub fn jbig2_decode(
    _buf: &[u8],
    _params: Option<&PdfDict>,
    _context: FilterContext<'_>,
) -> PdfResult<Vec<u8>> {
    Err(PdfError::FilterError(
        "JBIG2Decode is not implemented without an in-process decoder".to_string(),
    ))
}

#[cfg(test)]
mod tests {
    use crate::filter::FilterContext;

    use super::jbig2_decode;

    #[test]
    fn test_jbig2_decode_reports_unsupported() {
        let err = jbig2_decode(b"jbig2", None, FilterContext::default()).unwrap_err();
        assert!(err
            .to_string()
            .contains("JBIG2Decode is not implemented without an in-process decoder"));
    }
}
