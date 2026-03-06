pub mod ascii_85;
pub mod ascii_hex;
pub mod ccittfax;
pub mod dct;
pub mod flate;
pub mod jbig2;
pub mod lzw;
pub mod run_length;

use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_dict::PdfDict,
};

pub fn apply_filter(name: &str, input: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    match name {
        "AHx" | "ASCIIHexDecode" => ascii_hex::ascii_hex_decode(input),
        "A85" | "ASCII85Decode" => ascii_85::ascii_85_decode(input),
        "LZW" | "LZWDecode" => lzw::lzw_decode(input, params),
        "Fl" | "FlateDecode" => flate::flate_decode(input, params),
        "RL" | "RunLengthDecode" => run_length::run_length_decode(input),
        "DCT" | "DCTDecode" => dct::dct_decode(input, params),
        "CCF" | "CCITTFaxDecode" => ccittfax::ccittfax_decode(input, params),
        "JBIG2Decode" => jbig2::jbig2_decode(input, params),
        _ => Err(PdfError::FilterError(format!("unimplemented:{:?}", name))),
    }
}
