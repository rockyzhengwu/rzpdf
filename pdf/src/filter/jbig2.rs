use crate::error::{PdfError, PdfResult};
use crate::objects::pdf_dict::PdfDict;

pub fn jbig2_decode(buf: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    return Err(PdfError::FilterError(
        "Jbig2_decoder is not implemented".to_string(),
    ));
}
