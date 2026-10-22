//! Validated tiling-pattern dictionary. Execution belongs to the interpreter.
use crate::{
    error::{PdfError, PdfResult},
    geom::{matrix::Matrix, rectangle::Rectangle},
    objects::{PdfObject, pdf_dict::PdfDict},
    pdf_context::PDFContext,
};
use std::io::{Read, Seek};

#[derive(Debug, Clone)]
pub struct TilingPattern {
    pub paint_type: u8,
    pub tiling_type: u8,
    pub bbox: Rectangle,
    pub xstep: f32,
    pub ystep: f32,
    pub matrix: Matrix,
    pub resources: PdfDict,
}

impl TilingPattern {
    pub fn try_new<R: Seek + Read>(obj: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let obj = ctx.resolve(obj)?;
        let stream = obj.as_stream().ok_or_else(|| invalid("must be a stream"))?;
        let required = |key: &str| {
            ctx.resolve(
                stream
                    .dict()
                    .get(key)
                    .ok_or_else(|| invalid(&format!("missing {key}")))?,
            )
        };
        if let Some(kind) = stream.dict().get("Type")
            && ctx.resolve(kind)?.as_name().map(|n| n.name()) != Some("Pattern")
        {
            return Err(invalid("Type must be Pattern"));
        }
        if required("PatternType")?.as_u8() != Some(1) {
            return Err(invalid("PatternType must be 1"));
        }
        let paint_type = required("PaintType")?
            .as_u8()
            .filter(|v| (1..=2).contains(v))
            .ok_or_else(|| invalid("PaintType must be 1 or 2"))?;
        let tiling_type = required("TilingType")?
            .as_u8()
            .filter(|v| (1..=3).contains(v))
            .ok_or_else(|| invalid("TilingType must be 1 to 3"))?;
        let bbox = Rectangle::try_from_with_context(&required("BBox")?, ctx)?;
        if ![bbox.llx(), bbox.lly(), bbox.urx(), bbox.ury()]
            .iter()
            .all(|v| v.is_finite())
            || bbox.llx() >= bbox.urx()
            || bbox.lly() >= bbox.ury()
        {
            return Err(invalid("BBox must have finite positive area"));
        }
        let step = |key| {
            required(key)?
                .as_f32()
                .filter(|v| v.is_finite() && *v != 0.)
                .ok_or_else(|| invalid("steps must be finite and nonzero"))
        };
        let matrix = stream
            .dict()
            .get("Matrix")
            .map(|value| Matrix::try_from(&ctx.resolve(value)?))
            .transpose()?
            .unwrap_or_else(Matrix::identity);
        matrix.checked_inverse()?;
        let resources = if stream.dict().get("Resources").is_none()
            && ctx.options().mode == crate::parser::syntax::ParseMode::Compatible
        {
            ctx.record_structure_recovery(
                crate::parser::diagnostic::StructureReason::PatternResources,
            )?;
            PdfDict::default()
        } else {
            required("Resources")?
                .into_dict()
                .ok_or_else(|| invalid("Resources must be a dictionary"))?
        };
        Ok(Self {
            paint_type,
            tiling_type,
            bbox,
            xstep: step("XStep")?,
            ystep: step("YStep")?,
            matrix,
            resources,
        })
    }
}

fn invalid(message: &str) -> PdfError {
    PdfError::ColorError(format!("tiling pattern {message}"))
}

#[cfg(test)]
mod compatibility_tests {
    use super::*;
    #[test]
    fn missing_pattern_resources_recover_as_empty_without_inheriting() {
        use crate::{
            io::stream_reader::StreamReader,
            objects::pdf_stream::PdfStream,
            parser::{
                options::ParseOptions,
                syntax::{ParseMode, SyntaxParser},
            },
        };
        use std::io::Cursor;
        for mode in [ParseMode::Strict, ParseMode::Compatible] {
            let ctx = PDFContext::<Cursor<Vec<u8>>>::new_without_source(
                PdfDict::default(),
                ParseOptions {
                    mode,
                    ..Default::default()
                },
            );
            for (entry, valid) in [
                ("", mode == ParseMode::Compatible),
                ("/Resources << >>", true),
                ("/Resources null", false),
            ] {
                let source = format!(
                    "<< /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 4 4] /XStep 4 /YStep 4 {entry} >>"
                );
                let dict = SyntaxParser::new(
                    StreamReader::try_new(Cursor::new(source.into_bytes())).unwrap(),
                )
                .get_object()
                .unwrap()
                .into_dict()
                .unwrap();
                let stream = PdfStream::new(dict, b"0 0 4 4 re f".to_vec());
                let result = TilingPattern::try_new(&PdfObject::PdfStream(stream), &ctx);
                assert_eq!(result.is_ok(), valid, "{mode:?} {entry}");
                if let Ok(pattern) = result {
                    assert_eq!((&pattern.resources).into_iter().count(), 0);
                }
            }
        }
    }
}
