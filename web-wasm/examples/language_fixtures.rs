//! Deliberately distinct synthetic shapes with one Unicode mapping for locale routing.
use pdf::{
    document::{Document, resource_manager::ResourceKind},
    io::stream_reader::StreamReader,
    objects::{
        PdfObject, object_id::ObjectId, pdf_reference::PdfReference, pdf_stream::PdfStream,
        pdf_string::PdfString,
    },
    page::page_builder::PageBuilder,
    parser::syntax::SyntaxParser,
};
const SYMBOL: &[u8] = include_bytes!("../../pdf/tests/fixtures/synthetic-symbol.ttf");
fn parse(text: &str) -> PdfObject {
    SyntaxParser::new(
        StreamReader::try_new(std::io::Cursor::new(text.as_bytes().to_vec())).unwrap(),
    )
    .get_object()
    .unwrap()
}
fn variant() -> Vec<u8> {
    let mut b = SYMBOL.to_vec();
    let tables = u16::from_be_bytes([b[4], b[5]]) as usize;
    let record = (0..tables)
        .map(|i| 12 + i * 16)
        .find(|i| &b[*i..*i + 4] == b"head")
        .unwrap();
    let offset = u32::from_be_bytes(b[record + 8..record + 12].try_into().unwrap()) as usize;
    let len = u32::from_be_bytes(b[record + 12..record + 16].try_into().unwrap()) as usize;
    b[offset + 18..offset + 20].copy_from_slice(&2000u16.to_be_bytes());
    b[offset + 8..offset + 12].fill(0);
    let sum = |data: &[u8]| {
        data.chunks(4).fold(0u32, |sum, p| {
            let mut v = [0; 4];
            v[..p.len()].copy_from_slice(p);
            sum.wrapping_add(u32::from_be_bytes(v))
        })
    };
    let table = sum(&b[offset..offset + len]);
    b[record + 4..record + 8].copy_from_slice(&table.to_be_bytes());
    let adjustment = 0xB1B0_AFBAu32.wrapping_sub(sum(&b));
    b[offset + 8..offset + 12].copy_from_slice(&adjustment.to_be_bytes());
    b
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let out = std::path::PathBuf::from(std::env::args().nth(1).ok_or("output directory required")?);
    std::fs::create_dir_all(&out)?;
    std::fs::write(out.join("language-zh.ttf"), SYMBOL)?;
    std::fs::write(out.join("language-ja.ttf"), variant())?;
    let d = Document::new()?;
    d.add_page_with_builder(0, PageBuilder::new().size(300., 100.))?;
    let catalog = ObjectId::new(2, 0);
    let mut root = d.resolve_object(catalog)?.into_dict().unwrap();
    root.try_insert(
        "Lang".into(),
        PdfObject::PdfString(PdfString::from_text("ja-JP")?),
    )?;
    d.update_object(catalog, PdfObject::PdfDict(root))?;
    let cmap = d.try_insert_object(PdfObject::PdfStream(PdfStream::new(
        Default::default(),
        b"1 begincodespacerange <00> <ff> endcodespacerange 1 beginbfchar <20> <00d7> endbfchar"
            .to_vec(),
    )))?;
    let mut font=parse("<< /Type /Font /Subtype /TrueType /BaseFont /MissingLocale /FirstChar 32 /LastChar 32 /Widths [1000] /FontDescriptor << /Flags 0 >> >>").into_dict().unwrap();
    font.try_insert(
        "ToUnicode".into(),
        PdfObject::PdfReference(PdfReference::from_id(cmap)),
    )?;
    let font = d.try_insert_object(PdfObject::PdfDict(font))?;
    let name = d
        .resources()
        .register_existing(0, ResourceKind::Font, font)?;
    let show = |x| format!("BT /{name} 20 Tf {x} 50 Td <20> Tj ET ");
    let form = d.try_insert_object(PdfObject::PdfStream(PdfStream::new(
        parse("<< /Subtype /Form /BBox [0 0 300 100] >>")
            .into_dict()
            .unwrap(),
        format!(
            "/Span << /Lang (ja-JP) >> BDC {} EMC {}",
            show(100),
            show(140)
        )
        .into_bytes(),
    )))?;
    let form = d
        .resources()
        .register_existing(0, ResourceKind::Form, form)?;
    d.replace_page_contents(0,format!("{} /Span << /Lang (zh-Hant-HK) >> BDC {} /{form} Do {} EMC {} /Span << /Lang () >> BDC {} EMC",show(20),show(60),show(180),show(220),show(260)).into_bytes())?;
    std::fs::write(out.join("language.pdf"), d.save_to_bytes()?)?;
    println!("language fixtures generated");
    Ok(())
}
