//! Explicit structure authoring over an independent two-Tj page fixture.
use pdf::{
    document::{
        PdfDocument,
        structure::{StructureAttributeWrite, StructureElementDefinition, StructureRemoval},
    },
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict, pdf_name::PdfName},
    page::marked_text::MarkedTextOptions,
    parser::syntax::ParseMode,
};
fn main() -> PdfResult<()> {
    let mut args = std::env::args_os().skip(1);
    let input = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("expected input.pdf new-output.pdf".into()))?;
    let output = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("expected new-output.pdf".into()))?;
    if args.next().is_some() {
        return Err(PdfError::DocumentError("too many arguments".into()));
    }
    let doc = PdfDocument::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?;
    if doc.structure_tree()?.is_some() || doc.try_page_count()? != 1 {
        return Err(PdfError::DocumentError(
            "example expects one unstructured page with exactly two Tj operations".into(),
        ));
    }
    let shows: Vec<_> = doc
        .get_page(0)?
        .content_program()?
        .operations()
        .iter()
        .enumerate()
        .filter(|(_, o)| o.operator.name() == "Tj")
        .map(|(i, _)| i)
        .collect();
    if shows.len() != 2 {
        return Err(PdfError::DocumentError(
            "expected exactly two Tj operations".into(),
        ));
    }
    let group = doc.add_structure_element(None, 0, &StructureElementDefinition::new("Document"))?;
    let first =
        doc.add_structure_element(Some(&group), 0, &StructureElementDefinition::new("P"))?;
    let second =
        doc.add_structure_element(Some(&group), 1, &StructureElementDefinition::new("P"))?;
    doc.mark_structure_content(&first, 0, &doc.stamp(), shows[0]..shows[0] + 1, 0, None)?;
    let second_show = doc
        .get_page(0)?
        .content_program()?
        .operations()
        .iter()
        .enumerate()
        .filter(|(_, o)| o.operator.name() == "Tj")
        .nth(1)
        .unwrap()
        .0;
    doc.mark_structure_content(
        &second,
        0,
        &doc.stamp(),
        second_show..second_show + 1,
        0,
        Some("你好"),
    )?;
    doc.move_structure_element(&second, Some(&group), 0)?;
    doc.set_structure_role_mapping("CustomParagraph", Some("P"))?;
    let mut definition = StructureElementDefinition::new("CustomParagraph");
    definition.title = Some("标题".into());
    definition.language = Some("zh-CN".into());
    let mut values = PdfDict::default();
    values.try_insert(
        "Placement".into(),
        PdfObject::PdfName(PdfName::new("Block".into())),
    )?;
    definition.attributes.push(StructureAttributeWrite {
        owner: "Layout".into(),
        values,
    });
    doc.update_structure_element(&first, &definition)?;
    let empty =
        doc.add_structure_element(Some(&group), 2, &StructureElementDefinition::new("Div"))?;
    doc.remove_structure_element(&empty, StructureRemoval::EmptyOnly)?;
    let structured = doc
        .extract_text_structure_order(Default::default())?
        .unwrap()
        .text(&Default::default())?;
    let raw = doc
        .get_page(0)?
        .extract_marked_text(MarkedTextOptions {
            use_actual_text: false,
            include_artifacts: true,
        })?
        .text(&Default::default())?;
    let bytes = doc.save_to_bytes()?;
    let reopened = PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
    reopened.structure_associations()?;
    if reopened
        .extract_text_structure_order(Default::default())?
        .unwrap()
        .text(&Default::default())?
        != structured
    {
        return Err(PdfError::DocumentError(
            "strict reopen changed structure text".into(),
        ));
    }
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(output)?
        .write_all(&bytes)?;
    println!("structure={structured:?} raw={raw:?} elements=3");
    Ok(())
}
