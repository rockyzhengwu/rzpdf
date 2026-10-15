//! Nested backdrop, combined paint, Pattern/mask and all standard blend modes.
use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    error::PdfResult,
    io::stream_reader::StreamReader,
    objects::{PdfObject, object_id::ObjectId, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::{ParseMode, SyntaxParser},
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
fn stream(doc: &PdfDocument<Cursor<Vec<u8>>>, dict: &str, code: &str) -> PdfResult<ObjectId> {
    doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
        parse(dict).into_dict().unwrap(),
        code.as_bytes().to_vec(),
    )?))
}
fn main() -> PdfResult<()> {
    let output = std::env::args_os().nth(1).expect("new PDF path");
    let doc = PdfDocument::new()?;
    for (index, isolated, knockout) in [(0, true, false), (1, true, true), (2, false, true)] {
        doc.add_page_with_builder(index, PageBuilder::new().size(60., 40.))?;
        let child = stream(
            &doc,
            "<< /Subtype /Form /BBox [0 0 40 40] /Group << /S /Transparency >> /Resources << /ExtGState << /H << /ca 0.5 /BM /Multiply >> >> >> >>",
            "/H gs 0.5 g 10 0 20 40 re f",
        )?;
        let outer = stream(
            &doc,
            &format!(
                "<< /Subtype /Form /BBox [0 0 40 40] /Group << /S /Transparency /I {isolated} /K {knockout} >> /Resources << /XObject << /C {} 0 R >> >> >>",
                child.obj_num()
            ),
            "1 0 0 rg 0 0 40 40 re f /C Do",
        )?;
        let name = doc
            .resources()
            .register_existing(index, ResourceKind::Form, outer)?;
        doc.replace_page_contents(
            index,
            format!("0 1 0 rg 0 0 60 40 re f /{name} Do").into_bytes(),
        )?;
    }
    doc.add_page_with_builder(
        3,
        PageBuilder::new().size(60., 40.).resources(
            parse("<< /ExtGState << /H << /ca 0.5 /CA 0.25 >> >> >>")
                .into_dict()
                .unwrap(),
        ),
    )?;
    doc.replace_page_contents(
        3,
        b"0 1 0 rg 0 0 60 40 re f /H gs 1 0 0 rg 0 0 1 RG 10 w 10 10 30 20 re B".to_vec(),
    )?;
    for index in [4, 5] {
        doc.add_page_with_builder(index, PageBuilder::new().size(60., 40.))?;
        let id = if index == 4 {
            stream(
                &doc,
                "<< /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 10 10] /XStep 10 /YStep 10 /Resources << /ExtGState << /M << /BM /Multiply >> >> >> >>",
                "/M gs 0.5 g 0 0 10 10 re f",
            )?
        } else {
            doc.try_insert_object(parse("<< /PatternType 2 /ExtGState << /BM /Multiply >> /Shading << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 60 0] /Extend [true true] /Function << /FunctionType 2 /Domain [0 1] /C0 [0.5] /C1 [0.5] /N 1 >> >> >>"))?
        };
        let name = doc
            .resources()
            .register_existing(index, ResourceKind::Pattern, id)?;
        doc.replace_page_contents(
            index,
            format!("0 1 0 rg 0 0 60 40 re f /Pattern cs /{name} scn 0 0 60 40 re f").into_bytes(),
        )?;
    }
    let source = PdfDocument::from_bytes_with_mode(
        include_bytes!("../tests/fixtures/v07/fonts.pdf").to_vec(),
        ParseMode::Strict,
    )?;
    let res = source.get_page(2)?.resource()?;
    let id = res
        .get("Font")
        .unwrap()
        .get_attr("F0")
        .unwrap()
        .as_object_id()
        .unwrap();
    let font = doc.import_resource_from(&source, &source.object_handle(id)?, ResourceKind::Font)?;
    for (offset, mode) in [
        "Normal",
        "Multiply",
        "Screen",
        "Overlay",
        "Darken",
        "Lighten",
        "ColorDodge",
        "ColorBurn",
        "HardLight",
        "SoftLight",
        "Difference",
        "Exclusion",
        "Hue",
        "Saturation",
        "Color",
        "Luminosity",
    ]
    .into_iter()
    .enumerate()
    {
        let index = 6 + offset as u32;
        doc.add_page_with_builder(
            index,
            PageBuilder::new().size(60., 40.).resources(
                parse(&format!(
                    "<< /ExtGState << /M << /BM /{mode} /ca 0.5 >> >> >>"
                ))
                .into_dict()
                .unwrap(),
            ),
        )?;
        let font = doc
            .resources()
            .register_handle(index, ResourceKind::Font, &font)?;
        let image = doc
            .resources()
            .embed_rgba_image(index, 1, 1, vec![204, 51, 153, 255])?;
        doc.replace_page_contents(index,format!("0.2 0.6 0.4 rg 0 0 60 40 re f /M gs 0.8 0.2 0.6 rg 0 0 20 40 re f q 20 0 0 40 20 0 cm /{image} Do Q BT /{font} 40 Tf 1 0 0 1 40 0 Tm (A) Tj ET").into_bytes())?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&doc.save_to_bytes()?)?;
    Ok(())
}
