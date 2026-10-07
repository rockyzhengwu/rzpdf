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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        objects::pdf_dict::PdfDict,
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::{collections::HashMap, io::Cursor};

    #[test]
    fn type2_defaults_dimensions_and_invalid_inputs() {
        let syntax =
            SyntaxParser::new(StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap());
        let ctx = PDFContext::try_new(
            PdfParser::new(syntax),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let parse = |source: &str| {
            let obj = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap();
            Exponential::try_new(&obj, &ctx)
        };
        let function = parse("<< /N 2 /Domain [0 1] >>").unwrap();
        assert_eq!(function.eval(&[0.5]).unwrap(), vec![0.25]);
        assert_eq!(function.eval(&[2.0]).unwrap(), vec![1.0]);
        assert!(function.eval(&[]).is_err());
        assert!(function.eval(&[f32::NAN]).is_err());
        assert!(function.eval(&[0.0, 1.0]).is_err());
        assert!(Exponential::default().eval(&[0.0]).is_err());
        for source in [
            "null",
            "<< /N 1 /Domain [] >>",
            "<< /N 1 /Domain [0 1 2] >>",
            "<< /N 1 /Domain [1 0] >>",
            "<< /N 1 /Domain [0 1] /C0 [0 1] >>",
            "<< /N 1 /Domain [0 1] /Range [0] >>",
            "<< /N 1 /Domain [0 1] /Range [1 0] >>",
            "<< /N 0.5 /Domain [-1 1] >>",
            "<< /N -1 /Domain [0 1] >>",
        ] {
            assert!(parse(source).is_err(), "{source}");
        }
        let function =
            parse("<< /N 1 /Domain [0 1] /C0 [0 1] /C1 [1 0] /Range [0 0.5 0 1] >>").unwrap();
        assert_eq!(function.eval(&[1.0]).unwrap(), vec![0.5, 0.0]);
    }
}

impl Exponential {
    pub(super) fn dimensions(&self) -> (usize, usize) {
        (self.m, self.n)
    }

    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        let dict = obj
            .as_dict()
            .ok_or_else(|| PdfError::FunctionError("Type2 requires a dictionary".to_string()))?;
        let number_array = |key: &str| -> PdfResult<Option<Vec<f32>>> {
            let Some(value) = dict.get(key) else {
                return Ok(None);
            };
            let value = ctx.resolve(value)?;
            let array = value
                .as_array()
                .ok_or_else(|| PdfError::FunctionError(format!("{key} requires an array")))?;
            let mut result = Vec::new();
            for value in array {
                let value = ctx.resolve(value)?;
                result.push(value.as_f32().filter(|v| v.is_finite()).ok_or_else(|| {
                    PdfError::FunctionError(format!("{key} requires finite numbers"))
                })?);
            }
            Ok(Some(result))
        };
        let exponent = ctx.resolve(
            dict.get("N")
                .ok_or_else(|| PdfError::FunctionError("Type2 requires N".to_string()))?,
        )?;
        let en = exponent
            .as_f32()
            .filter(|v| v.is_finite())
            .ok_or_else(|| PdfError::FunctionError("N requires a finite number".to_string()))?;
        let domain = number_array("Domain")?
            .ok_or_else(|| PdfError::FunctionError("Type2 requires Domain".to_string()))?;
        if domain.len() != 2 || domain[0] > domain[1] {
            return Err(PdfError::FunctionError(
                "Type2 requires one ordered Domain pair".to_string(),
            ));
        }
        if (domain[0] < 0.0 && en.fract() != 0.0)
            || (domain[0] <= 0.0 && domain[1] >= 0.0 && en < 0.0)
        {
            return Err(PdfError::FunctionError(
                "Domain contains undefined exponentiation".to_string(),
            ));
        }
        let c0 = number_array("C0")?.unwrap_or_else(|| vec![0.0]);
        let c1 = number_array("C1")?.unwrap_or_else(|| vec![1.0]);
        if c0.is_empty() || c0.len() != c1.len() {
            return Err(PdfError::FunctionError(
                "C0/C1 require equal nonempty lengths".to_string(),
            ));
        }
        let range = number_array("Range")?.unwrap_or_default();
        if dict.contains_key("Range")
            && (range.len() != c0.len() * 2 || range.chunks_exact(2).any(|pair| pair[0] > pair[1]))
        {
            return Err(PdfError::FunctionError(
                "Range requires one ordered pair per output".to_string(),
            ));
        }
        Ok(Self {
            n: c0.len(),
            m: 1,
            domain,
            range,
            c0,
            c1,
            en,
        })
    }
    pub fn eval(&self, inputs: &[f32]) -> PdfResult<Vec<f32>> {
        if inputs.len() != 1
            || !inputs[0].is_finite()
            || self.domain.len() != 2
            || self.n == 0
            || self.c0.len() != self.n
            || self.c1.len() != self.n
            || (!self.range.is_empty() && self.range.len() != 2 * self.n)
        {
            return Err(PdfError::FunctionError(
                "Invalid Type2 function or input dimensions".to_string(),
            ));
        }
        let mut out = Vec::new();
        let mut x = inputs[0];
        if x < self.domain[0] {
            x = self.domain[0]
        } else if x > self.domain[1] {
            x = self.domain[1];
        }
        for i in 0..self.n {
            let mut v = self.c0[i] + x.powf(self.en) * (self.c1[i] - self.c0[i]);
            if !v.is_finite() {
                return Err(PdfError::FunctionError(
                    "Type2 result is non-finite".to_string(),
                ));
            }
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
