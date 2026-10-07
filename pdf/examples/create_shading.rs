//! Deterministic shading fixture for independent-reader comparisons.
//! Usage: cargo run -p pdf --example create_shading -- output.pdf [--flate]
use pdf::{
    document::{
        PdfDocument,
        resource_manager::ResourceKind,
        save_options::{SaveOptions, StreamCompression},
    },
    error::{PdfError, PdfResult},
    io::stream_reader::StreamReader,
    page::page_builder::PageBuilder,
    parser::syntax::SyntaxParser,
};

fn main() -> PdfResult<()> {
    let mut args = std::env::args_os().skip(1);
    let path = args.next().unwrap_or_else(|| "shading.pdf".into());
    let compression = match args.next() {
        None => StreamCompression::Preserve,
        Some(flag) if flag == "--flate" => StreamCompression::FlateUnencoded,
        Some(_) => return Err(PdfError::DocumentError("expected --flate".into())),
    };
    if args.next().is_some() {
        return Err(PdfError::DocumentError("too many arguments".into()));
    }
    let doc = PdfDocument::new()?;
    for (index, (geometry, extra, prefix)) in [
        ("/ShadingType 2 /Coords [20 0 180 0]", "", ""),
        ("/ShadingType 3 /Coords [100 60 0 100 60 50]", "", ""),
        (
            "/ShadingType 3 /Coords [60 60 45 140 60 10]",
            "/Extend [true true]",
            "",
        ),
        (
            "/ShadingType 2 /Coords [40 0 160 0]",
            "/Extend [true true] /BBox [20 20 180 100] /Background [1 0 0]",
            "50 30 100 60 re W n ",
        ),
        (
            "/ShadingType 2 /Coords [20 0 180 0]",
            "/Extend [true true]",
            "0 1 -1 0 160 -40 cm ",
        ),
        (
            "/ShadingType 3 /Coords [100 60 0 100 60 50]",
            "",
            "1 0.2 0.3 0.6 -18 4 cm ",
        ),
    ]
    .into_iter()
    .enumerate()
    {
        let index = index as u32;
        doc.add_page_with_builder(
            index,
            PageBuilder::new()
                .size(200.0, 120.0)
                .rotation(if index == 4 { 90 } else { 0 }),
        )?;
        let source = format!(
            "<< {geometry} {extra} /ColorSpace /DeviceRGB /Function << /FunctionType 2 /Domain [0 1] /C0 [0 0 1] /C1 [1 0 0] /N 2 >> >>"
        );
        let object = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
            source.into_bytes(),
        ))?)
        .get_object()?;
        let id = doc.try_insert_object(object)?;
        let name = doc
            .resources()
            .register_existing(index, ResourceKind::Shading, id)?;
        doc.replace_page_contents(index, format!("{prefix}/{name} sh").into_bytes())?;
    }
    // Shading uses an explicit color space; its name is local to Form resources.
    doc.add_page_with_builder(6, PageBuilder::new().size(200.0, 120.0))?;
    let source = b"<< /Type /XObject /Subtype /Form /BBox [0 0 80 80] /Resources << /Shading << /S << /ShadingType 2 /ColorSpace /DeviceGray /Coords [0 0 80 0] /Function << /FunctionType 2 /Domain [0 1] /C0 [0] /C1 [1] /N 2 >> >> >> >> >>";
    let dict = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
        source.to_vec(),
    ))?)
    .get_object()?
    .into_dict()
    .ok_or_else(|| PdfError::ObjectError("expected Form dictionary".into()))?;
    let form = pdf::objects::pdf_stream::PdfStream::from_decoded_data(dict, b"/S sh".to_vec())?;
    let id = doc.try_insert_object(pdf::objects::PdfObject::PdfStream(form))?;
    let name = doc
        .resources()
        .register_existing(6, ResourceKind::Form, id)?;
    doc.replace_page_contents(
        6,
        format!("q 1 0 0 1 10 20 cm /{name} Do Q q 1 0 0 1 110 20 cm /{name} Do Q").into_bytes(),
    )?;
    doc.add_page_with_builder(7, PageBuilder::new().size(200.0, 120.0))?;
    let dict = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
        b"<< /FunctionType 4 /Domain [0 1 0 1] /Range [0 1 0 1 0 1] >>".to_vec(),
    ))?)
    .get_object()?
    .into_dict()
    .ok_or_else(|| PdfError::ObjectError("expected function dictionary".into()))?;
    let function = doc.try_insert_object(pdf::objects::PdfObject::PdfStream(
        pdf::objects::pdf_stream::PdfStream::from_decoded_data(
            dict,
            b"{ 2 copy add 2 div }".to_vec(),
        )?,
    ))?;
    let source = format!(
        "<< /ShadingType 1 /ColorSpace /DeviceRGB /Matrix [160 0 0 80 20 20] /Function {} {} R >>",
        function.obj_num(),
        function.gen_num()
    );
    let shading = SyntaxParser::new(StreamReader::try_new(std::io::Cursor::new(
        source.into_bytes(),
    ))?)
    .get_object()?;
    let id = doc.try_insert_object(shading)?;
    let name = doc
        .resources()
        .register_existing(7, ResourceKind::Shading, id)?;
    doc.replace_page_contents(7, format!("/{name} sh").into_bytes())?;
    doc.save_to_file_with_options(
        path,
        SaveOptions {
            compression,
            ..Default::default()
        },
    )
}
