//! TrueType ET clipping corpus: paint modes, signed sizes, compound clips.
use pdf::{
    document::{PdfDocument, resource_manager::ResourceKind},
    error::PdfResult,
    page::page_builder::PageBuilder,
    parser::syntax::ParseMode,
};
use std::{fs::OpenOptions, io::Write};
fn main() -> PdfResult<()> {
    let output = std::env::args_os().nth(1).expect("new PDF path");
    let source = PdfDocument::from_bytes_with_mode(
        include_bytes!("../tests/fixtures/v07/fonts.pdf").to_vec(),
        ParseMode::Strict,
    )?;
    let resources = source.get_page(2)?.resource()?;
    let id = resources
        .get("Font")
        .unwrap()
        .get_attr("F0")
        .unwrap()
        .as_object_id()
        .unwrap();
    let doc = PdfDocument::new()?;
    let handle =
        doc.import_resource_from(&source, &source.object_handle(id)?, ResourceKind::Font)?;
    let cases = [
        "20 Tf 4 Tr 1 0 0 1 5 5 Tm (A) Tj",
        "20 Tf 5 Tr 1 0 0 1 5 5 Tm (A) Tj",
        "20 Tf 6 Tr 1 0 0 1 5 5 Tm (A) Tj",
        "20 Tf 7 Tr 1 0 0 1 5 5 Tm (A) Tj",
        "20 Tf 7 Tr 1 0 0 1 5 5 Tm (A) Tj 1 0 0 1 25 5 Tm (A) Tj",
        "-20 Tf 7 Tr 1 0 0 1 35 25 Tm (A) Tj",
        "0 Tf 7 Tr 1 0 0 1 5 5 Tm (A) Tj",
        "20 Tf 7 Tr -100 Tz 1 0 0 1 35 5 Tm (A) Tj",
        "20 Tf 7 Tr 1 0.3 0.4 1 5 5 Tm (A) Tj",
    ];
    for (index, case) in cases.into_iter().enumerate() {
        doc.add_page_with_builder(index as u32, PageBuilder::new().size(40., 40.))?;
        let font = doc
            .resources()
            .register_handle(index as u32, ResourceKind::Font, &handle)?;
        doc.replace_page_contents(index as u32,format!("q 0 0 1 rg 0 1 0 RG 1 w BT /{font} {case} ET 1 0 0 rg 0 0 40 15 re f Q 0.5 g 0 35 40 5 re f").into_bytes())?;
    }
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&doc.save_to_bytes()?)?;
    Ok(())
}
