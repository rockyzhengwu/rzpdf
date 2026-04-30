use crate::error::{PdfError, PdfResult};
use crate::objects::pdf_dict::PdfDict;
use crate::filter::predictor::apply_predictor;
use flate2::bufread::ZlibDecoder;
use std::io::Read;

pub fn flate_decode(input: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    let mut decoder = ZlibDecoder::new(input);
    let mut decompressed = Vec::new();
    decoder.read_to_end(&mut decompressed).map_err(|e| {
        PdfError::FilterError(format!("FlateDecode zlib decompression failed: {e}"))
    })?;
    apply_predictor(decompressed, params, "FlateDecode")
}

#[cfg(test)]
mod tests {
    use super::flate_decode;

    #[test]
    fn test_flated_decode() {
        let encoded = [
            0x78, 0x9c, 0x4b, 0xcb, 0xcf, 0x07, 0x00, 0x02, 0x82, 0x01, 0x45,
        ];
        let res = flate_decode(&encoded, None).unwrap();
        assert_eq!(res, b"foo");
    }

    #[test]
    fn test_flated_decode_rejects_truncated_stream() {
        let encoded = [
            0x78, 0x9c, 0x4b, 0xcb, 0xcf, 0x07, 0x00, 0x02, 0x82, 0x01, 0x46,
        ];
        assert!(flate_decode(&encoded, None).is_err());
    }
}
