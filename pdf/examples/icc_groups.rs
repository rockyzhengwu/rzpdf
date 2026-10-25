//! Generated, non-private ICC working-space and transform-budget regressions.
use pdf::{
    document::{Document, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::SyntaxParser,
};
use std::io::Cursor;

fn object(source: &str) -> PdfObject {
    SyntaxParser::new(StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap())
        .get_object()
        .unwrap()
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let output = std::path::PathBuf::from(
        std::env::args_os()
            .nth(1)
            .ok_or("provide a NEW output directory")?,
    );
    std::fs::create_dir(&output)?;
    let many = Document::new()?;
    many.add_page_with_builder(0, PageBuilder::new().size(30., 20.))?;
    many.add_page_with_builder(
        1,
        PageBuilder::new()
            .size(30., 20.)
            .content(b"0 0 30 20 re f".to_vec()),
    )?;
    let mut content = String::new();
    for index in 0..20 {
        let data = moxcms::ColorProfile::new_gray_with_gamma(1. + index as f32 * 0.05).encode()?;
        let profile = many.try_insert_object(PdfObject::PdfStream(
            PdfStream::from_decoded_data(object("<< /N 1 >>").into_dict().unwrap(), data)?,
        ))?;
        let cs =
            many.try_insert_object(object(&format!("[/ICCBased {} 0 R]", profile.obj_num())))?;
        let name = many
            .resources()
            .register_existing(0, ResourceKind::ColorSpace, cs)?;
        content.push_str(&format!("/{name} cs 0.5 sc 0 0 30 20 re f\n"));
    }
    many.replace_page_contents(0, content.into_bytes())?;
    std::fs::write(output.join("many-profiles.pdf"), many.save_to_bytes()?)?;

    let mut linear = moxcms::ColorProfile::new_srgb();
    let curve = moxcms::ColorProfile::new_gray_with_gamma(1.).gray_trc;
    linear.red_trc = curve.clone();
    linear.green_trc = curve.clone();
    linear.blue_trc = curve;
    for unsupported in [false, true] {
        let doc = Document::new()?;
        doc.add_page_with_builder(0, PageBuilder::new().size(30., 20.))?;
        let cs = if unsupported {
            "/DeviceCMYK".to_owned()
        } else {
            let profile =
                doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
                    object("<< /N 3 >>").into_dict().unwrap(),
                    linear.encode()?,
                )?))?;
            let cs =
                doc.try_insert_object(object(&format!("[/ICCBased {} 0 R]", profile.obj_num())))?;
            format!("{} 0 R", cs.obj_num())
        };
        let dict = object(&format!("<< /Subtype /Form /BBox [0 0 30 20] /Group << /S /Transparency /I true /CS {cs} >> /Resources << /ExtGState << /Half << /ca 0.5 >> >> /Font << /F << /Type /Font /Subtype /Type1 /BaseFont /Courier >> >> >> >>")).into_dict().unwrap();
        let form = doc.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(dict,
            b"0 0 0 rg 0 0 30 20 re f /Half gs 1 1 1 rg 0 0 30 20 re f BT /F 10 Tf 3 Tr (ABC) Tj ET".to_vec())?))?;
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, form)?;
        doc.replace_page_contents(0, format!("/{name} Do").into_bytes())?;
        std::fs::write(
            output.join(if unsupported {
                "unsupported-group.pdf"
            } else {
                "linear-group.pdf"
            }),
            doc.save_to_bytes()?,
        )?;
    }
    // Backend regression: sparse CID 42 maps to physical GID 2. Native
    // CoreText and the browser's FreeType port must both paint this glyph.
    let cff = Document::new()?;
    cff.add_page_with_builder(0, PageBuilder::new().size(40., 40.))?;
    let program = cff.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
        object("<< /Subtype /CIDFontType0C >>").into_dict().unwrap(),
        include_bytes!("../tests/fixtures/synthetic-cid.cff").to_vec(),
    )?))?;
    let font = cff.try_insert_object(object(&format!("<< /Type /Font /Subtype /Type0 /BaseFont /RzpdfSyntheticCID /Encoding /Identity-H /DescendantFonts [<< /Subtype /CIDFontType0 /BaseFont /RzpdfSyntheticCID /CIDSystemInfo << /Registry (Adobe) /Ordering (Identity) /Supplement 0 >> /FontDescriptor << /FontFile3 {} 0 R >> >>] >>", program.obj_num())))?;
    let name = cff
        .resources()
        .register_existing(0, ResourceKind::Font, font)?;
    cff.replace_page_contents(
        0,
        format!("BT /{name} 20 Tf 1 0 0 1 5 10 Tm <002a> Tj ET").into_bytes(),
    )?;
    std::fs::write(output.join("sparse-cid.pdf"), cff.save_to_bytes()?)?;

    let cyclic = Document::new()?;
    let space = cyclic.try_insert_object(PdfObject::PdfNull)?;
    cyclic.update_object(
        space,
        object(&format!(
            "[/DeviceN [/Spot] {} 0 R << /FunctionType 2 /Domain [0 1] /N 1 >>]",
            space.obj_num()
        )),
    )?;
    cyclic.add_page_with_builder(
        0,
        PageBuilder::new()
            .size(30., 20.)
            .resources(
                object(&format!(
                    "<< /ColorSpace << /C {} 0 R >> >>",
                    space.obj_num()
                ))
                .into_dict()
                .unwrap(),
            )
            .content(b"/C cs 0.5 sc 0 0 30 20 re f".to_vec()),
    )?;
    cyclic.add_page_with_builder(
        1,
        PageBuilder::new()
            .size(30., 20.)
            .content(b"0 0 30 20 re f".to_vec()),
    )?;
    std::fs::write(output.join("cyclic-color.pdf"), cyclic.save_to_bytes()?)?;
    Ok(())
}
