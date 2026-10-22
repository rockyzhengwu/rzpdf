//! Original, licensed-font input samples for Web subset/span editing.
use pdf::{
    document::{Document, embedded_font::TrueTypeEmbeddingOptions, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{PdfObject, pdf_stream::PdfStream},
    page::page_builder::PageBuilder,
    parser::syntax::SyntaxParser,
};
use std::io::Cursor;
const FONT: &[u8] = include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf");
fn make(font: &[u8]) -> Result<Document, Box<dyn std::error::Error>> {
    let d = Document::new()?;
    for i in 0..2 {
        d.add_page_with_builder(i, PageBuilder::new().size(300., 300.))?;
    }
    let f = d.resources().embed_truetype_font(
        0,
        font.to_vec(),
        "你",
        TrueTypeEmbeddingOptions {
            postscript_name: "NotoSansSC-Thin".into(),
            cap_height: 733.,
            stem_v: 80.,
            serif: false,
        },
    )?;
    let name = f.register_on_page(&d, 0)?;
    let hex = f
        .encoding()
        .encode_text("你")?
        .iter()
        .map(|b| format!("{b:02X}"))
        .collect::<String>();
    let mut dict = SyntaxParser::new(StreamReader::try_new(Cursor::new(
        b"<< /Subtype /Form /BBox [0 0 100 80] >>".to_vec(),
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
        format!("BT /{name} 14 Tf 10 30 Td <{hex}> Tj <{hex}> Tj ET").into_bytes(),
    )?))?;
    let call = d
        .resources()
        .register_existing(0, ResourceKind::Form, form)?;
    d.replace_page_contents(0,format!("BT /{name} 14 Tf 20 260 Td <{hex}> Tj <{hex}> Tj 0 -40 Td [<{hex}> -50 <{hex}>] TJ ET q 1 0 0 1 20 80 cm /{call} Do Q q 1 0 0 1 160 80 cm /{call} Do Q").into_bytes())?;
    let second = f.register_on_page(&d, 1)?;
    d.replace_page_contents(
        1,
        format!("BT /{second} 14 Tf 20 200 Td <{hex}> Tj ET").into_bytes(),
    )?;
    Ok(d)
}
fn checksum(data: &[u8]) -> u32 {
    data.chunks(4).fold(0u32, |s, c| {
        let mut word = [0; 4];
        word[..c.len()].copy_from_slice(c);
        s.wrapping_add(u32::from_be_bytes(word))
    })
}
fn restricted_source() -> Vec<u8> {
    let mut bytes = FONT.to_vec();
    let count = u16::from_be_bytes(bytes[4..6].try_into().unwrap()) as usize;
    let mut head = 0;
    for index in 0..count {
        let base = 12 + index * 16;
        let offset = u32::from_be_bytes(bytes[base + 8..base + 12].try_into().unwrap()) as usize;
        let length = u32::from_be_bytes(bytes[base + 12..base + 16].try_into().unwrap()) as usize;
        if &bytes[base..base + 4] == b"OS/2" {
            bytes[offset + 8..offset + 10].copy_from_slice(&0x100u16.to_be_bytes());
            let sum = checksum(&bytes[offset..offset + length]);
            bytes[base + 4..base + 8].copy_from_slice(&sum.to_be_bytes());
        }
        if &bytes[base..base + 4] == b"head" {
            head = offset;
        }
    }
    bytes[head + 8..head + 12].fill(0);
    let adjustment = 0xB1B0AFBAu32.wrapping_sub(checksum(&bytes));
    bytes[head + 8..head + 12].copy_from_slice(&adjustment.to_be_bytes());
    bytes
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let root =
        std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&root)?;
    std::fs::write(root.join("font-write.pdf"), make(FONT)?.save_to_bytes()?)?;
    std::fs::write(root.join("font-write-source.ttf"), FONT)?;
    let restricted = restricted_source();
    std::fs::write(root.join("font-write-no-subset.ttf"), &restricted)?;
    std::fs::write(
        root.join("font-write-no-subset.pdf"),
        make(&restricted)?.save_to_bytes()?,
    )?;
    std::fs::write(
        root.join("font-write-wrong.ttf"),
        include_bytes!("../../pdf/tests/fixtures/v03/RzpdfTestSans-composite.ttf"),
    )?;
    // Hidden text lies between two visible events; an apparent visible-only span must refuse.
    use pdf::document::{
        optional_content_edit::{LayerBaseState, LayerConfiguration, LayerIntent},
        resource_manager::StandardFont,
    };
    let hidden = Document::new()?;
    hidden.add_page_with_builder(0, PageBuilder::new().size(300., 300.))?;
    let font = hidden
        .resources()
        .register_standard_font(0, StandardFont::Helvetica)?;
    hidden.replace_page_contents(
        0,
        format!("BT /{font} 12 Tf 20 200 Td (A) Tj ET").into_bytes(),
    )?;
    let budget = pdf::budget::OperationBudget::new(64 * 1024 * 1024);
    let layer = hidden.create_layer("Hidden", LayerIntent::View, &budget)?;
    let target = hidden.layer_configuration_target(None)?;
    hidden.write_layer_configuration(
        Some(&target),
        &LayerConfiguration {
            name: None,
            base: LayerBaseState::On,
            on: vec![],
            off: vec![layer.clone()],
        },
        &budget,
    )?;
    hidden.append_layer_content(
        0,
        &layer,
        format!("BT /{font} 12 Tf 20 170 Td (SECRET) Tj ET").as_bytes(),
        &budget,
    )?;
    hidden.append_page_contents(
        0,
        format!("BT /{font} 12 Tf 20 140 Td (B) Tj ET").into_bytes(),
    )?;
    std::fs::write(
        root.join("font-write-hidden-span.pdf"),
        hidden.save_to_bytes()?,
    )?;
    let display = Document::new()?;
    display.add_page_with_builder(0, PageBuilder::new().size(300., 100.))?;
    let parse = |bytes: Vec<u8>| {
        SyntaxParser::new(StreamReader::try_new(Cursor::new(bytes)).unwrap())
            .get_object()
            .unwrap()
            .into_dict()
            .unwrap()
    };
    let mut missing=parse(b"<< /Type /Font /Subtype /TrueType /BaseFont /AAAAAA+MissingFace /FirstChar 31 /LastChar 32 /Widths [1000 1000] /FontDescriptor << /Flags 4 >> >>".to_vec());
    let cmap=display.try_insert_object(PdfObject::PdfStream(PdfStream::new(Default::default(),b"1 begincodespacerange <00> <ff> endcodespacerange 2 beginbfchar <20> <65b0> <1f> <00d7> endbfchar".to_vec())))?;
    missing.try_insert(
        "ToUnicode".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(cmap)),
    )?;
    let object = display.try_insert_object(PdfObject::PdfDict(missing))?;
    let name = display
        .resources()
        .register_existing(0, ResourceKind::Font, object)?;
    display.replace_page_contents(
        0,
        format!("BT /{name} 20 Tf 20 40 Td <201f20> Tj ET").into_bytes(),
    )?;
    display.add_page_with_builder(1, PageBuilder::new().size(300., 100.))?;
    let mut reversed=parse(b"<< /Type /Font /Subtype /TrueType /BaseFont /AAAAAA+MissingFace /FirstChar 31 /LastChar 32 /Widths [1000 1000] /FontDescriptor << /Flags 4 >> >>".to_vec());
    let reverse_cmap=display.try_insert_object(PdfObject::PdfStream(PdfStream::new(Default::default(),b"1 begincodespacerange <00> <ff> endcodespacerange 2 beginbfchar <20> <00d7> <1f> <65b0> endbfchar".to_vec())))?;
    reversed.try_insert(
        "ToUnicode".into(),
        PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(
            reverse_cmap,
        )),
    )?;
    let reverse_object = display.try_insert_object(PdfObject::PdfDict(reversed))?;
    let reverse_name =
        display
            .resources()
            .register_existing(1, ResourceKind::Font, reverse_object)?;
    display.replace_page_contents(
        1,
        format!("BT /{reverse_name} 20 Tf 20 40 Td <201f20> Tj ET").into_bytes(),
    )?;
    let font_resources = format!("/Resources << /Font << /F {} 0 R >> >>", object.obj_num());
    let program = b"BT /F 20 Tf 20 40 Td <201f20> Tj ET".to_vec();
    let nested = display.try_insert_object(PdfObject::PdfStream(PdfStream::new(
        parse(format!("<< /Subtype /Form /BBox [0 0 300 100] {font_resources} >>").into_bytes()),
        program.clone(),
    )))?;
    display.add_page_with_builder(
        2,
        PageBuilder::new()
            .size(300., 100.)
            .resources(parse(
                format!("<< /XObject << /Nested {} 0 R >> >>", nested.obj_num()).into_bytes(),
            ))
            .content(b"/Nested Do".to_vec()),
    )?;
    let pattern=display.try_insert_object(PdfObject::PdfStream(PdfStream::new(parse(format!("<< /Type /Pattern /PatternType 1 /PaintType 1 /TilingType 1 /BBox [0 0 100 100] /XStep 100 /YStep 100 {font_resources} >>").into_bytes()),program.clone())))?;
    display.add_page_with_builder(
        3,
        PageBuilder::new()
            .size(300., 100.)
            .resources(parse(
                format!("<< /Pattern << /Tile {} 0 R >> >>", pattern.obj_num()).into_bytes(),
            ))
            .content(b"/Pattern cs /Tile scn 0 0 300 100 re f".to_vec()),
    )?;
    let mask=display.try_insert_object(PdfObject::PdfStream(PdfStream::new(parse(format!("<< /Subtype /Form /BBox [0 0 300 100] /Group << /S /Transparency /CS /DeviceRGB /I true >> {font_resources} >>").into_bytes()),program.clone())))?;
    display.add_page_with_builder(
        4,
        PageBuilder::new()
            .size(300., 100.)
            .resources(parse(
                format!(
                    "<< /ExtGState << /Mask << /SMask << /S /Alpha /G {} 0 R >> >> >> >>",
                    mask.obj_num()
                )
                .into_bytes(),
            ))
            .content(b"/Mask gs 0 0 1 rg 0 0 300 100 re f".to_vec()),
    )?;
    let mut glyph_program = b"80 0 d0 ".to_vec();
    glyph_program.extend_from_slice(b"0 0 1 rg 20 30 60 40 re f");
    let glyph = display.try_insert_object(PdfObject::PdfStream(PdfStream::new(
        Default::default(),
        glyph_program,
    )))?;
    let type3=display.try_insert_object(PdfObject::PdfDict(parse(format!("<< /Type /Font /Subtype /Type3 /FontMatrix [1 0 0 1 0 0] /FontBBox [0 0 100 100] /FirstChar 65 /LastChar 65 /Widths [80] /Encoding << /Differences [65 /A] >> /CharProcs << /A {} 0 R >> {font_resources} >>",glyph.obj_num()).into_bytes())))?;
    display.add_page_with_builder(
        5,
        PageBuilder::new()
            .size(300., 100.)
            .resources(parse(
                format!("<< /Font << /T3 {} 0 R >> >>", type3.obj_num()).into_bytes(),
            ))
            .content(b"BT /T3 1 Tf 0 0 Td (A) Tj ET".to_vec()),
    )?;
    for (page,mapping) in [(6,b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <01> <0041> endbfchar".as_slice()),(7,b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <20> <00660069> endbfchar".as_slice())] {
        let mut font=parse(b"<< /Type /Font /Subtype /TrueType /BaseFont /MissingMapping /FirstChar 32 /LastChar 32 /Widths [1000] /FontDescriptor << /Flags 4 >> >>".to_vec());
        let cmap=display.try_insert_object(PdfObject::PdfStream(PdfStream::new(Default::default(),mapping.to_vec())))?;
        font.try_insert("ToUnicode".into(),PdfObject::PdfReference(pdf::objects::pdf_reference::PdfReference::from_id(cmap)))?;
        let id=display.try_insert_object(PdfObject::PdfDict(font))?;
        display.add_page_with_builder(page,PageBuilder::new().size(300.,100.).resources(parse(format!("<< /Font << /F {} 0 R >> >>",id.obj_num()).into_bytes())).content(b"BT /F 20 Tf 20 40 Td <2020> Tj ET".to_vec()))?;
    }
    display.add_page_with_builder(
        8,
        PageBuilder::new()
            .size(300., 100.)
            .resources(parse(
                format!("<< /Font << /F {} 0 R >> >>", object.obj_num()).into_bytes(),
            ))
            .content(b"0 0 1 rg 0 0 20 20 re f".to_vec()),
    )?;
    let hidden_layer = display.create_layer("Hidden missing font", LayerIntent::View, &budget)?;
    let configuration = display.layer_configuration_target(None)?;
    display.write_layer_configuration(
        Some(&configuration),
        &LayerConfiguration {
            name: None,
            base: LayerBaseState::On,
            on: vec![],
            off: vec![hidden_layer.clone()],
        },
        &budget,
    )?;
    display.append_layer_content(8, &hidden_layer, &program, &budget)?;
    display.add_page_with_builder(9, PageBuilder::new().size(300., 100.))?;
    let annotation = display.add_annotation(
        9,
        &pdf::document::annotations::Annotation::new(
            pdf::document::annotations::AnnotationKind::Square,
            [0., 0., 300., 100.],
        ),
    )?;
    let mut annotation_dict = display
        .resolve_object(annotation.object_id())?
        .into_dict()
        .unwrap();
    annotation_dict.try_insert(
        "AP".into(),
        PdfObject::PdfDict(parse(
            format!("<< /N {} 0 R >>", nested.obj_num()).into_bytes(),
        )),
    )?;
    display.update_object(annotation.object_id(), PdfObject::PdfDict(annotation_dict))?;
    let mut unsupported_glyph = b"80 0 d0 ".to_vec();
    unsupported_glyph.extend_from_slice(&program);
    let unsupported_glyph = display.try_insert_object(PdfObject::PdfStream(PdfStream::new(
        Default::default(),
        unsupported_glyph,
    )))?;
    let unsupported_type3=display.try_insert_object(PdfObject::PdfDict(parse(format!("<< /Type /Font /Subtype /Type3 /FontMatrix [1 0 0 1 0 0] /FontBBox [0 0 100 100] /FirstChar 65 /LastChar 65 /Widths [80] /Encoding << /Differences [65 /A] >> /CharProcs << /A {} 0 R >> {font_resources} >>",unsupported_glyph.obj_num()).into_bytes())))?;
    display.add_page_with_builder(
        10,
        PageBuilder::new()
            .size(300., 100.)
            .resources(parse(
                format!("<< /Font << /T3 {} 0 R >> >>", unsupported_type3.obj_num()).into_bytes(),
            ))
            .content(b"BT /T3 1 Tf 0 0 Td (A) Tj ET".to_vec()),
    )?;
    let mut protection = pdf::security::EncryptionOptions::new("user", "owner")?;
    protection.permissions.copy = false;
    std::fs::write(
        root.join("font-service-no-copy.pdf"),
        display.save_to_bytes_with_security(
            Default::default(),
            pdf::security::SaveSecurity::Set(&protection),
        )?,
    )?;
    std::fs::write(root.join("font-service.pdf"), display.save_to_bytes()?)?;
    std::fs::write(
        root.join("font-service-symbol.ttf"),
        include_bytes!("../../pdf/tests/fixtures/synthetic-symbol.ttf"),
    )?;
    println!("font subset/span fixtures generated");
    Ok(())
}
