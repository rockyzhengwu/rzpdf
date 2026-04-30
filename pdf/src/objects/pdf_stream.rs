use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    filter::{FilterContext, apply_filters},
    objects::{PdfObject, pdf_dict::PdfDict},
    pdf_context::PDFContext,
};

#[derive(Debug, PartialEq, Clone)]
pub struct PdfStream {
    dict: PdfDict,
    data: Vec<u8>,
}

impl PdfStream {
    pub fn new(dict: PdfDict, data: Vec<u8>) -> Self {
        PdfStream { dict, data }
    }

    pub fn dict(&self) -> &PdfDict {
        &self.dict
    }

    pub fn raw_data(&self) -> &[u8] {
        self.data.as_slice()
    }

    pub fn decode_data<R: Seek + Read>(&self, ctx: &PDFContext<R>) -> PdfResult<Vec<u8>> {
        let params = match self.dict.get("DecodeParms") {
            Some(obj) => Some(ctx.resolve_owned(obj)?),
            None => None,
        };
        let jbig2_globals = resolve_jbig2_globals(params.as_ref(), ctx)?;
        if let Some(filter) = self.dict.get("Filter") {
            apply_filters(
                filter,
                params.as_ref(),
                self.data.as_slice(),
                FilterContext {
                    stream_dict: Some(&self.dict),
                    jbig2_globals: jbig2_globals.as_deref(),
                },
            )
        } else {
            Ok(self.data.clone())
        }
    }
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
    let resolved = ctx.resolve_owned(globals)?;
    let stream = resolved.as_stream().ok_or(PdfError::FilterError(
        "JBIG2Globals must resolve to a stream".to_string(),
    ))?;
    stream.decode_data(ctx).map(Some)
}
