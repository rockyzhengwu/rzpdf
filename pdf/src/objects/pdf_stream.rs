use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    filter::apply_filter,
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
            Some(obj) => Some(ctx.resolve(obj)?),
            None => None,
        };
        if let Some(filter) = self.dict.get("Filter") {
            match filter {
                PdfObject::PdfName(n) => match params {
                    Some(PdfObject::PdfDict(d)) => {
                        apply_filter(n.name(), self.data.as_slice(), Some(&d))
                    }
                    None => apply_filter(n.name(), self.data.as_slice(), None),
                    _ => Err(PdfError::FilterError(
                        "Filter param must be Dict ".to_string(),
                    )),
                },
                PdfObject::PdfArray(filters) => match params {
                    Some(PdfObject::PdfArray(param_arr)) => {
                        assert_eq!(filters.len(), param_arr.len());
                        let mut data = self.data.clone();
                        for (name, param) in filters.into_iter().zip(param_arr.into_iter()) {
                            data = apply_filter(
                                name.as_name().unwrap().name(),
                                data.as_slice(),
                                Some(param.as_dict().unwrap()),
                            )?;
                        }
                        Ok(data)
                    }
                    None => {
                        let mut data = self.data.clone();
                        for name in filters.into_iter() {
                            data = apply_filter(
                                name.as_name().unwrap().name(),
                                data.as_slice(),
                                None,
                            )?;
                        }
                        Ok(data)
                    }
                    _ => Err(PdfError::FilterError(
                        "Filter param must be Dict ".to_string(),
                    )),
                },
                _ => Err(PdfError::FilterError(
                    "Stream filter must be PdfName or Array".to_string(),
                )),
            }
        } else {
            Ok(self.data.clone())
        }
    }
}
