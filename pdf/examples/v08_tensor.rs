//! Deterministic Type7 corpus: ordinary and independently moved interiors.
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
fn main() -> PdfResult<()> {
    let output = std::env::args_os()
        .nth(1)
        .ok_or_else(|| PdfError::DocumentError("expected new output PDF".into()))?;
    let doc = PdfDocument::new()?;
    for index in 0..4 {
        let moved = index >= 2;
        let function = index % 2 == 1;
        doc.add_page_with_builder(index, PageBuilder::new().size(40., 40.))?;
        let mut points = [
            [0u8, 0],
            [0, 80],
            [0, 160],
            [0, 240],
            [80, 240],
            [160, 240],
            [240, 240],
            [240, 160],
            [240, 80],
            [240, 0],
            [160, 0],
            [80, 0],
            [80, 80],
            [80, 160],
            [160, 160],
            [160, 80],
        ];
        if moved {
            points[12][0] += 64;
        }
        let mut data = vec![0];
        for point in points {
            data.extend(point);
        }
        data.extend([0, 0, 255, 255]);
        let f = if function {
            "/Function << /FunctionType 2 /Domain [0 1] /N 2 >>"
        } else {
            ""
        };
        let dictionary = format!(
            "<< /ShadingType 7 /ColorSpace /DeviceGray /BitsPerFlag 8 /BitsPerCoordinate 8 /BitsPerComponent 8 /Decode [0 255 0 255 0 1] {f} >>"
        );
        let dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(dictionary.into_bytes()))?)
            .get_object()?
            .into_dict()
            .unwrap();
        let id = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dict, data,
        )?))?;
        let name = doc
            .resources()
            .register_existing(index, ResourceKind::Shading, id)?;
        doc.replace_page_contents(index, format!("0.1 0 0 0.1 2 2 cm /{name} sh").into_bytes())?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&doc.save_to_bytes()?)?;
    Ok(())
}
