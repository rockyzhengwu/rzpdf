//! Explicit logical/declared-order inspection; no inferred layout or PDF/UA claim.
use pdf::{
    document::PdfDocument,
    error::{PdfError, PdfResult},
    page::marked_text::MarkedTextOptions,
    parser::syntax::ParseMode,
};
fn report(doc: &PdfDocument<std::io::Cursor<Vec<u8>>>) -> PdfResult<Vec<String>> {
    let mut lines = Vec::new();
    for i in 0..doc.try_page_count()? {
        let mut page = doc.get_page(i as u32)?;
        let logical = page.extract_marked_text(Default::default())?;
        let raw = page.extract_marked_text(MarkedTextOptions {
            use_actual_text: false,
            include_artifacts: true,
        })?;
        lines.push(format!(
            "page[{i}] logical={:?} raw={:?}",
            logical.text(&Default::default())?,
            raw.text(&Default::default())?
        ));
        lines.push(format!(
            "page[{i}] logical_units={} replacements={}",
            logical.spans.len(),
            logical
                .spans
                .iter()
                .filter(|s| s.replacement.is_some())
                .count()
        ));
    }
    if let Some(structure) = doc.extract_text_structure_order(Default::default())? {
        lines.push(format!(
            "structure={:?} unassociated={} object_refs={}",
            structure.text(&Default::default())?,
            structure.unassociated_spans,
            structure.unextracted_object_references
        ));
    } else {
        lines.push("structure=absent".into());
    }
    Ok(lines)
}
fn main() -> PdfResult<()> {
    let mut args = std::env::args_os().skip(1);
    let input = args
        .next()
        .ok_or_else(|| PdfError::DocumentError("expected input.pdf [new-output.pdf]".into()))?;
    let output = args.next();
    if args.next().is_some() {
        return Err(PdfError::DocumentError("too many arguments".into()));
    }
    let doc = PdfDocument::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?;
    let stamp = doc.stamp();
    let lines = report(&doc)?;
    assert!(doc.is_stamp_current(&stamp));
    if let Some(output) = output {
        use std::io::Write;
        let bytes = doc.save_to_bytes()?;
        let reopened = PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
        assert_eq!(lines, report(&reopened)?);
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?
            .write_all(&bytes)?;
    }
    for line in lines {
        println!("{line}");
    }
    Ok(())
}
