//! Validate the supported PDF 1.x ownership subset, not PDF/UA conformance.
use pdf::{
    document::{PdfDocument, structure::StructureAssociation},
    error::{PdfError, PdfResult},
    parser::syntax::ParseMode,
};
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
    let result = doc.structure_associations()?;
    match &result {
        None => println!("structure=absent"),
        Some(result) => {
            let marked = result
                .items
                .iter()
                .filter(|item| matches!(item, StructureAssociation::MarkedContent { .. }))
                .count();
            println!(
                "associations={} marked_content={} object_references={}",
                result.items.len(),
                marked,
                result.items.len() - marked
            );
            println!("scope=pdf1-page-form-annotation-xobject-ownership conformance=not_checked");
        }
    }
    assert!(doc.is_stamp_current(&stamp));
    if let Some(output) = output {
        use std::io::Write;
        let bytes = doc.save_to_bytes()?;
        let reopened = PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
        let again = reopened.structure_associations()?;
        assert_eq!(
            result.as_ref().map(|r| r.items.len()),
            again.as_ref().map(|r| r.items.len())
        );
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?
            .write_all(&bytes)?;
    }
    Ok(())
}
