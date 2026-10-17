//! Inspect hierarchy only; not a Tagged PDF conformance or content validator.
use pdf::{
    document::{PdfDocument, structure::StructureKid},
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
    match doc.structure_tree()? {
        None => println!("structure=absent"),
        Some(tree) => {
            println!(
                "elements={} content_validation=not_performed parent_tree_validation=not_performed",
                tree.elements.len()
            );
            for node in tree.elements {
                println!(
                    "role={} mapped_role={:?} raw_content={} actual_text={:?}",
                    node.role,
                    node.mapped_role,
                    node.children
                        .iter()
                        .filter(|k| matches!(k, StructureKid::UnvalidatedContent(_)))
                        .count(),
                    node.actual_text
                );
            }
        }
    }
    assert!(doc.is_stamp_current(&stamp));
    if let Some(output) = output {
        use std::io::Write;
        let bytes = doc.save_to_bytes()?;
        PdfDocument::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?.structure_tree()?;
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output)?
            .write_all(&bytes)?;
    }
    Ok(())
}
