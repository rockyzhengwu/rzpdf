//! Small deterministic interoperability corpus. Never overwrites output files.
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
fn parse(source: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}
fn main() -> PdfResult<()> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or_else(|| PdfError::DocumentError("expected new output PDF".into()))?;
    let doc = PdfDocument::new()?;
    let cases = [
        (1, "", "1 0 0 rg 0 0 4 4 re f", "0 0 60 60 re f"),
        (
            2,
            "/XStep -10 /Matrix [1 0 0 1 2 3]",
            "0 0 4 4 re f",
            "0 0 60 60 re f",
        ),
        (
            1,
            "/Matrix [1 0 0 1 2 3]",
            "1 0 0 rg 0 0 4 4 re f",
            "q 1 0 0 1 5 7 cm 0 0 50 45 re f Q",
        ),
        (1, "", "0 0 1 rg 0 0 4 4 re f", "6 w 5 5 50 50 re S"),
        (
            1,
            "/Resources << /Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> >>",
            "0 0 1 rg BT /F 5 Tf 0 0 Td (A) Tj ET",
            "0 0 60 60 re f",
        ),
        (
            1,
            "/BBox [0 0 14 14]",
            "1 0 0 rg 0 0 14 7 re f 0 0 1 rg 0 7 14 7 re f",
            "0 0 60 60 re f",
        ),
    ];
    for (index, (paint, extra, cell, shape)) in cases.iter().enumerate() {
        doc.add_page_with_builder(index as u32, PageBuilder::new().size(60., 60.))?;
        let dict=parse(&format!("<< /Type /Pattern /PatternType 1 /PaintType {paint} /TilingType 1 /BBox [0 0 4 4] /XStep 10 /YStep 10 /Resources << >> {extra} >>")).into_dict().unwrap();
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dict,
            cell.as_bytes().to_vec(),
        )?))?;
        let name = doc
            .resources()
            .register_existing(index as u32, ResourceKind::Pattern, id)?;
        let selected = if *paint == 2 {
            let cs = doc
                .resources()
                .register_color_space(index as u32, parse("[/Pattern /DeviceRGB]"))?;
            format!("/{cs} cs 0 0.5 1 /{name} scn")
        } else {
            format!("/Pattern cs /Pattern CS /{name} scn /{name} SCN")
        };
        doc.replace_page_contents(index as u32, format!("{selected} {shape}").into_bytes())?;
    }
    for index in [6, 7] {
        doc.add_page_with_builder(index, PageBuilder::new().size(60., 60.))?;
        let shade = "<< /ShadingType 2 /ColorSpace /DeviceRGB /Coords [20 0 40 0] /Domain [0 1] /Background [0 1 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [1 0 0] /C1 [0 0 1] /N 1 >> >>";
        let (name, content) = if index == 6 {
            let id = doc.try_insert_object(parse(&format!(
                "<< /Type /Pattern /PatternType 2 /Matrix [1 0 0 1 0 0] /Shading {shade} >>"
            )))?;
            let name = doc
                .resources()
                .register_existing(index, ResourceKind::Pattern, id)?;
            (name, "pattern")
        } else {
            let id = doc.try_insert_object(parse(shade))?;
            (
                doc.resources()
                    .register_existing(index, ResourceKind::Shading, id)?,
                "direct",
            )
        };
        let code = if content == "pattern" {
            format!("/Pattern cs /{name} scn 0 0 60 60 re f")
        } else {
            format!("/{name} sh")
        };
        doc.replace_page_contents(index, code.into_bytes())?;
    }
    for index in 8..12 {
        doc.add_page_with_builder(
            index,
            PageBuilder::new()
                .size(60., 60.)
                .rotation(if index == 11 { 90 } else { 0 }),
        )?;
        let dict=parse("<< /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 4 4] /XStep 10 /YStep 10 /Resources << >> >>").into_dict().unwrap();
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dict,
            b"0 0 1 rg 0 0 4 4 re f".to_vec(),
        )?))?;
        let name = doc
            .resources()
            .register_existing(index, ResourceKind::Pattern, id)?;
        let code = match index {
            8 => {
                let font = doc.resources().register_standard_font(
                    index,
                    pdf::document::resource_manager::StandardFont::Courier,
                )?;
                format!("/Pattern cs /{name} scn BT /{font} 48 Tf 1 0.1 0.2 1 5 5 Tm (A) Tj ET")
                    .into_bytes()
            }
            9 => {
                let mut code=format!("/Pattern cs /{name} scn q 60 0 0 60 0 0 cm BI /W 2 /H 2 /BPC 1 /IM true /D [0 1] ID ").into_bytes();
                code.extend_from_slice(&[0x40, 0x80]);
                code.extend_from_slice(b" EI Q");
                code
            }
            10 => {
                let dict=parse(&format!("<< /Subtype /Form /BBox [0 0 40 40] /Matrix [1 0 0 1 7 8] /Resources << /Pattern << /P {} 0 R >> >> >>",id.obj_num())).into_dict().unwrap();
                let fid =
                    doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
                        dict,
                        b"1 0 0 1 3 5 cm /Pattern cs /P scn 0 0 35 35 re f".to_vec(),
                    )?))?;
                let form = doc
                    .resources()
                    .register_existing(index, ResourceKind::Form, fid)?;
                format!("/{form} Do").into_bytes()
            }
            _ => format!(
                "/Pattern cs /{name} scn 5 5 40 40 re W n 1 0.2 0.3 1 0 0 cm 0 0 60 60 re f"
            )
            .into_bytes(),
        };
        doc.replace_page_contents(index, code)?;
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?;
    file.write_all(&doc.save_to_bytes()?)?;
    Ok(())
}
