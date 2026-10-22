use std::io::{Read, Seek};

use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

#[derive(Debug, Clone, Default)]
pub struct PostScriptFunction {
    domain: Vec<f32>,
    range: Vec<f32>,
    program: Vec<super::calculator::Instruction>,
    max_nodes: usize,
    max_bytes: usize,
    max_depth: usize,
    allow_extra_outputs: bool,
}

impl PostScriptFunction {
    pub(crate) fn input_domain(&self) -> &[f32] {
        &self.domain
    }
    pub(super) fn retained_own_bytes(&self) -> PdfResult<usize> {
        super::retained_total(&[
            super::retained_vector_bytes(&self.domain)?,
            super::retained_vector_bytes(&self.range)?,
            super::retained_vector_bytes(&self.program)?,
        ])
    }
    pub(super) fn dimensions(&self) -> (usize, usize) {
        (self.domain.len() / 2, self.range.len() / 2)
    }

    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        let stream = obj
            .as_stream()
            .ok_or_else(|| PdfError::FunctionError("Type4 requires a stream".to_string()))?;

        let pairs = |key: &str| -> PdfResult<Vec<f32>> {
            let value = ctx.resolve(
                stream
                    .dict()
                    .get(key)
                    .ok_or_else(|| PdfError::FunctionError(format!("Type4 requires {key}")))?,
            )?;
            let array = value
                .as_array()
                .ok_or_else(|| PdfError::FunctionError(format!("Type4 {key} must be an array")))?;
            if array.is_empty() || array.len() % 2 != 0 {
                return Err(PdfError::FunctionError(format!(
                    "Type4 {key} requires nonempty pairs"
                )));
            }
            let bytes = array
                .len()
                .checked_mul(std::mem::size_of::<f32>())
                .ok_or_else(|| PdfError::ResourceLimit("Type4 parameter size".into()))?;
            if array.len() > ctx.limits().max_objects || bytes > ctx.limits().max_decoded_bytes {
                return Err(PdfError::ResourceLimit("Type4 parameter array".into()));
            }
            let mut result = Vec::new();
            result
                .try_reserve_exact(array.len())
                .map_err(|_| PdfError::ResourceLimit("Type4 parameter allocation".into()))?;
            for item in array {
                let value = ctx
                    .resolve(item)?
                    .as_f32()
                    .filter(|n| n.is_finite())
                    .ok_or_else(|| {
                        PdfError::FunctionError(format!("Type4 {key} requires finite numbers"))
                    })?;
                result.push(value);
            }
            if result.as_chunks::<2>().0.iter().any(|p| p[0] > p[1]) {
                return Err(PdfError::FunctionError(format!(
                    "Type4 {key} requires ordered pairs"
                )));
            }
            Ok(result)
        };
        // Reject invalid parameters before doing any stream decoding.
        let domain = pairs("Domain")?;
        let range = pairs("Range")?;
        let data = stream.decode_data(ctx)?;
        let limits = ctx.limits();
        let program = super::calculator::compile(
            &data,
            limits.max_objects,
            limits.max_decoded_bytes,
            limits.max_depth,
        )?;
        Ok(Self {
            domain,
            range,
            program,
            max_nodes: limits.max_objects,
            max_bytes: limits.max_decoded_bytes,
            max_depth: limits.max_depth,
            allow_extra_outputs: ctx.options().mode == crate::parser::syntax::ParseMode::Compatible,
        })
    }

    pub fn eval(&self, inputs: &[f32]) -> PdfResult<Vec<f32>> {
        if self.domain.is_empty()
            || self.range.is_empty()
            || inputs.len() != self.domain.len() / 2
            || inputs.iter().any(|value| !value.is_finite())
        {
            return Err(PdfError::FunctionError(
                "Type4 invalid input dimensions or values".into(),
            ));
        }
        let mut clipped = Vec::new();
        clipped
            .try_reserve_exact(inputs.len())
            .map_err(|_| PdfError::ResourceLimit("Type4 input allocation".into()))?;
        for (&input, bounds) in inputs.iter().zip(self.domain.as_chunks::<2>().0) {
            clipped.push(input.clamp(bounds[0], bounds[1]));
        }
        let stack = super::calculator::evaluate(
            &self.program,
            &clipped,
            self.max_nodes,
            self.max_bytes,
            self.max_depth,
        )?;
        let outputs = self.range.len() / 2;
        if stack.len() < outputs || (!self.allow_extra_outputs && stack.len() != outputs) {
            return Err(PdfError::FunctionError(
                "Type4 output component count mismatch".into(),
            ));
        }
        let mut output = Vec::new();
        output
            .try_reserve_exact(outputs)
            .map_err(|_| PdfError::ResourceLimit("Type4 output allocation".into()))?;
        let skipped = stack.len() - outputs;
        for (value, bounds) in stack
            .into_iter()
            .skip(skipped)
            .zip(self.range.as_chunks::<2>().0)
        {
            let number = match value {
                super::calculator::Value::Integer(value) => f64::from(value),
                super::calculator::Value::Real(value) => value,
                _ => {
                    return Err(PdfError::FunctionError(
                        "Type4 output must be numeric".into(),
                    ));
                }
            };
            output.push(number.clamp(f64::from(bounds[0]), f64::from(bounds[1])) as f32);
        }
        Ok(output)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::PdfDocument,
        function::create_function,
        io::stream_reader::StreamReader,
        objects::{pdf_reference::PdfReference, pdf_stream::PdfStream},
        parser::{
            options::{Limits, ParseOptions},
            syntax::{ParseMode, SyntaxParser},
        },
    };
    use std::io::Cursor;

    fn object(source: &str) -> PdfObject {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap())
            .get_object()
            .unwrap()
    }

    fn stream(parameters: &str, data: &[u8]) -> PdfObject {
        PdfObject::PdfStream(PdfStream::new(
            object(parameters).into_dict().unwrap(),
            data.to_vec(),
        ))
    }

    #[test]
    fn type4_public_eval_clips_and_requires_exact_numeric_outputs() {
        let doc = PdfDocument::new_with_options(ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        })
        .unwrap();
        let make = |code: &[u8]| {
            PostScriptFunction::try_new(
                &stream("<< /Domain [0 1] /Range [0.25 0.75] >>", code),
                doc.ctx(),
            )
            .unwrap()
        };
        let identity = make(b"{}");
        assert_eq!(identity.eval(&[-1.0]).unwrap(), vec![0.25]);
        assert_eq!(identity.eval(&[2.0]).unwrap(), vec![0.75]);
        assert_eq!(identity.eval(&[0.5]).unwrap(), vec![0.5]);
        assert_eq!(
            make(b"{pop 1000000000 dup mul dup mul 1000 mul}")
                .eval(&[0.0])
                .unwrap(),
            vec![0.75]
        );
        for inputs in [vec![], vec![0.0, 1.0], vec![f32::NAN], vec![f32::INFINITY]] {
            assert!(matches!(
                identity.eval(&inputs),
                Err(PdfError::FunctionError(_))
            ));
        }
        for source in [
            b"{pop}".as_slice(),
            b"{pop 1 2}",
            b"{pop true}",
            b"{pop {1}}",
        ] {
            assert!(matches!(
                make(source).eval(&[0.0]),
                Err(PdfError::FunctionError(_))
            ));
        }
        assert!(matches!(
            PostScriptFunction::default().eval(&[]),
            Err(PdfError::FunctionError(_))
        ));
    }

    #[test]
    fn compatible_type4_reads_only_required_top_numeric_results() {
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let doc = PdfDocument::new_with_options(ParseOptions {
                mode,
                ..Default::default()
            })
            .unwrap();
            let make = |code: &[u8]| {
                PostScriptFunction::try_new(
                    &stream("<< /Domain [0 1] /Range [0 1 0 1] >>", code),
                    doc.ctx(),
                )
                .unwrap()
            };
            let result = make(b"{ pop true 0.25 0.75 }").eval(&[0.]);
            if mode == ParseMode::Compatible {
                assert_eq!(result.unwrap(), vec![0.25, 0.75]);
            } else {
                assert!(result.is_err());
            }
            assert!(make(b"{ pop 0.5 }").eval(&[0.]).is_err());
            assert!(make(b"{ pop 0.5 true }").eval(&[0.]).is_err());
        }
    }

    #[test]
    fn indirect_type4_parameters_and_members_survive_strict_reopen() {
        let doc = PdfDocument::new().unwrap();
        let upper = doc.try_insert_object(object("1")).unwrap();
        let domain = doc
            .try_insert_object(object(&format!(
                "[0 {} {} R 0 1]",
                upper.obj_num(),
                upper.gen_num()
            )))
            .unwrap();
        let range = doc
            .try_insert_object(object(&format!(
                "[0 {} {} R]",
                upper.obj_num(),
                upper.gen_num()
            )))
            .unwrap();
        let id = doc
            .try_insert_object(stream(
                &format!(
                    "<< /FunctionType 4 /Domain {} {} R /Range {} {} R >>",
                    domain.obj_num(),
                    domain.gen_num(),
                    range.obj_num(),
                    range.gen_num()
                ),
                b"{ add }",
            ))
            .unwrap();
        let reference = PdfObject::PdfReference(PdfReference::from_id(id));
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for doc in [&doc, &reopened] {
            let function = create_function(&reference, doc.ctx()).unwrap();
            assert_eq!(function.dimensions(), (2, 1));
            assert_eq!(function.eval(&[0.2, 0.3]).unwrap(), vec![0.5]);
        }
    }

    #[test]
    fn type4_invalid_parameters_are_rejected_before_stream_decode() {
        let doc = PdfDocument::new().unwrap();
        for key in ["Domain", "Range"] {
            for value in [
                "[]",
                "[0]",
                "[1 0]",
                "[0 /Wrong]",
                "null",
                "1",
                "[0 400000000000000000000000000000000000000]",
            ] {
                let other = if key == "Domain" { "Range" } else { "Domain" };
                let source = stream(
                    &format!(
                        "<< /FunctionType 4 /{key} {value} /{other} [0 1] /Filter /UnknownCodec >>"
                    ),
                    b"{} ",
                );
                assert!(
                    matches!(
                        PostScriptFunction::try_new(&source, doc.ctx()),
                        Err(PdfError::FunctionError(_))
                    ),
                    "{key}: {value}"
                );
            }
        }
        for source in [
            "<< /Range [0 1] /Filter /UnknownCodec >>",
            "<< /Domain [0 1] /Filter /UnknownCodec >>",
        ] {
            assert!(matches!(
                PostScriptFunction::try_new(&stream(source, b"{}"), doc.ctx()),
                Err(PdfError::FunctionError(_))
            ));
        }
        assert!(matches!(
            PostScriptFunction::try_new(
                &stream(
                    "<< /Domain [0 1] /Range [0 1] /Filter /UnknownCodec >>",
                    b"{}"
                ),
                doc.ctx()
            ),
            Err(PdfError::FilterError(_))
        ));
    }

    #[test]
    fn type4_parameter_limits_and_decode_errors_allow_retry() {
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                max_objects: 3,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        assert!(matches!(
            PostScriptFunction::try_new(
                &stream("<< /Domain [0 1 0 1] /Range [0 1] >>", b"{}"),
                doc.ctx()
            ),
            Err(PdfError::ResourceLimit(_))
        ));
        assert!(
            PostScriptFunction::try_new(
                &stream("<< /Domain [0 1] /Range [0 1] >>", b"{}"),
                doc.ctx()
            )
            .is_ok()
        );
        for limit in [7, 8] {
            let doc = PdfDocument::new_with_options(ParseOptions {
                limits: Limits {
                    max_decoded_bytes: limit,
                    ..Default::default()
                },
                ..Default::default()
            })
            .unwrap();
            let good = stream("<< /Domain [0 1] /Range [0 1] >>", b"{}");
            if limit == 7 {
                assert!(matches!(
                    PostScriptFunction::try_new(&good, doc.ctx()),
                    Err(PdfError::ResourceLimit(_))
                ));
                continue;
            }
            let stamp = doc.stamp();
            assert!(matches!(
                PostScriptFunction::try_new(
                    &stream("<< /Domain [0 1 0 1] /Range [0 1] >>", b"{}"),
                    doc.ctx()
                ),
                Err(PdfError::ResourceLimit(_))
            ));
            assert!(
                PostScriptFunction::try_new(
                    &stream(
                        "<< /Domain [0 1] /Range [0 1] /Filter /FlateDecode >>",
                        b"broken"
                    ),
                    doc.ctx()
                )
                .is_err()
            );
            assert!(PostScriptFunction::try_new(&good, doc.ctx()).is_ok());
            assert!(doc.is_stamp_current(&stamp));
        }
    }

    #[test]
    fn type4_parameter_reference_cycles_return_errors_and_recover() {
        let doc = PdfDocument::new().unwrap();
        let id = doc.try_insert_object(PdfObject::PdfNull).unwrap();
        let reference = PdfObject::PdfReference(PdfReference::from_id(id));
        doc.update_object(id, reference).unwrap();
        let cyclic = stream(
            &format!(
                "<< /Domain [0 {} {} R] /Range [0 1] >>",
                id.obj_num(),
                id.gen_num()
            ),
            b"{}",
        );
        let stamp = doc.stamp();
        for _ in 0..2 {
            assert!(PostScriptFunction::try_new(&cyclic, doc.ctx()).is_err());
        }
        assert!(
            PostScriptFunction::try_new(
                &stream("<< /Domain [0 1] /Range [0 1] >>", b"{}"),
                doc.ctx()
            )
            .is_ok()
        );
        assert!(doc.is_stamp_current(&stamp));
    }
}
