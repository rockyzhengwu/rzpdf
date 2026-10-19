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

#[derive(Clone, Copy)]
pub struct FilterContext<'a> {
    /// True only when a document parser already applied the explicit Crypt
    /// stage. Does not authenticate or decrypt caller-owned ciphertext.
    pub crypt_already_processed: bool,
    pub stream_dict: Option<&'a PdfDict>,
    pub jbig2_globals: Option<&'a [u8]>,
    pub max_decoded_bytes: usize,
    pub max_image_pixels: u64,
}

impl Default for FilterContext<'_> {
    fn default() -> Self {
        Self {
            crypt_already_processed: false,
            stream_dict: None,
            jbig2_globals: None,
            max_decoded_bytes: crate::parser::options::Limits::default().max_decoded_bytes,
            max_image_pixels: crate::parser::options::Limits::default().max_image_pixels,
        }
    }
}

pub fn apply_filter(name: &str, input: &[u8], params: Option<&PdfDict>) -> PdfResult<Vec<u8>> {
    apply_filter_with_context(name, input, params, FilterContext::default())
}

pub(crate) fn check_output_size(
    current: usize,
    additional: usize,
    limit: usize,
    filter: &str,
) -> PdfResult<()> {
    if additional > limit.saturating_sub(current) || current > limit {
        return Err(PdfError::ResourceLimit(format!(
            "{filter} exceeds {limit} decoded bytes"
        )));
    }
    Ok(())
}

pub fn apply_filter_with_context(
    name: &str,
    input: &[u8],
    params: Option<&PdfDict>,
    context: FilterContext<'_>,
) -> PdfResult<Vec<u8>> {
    match name {
        "AHx" | "ASCIIHexDecode" => {
            ascii_hex::ascii_hex_decode_limited(input, context.max_decoded_bytes)
        }
        "A85" | "ASCII85Decode" => {
            ascii_85::ascii_85_decode_limited(input, context.max_decoded_bytes)
        }
        "LZW" | "LZWDecode" => lzw::lzw_decode_limited(input, params, context.max_decoded_bytes),
        "Fl" | "FlateDecode" => {
            flate::flate_decode_limited(input, params, context.max_decoded_bytes)
        }
        "RL" | "RunLengthDecode" => {
            run_length::run_length_decode_limited(input, context.max_decoded_bytes)
        }
        "DCT" | "DCTDecode" => dct::dct_decode_limited(input, context),
        "CCF" | "CCITTFaxDecode" => {
            ccittfax::ccittfax_decode_limited(input, params, context.max_decoded_bytes)
        }
        "Crypt" => {
            check_output_size(0, input.len(), context.max_decoded_bytes, "Crypt")?;
            crypt::crypt_decode(
                input,
                if context.crypt_already_processed {
                    None
                } else {
                    params
                },
            )
        }
        "JPXDecode" => jpx::jpx_decode(input, params, context),
        "JBIG2Decode" => jbig2::jbig2_decode(input, params, context),
        _ => Err(PdfError::FilterError(format!("unsupported filter: {name}"))),
    }
}

pub fn apply_filters(
    filter: &PdfObject,
    params: Option<&PdfObject>,
    input: &[u8],
    context: FilterContext<'_>,
) -> PdfResult<Vec<u8>> {
    apply_filter_chain(
        filter,
        params,
        input,
        context,
        apply_filter_with_context,
        |input| {
            let mut copied = Vec::new();
            copied
                .try_reserve_exact(input.len())
                .map_err(|_| PdfError::ResourceLimit("identity filter allocation".into()))?;
            copied.extend_from_slice(input);
            Ok(copied)
        },
    )
}

/// Budget-aware dispatch. Decoders not yet migrated fail explicitly rather
/// than silently allocating outside this operation's pool.
pub fn apply_filter_with_budget(
    name: &str,
    input: &[u8],
    params: Option<&PdfDict>,
    context: FilterContext<'_>,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    match name {
        "AHx" | "ASCIIHexDecode" => {
            ascii_hex::ascii_hex_decode_with_budget(input, context.max_decoded_bytes, budget)
        }
        "A85" | "ASCII85Decode" => {
            ascii_85::ascii_85_decode_with_budget(input, context.max_decoded_bytes, budget)
        }
        "RL" | "RunLengthDecode" => {
            run_length::run_length_decode_with_budget(input, context.max_decoded_bytes, budget)
        }
        "Fl" | "FlateDecode" => {
            flate::flate_decode_with_budget(input, params, context.max_decoded_bytes, budget)
        }
        "LZW" | "LZWDecode" => {
            lzw::lzw_decode_with_budget(input, params, context.max_decoded_bytes, budget)
        }
        "Crypt" => crypt::crypt_decode_with_budget(
            input,
            if context.crypt_already_processed {
                None
            } else {
                params
            },
            context.max_decoded_bytes,
            budget,
        ),
        "DCT" | "DCTDecode" => dct::dct_decode_with_budget(input, context, budget),
        "JPXDecode" => jpx::decode_with_budget(input, context, budget).map(|(_, bytes)| bytes),
        "JBIG2Decode" => jbig2::decode_with_budget(input, context, budget),
        "CCF" | "CCITTFaxDecode" => {
            ccittfax::ccittfax_decode_with_budget(input, params, context.max_decoded_bytes, budget)
        }
        _ => Err(PdfError::FilterError(format!(
            "filter has no budget-aware decoder: {name}"
        ))),
    }
}

/// Intermediate owners remain charged while the next stage is decoding.
/// Borrowed input must be charged by its owner if it belongs to this pool.
pub fn apply_filters_with_budget(
    filter: &PdfObject,
    params: Option<&PdfObject>,
    input: &[u8],
    context: FilterContext<'_>,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<crate::budget::BudgetedBytes> {
    apply_filter_chain(
        filter,
        params,
        input,
        context,
        |name, input, params, context| {
            apply_filter_with_budget(name, input, params, context, budget)
        },
        |input| crate::budget::BudgetedBytes::copy_from_slice(budget, input),
    )
}

fn apply_filter_chain<'a, T: AsRef<[u8]>>(
    filter: &PdfObject,
    params: Option<&PdfObject>,
    input: &[u8],
    context: FilterContext<'a>,
    decode: impl Fn(&str, &[u8], Option<&PdfDict>, FilterContext<'a>) -> PdfResult<T>,
    copy: impl Fn(&[u8]) -> PdfResult<T>,
) -> PdfResult<T> {
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
            decode(name.name(), input, params, context)
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

            if let Some(arr) = params_array
                && arr.len() != filters.len()
            {
                return Err(PdfError::FilterError(format!(
                    "filter count ({}) does not match DecodeParms count ({})",
                    filters.len(),
                    arr.len()
                )));
            }

            let stage = |index| {
                let filter_name = filters.get(index).and_then(PdfObject::as_name).ok_or(
                    PdfError::FilterError("filter array must contain only names".to_string()),
                )?;
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
                Ok::<_, PdfError>((filter_name.name(), params))
            };
            // Validate the entire chain before any decoder allocates output.
            // Borrow its entries instead of building another metadata table.
            for index in 0..filters.len() {
                stage(index)?;
            }
            let mut data: Option<T> = None;
            for index in 0..filters.len() {
                let (name, params) = stage(index)?;
                data = Some(decode(
                    name,
                    data.as_ref().map(AsRef::as_ref).unwrap_or(input),
                    params,
                    context,
                )?);
            }
            match data {
                Some(data) => Ok(data),
                None => {
                    check_output_size(
                        0,
                        input.len(),
                        context.max_decoded_bytes,
                        "identity filter",
                    )?;
                    copy(input)
                }
            }
        }
        _ => Err(PdfError::FilterError(
            "Filter must be a name or array".to_string(),
        )),
    }
}

#[cfg(test)]
mod limit_tests {
    use super::*;

    #[test]
    fn budgeted_chain_retains_intermediate_capacity_and_result_lease() {
        use crate::{
            budget::OperationBudget,
            objects::{pdf_array::PdfArray, pdf_name::PdfName},
        };
        let mut filters = PdfArray::default();
        for _ in 0..2 {
            filters
                .try_add_obj(PdfObject::PdfName(PdfName::new("AHx".into())))
                .unwrap();
        }
        let filters = PdfObject::PdfArray(filters);
        let context = FilterContext::default();
        let budget = OperationBudget::new(10);
        let neighbor = budget.reserve(1).unwrap();
        assert!(matches!(
            apply_filters_with_budget(&filters, None, b"3631363236333E>", context, &budget),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 1);
        drop(neighbor);
        let result =
            apply_filters_with_budget(&filters, None, b"3631363236333E>", context, &budget)
                .unwrap();
        assert_eq!(result.as_slice(), b"abc");
        assert_eq!(budget.live_bytes(), result.capacity());
        drop(result);
        assert_eq!(budget.live_bytes(), 0);
        assert!(apply_filter_with_budget("DCTDecode", b"", None, context, &budget).is_err());
        assert_eq!(budget.live_bytes(), 0);
        let empty = PdfObject::PdfArray(PdfArray::default());
        let result = apply_filters_with_budget(&empty, None, b"abc", context, &budget).unwrap();
        assert_eq!(result.as_slice(), b"abc");
        assert_eq!(budget.live_bytes(), result.capacity());
        drop(result);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn filter_chain_preflight_rejects_late_structure_before_decoding() {
        use crate::objects::{pdf_array::PdfArray, pdf_name::PdfName};
        let name = || PdfObject::PdfName(PdfName::new("ASCIIHexDecode".into()));
        let context = FilterContext {
            max_decoded_bytes: 0,
            ..Default::default()
        };
        let mut filters = PdfArray::default();
        filters.try_add_obj(name()).unwrap();
        filters.try_add_obj(PdfObject::PdfNull).unwrap();
        // First stage would exceed zero bytes; the later malformed entry wins.
        assert!(matches!(
            apply_filters(&PdfObject::PdfArray(filters), None, b"61>", context),
            Err(PdfError::FilterError(_))
        ));
        let mut filters = PdfArray::default();
        filters.try_add_obj(name()).unwrap();
        filters.try_add_obj(name()).unwrap();
        let mut params = PdfArray::default();
        params.try_add_obj(PdfObject::PdfNull).unwrap();
        params.try_add_obj(name()).unwrap();
        assert!(matches!(
            apply_filters(
                &PdfObject::PdfArray(filters),
                Some(&PdfObject::PdfArray(params)),
                b"61>",
                context
            ),
            Err(PdfError::FilterError(_))
        ));
    }

    #[test]
    fn filter_chain_decodes_previous_output_and_limits_intermediate_bytes() {
        use crate::objects::{pdf_array::PdfArray, pdf_name::PdfName};
        let mut array = PdfArray::default();
        for _ in 0..2 {
            array
                .try_add_obj(PdfObject::PdfName(PdfName::new("ASCIIHexDecode".into())))
                .unwrap();
        }
        let filters = PdfObject::PdfArray(array);
        let input = b"3631363236333E>"; // First result: 616263>; second: abc.
        let context = FilterContext {
            max_decoded_bytes: 7,
            ..Default::default()
        };
        assert_eq!(
            apply_filters(&filters, None, input, context).unwrap(),
            b"abc"
        );
        assert!(matches!(
            apply_filters(
                &filters,
                None,
                input,
                FilterContext {
                    max_decoded_bytes: 6,
                    ..context
                }
            ),
            Err(PdfError::ResourceLimit(_))
        ));
        let mut params = PdfArray::default();
        params.try_add_obj(PdfObject::PdfNull).unwrap();
        params.try_add_obj(PdfObject::PdfNull).unwrap();
        assert_eq!(
            apply_filters(&filters, Some(&PdfObject::PdfArray(params)), input, context).unwrap(),
            b"abc"
        );
        assert!(matches!(
            apply_filters(
                &filters,
                Some(&PdfObject::PdfArray(PdfArray::default())),
                input,
                context
            ),
            Err(PdfError::FilterError(_))
        ));
    }

    #[test]
    fn empty_filter_array_honors_output_limit() {
        let filters = PdfObject::PdfArray(crate::objects::pdf_array::PdfArray::default());
        let context = FilterContext {
            max_decoded_bytes: 3,
            ..Default::default()
        };
        assert_eq!(
            apply_filters(&filters, None, b"abc", context).unwrap(),
            b"abc"
        );
        assert!(matches!(
            apply_filters(&filters, None, b"abcd", context),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(
            apply_filters(
                &filters,
                None,
                b"",
                FilterContext {
                    max_decoded_bytes: 0,
                    ..context
                }
            )
            .unwrap(),
            b""
        );
    }

    #[test]
    fn expanding_filters_honor_custom_output_limits() {
        for (name, data, length) in [
            ("ASCIIHexDecode", b"010203>".as_slice(), 3),
            ("ASCII85Decode", b"zz~>".as_slice(), 8),
            ("RunLengthDecode", &[129, b'A', 128][..], 128),
            (
                "LZWDecode",
                &[0x80, 0x0B, 0x60, 0x50, 0x22, 0x0C, 0x0C, 0x85, 0x01][..],
                10,
            ),
        ] {
            let context = FilterContext {
                max_decoded_bytes: length - 1,
                ..Default::default()
            };
            assert!(
                matches!(
                    apply_filter_with_context(name, data, None, context),
                    Err(PdfError::ResourceLimit(_))
                ),
                "{name}"
            );
            let context = FilterContext {
                max_decoded_bytes: length,
                ..Default::default()
            };
            assert_eq!(
                apply_filter_with_context(name, data, None, context)
                    .unwrap()
                    .len(),
                length
            );
        }
    }

    #[test]
    fn ascii85_invalid_groups_return_errors_without_overflow() {
        assert!(ascii_85::ascii_85_decode(b"uuuuu~>").is_err());
        assert!(ascii_85::ascii_85_decode(b"!~>").is_err());
    }

    #[test]
    fn jpeg_headers_and_fax_parameters_are_checked_before_pixels() {
        let mut compressor = mozjpeg::Compress::new(mozjpeg::ColorSpace::JCS_RGB);
        compressor.set_size(2, 2);
        let mut compressor = compressor.start_compress(Vec::new()).unwrap();
        compressor.write_scanlines(&[0u8; 12]).unwrap();
        let jpeg = compressor.finish().unwrap();
        let context = FilterContext {
            max_image_pixels: 3,
            ..Default::default()
        };
        assert!(matches!(
            apply_filter_with_context("DCTDecode", &jpeg, None, context),
            Err(PdfError::ResourceLimit(_))
        ));
        let context = FilterContext {
            max_decoded_bytes: 11,
            ..Default::default()
        };
        assert!(matches!(
            apply_filter_with_context("DCTDecode", &jpeg, None, context),
            Err(PdfError::ResourceLimit(_))
        ));
        let context = FilterContext {
            max_decoded_bytes: 12,
            ..Default::default()
        };
        assert_eq!(
            apply_filter_with_context("DCTDecode", &jpeg, None, context)
                .unwrap()
                .len(),
            12
        );
        let context = FilterContext {
            max_decoded_bytes: 1,
            ..Default::default()
        };
        assert!(matches!(
            apply_filter_with_context("CCITTFaxDecode", &[], None, context),
            Err(PdfError::ResourceLimit(_))
        ));
    }
}
