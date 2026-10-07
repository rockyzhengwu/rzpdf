use crate::{
    document::{PdfDocument, save_options::PdfVersion},
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict},
};
use std::io::{Read, Seek};

/// Targeted known feature checks, not a full feature/conformance validator.
pub(super) fn validate_feature_versions<R: Read + Seek>(
    doc: &PdfDocument<R>,
    root: &PdfObject,
    version: PdfVersion,
) -> PdfResult<()> {
    if version >= PdfVersion::V1_6 {
        return Ok(());
    }
    let limits = doc.ctx().limits();
    let mut pending = Vec::new();
    pending
        .try_reserve(1)
        .map_err(|_| PdfError::ResourceLimit("feature validation allocation".into()))?;
    pending.push(root);
    let mut nodes = 0usize;
    while let Some(object) = pending.pop() {
        nodes = nodes
            .checked_add(1)
            .filter(|count| *count <= limits.max_objects)
            .ok_or_else(|| PdfError::ResourceLimit("feature validation nodes".into()))?;
        let dict = match object {
            PdfObject::PdfDict(dict) => Some((dict, false)),
            PdfObject::PdfStream(stream) => Some((stream.dict(), true)),
            _ => None,
        };
        if let Some((dict, stream)) = dict {
            if version < PdfVersion::V1_6 && !stream && name(doc, dict, "Type", "Page")? {
                if let Some(value) = dict.get("UserUnit") {
                    if !matches!(doc.ctx().resolve_owned(value)?, PdfObject::PdfNull) {
                        return Err(PdfError::UnsupportedFeature("page UserUnit requires PDF 1.6 or later; select an explicit version upgrade".into()));
                    }
                }
            }
            if stream {
                check_filters(doc, dict, version)?;
                if version < PdfVersion::V1_5
                    && (name(doc, dict, "Type", "ObjStm")? || name(doc, dict, "Type", "XRef")?)
                {
                    return Err(PdfError::UnsupportedFeature(
                        "object/xref stream requires PDF 1.5 or later; select an explicit version upgrade".into(),
                    ));
                }
                if version < PdfVersion::V1_5 && name(doc, dict, "Subtype", "Image")? {
                    if let Some(bits) = dict.get("BitsPerComponent") {
                        let bits = doc.ctx().resolve_owned(bits)?;
                        if bits
                            .as_number()
                            .is_some_and(|number| number.as_u64_checked().ok() == Some(16))
                        {
                            return Err(PdfError::UnsupportedFeature(
                                "16-bit image requires PDF 1.5 or later; select an explicit version upgrade".into(),
                            ));
                        }
                    }
                }
                if version < PdfVersion::V1_4 && name(doc, dict, "Type", "Metadata")? {
                    return Err(PdfError::UnsupportedFeature(
                        "metadata stream requires PDF 1.4 or later; select an explicit version upgrade".into(),
                    ));
                }
            }
            if version < PdfVersion::V1_4 {
                check_dictionary(doc, dict, stream)?;
            }
            for (_, value) in dict {
                push(&mut pending, value, limits.max_objects)?;
            }
        } else if let PdfObject::PdfArray(array) = object {
            for value in array {
                push(&mut pending, value, limits.max_objects)?;
            }
        }
    }
    Ok(())
}

fn check_filters<R: Read + Seek>(
    doc: &PdfDocument<R>,
    dict: &PdfDict,
    version: PdfVersion,
) -> PdfResult<()> {
    if version < PdfVersion::V1_2 {
        for key in ["F", "FFilter", "FDecodeParms"] {
            if let Some(value) = dict.get(key) {
                if !matches!(doc.ctx().resolve_owned(value)?, PdfObject::PdfNull) {
                    return Err(PdfError::UnsupportedFeature(
                        "external stream settings require PDF 1.2 or later; select an explicit version upgrade".into(),
                    ));
                }
            }
        }
    }
    let check = |value: &PdfObject| -> PdfResult<()> {
        let name = value
            .as_name()
            .ok_or_else(|| PdfError::ObjectError("stream filter must be a name".into()))?;
        let minimum = match name.name() {
            "FlateDecode" | "Fl" => PdfVersion::V1_2,
            "JBIG2Decode" => PdfVersion::V1_4,
            "JPXDecode" | "Crypt" => PdfVersion::V1_5,
            _ => return Ok(()),
        };
        if version < minimum {
            return Err(PdfError::UnsupportedFeature(format!(
                "stream filter requires PDF {} or later; select an explicit version upgrade",
                minimum.as_str()
            )));
        }
        Ok(())
    };
    for key in ["Filter", "FFilter"] {
        let Some(filter) = dict.get(key) else {
            continue;
        };
        match doc.ctx().resolve_owned(filter)? {
            PdfObject::PdfNull => {}
            PdfObject::PdfArray(array) => {
                if array.len() > doc.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("filter feature entries".into()));
                }
                for value in &array {
                    check(&doc.ctx().resolve_owned(value)?)?;
                }
            }
            value => check(&value)?,
        }
    }
    Ok(())
}

fn push<'a>(pending: &mut Vec<&'a PdfObject>, value: &'a PdfObject, limit: usize) -> PdfResult<()> {
    if pending.len() >= limit {
        return Err(PdfError::ResourceLimit(
            "feature validation pending nodes".into(),
        ));
    }
    pending
        .try_reserve(1)
        .map_err(|_| PdfError::ResourceLimit("feature validation allocation".into()))?;
    pending.push(value);
    Ok(())
}

fn name<R: Read + Seek>(
    doc: &PdfDocument<R>,
    dict: &PdfDict,
    key: &str,
    expected: &str,
) -> PdfResult<bool> {
    match dict.get(key) {
        Some(value) => Ok(doc
            .ctx()
            .resolve_owned(value)?
            .as_name()
            .is_some_and(|name| name.name() == expected)),
        None => Ok(false),
    }
}

fn reject() -> PdfError {
    PdfError::UnsupportedFeature(
        "PDF transparency requires 1.4 or later; select an explicit version upgrade".into(),
    )
}
fn transparent_state(dict: &PdfDict) -> bool {
    ["ca", "CA", "BM", "SMask", "AIS", "TK"]
        .iter()
        .any(|key| dict.contains_key(key))
}

fn check_dictionary<R: Read + Seek>(
    doc: &PdfDocument<R>,
    dict: &PdfDict,
    stream: bool,
) -> PdfResult<()> {
    if name(doc, dict, "Type", "ExtGState")? && transparent_state(dict) {
        return Err(reject());
    }
    if stream && name(doc, dict, "Subtype", "Image")? && dict.contains_key("SMask") {
        return Err(reject());
    }
    if name(doc, dict, "Type", "Group")? && name(doc, dict, "S", "Transparency")? {
        return Err(reject());
    }
    let page = name(doc, dict, "Type", "Page")? || name(doc, dict, "Type", "Pages")?;
    let form = stream && name(doc, dict, "Subtype", "Form")?;
    if page || form {
        if let Some(group) = dict.get("Group") {
            let group = doc.ctx().resolve_owned(group)?;
            if let Some(group) = group.as_dict() {
                if name(doc, group, "S", "Transparency")? {
                    return Err(reject());
                }
            }
        }
        if let Some(resources) = dict.get("Resources") {
            let resources = doc.ctx().resolve_owned(resources)?;
            if let Some(resources) = resources.as_dict() {
                if let Some(states) = resources.get("ExtGState") {
                    let states = doc.ctx().resolve_owned(states)?;
                    if matches!(states, PdfObject::PdfNull) {
                        return Ok(());
                    }
                    let states = states.as_dict().ok_or_else(|| {
                        PdfError::PageResourceError(
                            "ExtGState resource map must be dictionary".into(),
                        )
                    })?;
                    if states.into_iter().len() > doc.ctx().limits().max_objects {
                        return Err(PdfError::ResourceLimit(
                            "ExtGState feature map entries".into(),
                        ));
                    }
                    for (_, state) in states {
                        let state = doc.ctx().resolve_owned(state)?;
                        if matches!(state, PdfObject::PdfNull) {
                            continue;
                        }
                        let state = state.as_dict().ok_or_else(|| {
                            PdfError::PageResourceError(
                                "ExtGState resource must be dictionary".into(),
                            )
                        })?;
                        if transparent_state(state) {
                            return Err(reject());
                        }
                    }
                }
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        objects::{pdf_name::PdfName, pdf_number::PdfNumber},
        page::page_builder::PageBuilder,
        parser::syntax::ParseMode,
    };

    #[test]
    fn catalog_upgrade_satisfies_transparency_minimum_without_rewriting_source_header() {
        let doc = PdfDocument::new_with_version(PdfVersion::V1_3).unwrap();
        let mut state = PdfDict::default();
        state.insert("ca".into(), PdfObject::PdfNumber(PdfNumber::new(0.5)));
        let mut states = PdfDict::default();
        states.insert("G".into(), PdfObject::PdfDict(state));
        let mut resources = PdfDict::default();
        resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
        doc.add_page_with_builder(0, PageBuilder::new().resources(resources))
            .unwrap();
        assert!(doc.save_to_bytes().is_err());
        let root = doc
            .ctx()
            .trailer()
            .get("Root")
            .unwrap()
            .as_object_id()
            .unwrap();
        let mut catalog = doc.resolve_object(root).unwrap().into_dict().unwrap();
        catalog.insert(
            "Version".into(),
            PdfObject::PdfName(PdfName::new("1.4".into())),
        );
        doc.update_object(root, PdfObject::PdfDict(catalog))
            .unwrap();
        let bytes = doc.save_to_bytes().unwrap();
        assert!(bytes.starts_with(b"%PDF-1.3"));
        let reopened = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        assert_eq!(reopened.effective_version().unwrap(), PdfVersion::V1_4);
    }
}
