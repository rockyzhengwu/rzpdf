//! Validate immutable protection identity before append-only serialization.
//! No reauthentication, rekeying, or mutation of the source handler occurs.
use crate::{
    budget::OperationBudget,
    document::{PdfDocument, store::StoreEntry},
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict},
};
use std::io::{Read, Seek};

fn unsupported(message: &str) -> PdfError {
    PdfError::UnsupportedSecurity(format!("encrypted incremental save: {message}"))
}

pub(super) fn validate<R: Read + Seek>(
    document: &PdfDocument<R>,
    original: &PdfDict,
    current: &PdfDict,
    changes: &[(ObjectId, StoreEntry)],
    budget: &OperationBudget,
) -> PdfResult<()> {
    let ctx = document.ctx();
    let Some(handler) = ctx.source_security() else {
        if [original, current].iter().any(|d| {
            d.get("Encrypt")
                .is_some_and(|v| !matches!(v, PdfObject::PdfNull))
        }) {
            return Err(unsupported("unauthenticated encryption dictionary"));
        }
        return Ok(());
    };
    if current.get("Encrypt") != original.get("Encrypt") {
        return Err(unsupported(
            "Encrypt identity changed; use explicit full save",
        ));
    }
    // The caller cannot change either ID. Serialization preserves the first
    // and generates a fresh revision ID without changing this source snapshot.
    if current.get("ID") != original.get("ID") {
        return Err(unsupported("file IDs changed; use explicit full save"));
    }
    match ctx.encryption_dictionary_id() {
        Some(id) => {
            if original.get("Encrypt").and_then(PdfObject::as_object_id) != Some(id) {
                return Err(unsupported("original Encrypt identity mismatch"));
            }
            if changes.iter().any(|(changed, _)| *changed == id) {
                return Err(unsupported(
                    "Encrypt object changed or deleted; use explicit full save",
                ));
            }
        }
        None if original
            .get("Encrypt")
            .is_some_and(|v| v.as_dict().is_some()) => {}
        None => return Err(unsupported("original Encrypt dictionary missing")),
    }
    let (object, _lease) = ctx.resolve_owned_with_budget(
        original
            .get("Encrypt")
            .ok_or_else(|| unsupported("missing Encrypt"))?,
        budget,
    )?;
    if object.as_dict() != Some(&handler.dictionary) {
        return Err(unsupported(
            "encryption parameters differ from authenticated source",
        ));
    }
    document.validate_security_version(document.effective_version_with_budget(budget)?, budget)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        document::Document,
        objects::{pdf_array::PdfArray, pdf_string::PdfString},
        parser::{options::ParseOptions, syntax::ParseMode},
        security::{EncryptionOptions, SaveSecurity},
    };
    fn opened() -> Document {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let settings = EncryptionOptions::new("user", "owner").unwrap();
        let bytes = doc
            .save_to_bytes_with_security(Default::default(), SaveSecurity::Set(&settings))
            .unwrap();
        Document::from_bytes_with_password_and_options(
            bytes,
            "owner",
            ParseOptions {
                mode: ParseMode::Strict,
                ..Default::default()
            },
        )
        .unwrap()
    }
    #[test]
    fn mutable_trailer_cannot_remove_or_replace_encrypt_identity() {
        let doc = opened();
        let original = doc.ctx().trailer().clone();
        let stamp = doc.stamp();
        for value in [
            None,
            Some(PdfObject::PdfNull),
            Some(PdfObject::PdfDict(PdfDict::default())),
        ] {
            let mut current = original.clone();
            current.remove("Encrypt");
            if let Some(value) = value {
                current.try_insert("Encrypt".into(), value).unwrap();
            }
            assert!(
                validate(
                    &doc,
                    &original,
                    &current,
                    &[],
                    &doc.ctx().limits().operation_budget()
                )
                .is_err()
            );
        }
        assert_eq!(stamp, doc.stamp());
    }
    #[test]
    fn caller_cannot_change_either_file_identifier() {
        let doc = opened();
        let original = doc.ctx().trailer().clone();
        for index in [0, 1] {
            let ids = original.get("ID").unwrap().as_array().unwrap();
            let mut replaced = PdfArray::default();
            for i in 0..2 {
                replaced.add_obj(if i == index {
                    PdfObject::PdfString(PdfString::new(b"different".to_vec(), true))
                } else {
                    ids.get(i).unwrap().clone()
                });
            }
            let mut current = original.clone();
            current
                .try_insert("ID".into(), PdfObject::PdfArray(replaced))
                .unwrap();
            assert!(
                validate(
                    &doc,
                    &original,
                    &current,
                    &[],
                    &doc.ctx().limits().operation_budget()
                )
                .is_err()
            );
        }
    }
    #[test]
    fn plaintext_source_cannot_gain_an_unauthenticated_encrypt_trailer() {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        let original = doc.ctx().trailer().clone();
        let mut current = original.clone();
        current
            .try_insert("Encrypt".into(), PdfObject::PdfDict(PdfDict::default()))
            .unwrap();
        assert!(
            validate(
                &doc,
                &original,
                &current,
                &[],
                &doc.ctx().limits().operation_budget()
            )
            .is_err()
        );
    }
}
