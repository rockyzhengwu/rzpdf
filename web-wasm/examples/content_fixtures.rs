//! Original content-edit fixtures, including shared Form invocations.
use pdf::{
    document::{
        Document,
        embedded_font::TrueTypeEmbeddingOptions,
        resource_manager::{ResourceKind, StandardFont},
    },
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::SyntaxParser,
};
use std::io::Cursor;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&root)?;
    let d = Document::new()?;
    d.add_page_with_builder(0, PageBuilder::new().size(300., 300.))?;
    d.add_page_with_builder(1, PageBuilder::new().size(180., 180.))?;
    let font = d
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)?;
    let image = d.resources().embed_rgb_image(
        0,
        2,
        2,
        vec![255, 0, 0, 0, 255, 0, 0, 0, 255, 255, 255, 0],
    )?;
    let chinese = d.resources().embed_truetype_font(
        0,
        include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf").to_vec(),
        "你好世界",
        TrueTypeEmbeddingOptions {
            postscript_name: "NotoSansSC-Thin".into(),
            cap_height: 733.,
            stem_v: 80.,
            serif: false,
        },
    )?;
    let chinese_name = chinese.register_on_page(&d, 0)?;
    let codes = chinese.encoding().encode_text("你好")?;
    let hex = codes.iter().map(|b| format!("{b:02X}")).collect::<String>();
    let mut dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(
        b"<< /Type /XObject /Subtype /Form /BBox [0 0 120 80] >>".to_vec(),
    ))?)
    .get_object()?
    .into_dict()
    .unwrap();
    dict.try_insert(
        "Resources".into(),
        PdfObject::PdfDict(d.get_page(0)?.resource()?),
    )?;
    let form = d.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
        dict,
        format!("BT /{font} 12 Tf 10 20 Td (Nested) Tj ET q 20 0 0 20 10 40 cm /{image} Do Q")
            .into_bytes(),
    )?))?;
    let form_name = d
        .resources()
        .register_existing(0, ResourceKind::Form, form)?;
    d.replace_page_contents(0,format!("BT /{font} 12 Tf 20 260 Td (Alpha) Tj (Tail) Tj 0 -25 Td [(Ad) -50 (justed)] TJ ET BT /{chinese_name} 14 Tf 20 200 Td <{hex}> Tj ET q 30 0 0 30 200 240 cm /{image} Do Q q 1 0 0 1 20 80 cm /{form_name} Do Q q 1 0 0 1 160 80 cm /{form_name} Do Q").into_bytes())?;
    let second = d
        .resources()
        .register_standard_font(1, StandardFont::Helvetica)?;
    d.replace_page_contents(
        1,
        format!("BT /{second} 12 Tf 20 120 Td (Second) Tj ET").into_bytes(),
    )?;
    let large = Document::new()?;
    large.add_page_with_builder(0, PageBuilder::new().size(300., 300.))?;
    let large_font = large
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)?;
    large.replace_page_contents(
        0,
        format!(
            "BT /{large_font} 12 Tf 20 200 Td ({}) Tj ET",
            "A".repeat(17000)
        )
        .into_bytes(),
    )?;
    std::fs::write(
        root.join("content-edit-metadata-limit.pdf"),
        large.save_to_bytes()?,
    )?;
    let source = d.save_to_bytes()?;
    std::fs::write(root.join("content-edit.pdf"), &source)?;
    let navigation = Document::from_bytes(source.clone())?;
    navigation.add_flat_bookmark("Imported second", 1)?;
    navigation.add_page_link(0, [20., 20., 80., 40.], 1)?;
    std::fs::write(
        root.join("page-import-navigation.pdf"),
        navigation.save_to_bytes()?,
    )?;
    let newer = Document::new_with_version(pdf::document::save_options::PdfVersion::V2_0)?;
    newer.add_page_with_builder(0, PageBuilder::new().size(100., 100.))?;
    std::fs::write(root.join("page-import-newer.pdf"), newer.save_to_bytes()?)?;
    let signature_hex = include_str!("../../pdf/tests/fixtures/v011/workflow-certified.pdf.hex")
        .split_whitespace()
        .collect::<String>();
    let signature = (0..signature_hex.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&signature_hex[i..i + 2], 16))
        .collect::<Result<Vec<_>, _>>()?;
    std::fs::write(root.join("page-import-signed.pdf"), signature)?;
    let inline = Document::from_bytes(source.clone())?;
    let form_dict = inline
        .resolve_object(form)?
        .as_stream()
        .unwrap()
        .dict()
        .clone();
    inline.update_object(
        form,
        PdfObject::PdfStream(PdfStream::from_decoded_data(
            form_dict,
            b"q 20 0 0 20 10 40 cm BI /W 1 /H 1 /CS /RGB /BPC 8 ID \xff\x00\x00 EI Q".to_vec(),
        )?),
    )?;
    std::fs::write(
        root.join("content-edit-inline.pdf"),
        inline.save_to_bytes()?,
    )?;
    let rotated = Document::from_bytes(source.clone())?;
    rotated.set_page_rotation(0, 90)?;
    rotated.set_page_crop_box(
        0,
        Some(pdf::geom::rectangle::Rectangle::new(10., 10., 290., 290.)),
    )?;
    rotated.set_page_user_unit(0, Some(1.5))?;
    std::fs::write(
        root.join("content-edit-rotated.pdf"),
        rotated.save_to_bytes()?,
    )?;
    for (name, copy, modify) in [
        ("content-edit-denied", true, false),
        ("content-edit-no-copy", false, true),
        ("content-edit-encrypted", true, true),
    ] {
        let mut protection = pdf::security::EncryptionOptions::new("user", "owner")?;
        protection.permissions.copy = copy;
        protection.permissions.modify = modify;
        std::fs::write(
            root.join(format!("{name}.pdf")),
            d.save_to_bytes_with_security(
                Default::default(),
                pdf::security::SaveSecurity::Set(&protection),
            )?,
        )?;
    }
    std::fs::write(
        root.join("content-replacement.png"),
        include_bytes!("../../pdf/tests/fixtures/v01/pixels.png"),
    )?;
    println!("source content fixtures generated");
    Ok(())
}
