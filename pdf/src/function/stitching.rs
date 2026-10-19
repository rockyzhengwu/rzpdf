use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

use super::Function;

#[derive(Debug, Clone, Default)]
pub struct Stitching {
    domain: Vec<f32>,
    functions: Vec<Function>,
    bounds: Vec<f32>,
    encode: Vec<f32>,
    range: Vec<f32>,
    m: usize,
    n: usize,
}

impl Stitching {
    pub(crate) fn input_domain(&self) -> &[f32] {
        &self.domain
    }
    pub(super) fn retained_own_bytes(&self) -> PdfResult<usize> {
        super::retained_total(&[
            super::retained_vector_bytes(&self.domain)?,
            super::retained_vector_bytes(&self.functions)?,
            super::retained_vector_bytes(&self.bounds)?,
            super::retained_vector_bytes(&self.encode)?,
            super::retained_vector_bytes(&self.range)?,
        ])
    }
    pub(super) fn children(&self) -> &[super::Function] {
        &self.functions
    }
    pub(super) fn dimensions(&self) -> (usize, usize) {
        (self.m, self.n)
    }

    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let mut remaining = ctx.limits().max_objects;
        if remaining == 0 {
            return Err(PdfError::ResourceLimit(
                "PDF function total nodes".to_string(),
            ));
        }
        remaining -= 1;
        Self::try_new_at_depth(obj, ctx, 0, &mut remaining)
    }

    pub(crate) fn try_new_at_depth<R: Seek + Read>(
        obj: &PdfObject,
        ctx: &PDFContext<R>,
        depth: usize,
        remaining_nodes: &mut usize,
    ) -> PdfResult<Self> {
        if depth >= ctx.limits().max_depth {
            return Err(PdfError::ResourceLimit("PDF function nesting".to_string()));
        }
        let obj = ctx.resolve(obj)?;
        let dict = obj
            .as_dict()
            .ok_or_else(|| PdfError::FunctionError("Type3 requires a dictionary".to_string()))?;
        let array_values = |key: &str| -> PdfResult<Vec<f32>> {
            let value = ctx.resolve(
                dict.get(key)
                    .ok_or_else(|| PdfError::FunctionError(format!("Type3 requires {key}")))?,
            )?;
            let array = value
                .as_array()
                .ok_or_else(|| PdfError::FunctionError(format!("{key} must be an array")))?;
            array
                .into_iter()
                .map(|v| {
                    ctx.resolve(v)?
                        .as_f32()
                        .filter(|n| n.is_finite())
                        .ok_or_else(|| {
                            PdfError::FunctionError(format!("{key} must contain finite numbers"))
                        })
                })
                .collect()
        };
        let domain = array_values("Domain")?;
        if domain.len() != 2 || domain[0] >= domain[1] {
            return Err(PdfError::FunctionError(
                "Type3 requires a nonempty ordered Domain pair".to_string(),
            ));
        }
        let entries = ctx
            .resolve(dict.get("Functions").ok_or_else(|| {
                PdfError::FunctionError("Type3 requires Functions".to_string())
            })?)?;
        let entries = entries
            .as_array()
            .ok_or_else(|| PdfError::FunctionError("Functions must be an array".to_string()))?;
        if entries.is_empty() {
            return Err(PdfError::FunctionError(
                "Functions must not be empty".to_string(),
            ));
        }
        if entries.len() > ctx.limits().max_objects {
            return Err(PdfError::ResourceLimit("Type3 function count".to_string()));
        }
        if entries.len() > *remaining_nodes {
            return Err(PdfError::ResourceLimit(
                "PDF function total nodes".to_string(),
            ));
        }
        let encode = array_values("Encode")?;
        let bounds = array_values("Bounds")?;
        if encode.len() != entries.len() * 2
            || bounds.len() != entries.len() - 1
            || bounds.windows(2).any(|p| p[0] >= p[1])
            || bounds.iter().any(|b| *b < domain[0] || *b > domain[1])
        {
            return Err(PdfError::FunctionError(
                "Invalid Type3 Bounds/Encode dimensions or ordering".to_string(),
            ));
        }
        let functions = entries
            .into_iter()
            .map(|f| super::create_function_at_depth(f, ctx, depth + 1, remaining_nodes))
            .collect::<PdfResult<Vec<_>>>()?;
        let (_, n) = functions[0].dimensions();
        if n == 0 || functions.iter().any(|f| f.dimensions() != (1, n)) {
            return Err(PdfError::FunctionError(
                "Type3 requires single-input functions with matching output dimensions".into(),
            ));
        }
        let range = if dict.contains_key("Range") {
            let range = array_values("Range")?;
            if range.len() != n * 2 || range.as_chunks::<2>().0.iter().any(|p| p[0] > p[1]) {
                return Err(PdfError::FunctionError(
                    "Type3 Range requires one ordered pair per output".into(),
                ));
            }
            range
        } else {
            Vec::new()
        };
        Ok(Self {
            domain,
            functions,
            bounds,
            encode,
            range,
            m: 1,
            n,
        })
    }
    pub fn eval(&self, inputs: &[f32]) -> PdfResult<Vec<f32>> {
        if inputs.len() != 1 || !inputs[0].is_finite() || self.m != 1 || self.n == 0 {
            return Err(PdfError::FunctionError(
                "Invalid Type3 input or function".into(),
            ));
        }
        // ISO 32000-1, 7.10.4: internal bounds belong to the right-hand
        // interval; the final interval includes Domain[1]. Interpolate in f64
        // so finite f32 endpoints do not overflow during subtraction.
        let x = inputs[0].clamp(self.domain[0], self.domain[1]);
        let index = self.bounds.partition_point(|bound| *bound <= x);
        let low = if index == 0 {
            self.domain[0]
        } else {
            self.bounds[index - 1]
        };
        let high = self.bounds.get(index).copied().unwrap_or(self.domain[1]);
        let start = f64::from(self.encode[2 * index]);
        let end = f64::from(self.encode[2 * index + 1]);
        // A last bound equal to Domain[1] creates a singleton final interval.
        let mapped = if low == high {
            start
        } else {
            start
                + (f64::from(x) - f64::from(low)) / (f64::from(high) - f64::from(low))
                    * (end - start)
        } as f32;
        if !mapped.is_finite() {
            return Err(PdfError::FunctionError(
                "Type3 mapped input is non-finite".into(),
            ));
        }
        let mut output = self.functions[index].eval(&[mapped])?;
        if output.len() != self.n || output.iter().any(|value| !value.is_finite()) {
            return Err(PdfError::FunctionError("Invalid Type3 child output".into()));
        }
        if !self.range.is_empty() {
            for (value, pair) in output.iter_mut().zip(self.range.as_chunks::<2>().0) {
                *value = value.clamp(pair[0], pair[1]);
            }
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::PdfDocument, io::stream_reader::StreamReader, parser::syntax::SyntaxParser,
    };
    use std::io::Cursor;

    fn parse(source: &str) -> PdfResult<Stitching> {
        let doc = PdfDocument::new()?;
        let object = SyntaxParser::new(StreamReader::try_new(Cursor::new(
            source.as_bytes().to_vec(),
        ))?)
        .get_object()?;
        Stitching::try_new(&object, doc.ctx())
    }

    const CHILD: &str = "<< /FunctionType 2 /Domain [0 1] /N 1 >>";

    #[test]
    fn stitching_intervals_clipping_reverse_encode_and_discontinuities() {
        let f = parse(&format!("<< /Domain [2 6] /Bounds [4] /Encode [0 1 1 0] /Functions [{CHILD} << /FunctionType 2 /Domain [0 1] /N 1 /C0 [10] /C1 [20] >>] >>")).unwrap();
        for (x, expected) in [
            (0., 0.),
            (2., 0.),
            (3., 0.5),
            (4., 20.),
            (5., 15.),
            (6., 10.),
            (8., 10.),
        ] {
            assert_eq!(f.eval(&[x]).unwrap(), vec![expected]);
        }
        let below = f32::from_bits(4f32.to_bits() - 1);
        assert!(f.eval(&[below]).unwrap()[0] < 1.);
        for input in [
            vec![],
            vec![0., 1.],
            vec![f32::NAN],
            vec![f32::INFINITY],
            vec![f32::NEG_INFINITY],
        ] {
            assert!(f.eval(&input).is_err());
        }
        assert!(Stitching::default().eval(&[0.]).is_err());
    }

    #[test]
    fn stitching_single_child_nested_and_output_range() {
        let inner = "<< /FunctionType 3 /Domain [0 1] /Bounds [] /Encode [1 0] /Functions [<< /FunctionType 2 /Domain [0 1] /N 1 /C0 [-1 2] /C1 [3 -2] >>] >>";
        let f = parse(&format!(
            "<< /Domain [10 20] /Bounds [] /Encode [0 1] /Range [0 2 -1 1] /Functions [{inner}] >>"
        ))
        .unwrap();
        assert_eq!(f.eval(&[10.]).unwrap(), vec![2., -1.]);
        assert_eq!(f.eval(&[15.]).unwrap(), vec![1., 0.]);
        assert_eq!(f.eval(&[20.]).unwrap(), vec![0., 1.]);
    }

    #[test]
    fn stitching_domain_endpoint_bounds_and_wide_interpolation() {
        let f = parse(&format!("<< /Domain [0 1] /Bounds [0 1] /Encode [0 0 0 1 0.25 0.75] /Functions [{CHILD} {CHILD} {CHILD}] >>")).unwrap();
        assert_eq!(f.eval(&[0.]).unwrap(), vec![0.]);
        assert_eq!(f.eval(&[0.5]).unwrap(), vec![0.5]);
        assert_eq!(f.eval(&[1.]).unwrap(), vec![0.25]);
        assert_eq!(f.eval(&[2.]).unwrap(), vec![0.25]);
        let wide = "300000000000000000000000000000000000000";
        let f = parse(&format!("<< /Domain [-{wide} {wide}] /Bounds [] /Encode [-{wide} {wide}] /Functions [<< /FunctionType 2 /Domain [-{wide} {wide}] /C0 [0.5] /C1 [0.5] /N 1 >>] >>")).unwrap();
        assert_eq!(f.eval(&[0.]).unwrap(), vec![0.5]);
    }

    #[test]
    fn stitching_rejects_mismatched_children_and_invalid_ranges() {
        let multi = "<< /FunctionType 2 /Domain [0 1] /N 1 /C0 [0 0] /C1 [1 1] >>";
        assert!(
            parse(&format!(
                "<< /Domain [0 1] /Bounds [0.5] /Encode [0 1 0 1] /Functions [{CHILD} {multi}] >>"
            ))
            .is_err()
        );
        for range in ["[]", "[0 1 0 1]", "[2 1]", "/Bad", "null"] {
            assert!(parse(&format!("<< /Domain [0 1] /Bounds [] /Encode [0 1] /Range {range} /Functions [{CHILD}] >>")).is_err());
        }
        for bounds in ["[-0.1]", "[1.1]", "[0.5 0.5]"] {
            assert!(parse(&format!("<< /Domain [0 1] /Bounds {bounds} /Encode [0 1 0 1] /Functions [{CHILD} {CHILD}] >>")).is_err());
        }
    }

    #[test]
    fn stitching_sampled_child_and_indirect_parameters() {
        use crate::objects::pdf_stream::PdfStream;
        let doc = PdfDocument::new().unwrap();
        let object = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let child = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            object("<< /FunctionType 0 /Domain [0 1] /Range [0 1] /Size [2] /BitsPerSample 8 >>").into_dict().unwrap(),
            vec![0, 255],
        ).unwrap())).unwrap();
        let endpoint = doc.try_insert_object(object("0.75")).unwrap();
        let range = doc
            .try_insert_object(object(&format!(
                "[0 {} {} R]",
                endpoint.obj_num(),
                endpoint.gen_num()
            )))
            .unwrap();
        let functions = doc
            .try_insert_object(object(&format!(
                "[{} {} R]",
                child.obj_num(),
                child.gen_num()
            )))
            .unwrap();
        let domain = doc.try_insert_object(object("[2 6]")).unwrap();
        let bounds = doc.try_insert_object(object("[]")).unwrap();
        let encode = doc.try_insert_object(object("[1 0]")).unwrap();
        let f = Stitching::try_new(&object(&format!("<< /Domain {} {} R /Bounds {} {} R /Encode {} {} R /Range {} {} R /Functions {} {} R >>",
            domain.obj_num(), domain.gen_num(), bounds.obj_num(), bounds.gen_num(), encode.obj_num(), encode.gen_num(), range.obj_num(), range.gen_num(), functions.obj_num(), functions.gen_num())), doc.ctx()).unwrap();
        assert_eq!(f.eval(&[2.]).unwrap(), vec![0.75]);
        assert_eq!(f.eval(&[4.]).unwrap(), vec![0.5]);
        assert_eq!(f.eval(&[6.]).unwrap(), vec![0.]);

        let multi = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            object("<< /FunctionType 0 /Domain [0 1 0 1] /Range [0 1] /Size [2 2] /BitsPerSample 8 >>").into_dict().unwrap(),
            vec![0, 255, 255, 0],
        ).unwrap())).unwrap();
        assert!(
            Stitching::try_new(
                &object(&format!(
                    "<< /Domain [0 1] /Bounds [] /Encode [0 1] /Functions [{} {} R] >>",
                    multi.obj_num(),
                    multi.gen_num()
                )),
                doc.ctx()
            )
            .is_err()
        );
    }

    #[test]
    fn stitching_propagates_child_evaluation_errors() {
        let f = parse("<< /Domain [0 1] /Bounds [] /Encode [0 2] /Functions [<< /FunctionType 2 /Domain [0 2] /N 1 /C0 [0] /C1 [300000000000000000000000000000000000000] >>] >>").unwrap();
        assert!(matches!(f.eval(&[1.0]), Err(PdfError::FunctionError(_))));
    }
}
