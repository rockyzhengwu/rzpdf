use std::io::{Read, Seek};

use exponential::Exponential;
use postscript::PostScriptFunction;
use sampled::Sampled;
use stitching::Stitching;

use crate::{
    error::{PdfError, PdfResult},
    objects::PdfObject,
    pdf_context::PDFContext,
};

mod calculator;
pub mod exponential;
pub mod postscript;
pub mod sampled;
pub mod stitching;

fn retained_vector_bytes<T>(values: &Vec<T>) -> PdfResult<usize> {
    values
        .capacity()
        .checked_mul(std::mem::size_of::<T>())
        .ok_or_else(|| PdfError::ResourceLimit("function retained capacity overflow".into()))
}

fn retained_total(values: &[usize]) -> PdfResult<usize> {
    values.iter().try_fold(0usize, |total, value| {
        total
            .checked_add(*value)
            .ok_or_else(|| PdfError::ResourceLimit("function retained payload overflow".into()))
    })
}

#[derive(Debug, Clone)]
pub enum Function {
    Type0(Sampled),
    Type2(Exponential),
    Type3(Stitching),
    Type4(PostScriptFunction),
}

pub fn create_function<R: Seek + Read>(
    obj: &PdfObject,
    ctx: &PDFContext<R>,
) -> PdfResult<Function> {
    create_function_at_depth(obj, ctx, 0, &mut ctx.limits().max_objects)
}

pub(crate) fn create_function_at_depth<R: Seek + Read>(
    obj: &PdfObject,
    ctx: &PDFContext<R>,
    depth: usize,
    remaining_nodes: &mut usize,
) -> PdfResult<Function> {
    if depth >= ctx.limits().max_depth {
        return Err(PdfError::ResourceLimit("PDF function nesting".to_string()));
    }
    if *remaining_nodes == 0 {
        return Err(PdfError::ResourceLimit(
            "PDF function total nodes".to_string(),
        ));
    }
    *remaining_nodes -= 1;
    let obj = ctx.resolve(obj)?;
    let type_obj = obj
        .get_attr("FunctionType")
        .ok_or(PdfError::FunctionError("FunctionType is None".to_string()))?;
    let resolved_type = ctx.resolve(type_obj)?;
    let t = resolved_type
        .as_number()
        .ok_or_else(|| PdfError::FunctionError("FunctionType must be an integer".to_string()))?
        .as_u32_checked()?;
    match t {
        0 => {
            let s = Sampled::try_new(&obj, ctx)?;
            Ok(Function::Type0(s))
        }
        2 => {
            let function = Exponential::try_new(&obj, ctx)?;
            Ok(Function::Type2(function))
        }
        3 => {
            let function = Stitching::try_new_at_depth(&obj, ctx, depth, remaining_nodes)?;
            Ok(Function::Type3(function))
        }
        4 => {
            let function = PostScriptFunction::try_new(&obj, ctx)?;
            Ok(Function::Type4(function))
        }
        _ => Err(PdfError::FunctionError(format!(
            "FunctionType must be in [0,2,3,4] got:{:?}",
            t
        ))),
    }
}

impl Function {
    pub(crate) fn input_domain(&self) -> &[f32] {
        match self {
            Self::Type0(function) => function.input_domain(),
            Self::Type2(function) => function.input_domain(),
            Self::Type3(function) => function.input_domain(),
            Self::Type4(function) => function.input_domain(),
        }
    }
    pub(crate) fn two_input_domain(&self) -> Option<&[f32]> {
        match self {
            Self::Type0(function) => Some(function.input_domain()),
            Self::Type4(function) => Some(function.input_domain()),
            _ => None,
        }
    }
    pub(crate) fn retained_payload_bytes(
        &self,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<usize> {
        let mut work = Vec::new();
        work.try_reserve_exact(1)
            .map_err(|_| PdfError::ResourceLimit("function accounting allocation".into()))?;
        work.push(self);
        let mut nodes = limits.max_objects;
        let mut total = 0usize;
        while let Some(function) = work.pop() {
            nodes = nodes
                .checked_sub(1)
                .ok_or_else(|| PdfError::ResourceLimit("function accounting nodes".into()))?;
            let bytes = match function {
                Self::Type0(f) => f.retained_own_bytes()?,
                Self::Type2(f) => f.retained_own_bytes()?,
                Self::Type3(f) => f.retained_own_bytes()?,
                Self::Type4(f) => f.retained_own_bytes()?,
            };
            total = total
                .checked_add(bytes)
                .filter(|total| *total <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("function retained payload".into()))?;
            if let Self::Type3(f) = function {
                let children = f.children();
                let needed = work
                    .len()
                    .checked_add(children.len())
                    .filter(|needed| {
                        *needed <= nodes
                            && needed
                                .checked_mul(std::mem::size_of::<&Function>())
                                .is_some_and(|bytes| bytes <= limits.max_decoded_bytes)
                    })
                    .ok_or_else(|| {
                        PdfError::ResourceLimit("function accounting worklist".into())
                    })?;
                work.try_reserve_exact(needed - work.len()).map_err(|_| {
                    PdfError::ResourceLimit("function accounting allocation".into())
                })?;
                work.extend(children);
            }
        }
        Ok(total)
    }
    pub(crate) fn dimensions(&self) -> (usize, usize) {
        match self {
            Self::Type0(f) => f.dimensions(),
            Self::Type2(f) => f.dimensions(),
            Self::Type3(f) => f.dimensions(),
            Self::Type4(f) => f.dimensions(),
        }
    }

    pub fn eval(&self, inputs: &[f32]) -> PdfResult<Vec<f32>> {
        match self {
            Function::Type0(f) => f.eval(inputs),
            Function::Type2(f) => f.eval(inputs),
            Function::Type3(f) => f.eval(inputs),
            Function::Type4(f) => f.eval(inputs),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn function_tree_budget_counts_across_branches_and_recovers_after_error() {
        let syntax =
            SyntaxParser::new(StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap());
        let ctx = PDFContext::try_new(
            PdfParser::new(syntax),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let source = "<< /FunctionType 3 /Domain [0 1] /Bounds [0.5] /Encode [0 1 0 1] /Functions [<< /FunctionType 2 /Domain [0 1] /N 1 >> << /FunctionType 2 /Domain [0 1] /N 1 >>] >>";
        let obj = SyntaxParser::new(
            StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
        )
        .get_object()
        .unwrap();
        let mut remaining = 2;
        assert!(matches!(
            create_function_at_depth(&obj, &ctx, 0, &mut remaining),
            Err(PdfError::ResourceLimit(_))
        ));
        let mut remaining = 3;
        assert!(create_function_at_depth(&obj, &ctx, 0, &mut remaining).is_ok());
        assert_eq!(remaining, 0);
        assert!(create_function(&obj, &ctx).is_ok());
        let id = ctx.insert_object(PdfObject::PdfNull);
        let source = format!(
            "<< /FunctionType 3 /Domain [0 1] /Bounds [] /Encode [0 1] /Functions [{} {} R] >>",
            id.obj_num(),
            id.gen_num()
        );
        let cyclic =
            SyntaxParser::new(StreamReader::try_new(Cursor::new(source.into_bytes())).unwrap())
                .get_object()
                .unwrap();
        ctx.update_object(id, cyclic.clone()).unwrap();
        for _ in 0..2 {
            assert!(matches!(
                create_function(&cyclic, &ctx),
                Err(PdfError::ResourceLimit(_))
            ));
        }
        assert!(create_function(&obj, &ctx).is_ok());
    }
    use crate::{
        document::cross_ref_table::CrossRefTable,
        io::stream_reader::StreamReader,
        objects::{pdf_array::PdfArray, pdf_dict::PdfDict},
        parser::{parser::PdfParser, syntax::SyntaxParser},
    };
    use std::{collections::HashMap, io::Cursor};

    #[test]
    fn stitching_parameters_and_recursive_construction_are_bounded() {
        let syntax =
            SyntaxParser::new(StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap());
        let ctx = PDFContext::try_new(
            PdfParser::new(syntax),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        assert!(create_function(&PdfObject::PdfNull, &ctx).is_err());
        for source in [
            "<< /FunctionType 3 /Domain [0 1] /Functions [] /Bounds [] /Encode [] >>",
            "<< /FunctionType 3 /Domain [0 1 2] >>",
            "<< /FunctionType 4 /Domain [0 1] /Range [0 1] >>",
        ] {
            assert!(create_function(&parse(source), &ctx).is_err());
        }
        let template = parse("<< /FunctionType 3 /Domain [0 1] /Bounds [] /Encode [0 1] >>");
        let mut obj = parse("<< /FunctionType 2 /Domain [0 1] /N 1 >>");
        for i in 0..=ctx.limits().max_depth {
            let mut dict = template.as_dict().unwrap().clone();
            let mut functions = PdfArray::default();
            functions.add_obj(obj);
            dict.insert("Functions".into(), PdfObject::PdfArray(functions));
            obj = PdfObject::PdfDict(dict);
            if i == 0 {
                assert!(create_function(&obj, &ctx).is_ok());
            }
        }
        assert!(matches!(
            create_function(&obj, &ctx),
            Err(PdfError::ResourceLimit(_))
        ));
    }
}
