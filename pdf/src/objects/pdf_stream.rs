use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    filter::{FilterContext, apply_filters},
    objects::{PdfObject, pdf_dict::PdfDict},
    pdf_context::PDFContext,
};

/// Payload interpretation, not a serialized PDF dictionary entry.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamDataState {
    /// Bytes use the dictionary's encoding (identity if no Filter).
    RawEncoded,
    /// Bytes were explicitly supplied as decoded content; no encoding keys remain.
    Decoded,
}

#[derive(Debug, Clone)]
pub struct PdfStream {
    dict: PdfDict,
    data: Vec<u8>,
    state: StreamDataState,
    /// An authenticated source handler already processed the explicit Crypt
    /// stage. Not a serialized key; new/raw caller streams start false.
    crypt_processed: bool,
}

// Preserve PDF object equality: interpretation provenance is not PDF content.
impl PartialEq for PdfStream {
    fn eq(&self, other: &Self) -> bool {
        self.dict == other.dict && self.data == other.data
    }
}

impl PdfStream {
    pub fn new(dict: PdfDict, data: Vec<u8>) -> Self {
        PdfStream {
            dict,
            data,
            state: StreamDataState::RawEncoded,
            crypt_processed: false,
        }
    }

    pub fn from_decoded_data(dict: PdfDict, data: Vec<u8>) -> PdfResult<Self> {
        Self::new(dict, Vec::new()).with_decoded_data(data)
    }

    // The caller owns the payload reservation; this lease covers the rebuilt dictionary.
    pub(crate) fn from_decoded_data_with_budget(
        source: &PdfDict,
        data: Vec<u8>,
        budget: &crate::budget::OperationBudget,
        limits: crate::parser::options::Limits,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        if limits.max_depth == 0 {
            return Err(PdfError::ResourceLimit("stream dictionary depth".into()));
        }
        if ["F", "FFilter", "FDecodeParms"]
            .iter()
            .any(|key| source.get(key).is_some())
        {
            return Err(PdfError::UnsupportedFeature(
                "replacing external-file stream data".into(),
            ));
        }
        let mut builder =
            crate::objects::pdf_dict::BudgetedDictBuilder::new(budget, limits.max_objects)?;
        for (key, value) in source.into_iter() {
            if matches!(key.as_str(), "Length" | "Filter" | "DecodeParms" | "DL") {
                continue;
            }
            if key.len() > limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("stream dictionary key size".into()));
            }
            let (key, key_lease) = crate::budget::copy_string(key, budget)?;
            let (value, value_lease) =
                crate::objects::clone_object_with_budget(value, budget, limits, 1)?;
            builder.insert(key, value, key_lease, value_lease)?;
        }
        // Format into stack storage so even the temporary Length token needs no heap.
        let mut digits = [0u8; 40];
        let mut start = digits.len();
        let mut remaining = data.len();
        loop {
            start -= 1;
            digits[start] = b'0' + (remaining % 10) as u8;
            remaining /= 10;
            if remaining == 0 {
                break;
            }
        }
        if digits.len() - start > limits.max_token_bytes || "Length".len() > limits.max_token_bytes
        {
            return Err(PdfError::ResourceLimit("stream Length token size".into()));
        }
        let (token, token_lease) =
            crate::budget::copy_string(std::str::from_utf8(&digits[start..]).unwrap(), budget)?;
        let length = crate::objects::pdf_number::PdfNumber::from_owned_lexeme(token)?;
        let (key, key_lease) = crate::budget::copy_string("Length", budget)?;
        builder.insert(key, PdfObject::PdfNumber(length), key_lease, token_lease)?;
        let (dict, lease) = builder.finish()?;
        Ok((
            Self {
                dict,
                data,
                state: StreamDataState::Decoded,
                crypt_processed: false,
            },
            lease,
        ))
    }

    pub fn data_state(&self) -> StreamDataState {
        self.state
    }

    pub fn dict(&self) -> &PdfDict {
        &self.dict
    }

    pub fn raw_data(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub(crate) fn crypt_processed(&self) -> bool {
        self.crypt_processed
    }
    pub(crate) fn mark_crypt_processed(&mut self) {
        self.crypt_processed = true;
    }

    pub(crate) fn clone_with_budget(
        &self,
        budget: &crate::budget::OperationBudget,
        limits: crate::parser::options::Limits,
        depth: usize,
    ) -> PdfResult<(Self, crate::budget::MemoryLease)> {
        let (dict, mut lease) =
            crate::objects::clone_dict_with_budget(&self.dict, budget, limits, depth)?;
        if self.data.len() > limits.max_raw_stream_bytes {
            return Err(PdfError::ResourceLimit("stream clone byte size".into()));
        }
        let (data, mut data_lease) =
            crate::budget::BudgetedBytes::copy_from_slice(budget, &self.data)?
                .into_reserved_parts();
        lease.absorb(&mut data_lease)?;
        Ok((
            Self {
                dict,
                data,
                state: self.state,
                crypt_processed: self.crypt_processed,
            },
            lease,
        ))
    }

    /// Consume the stream without decoding its payload.
    pub fn into_raw_parts(self) -> (PdfDict, Vec<u8>) {
        (self.dict, self.data)
    }

    /// Replace the encoded payload with unfiltered, decoded bytes.
    /// External-file streams are deliberately unsupported. Other dictionary
    /// entries (including image attributes) remain the caller's responsibility.
    pub fn with_decoded_data(mut self, data: Vec<u8>) -> PdfResult<Self> {
        if ["F", "FFilter", "FDecodeParms"]
            .iter()
            .any(|key| self.dict.get(key).is_some())
        {
            return Err(PdfError::UnsupportedFeature(
                "replacing external-file stream data".into(),
            ));
        }
        let length = crate::objects::pdf_number::PdfNumber::new_from_lexeme(
            data.len().to_string().as_bytes(),
        )?;
        self.dict
            .try_insert("Length".into(), PdfObject::PdfNumber(length))?;
        for key in ["Filter", "DecodeParms", "DL"] {
            self.dict.remove(key);
        }
        self.data = data;
        self.state = StreamDataState::Decoded;
        self.crypt_processed = false;
        Ok(self)
    }

    pub fn decode_data<R: Seek + Read>(&self, ctx: &PDFContext<R>) -> PdfResult<Vec<u8>> {
        ctx.with_decode_depth(|| self.decode_data_inner(ctx))
    }

    fn decode_data_inner<R: Seek + Read>(&self, ctx: &PDFContext<R>) -> PdfResult<Vec<u8>> {
        self.decode_data_using(ctx, true, None, apply_filters, || {
            self.copy_unfiltered_data(ctx)
        })
    }

    pub(crate) fn decode_jpx_image_data<R: Seek + Read>(
        &self,
        ctx: &PDFContext<R>,
        preserve_indices: bool,
    ) -> PdfResult<Vec<u8>> {
        ctx.with_decode_depth(|| {
            self.decode_data_using(
                ctx,
                true,
                None,
                |filter, params, input, mut context| {
                    context.jpx_preserve_indices = preserve_indices;
                    apply_filters(filter, params, input, context)
                },
                || self.copy_unfiltered_data(ctx),
            )
        })
    }

    pub(crate) fn encoded_jpx_payload<R: Seek + Read>(
        &self,
        ctx: &PDFContext<R>,
    ) -> PdfResult<Vec<u8>> {
        ctx.with_decode_depth(|| {
            self.decode_data_using(ctx, false, None, crate::filter::encoded_jpx_payload, || {
                Err(PdfError::FilterError(
                    "JPX requires an encoded filter stage".into(),
                ))
            })
        })
    }

    /// Return separately owned decoded bytes charged to the caller's pool.
    /// Resolved Filter/DecodeParms copies share the pool during decoding.
    /// Stored source bytes and first source loading/native state are excluded.
    /// Non-migrated codecs explicitly fail in budget dispatch.
    pub fn decode_data_with_budget<R: Seek + Read>(
        &self,
        ctx: &PDFContext<R>,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<crate::budget::BudgetedBytes> {
        ctx.with_decode_depth(|| {
            self.decode_data_using(
                ctx,
                false,
                Some(budget),
                |filter, params, input, context| {
                    crate::filter::apply_filters_with_budget(filter, params, input, context, budget)
                },
                || {
                    if self.data.len() > ctx.limits().max_decoded_bytes {
                        return Err(PdfError::ResourceLimit("unfiltered stream size".into()));
                    }
                    crate::budget::BudgetedBytes::copy_from_slice(budget, &self.data)
                },
            )
        })
    }

    fn decode_data_using<R: Seek + Read, T>(
        &self,
        ctx: &PDFContext<R>,
        load_globals: bool,
        budget: Option<&crate::budget::OperationBudget>,
        decode: impl Fn(&PdfObject, Option<&PdfObject>, &[u8], FilterContext<'_>) -> PdfResult<T>,
        copy: impl Fn() -> PdfResult<T>,
    ) -> PdfResult<T> {
        // External payloads are not loaded by this API. Returning the inline
        // bytes would silently substitute a different stream's contents.
        if ["F", "FFilter", "FDecodeParms"]
            .iter()
            .any(|key| self.dict.get(key).is_some())
        {
            return Err(PdfError::UnsupportedFeature(
                "decoding external-file stream data".into(),
            ));
        }
        if self.state == StreamDataState::Decoded {
            return copy();
        }
        let mut settings_lease = budget.map(|budget| budget.reserve(0)).transpose()?;
        let mut resolve = |object: &PdfObject, params| {
            if let Some(budget) = budget {
                let (object, mut lease) =
                    resolve_filter_setting_with_budget(object, ctx, params, budget)?;
                settings_lease.as_mut().unwrap().absorb(&mut lease)?;
                Ok(object)
            } else {
                resolve_filter_setting(object, ctx, params)
            }
        };
        let params = match self.dict.get("DecodeParms") {
            Some(obj) => Some(resolve(obj, true)?),
            None => None,
        };
        let filter = self
            .dict
            .get("Filter")
            .map(|obj| resolve(obj, false))
            .transpose()?;
        let has_jbig2 = filter.as_ref().is_some_and(|filter| match filter {
            PdfObject::PdfName(name) => name.name() == "JBIG2Decode",
            PdfObject::PdfArray(array) => array
                .into_iter()
                .any(|f| f.as_name().is_some_and(|n| n.name() == "JBIG2Decode")),
            _ => false,
        });
        let has_dct = filter.as_ref().is_some_and(|filter| match filter {
            PdfObject::PdfName(name) => matches!(name.name(), "DCT" | "DCTDecode"),
            PdfObject::PdfArray(array) => array.into_iter().any(|f| {
                f.as_name()
                    .is_some_and(|n| matches!(n.name(), "DCT" | "DCTDecode"))
            }),
            _ => false,
        });
        let globals_with_budget = if let Some(budget) = budget.filter(|_| has_jbig2) {
            resolve_jbig2_globals_with_budget(params.as_ref(), ctx, budget)?
        } else {
            None
        };
        let jbig2_globals = if has_jbig2 && load_globals && budget.is_none() {
            resolve_jbig2_globals(params.as_ref(), ctx)?
        } else {
            None
        };
        let local = ctx.limits().operation_budget();
        let normalized = if (has_jbig2 || has_dct)
            && ["Width", "Height", "BitsPerComponent"].iter().any(|key| {
                self.dict
                    .get(key)
                    .is_some_and(|v| v.as_reference().is_some())
            }) {
            let pool = budget.unwrap_or(&local);
            let (mut dict, mut lease) =
                crate::objects::clone_dict_with_budget(&self.dict, pool, ctx.limits(), 0)?;
            for key in ["Width", "Height", "BitsPerComponent"] {
                if let Some(value) = dict.get(key) {
                    let (resolved, mut value_lease) =
                        resolve_setting_reference_with_budget(value, ctx, pool)?;
                    dict.replace_existing(key, resolved)?;
                    lease.absorb(&mut value_lease)?;
                }
            }
            Some((dict, lease))
        } else {
            None
        };
        if let Some(filter) = filter
            .as_ref()
            .filter(|obj| !matches!(obj, PdfObject::PdfNull))
        {
            let recovered = std::cell::Cell::new(false);
            let recovered_parameters = std::cell::Cell::new(false);
            let result = decode(
                filter,
                params.as_ref(),
                self.data.as_slice(),
                FilterContext {
                    jpx_preserve_indices: false,
                    stream_dict: Some(
                        normalized
                            .as_ref()
                            .map(|(dict, _)| dict)
                            .unwrap_or(&self.dict),
                    ),
                    jbig2_globals: globals_with_budget
                        .as_ref()
                        .map(|bytes| bytes.as_slice())
                        .or(jbig2_globals.as_deref()),
                    max_decoded_bytes: ctx.limits().max_decoded_bytes,
                    max_image_pixels: ctx.limits().max_image_pixels,
                    crypt_already_processed: self.crypt_processed,
                    compatible: ctx.options().mode == crate::parser::syntax::ParseMode::Compatible,
                    recovered: Some(&recovered),
                    recovered_filter_parameters: Some(&recovered_parameters),
                },
            );
            if recovered_parameters.get() {
                ctx.record_structure_recovery(
                    crate::parser::diagnostic::StructureReason::FilterParameters,
                )?;
            }
            if recovered.get() {
                ctx.record_structure_recovery(
                    crate::parser::diagnostic::StructureReason::ImageTailRecovery,
                )?;
            }
            result
        } else {
            copy()
        }
    }

    fn copy_unfiltered_data<R: Seek + Read>(&self, ctx: &PDFContext<R>) -> PdfResult<Vec<u8>> {
        if self.data.len() > ctx.limits().max_decoded_bytes {
            return Err(PdfError::ResourceLimit("unfiltered stream size".into()));
        }
        let mut data = Vec::new();
        data.try_reserve_exact(self.data.len())
            .map_err(|_| PdfError::ResourceLimit("unfiltered stream allocation".into()))?;
        data.extend_from_slice(&self.data);
        Ok(data)
    }
}

/// Resolve only the filter settings, not arbitrary stream dictionary graphs.
/// The context chooses the latest or source view for every referenced item.
pub(crate) fn resolve_setting_reference_with_budget<R: Seek + Read>(
    obj: &PdfObject,
    ctx: &PDFContext<R>,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
    let (mut lease, mut object) = {
        let (object, lease) =
            crate::objects::clone_object_with_budget(obj, budget, ctx.limits(), 0)?;
        (lease, object)
    };
    let mut depth = 0;
    while object.as_reference().is_some() {
        if depth >= ctx.limits().max_depth {
            return Err(PdfError::ResourceLimit(
                "filter setting reference depth".into(),
            ));
        }
        let (next, next_lease) = ctx.resolve_owned_with_budget(&object, budget)?;
        object = next;
        lease = next_lease;
        depth += 1;
    }
    Ok((object, lease))
}

pub(crate) fn resolve_filter_setting_with_budget<R: Seek + Read>(
    obj: &PdfObject,
    ctx: &PDFContext<R>,
    params: bool,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
    let (object, source_lease) = resolve_setting_reference_with_budget(obj, ctx, budget)?;
    if let PdfObject::PdfArray(array) = &object {
        if array.len() > ctx.limits().max_objects {
            return Err(PdfError::ResourceLimit("filter setting array items".into()));
        }
        let mut lease = budget.reserve(
            array
                .len()
                .checked_mul(std::mem::size_of::<PdfObject>())
                .ok_or_else(|| PdfError::ResourceLimit("filter settings table size".into()))?,
        )?;
        let mut items = Vec::new();
        items
            .try_reserve_exact(array.len())
            .map_err(|_| PdfError::ResourceLimit("filter settings table allocation".into()))?;
        lease.resize(
            items
                .capacity()
                .checked_mul(std::mem::size_of::<PdfObject>())
                .ok_or_else(|| PdfError::ResourceLimit("filter settings table capacity".into()))?,
        )?;
        for item in array {
            let (item, item_lease) = resolve_setting_reference_with_budget(item, ctx, budget)?;
            let (item, mut item_lease) = if params {
                resolve_parameter_dict_with_budget(item, item_lease, ctx, budget)?
            } else {
                (item, item_lease)
            };
            items.push(item);
            lease.absorb(&mut item_lease)?;
        }
        drop(object);
        drop(source_lease);
        Ok((
            PdfObject::PdfArray(crate::objects::pdf_array::PdfArray::from_items(items)),
            lease,
        ))
    } else if params {
        resolve_parameter_dict_with_budget(object, source_lease, ctx, budget)
    } else {
        Ok((object, source_lease))
    }
}

const PARAMETER_KEYS: &[&str] = &[
    "Predictor",
    "Columns",
    "Colors",
    "BitsPerComponent",
    "EarlyChange",
    "K",
    "Rows",
    "EndOfLine",
    "EncodedByteAlign",
    "EndOfBlock",
    "BlackIs1",
    "DamagedRowsBeforeError",
];

fn resolve_parameter_dict_with_budget<R: Seek + Read>(
    object: PdfObject,
    lease: crate::budget::MemoryLease,
    ctx: &PDFContext<R>,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<(PdfObject, crate::budget::MemoryLease)> {
    let PdfObject::PdfDict(dict) = &object else {
        return Ok((object, lease));
    };
    let mut builder = super::pdf_dict::BudgetedDictBuilder::new(budget, ctx.limits().max_objects)?;
    for (key, value) in dict {
        let (value, value_lease) = if PARAMETER_KEYS.contains(&key.as_str()) {
            resolve_setting_reference_with_budget(value, ctx, budget)?
        } else {
            crate::objects::clone_object_with_budget(value, budget, ctx.limits(), 0)?
        };
        let (key, key_lease) = crate::budget::copy_string(key, budget)?;
        builder.insert(key, value, key_lease, value_lease)?;
    }
    let (dict, result_lease) = builder.finish()?;
    drop(object);
    drop(lease);
    Ok((PdfObject::PdfDict(dict), result_lease))
}

pub(crate) fn resolve_filter_setting<R: Seek + Read>(
    obj: &PdfObject,
    ctx: &PDFContext<R>,
    params: bool,
) -> PdfResult<PdfObject> {
    let obj = resolve_setting_reference(obj, ctx)?;
    match obj {
        PdfObject::PdfArray(array) => {
            if array.len() > ctx.limits().max_objects {
                return Err(PdfError::ResourceLimit("filter setting array items".into()));
            }
            let mut result = crate::objects::pdf_array::PdfArray::default();
            for item in &array {
                let item = resolve_setting_reference(item, ctx)?;
                let item = if params {
                    resolve_parameter_dict(item, ctx)?
                } else {
                    item
                };
                result.try_add_obj(item)?;
            }
            Ok(PdfObject::PdfArray(result))
        }
        obj if params => resolve_parameter_dict(obj, ctx),
        obj => Ok(obj),
    }
}

fn resolve_parameter_dict<R: Seek + Read>(
    obj: PdfObject,
    ctx: &PDFContext<R>,
) -> PdfResult<PdfObject> {
    let PdfObject::PdfDict(mut dict) = obj else {
        return Ok(obj);
    };
    // Stream-valued JBIG2Globals is resolved lazily by its existing handler.
    for &key in PARAMETER_KEYS {
        if let Some(value) = dict.get(key) {
            let value = resolve_setting_reference(value, ctx)?;
            dict.try_insert(key.into(), value)?;
        }
    }
    Ok(PdfObject::PdfDict(dict))
}

pub(crate) fn resolve_setting_reference<R: Seek + Read>(
    obj: &PdfObject,
    ctx: &PDFContext<R>,
) -> PdfResult<PdfObject> {
    let mut object = obj.clone();
    let mut depth = 0usize;
    while matches!(object, PdfObject::PdfReference(_)) {
        if depth >= ctx.limits().max_depth {
            return Err(PdfError::ResourceLimit(
                "filter setting reference depth".into(),
            ));
        }
        object = ctx.resolve_owned(&object)?;
        depth += 1;
    }
    Ok(object)
}

fn resolve_jbig2_globals_with_budget<R: Seek + Read>(
    params: Option<&PdfObject>,
    ctx: &PDFContext<R>,
    budget: &crate::budget::OperationBudget,
) -> PdfResult<Option<crate::budget::BudgetedBytes>> {
    let globals = match params {
        Some(PdfObject::PdfDict(dict)) => dict.get("JBIG2Globals"),
        Some(PdfObject::PdfArray(array)) => {
            let mut found = None;
            for item in array {
                if let Some(value) = item.as_dict().and_then(|d| d.get("JBIG2Globals")) {
                    if found.is_some() {
                        return Err(PdfError::UnsupportedFeature(
                            "multiple JBIG2Globals stages".into(),
                        ));
                    }
                    found = Some(value);
                }
            }
            found
        }
        _ => None,
    };
    let Some(globals) = globals else {
        return Ok(None);
    };
    let (object, _lease) = resolve_setting_reference_with_budget(globals, ctx, budget)?;
    let stream = object
        .as_stream()
        .ok_or_else(|| PdfError::FilterError("JBIG2Globals must resolve to a stream".into()))?;
    stream.decode_data_with_budget(ctx, budget).map(Some)
}

fn resolve_jbig2_globals<R: Seek + Read>(
    params: Option<&PdfObject>,
    ctx: &PDFContext<R>,
) -> PdfResult<Option<Vec<u8>>> {
    let Some(params) = params else {
        return Ok(None);
    };
    match params {
        PdfObject::PdfDict(dict) => resolve_jbig2_globals_from_dict(dict, ctx),
        PdfObject::PdfArray(array) => {
            for item in array {
                let Some(dict) = item.as_dict() else {
                    continue;
                };
                if let Some(data) = resolve_jbig2_globals_from_dict(dict, ctx)? {
                    return Ok(Some(data));
                }
            }
            Ok(None)
        }
        _ => Ok(None),
    }
}

fn resolve_jbig2_globals_from_dict<R: Seek + Read>(
    dict: &PdfDict,
    ctx: &PDFContext<R>,
) -> PdfResult<Option<Vec<u8>>> {
    let Some(globals) = dict.get("JBIG2Globals") else {
        return Ok(None);
    };
    let resolved = resolve_setting_reference(globals, ctx)?;
    let stream = resolved.as_stream().ok_or(PdfError::FilterError(
        "JBIG2Globals must resolve to a stream".to_string(),
    ))?;
    stream.decode_data(ctx).map(Some)
}

#[cfg(test)]
mod setting_tests {
    use super::*;

    #[test]
    fn budgeted_globals_stream_and_cycle_share_the_operation_pool() {
        use crate::{
            budget::OperationBudget,
            document::Document,
            objects::{pdf_name::PdfName, pdf_reference::PdfReference},
            parser::options::{Limits, ParseOptions},
        };
        let globals = include_bytes!("../../tests/fixtures/v06/patterns-global.jb2");
        let page = include_bytes!("../../tests/fixtures/v06/halftone-0.jb2");
        let doc = Document::new_with_options(ParseOptions {
            limits: Limits {
                max_depth: 8,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let id = doc
            .try_insert_object(PdfObject::PdfStream(PdfStream::new(
                Default::default(),
                globals.to_vec(),
            )))
            .unwrap();
        let mut params = PdfDict::default();
        params.insert(
            "JBIG2Globals".into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        );
        let mut dict = PdfDict::default();
        dict.insert(
            "Filter".into(),
            PdfObject::PdfName(PdfName::new("JBIG2Decode".into())),
        );
        dict.insert("DecodeParms".into(), PdfObject::PdfDict(params));
        let stream = PdfStream::new(dict, page.to_vec());
        let pool = OperationBudget::new(8 * 1024 * 1024);
        let decoded = stream.decode_data_with_budget(doc.ctx(), &pool).unwrap();
        assert_eq!(
            decoded.as_slice(),
            crate::filter::jbig2::jbig2_decode(
                page,
                None,
                FilterContext {
                    jbig2_globals: Some(globals),
                    ..Default::default()
                }
            )
            .unwrap()
        );
        drop(decoded);
        assert_eq!(pool.live_bytes(), 0);
        doc.update_object(id, PdfObject::PdfStream(stream.clone()))
            .unwrap();
        assert!(matches!(
            stream.decode_data_with_budget(doc.ctx(), &pool),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(pool.live_bytes(), 0);
    }

    #[test]
    fn decoded_dictionary_rebuild_budget_tracks_length_and_peak() {
        use crate::budget::OperationBudget;
        use crate::objects::pdf_number::PdfNumber;
        let mut source = PdfDict::default();
        source.insert("Length".into(), PdfObject::PdfNumber(PdfNumber::new(99.0)));
        source.insert("Filter".into(), PdfObject::PdfNull);
        source.insert("DecodeParms".into(), PdfObject::PdfNull);
        source.insert("DL".into(), PdfObject::PdfNumber(PdfNumber::new(99.0)));
        source.insert("Custom".into(), PdfObject::PdfNumber(PdfNumber::new(25.0)));
        let limits = crate::parser::options::Limits::default();
        let probe = OperationBudget::new(16384);
        let expected = PdfStream::from_decoded_data(source.clone(), b"abc".to_vec()).unwrap();
        let (output, lease) =
            PdfStream::from_decoded_data_with_budget(&source, b"abc".to_vec(), &probe, limits)
                .unwrap();
        assert_eq!(output, expected);
        assert_eq!(output.data_state(), StreamDataState::Decoded);
        assert_eq!(probe.live_bytes(), lease.bytes());
        let peak = probe.peak_bytes();
        drop(output);
        drop(lease);
        assert_eq!(probe.live_bytes(), 0);
        let pool = OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(
            PdfStream::from_decoded_data_with_budget(&source, b"abc".to_vec(), &pool, limits)
                .is_err()
        );
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let (output, lease) =
            PdfStream::from_decoded_data_with_budget(&source, b"abc".to_vec(), &pool, limits)
                .unwrap();
        assert_eq!(output, expected);
        drop(output);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        source.insert("F".into(), PdfObject::PdfNull);
        assert!(
            PdfStream::from_decoded_data_with_budget(&source, Vec::new(), &pool, limits).is_err()
        );
        assert_eq!(pool.live_bytes(), 0);
    }
    use crate::objects::{pdf_array::PdfArray, pdf_name::PdfName, pdf_reference::PdfReference};

    #[test]
    fn budgeted_stream_decode_preserves_source_and_owns_result_capacity() {
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        let mut filters = PdfArray::default();
        for _ in 0..2 {
            filters
                .try_add_obj(PdfObject::PdfName(PdfName::new("AHx".into())))
                .unwrap();
        }
        let id = ctx.try_insert_object(PdfObject::PdfArray(filters)).unwrap();
        let mut dict = PdfDict::default();
        dict.try_insert(
            "Filter".into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        )
        .unwrap();
        let stream = PdfStream::new(dict, b"3631363236333E>".to_vec());
        let probe = crate::budget::OperationBudget::new(16384);
        let result = stream.decode_data_with_budget(&ctx, &probe).unwrap();
        let peak = probe.peak_bytes();
        assert!(peak >= 10);
        drop(result);
        assert_eq!(probe.live_bytes(), 0);
        let budget = crate::budget::OperationBudget::new(peak);
        let neighbor = budget.reserve(1).unwrap();
        assert!(matches!(
            stream.decode_data_with_budget(&ctx, &budget),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(budget.live_bytes(), 1);
        assert_eq!(stream.raw_data(), b"3631363236333E>");
        assert_eq!(stream.data_state(), StreamDataState::RawEncoded);
        drop(neighbor);
        let result = stream.decode_data_with_budget(&ctx, &budget).unwrap();
        assert_eq!(result.as_slice(), b"abc");
        assert_eq!(budget.live_bytes(), result.capacity());
        drop(result);
        assert_eq!(budget.live_bytes(), 0);
        let decoded = PdfStream::from_decoded_data(PdfDict::default(), b"plain".to_vec()).unwrap();
        let result = decoded.decode_data_with_budget(&ctx, &budget).unwrap();
        assert_eq!(result.as_slice(), b"plain");
        assert_eq!(budget.live_bytes(), result.capacity());
        drop(result);
        assert_eq!(budget.live_bytes(), 0);
    }

    #[test]
    fn budgeted_parameter_resolution_preserves_unknown_references_and_releases_temporaries() {
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        let number = ctx
            .try_insert_object(PdfObject::PdfNumber(
                crate::objects::pdf_number::PdfNumber::new(3.0),
            ))
            .unwrap();
        let mut dict = PdfDict::default();
        dict.insert(
            "Columns".into(),
            PdfObject::PdfReference(PdfReference::from_id(number)),
        );
        dict.insert(
            "Unknown".into(),
            PdfObject::PdfReference(PdfReference::from_id(number)),
        );
        let id = ctx.try_insert_object(PdfObject::PdfDict(dict)).unwrap();
        let mut array = PdfArray::default();
        array.add_obj(PdfObject::PdfReference(PdfReference::from_id(id)));
        array.add_obj(PdfObject::PdfNull);
        let object = PdfObject::PdfArray(array);
        let expected = resolve_filter_setting(&object, &ctx, true).unwrap();
        let probe = crate::budget::OperationBudget::new(16384);
        let (copy, lease) =
            resolve_filter_setting_with_budget(&object, &ctx, true, &probe).unwrap();
        assert_eq!(copy, expected);
        let dict = copy.as_array().unwrap().get(0).unwrap().as_dict().unwrap();
        assert_eq!(
            dict.get("Columns").unwrap().as_number().unwrap().lexeme(),
            "3"
        );
        assert!(dict.get("Unknown").unwrap().as_reference().is_some());
        let final_bytes = probe.live_bytes();
        let peak = probe.peak_bytes();
        drop(copy);
        drop(lease);
        assert_eq!(probe.live_bytes(), 0);
        let pool = crate::budget::OperationBudget::new(peak);
        let neighbor = pool.reserve(1).unwrap();
        assert!(resolve_filter_setting_with_budget(&object, &ctx, true, &pool).is_err());
        assert_eq!(pool.live_bytes(), 1);
        drop(neighbor);
        let (copy, lease) = resolve_filter_setting_with_budget(&object, &ctx, true, &pool).unwrap();
        assert_eq!(copy, expected);
        assert_eq!(pool.live_bytes(), final_bytes);
        drop(copy);
        drop(lease);
        assert_eq!(pool.live_bytes(), 0);
        let mut dict = PdfDict::default();
        dict.insert(
            "Filter".into(),
            PdfObject::PdfName(PdfName::new("AHx".into())),
        );
        dict.insert(
            "DecodeParms".into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        );
        let budget = crate::budget::OperationBudget::new(16384);
        let stream = PdfStream::new(dict.clone(), b"616263>".to_vec());
        let result = stream.decode_data_with_budget(&ctx, &budget).unwrap();
        assert_eq!(result.as_slice(), b"abc");
        assert_eq!(budget.live_bytes(), result.capacity());
        drop(result);
        let neighbor = budget.reserve(3).unwrap();
        let invalid = PdfStream::new(dict, b"GG>".to_vec());
        assert!(invalid.decode_data_with_budget(&ctx, &budget).is_err());
        assert_eq!(budget.live_bytes(), 3);
        drop(neighbor);
    }

    #[test]
    fn external_stream_settings_do_not_decode_inline_placeholder_bytes() {
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        for key in ["F", "FFilter", "FDecodeParms"] {
            for value in [
                PdfObject::PdfNull,
                PdfObject::PdfName(PdfName::new("external".into())),
            ] {
                let mut dict = PdfDict::default();
                dict.insert(key.into(), value);
                let stream = PdfStream::new(dict.clone(), b"inline placeholder".to_vec());
                assert!(matches!(
                    stream.decode_data(&ctx),
                    Err(PdfError::UnsupportedFeature(_))
                ));
                assert_eq!(stream.dict(), &dict);
                assert_eq!(stream.raw_data(), b"inline placeholder");
            }
        }
    }

    #[test]
    fn decoded_state_cleans_encoding_and_honors_decode_limit() {
        let mut dict = PdfDict::default();
        dict.try_insert(
            "Filter".into(),
            PdfObject::PdfName(PdfName::new("ASCIIHexDecode".into())),
        )
        .unwrap();
        let raw = PdfStream::new(dict.clone(), b"0102>".to_vec());
        assert_eq!(raw.data_state(), StreamDataState::RawEncoded);
        let decoded = PdfStream::from_decoded_data(dict, vec![1, 2]).unwrap();
        assert_eq!(decoded.data_state(), StreamDataState::Decoded);
        assert!(decoded.dict().get("Filter").is_none());
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        assert_eq!(
            raw.decode_data(&ctx).unwrap(),
            decoded.decode_data(&ctx).unwrap()
        );
        let cloned = decoded.clone();
        assert_eq!(cloned.data_state(), StreamDataState::Decoded);
        let (dict, data) = decoded.into_raw_parts();
        let reparsed = PdfStream::new(dict, data);
        assert_eq!(cloned, reparsed);
        assert_eq!(reparsed.data_state(), StreamDataState::RawEncoded);
        let limited: PDFContext<std::io::Cursor<Vec<u8>>> = PDFContext::new_without_source(
            PdfDict::default(),
            crate::parser::options::ParseOptions {
                limits: crate::parser::options::Limits {
                    max_decoded_bytes: 1,
                    ..Default::default()
                },
                ..Default::default()
            },
        );
        assert!(matches!(
            cloned.decode_data(&limited),
            Err(PdfError::ResourceLimit(_))
        ));
    }

    #[test]
    fn filter_arrays_resolve_indirect_null_parameters_and_latest_names() {
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        let name = ctx
            .try_insert_object(PdfObject::PdfName(PdfName::new("ASCIIHexDecode".into())))
            .unwrap();
        let null = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        let mut filter = PdfArray::default();
        filter.add_obj(PdfObject::PdfReference(PdfReference::from_id(name)));
        let mut params = PdfArray::default();
        params.add_obj(PdfObject::PdfReference(PdfReference::from_id(null)));
        let mut dict = PdfDict::default();
        dict.try_insert("Filter".into(), PdfObject::PdfArray(filter))
            .unwrap();
        dict.try_insert("DecodeParms".into(), PdfObject::PdfArray(params))
            .unwrap();
        let stream = PdfStream::new(dict, b"0102>".to_vec());
        assert_eq!(stream.decode_data(&ctx).unwrap(), [1, 2]);
        ctx.update_object(name, PdfObject::PdfName(PdfName::new("unsupported".into())))
            .unwrap();
        assert!(stream.decode_data(&ctx).is_err());
    }

    #[test]
    fn null_filter_is_unfiltered_and_reference_cycles_are_bounded() {
        let ctx: PDFContext<std::io::Cursor<Vec<u8>>> =
            PDFContext::new_without_source(PdfDict::default(), Default::default());
        let id = ctx.try_insert_object(PdfObject::PdfNull).unwrap();
        let mut dict = PdfDict::default();
        dict.try_insert(
            "Filter".into(),
            PdfObject::PdfReference(PdfReference::from_id(id)),
        )
        .unwrap();
        let stream = PdfStream::new(dict, b"plain".to_vec());
        assert_eq!(stream.decode_data(&ctx).unwrap(), b"plain");
        ctx.update_object(id, PdfObject::PdfReference(PdfReference::from_id(id)))
            .unwrap();
        assert!(matches!(
            stream.decode_data(&ctx),
            Err(PdfError::ResourceLimit(_))
        ));
    }
}
