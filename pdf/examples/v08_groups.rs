//! Isolated RGB group corpus with independent group/member opacity.
use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::SyntaxParser,
};
use std::{
    fs::OpenOptions,
    io::{Cursor, Write},
};
fn parse(s: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(s.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn main() -> PdfResult<()> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or_else(|| PdfError::DocumentError("expected new output PDF".into()))?;
    let doc = PdfDocument::new()?;
    for (index, (alpha, blend)) in [
        (0.0, "Normal"),
        (0.25, "Normal"),
        (0.5, "Normal"),
        (0.75, "Multiply"),
        (1.0, "Screen"),
    ]
    .into_iter()
    .enumerate()
    {
        let resources = parse(&format!(
            "<< /ExtGState << /Group << /ca {alpha} /BM /{blend} >> >> >>"
        ))
        .into_dict()
        .unwrap();
        doc.add_page_with_builder(
            index as u32,
            PageBuilder::new().size(60., 40.).resources(resources),
        )?;
        let dict=parse("<< /Subtype /Form /BBox [0 0 40 40] /Matrix [1 0 0 1 5 0] /Group << /S /Transparency /I true /K false /CS /DeviceRGB >> /Resources << /ExtGState << /Member << /ca 0.5 /BM /Multiply >> >> >> >>").into_dict().unwrap();
        let data = b"1 0 0 rg 0 0 30 40 re f q /Member gs 0 0 1 rg 15 5 25 30 re f Q".to_vec();
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dict, data,
        )?))?;
        let form = doc
            .resources()
            .register_existing(index as u32, ResourceKind::Form, id)?;
        doc.replace_page_contents(
            index as u32,
            format!("0 1 0 rg 0 0 60 40 re f q /Group gs /{form} Do Q 1 1 0 rg 50 0 10 40 re f")
                .into_bytes(),
        )?;
    }
    for (index, isolated) in [(5, true), (6, false)] {
        let resources = parse("<< /ExtGState << /Half << /ca 0.5 >> >> >>")
            .into_dict()
            .unwrap();
        doc.add_page_with_builder(
            index,
            PageBuilder::new().size(60., 40.).resources(resources),
        )?;
        let dict=parse(&format!("<< /Subtype /Form /BBox [0 0 40 40] /Matrix [1 0 0 1 5 0] /Group << /S /Transparency /I {isolated} >> /Resources << /ExtGState << /Multiply << /BM /Multiply >> >> >> >>")).into_dict().unwrap();
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dict,
            b"/Multiply gs 0.5 0.5 0.5 rg 0 0 40 40 re f".to_vec(),
        )?))?;
        let name = doc
            .resources()
            .register_existing(index, ResourceKind::Form, id)?;
        doc.replace_page_contents(
            index,
            format!("0 1 0 rg 0 0 60 40 re f q /Half gs /{name} Do Q 1 1 0 rg 50 0 10 40 re f")
                .into_bytes(),
        )?;
    }
    for (index, isolated, knockout) in [(7, true, true), (8, false, true), (9, true, false)] {
        doc.add_page_with_builder(index, PageBuilder::new().size(60., 40.))?;
        let dict=parse(&format!("<< /Subtype /Form /BBox [0 0 50 40] /Group << /S /Transparency /I {isolated} /K {knockout} >> /Resources << /ExtGState << /Half << /ca 0.5 >> /Zero << /ca 0 >> /Multiply << /ca 0.5 /BM /Multiply >> >> >> >>")).into_dict().unwrap();
        let data=b"1 0 0 rg 0 0 40 40 re f q /Half gs 0 0 1 rg 10 0 10 40 re f Q q /Zero gs 0 0 1 rg 20 0 10 40 re f Q q /Multiply gs 0.5 0.5 0.5 rg 30 0 10 40 re f Q".to_vec();
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dict, data,
        )?))?;
        let name = doc
            .resources()
            .register_existing(index, ResourceKind::Form, id)?;
        doc.replace_page_contents(
            index,
            format!("0 1 0 rg 0 0 60 40 re f /{name} Do").into_bytes(),
        )?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&doc.save_to_bytes()?)?;
    Ok(())
}
