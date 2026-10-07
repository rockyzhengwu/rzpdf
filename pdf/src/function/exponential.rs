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
    fn wide_finite_coefficients_and_range_clipping_do_not_overflow_early() {
        let doc = crate::document::PdfDocument::new().unwrap();
        let parse = |source: &str| {
            let object = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap();
            Exponential::try_new(&object, doc.ctx()).unwrap()
        };
        let f = parse(
            "<< /Domain [0 1] /N 1 /C0 [-300000000000000000000000000000000000000] /C1 [300000000000000000000000000000000000000] >>",
        );
        assert_eq!(f.eval(&[0.5]).unwrap(), vec![0.0]);
        assert_eq!(f.eval(&[0.0]).unwrap(), f.c0);
        assert_eq!(f.eval(&[1.0]).unwrap(), f.c1);
        let clipped = parse("<< /Domain [0 2] /N 128 /Range [0 1] >>");
        assert_eq!(clipped.eval(&[2.0]).unwrap(), vec![1.0]);
        let huge = parse("<< /Domain [0 2] /N 2000 /Range [0 1] >>");
        assert_eq!(huge.eval(&[2.0]).unwrap(), vec![1.0]);
        let constant = parse("<< /Domain [0 2] /N 2000 /C0 [0.5] /C1 [0.5] >>");
        assert_eq!(constant.eval(&[2.0]).unwrap(), vec![0.5]);
        let unbounded = parse("<< /Domain [0 2] /N 128 >>");
        assert!(matches!(
            unbounded.eval(&[2.0]),
            Err(PdfError::FunctionError(_))
        ));
        assert_eq!(unbounded.eval(&[1.0]).unwrap(), vec![1.0]);
    }

    #[test]
    fn parameter_arrays_have_exact_limits_and_reject_before_member_resolution() {
        use crate::{
            document::PdfDocument,
            objects::pdf_reference::PdfReference,
            parser::options::{Limits, ParseOptions},
        };
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let make = |bytes| {
            PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_objects: 2,
                    max_decoded_bytes: bytes,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap()
        };
        let good = parse("<< /Domain [0 1] /N 1 /C0 [0] /C1 [1] /Range [0 1] >>");
        let exact = make(8);
        assert_eq!(
            Exponential::try_new(&good, exact.ctx())
                .unwrap()
                .eval(&[0.5])
                .unwrap(),
            vec![0.5]
        );
        assert!(matches!(
            Exponential::try_new(&good, make(7).ctx()),
            Err(PdfError::ResourceLimit(_))
        ));
        for key in ["C0", "C1", "Range"] {
            let object = parse(&format!("<< /Domain [0 1] /N 1 /{key} [0 1 2] >>"));
            for _ in 0..2 {
                assert!(
                    matches!(
                        Exponential::try_new(&object, exact.ctx()),
                        Err(PdfError::ResourceLimit(_))
                    ),
                    "{key}"
                );
            }
            assert!(Exponential::try_new(&good, exact.ctx()).is_ok());
        }
        let doc = PdfDocument::new().unwrap();
        let cycle = doc.try_insert_object(PdfObject::PdfNull).unwrap();
        doc.update_object(cycle, PdfObject::PdfReference(PdfReference::from_id(cycle)))
            .unwrap();
        let bad = parse(&format!(
            "<< /Domain [0 1 {} {} R] /N 1 >>",
            cycle.obj_num(),
            cycle.gen_num()
        ));
        assert!(matches!(
            Exponential::try_new(&bad, doc.ctx()),
            Err(PdfError::FunctionError(_))
        ));
        assert!(Exponential::try_new(&good, doc.ctx()).is_ok());
    }

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
    pub(crate) fn input_domain(&self) -> &[f32] {
        &self.domain
    }
    pub(super) fn retained_own_bytes(&self) -> PdfResult<usize> {
        super::retained_total(&[
            super::retained_vector_bytes(&self.domain)?,
            super::retained_vector_bytes(&self.range)?,
            super::retained_vector_bytes(&self.c0)?,
            super::retained_vector_bytes(&self.c1)?,
        ])
    }
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
            if key == "Domain" && array.len() != 2 {
                return Err(PdfError::FunctionError(
                    "Type2 requires one ordered Domain pair".into(),
                ));
            }
            if array.len() > ctx.limits().max_objects
                || array
                    .len()
                    .checked_mul(std::mem::size_of::<f32>())
                    .is_none_or(|bytes| bytes > ctx.limits().max_decoded_bytes)
            {
                return Err(PdfError::ResourceLimit(format!("Type2 {key} parameters")));
            }
            let mut result = Vec::new();
            result
                .try_reserve_exact(array.len())
                .map_err(|_| PdfError::ResourceLimit(format!("Type2 {key} allocation")))?;
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
        out.try_reserve_exact(self.n)
            .map_err(|_| PdfError::ResourceLimit("Type2 output allocation".into()))?;
        let mut x = inputs[0];
        if x < self.domain[0] {
            x = self.domain[0]
        } else if x > self.domain[1] {
            x = self.domain[1];
        }
        let power = f64::from(x).powf(f64::from(self.en));
        for i in 0..self.n {
            let c0 = f64::from(self.c0[i]);
            let c1 = f64::from(self.c1[i]);
            // Finite f32 endpoints can overflow when subtracted in f32.
            // Constant outputs do not depend on even an overflowing power.
            let mut v = if c0 == c1 { c0 } else { c0 + power * (c1 - c0) };
            if v.is_nan() {
                return Err(PdfError::FunctionError(
                    "Type2 result is non-finite".to_string(),
                ));
            }
            if !self.range.is_empty() {
                let l = f64::from(self.range[2 * i]);
                let m = f64::from(self.range[2 * i + 1]);
                if v > m {
                    v = m;
                } else if v < l {
                    v = l;
                }
            }
            let v = v as f32;
            if !v.is_finite() {
                return Err(PdfError::FunctionError("Type2 result is non-finite".into()));
            }
            out.push(v)
        }
        Ok(out)
    }
}
