use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone, Default)]
pub struct Exponential {
    domain: Vec<f32>,
    range: Vec<f32>,
    c0: Vec<f32>,
    c1: Vec<f32>,
    m: usize,
    n: usize,
    en: f32,
}

impl Exponential {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let mut function = Exponential::default();
        if let Some(en) = obj.as_dict().unwrap().get("N") {
            let v = en.as_f32().ok_or(PdfError::FunctionError(
                "Type2 Function N is not an nubmer".to_string(),
            ))?;
            function.en = v;
        } else {
            return Err(PdfError::FunctionError(
                "type2 Fuction N is None".to_string(),
            ));
        }

        let domain = obj
            .as_dict()
            .unwrap()
            .get("Domain")
            .ok_or(PdfError::FunctionError(
                "Type2 Function Domain is None".to_string(),
            ))?
            .as_array()
            .ok_or(PdfError::FunctionError(
                "Type2 function domain is not an array".to_string(),
            ))?;
        for v in domain.into_iter() {
            let value = v.as_f32().ok_or(PdfError::FunctionError(
                "Domain element is not number".to_string(),
            ))?;
            function.domain.push(value);
        }
        function.m = function.domain.len() / 2;
        if function.m != 1 {
            return Err(PdfError::FunctionError(
                "Type2 function with more than one input".to_string(),
            ))?;
        }

        if let Some(r) = obj.as_dict().unwrap().get("Range") {
            let range = r.as_array().ok_or(PdfError::FunctionError(
                "Type2 Function Range is not an array".to_string(),
            ))?;
            for v in range.into_iter() {
                let value = v.as_f32().ok_or(PdfError::FunctionError(
                    "Sampled Function range element is not an number".to_string(),
                ))?;
                function.range.push(value);
            }
            function.n = function.range.len() / 2;
        }

        if let Some(c0) = obj.as_dict().unwrap().get("C0") {
            let array = c0.as_array().ok_or(PdfError::FunctionError(
                "Type2 Function c0 is not an array".to_string(),
            ))?;
            if function.n != 0 && array.len() != function.n {
                return Err(PdfError::FunctionError(
                    "Type2 Function c0 len is error".to_string(),
                ));
            }
            for v in array.into_iter() {
                let value = v.as_f32().ok_or(PdfError::FunctionError(
                    "Type2 C0 element is not number".to_string(),
                ))?;
                function.c0.push(value);
            }
            function.n = array.len();
        } else {
            function.c0 = vec![0.0; function.n];
        }
        if let Some(c1) = obj.as_dict().unwrap().get("C1") {
            let array = c1.as_array().ok_or(PdfError::FunctionError(
                "Type2 Function c0 is not an array".to_string(),
            ))?;
            if function.n != 0 && array.len() != function.n {
                return Err(PdfError::FunctionError(
                    "Type2 Function c0 len is error".to_string(),
                ));
            }
            for v in array.into_iter() {
                let value = v.as_f32().ok_or(PdfError::FunctionError(
                    "Type2 C1 element is not number".to_string(),
                ))?;
                function.c1.push(value);
            }
        } else {
            function.c1 = vec![1.0; function.n];
        }
        Ok(function)
    }

    pub fn eval(&self, inputs: &[f32]) -> PdfResult<Vec<f32>> {
        let mut out = Vec::new();
        let mut x = inputs[0];
        if x < self.domain[0] {
            x = self.domain[0]
        } else if x > self.domain[1] {
            x = self.domain[1];
        }
        for i in 0..self.n {
            let mut v = self.c0[i] + x.powf(self.en) * (self.c1[i] - self.c0[i]);
            if !self.range.is_empty() {
                let l = self.range[2 * i];
                let m = self.range[2 * i + 1];
                if v > m {
                    v = m;
                } else if v < l {
                    v = l;
                }
            }
            out.push(v)
        }
        Ok(out)
    }
}
