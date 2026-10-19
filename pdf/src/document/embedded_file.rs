//! Shared bounded Filespec/EF primitives. Never access an external file.
use super::PdfDocument;
use crate::{
    budget::{MemoryLease, OperationBudget},
    error::{PdfError, PdfResult},
    objects::{PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_stream::PdfStream},
};
use std::io::{Read, Seek};

pub(super) fn invalid(message: &str) -> PdfError {
    PdfError::DocumentError(format!("associated file: {message}"))
}

/// Preserve each legacy caller's explicit selection policy. AF additionally
/// validates names, identity and ambiguity before using this common selector.
pub(super) fn payload_reference(ef: &PdfDict, unicode_preferred: bool) -> PdfResult<&PdfObject> {
    let value = if unicode_preferred {
        ef.get("UF").or_else(|| ef.get("F"))
    } else {
        ef.get("F")
    };
    value.ok_or_else(|| invalid("embedded Filespec payload missing"))
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Resolve one bounded reference chain, retaining the final owned value.
    pub(super) fn file_resolve(
        &self,
        input: &PdfObject,
        budget: &OperationBudget,
    ) -> PdfResult<(PdfObject, MemoryLease)> {
        let mut seen: [Option<ObjectId>; 32] = [None; 32];
        let (mut value, mut lease) =
            crate::objects::clone_object_with_budget(input, budget, self.ctx.limits(), 0)?;
        let mut depth = 0;
        while let Some(reference) = value.as_reference() {
            let id = reference.id();
            if depth >= self.ctx.limits().max_depth.min(32) {
                return Err(PdfError::ResourceLimit("Filespec reference depth".into()));
            }
            if seen[..depth].contains(&Some(id)) {
                return Err(invalid("cyclic reference"));
            }
            seen[depth] = Some(id);
            depth += 1;
            let (next, next_lease) = self.ctx.resolve_owned_with_budget(&value, budget)?;
            value = next;
            lease = next_lease;
        }
        Ok((value, lease))
    }

    pub(super) fn file_dict(
        &self,
        input: &PdfObject,
        budget: &OperationBudget,
    ) -> PdfResult<(PdfDict, MemoryLease)> {
        let (object, lease) = self.file_resolve(input, budget)?;
        Ok((
            object
                .into_dict()
                .ok_or_else(|| invalid("dictionary expected"))?,
            lease,
        ))
    }

    /// The AF authoring profile accepts only a single embedded payload.
    /// Legacy attachment and annotation entry points retain their own policies.
    pub(super) fn file_stream(
        &self,
        spec: &PdfDict,
        budget: &OperationBudget,
    ) -> PdfResult<Option<(PdfStream, MemoryLease)>> {
        let Some(ef) = spec.get("EF") else {
            return Ok(None);
        };
        let (ef, _lease) = self.file_dict(ef, budget)?;
        if ef
            .into_iter()
            .any(|(key, _)| !matches!(key.as_str(), "F" | "UF"))
        {
            return Err(PdfError::UnsupportedFeature(
                "multi-platform Filespec EF".into(),
            ));
        }
        for key in ["F", "UF"] {
            if ef.contains_key(key) && !spec.contains_key(key) {
                return Err(invalid("EF key has no matching Filespec filename"));
            }
        }
        if let (Some(f), Some(uf)) = (ef.get("F"), ef.get("UF"))
            && f != uf
        {
            return Err(PdfError::UnsupportedFeature(
                "ambiguous EF F/UF payloads".into(),
            ));
        }
        let value = payload_reference(&ef, true)?;
        if value.as_reference().is_none() {
            return Err(invalid("embedded stream must be indirect"));
        }
        let (value, lease) = self.file_resolve(value, budget)?;
        let stream = value
            .into_stream()
            .ok_or_else(|| invalid("EF value is not a stream"))?;
        if ["F", "FFilter", "FDecodeParms"]
            .iter()
            .any(|key| stream.dict().contains_key(key))
        {
            return Err(PdfError::UnsupportedFeature("external-file stream".into()));
        }
        let kind = stream
            .dict()
            .get("Type")
            .ok_or_else(|| invalid("EmbeddedFile Type missing"))?;
        let (kind, _kind_lease) = self.file_resolve(kind, budget)?;
        if kind.as_name().map(|n| n.name()) != Some("EmbeddedFile") {
            return Err(invalid("stream Type must be EmbeddedFile"));
        }
        Ok(Some((stream, lease)))
    }
}
