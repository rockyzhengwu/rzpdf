//! Read-only namespace and local attribute inspection, not PDF/UA validation.
use pdf::{
    document::{
        PdfDocument,
        structure::{StructureAttributeLookup, StructureAttributeOwner},
    },
    error::{PdfError, PdfResult},
    objects::PdfObject,
    parser::syntax::ParseMode,
};
fn scalar(value: &PdfObject) -> String {
    if let Some(name) = value.as_name() {
        format!("/{}", name.name())
    } else if let Some(number) = value.as_number() {
        format!("{number:?}")
    } else {
        "raw-object".into()
    }
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
    if let Some(tree) = doc.structure_tree()? {
        println!(
            "elements={} namespaces={} classes={}",
            tree.elements.len(),
            tree.namespaces.len(),
            tree.attribute_classes.len()
        );
        for node in &tree.elements {
            println!(
                "role={} endpoint={} namespace={:?} status={:?} bindings={} revision={}",
                node.role,
                node.resolved_role.name,
                node.resolved_role.namespace_uri,
                node.resolved_role.status,
                node.attribute_bindings.len(),
                node.revision
            );
            for (owner, name) in [("Layout", "TextAlign"), ("Layout", "WritingMode")] {
                let lookup = node.attribute(&StructureAttributeOwner::Name(owner.into()), name);
                let text = match lookup {
                    StructureAttributeLookup::Missing => "missing".into(),
                    StructureAttributeLookup::Value { value, .. } => scalar(value),
                    StructureAttributeLookup::Ambiguous(v) => format!("ambiguous:{}", v.len()),
                    StructureAttributeLookup::Opaque(v) => format!("opaque:{}", v.len()),
                };
                println!("{owner}.{name}={text}");
            }
        }
    } else {
        println!("structure=absent");
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
