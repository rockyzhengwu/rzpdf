pub mod ascii_85;
pub mod ascii_hex;
pub mod ccittfax;
pub mod crypt;
pub mod dct;
pub mod flate;
pub mod jbig2;
pub mod jpx;
pub mod lzw;
pub mod pnm;
pub mod predictor;
pub mod run_length;

use crate::{
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict},
};

#[derive(Clone, Copy, Default)]
pub struct FilterContext<'a> {
    pub stream_dict: Option<&'a PdfDict>,
    pub jbig2_globals: Option<&'a [u8]>,
}

pub fn apply_filter(name: &str, input: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    apply_filter_with_context(name, input, params, FilterContext::default())
}

pub fn apply_filter_with_context(
    name: &str,
    input: &[u8],
    params: Option<&PdfDict>,
    context: FilterContext<'_>,
) -> PdfResult<Vec<u8>> {
    match name {
        "AHx" | "ASCIIHexDecode" => ascii_hex::ascii_hex_decode(input),
        "A85" | "ASCII85Decode" => ascii_85::ascii_85_decode(input),
        "LZW" | "LZWDecode" => lzw::lzw_decode(input, params),
        "Fl" | "FlateDecode" => flate::flate_decode(input, params),
        "RL" | "RunLengthDecode" => run_length::run_length_decode(input),
        "DCT" | "DCTDecode" => dct::dct_decode(input, params),
        "CCF" | "CCITTFaxDecode" => ccittfax::ccittfax_decode(input, params),
        "Crypt" => crypt::crypt_decode(input, params),
        "JPXDecode" => jpx::jpx_decode(input, params, context),
        "JBIG2Decode" => jbig2::jbig2_decode(input, params, context),
        _ => Err(PdfError::FilterError(format!(
            "unsupported filter: {name}"
        ))),
    }
}

pub fn apply_filters(
    filter: &PdfObject,
    params: Option<&PdfObject>,
    input: &[u8],
    context: FilterContext<'_>,
) -> PdfResult<Vec<u8>> {
    match filter {
        PdfObject::PdfName(name) => {
            let params = match params {
                Some(PdfObject::PdfDict(dict)) => Some(dict),
                None | Some(PdfObject::PdfNull) => None,
                Some(_) => {
                    return Err(PdfError::FilterError(
                        "single filter DecodeParms must be a dictionary".to_string(),
                    ));
                }
            };
            apply_filter_with_context(name.name(), input, params, context)
        }
        PdfObject::PdfArray(filters) => {
            let params_array = match params {
                Some(PdfObject::PdfArray(arr)) => Some(arr),
                None | Some(PdfObject::PdfNull) => None,
                Some(_) => {
                    return Err(PdfError::FilterError(
                        "filter array DecodeParms must be an array".to_string(),
                    ));
                }
            };

            if let Some(arr) = params_array {
                if arr.len() != filters.len() {
                    return Err(PdfError::FilterError(format!(
                        "filter count ({}) does not match DecodeParms count ({})",
                        filters.len(),
                        arr.len()
                    )));
                }
            }

            let mut data = input.to_vec();
            for index in 0..filters.len() {
                let filter_name = filters
                    .get(index)
                    .and_then(PdfObject::as_name)
                    .ok_or(PdfError::FilterError(
                        "filter array must contain only names".to_string(),
                    ))?;
                let params = params_array
                    .and_then(|arr| arr.get(index))
                    .map(|param| {
                        if matches!(param, PdfObject::PdfNull) {
                            Ok(None)
                        } else {
                            param.as_dict().map(Some).ok_or(PdfError::FilterError(
                                "filter array DecodeParms items must be dictionaries".to_string(),
                            ))
                        }
                    })
                    .transpose()?
                    .flatten();
                data = apply_filter_with_context(filter_name.name(), &data, params, context)?;
            }
            Ok(data)
        }
        _ => Err(PdfError::FilterError(
            "Filter must be a name or array".to_string(),
        )),
    }
}
