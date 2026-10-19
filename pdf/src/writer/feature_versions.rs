use crate::{
    document::{PdfDocument, save_options::PdfVersion},
    error::{PdfError, PdfResult},
    objects::{PdfObject, pdf_dict::PdfDict},
};
use std::io::{Read, Seek};

struct FeatureDocument<'a, R: Read + Seek> {
    doc: &'a PdfDocument<R>,
    budget: Option<&'a crate::budget::OperationBudget>,
}
impl<R: Read + Seek> std::ops::Deref for FeatureDocument<'_, R> {
    type Target = PdfDocument<R>;
    fn deref(&self) -> &Self::Target {
        self.doc
    }
}
struct FeatureObject {
    object: PdfObject,
    _lease: Option<crate::budget::MemoryLease>,
}
impl std::ops::Deref for FeatureObject {
    type Target = PdfObject;
    fn deref(&self) -> &Self::Target {
        &self.object
    }
}
impl<R: Read + Seek> FeatureDocument<'_, R> {
    fn resolve(&self, value: &PdfObject) -> PdfResult<FeatureObject> {
        if let Some(budget) = self.budget {
            let (object, lease) = self.ctx().resolve_owned_with_budget(value, budget)?;
            Ok(FeatureObject {
                object,
                _lease: Some(lease),
            })
        } else {
            Ok(FeatureObject {
                object: self.ctx().resolve_owned(value)?,
                _lease: None,
            })
        }
    }
}

/// Targeted known feature checks, not a full feature/conformance validator.
#[cfg(test)]
pub(super) fn validate_feature_versions<R: Read + Seek>(
    doc: &PdfDocument<R>,
    root: &PdfObject,
    version: PdfVersion,
) -> PdfResult<()> {
    validate_feature_versions_with_budget(doc, root, version, None)
}

pub(super) fn validate_feature_versions_with_budget<R: Read + Seek>(
    doc: &PdfDocument<R>,
    root: &PdfObject,
    version: PdfVersion,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<()> {
    if version >= PdfVersion::V1_7 {
        return Ok(());
    }
    let doc = &FeatureDocument { doc, budget };
    let limits = doc.ctx().limits();
    let mut _pending_lease = None;
    let mut pending = Vec::new();
    push(
        &mut pending,
        root,
        limits.max_objects,
        &mut _pending_lease,
        budget,
    )?;
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
            let optional_group = name(doc, dict, "Type", "OCG")?;
            let optional_membership = name(doc, dict, "Type", "OCMD")?;
            if version < PdfVersion::V1_5
                && (optional_group
                    || optional_membership
                    || dict.contains_key("OCProperties")
                    || dict.contains_key("OC") && (stream || name(doc, dict, "Type", "Annot")?))
            {
                return Err(PdfError::UnsupportedFeature("optional content requires PDF 1.5 or later; select an explicit version upgrade".into()));
            }
            if version < PdfVersion::V1_6 && optional_membership && dict.contains_key("VE") {
                return Err(PdfError::UnsupportedFeature("optional-content VE requires PDF 1.6 or later; select an explicit version upgrade".into()));
            }
            if !stream
                && name(doc, dict, "Type", "Catalog")?
                && dict.contains_key("AcroForm")
                && version < PdfVersion::V1_2
            {
                return Err(PdfError::UnsupportedFeature(
                    "AcroForm requires PDF 1.2 or later; select an explicit version upgrade".into(),
                ));
            }
            if !stream
                && (dict.contains_key("FT") || dict.contains_key("T") && dict.contains_key("Ff"))
            {
                let flags = match dict.get("Ff") {
                    Some(value) => doc
                        .resolve(value)?
                        .as_number()
                        .ok_or_else(|| PdfError::DocumentError("field flags integer".into()))?
                        .as_u32_checked()?,
                    None => 0,
                };
                let minimum = if flags & ((1 << 25) | (1 << 26)) != 0 {
                    PdfVersion::V1_5
                } else if flags & ((1 << 20) | (1 << 21) | (1 << 22) | (1 << 23)) != 0
                    || name(doc, dict, "FT", "Btn")? && dict.contains_key("Opt")
                    || name(doc, dict, "FT", "Ch")? && dict.contains_key("I")
                {
                    PdfVersion::V1_4
                } else if name(doc, dict, "FT", "Sig")? {
                    PdfVersion::V1_3
                } else {
                    PdfVersion::V1_2
                };
                if version < minimum {
                    return Err(PdfError::UnsupportedFeature(format!(
                        "form field requires PDF {} or later; select an explicit version upgrade",
                        minimum.as_str()
                    )));
                }
            }
            if !stream
                && (name(doc, dict, "Type", "Annot")?
                    || dict.contains_key("Rect") && dict.contains_key("Subtype"))
            {
                let flags = match dict.get("F") {
                    Some(v) => doc
                        .resolve(v)?
                        .as_number()
                        .ok_or_else(|| {
                            PdfError::DocumentError("annotation flags must be integer".into())
                        })?
                        .as_u32_checked()?,
                    None => 0,
                };
                let minimum = if flags & 512 != 0 {
                    PdfVersion::V1_7
                } else if dict.contains_key("RT") {
                    PdfVersion::V1_6
                } else if flags & 256 != 0
                    || dict.contains_key("IRT")
                    || name(doc, dict, "Subtype", "Polygon")?
                    || name(doc, dict, "Subtype", "PolyLine")?
                {
                    PdfVersion::V1_5
                } else if dict.contains_key("CA") {
                    PdfVersion::V1_4
                } else {
                    PdfVersion::V1_0
                };
                if version < minimum {
                    return Err(PdfError::UnsupportedFeature(format!(
                        "annotation requires PDF {} or later; select an explicit version upgrade",
                        minimum.as_str()
                    )));
                }
            }
            if version < PdfVersion::V1_3
                && let Some(value) = dict.get("ColorSpace")
            {
                let value = doc.resolve(value)?;
                if let PdfObject::PdfDict(resources) = &*value {
                    for (_, space) in resources {
                        check_color_space_version(doc, space, version)?;
                    }
                } else {
                    check_color_space_version(doc, &value, version)?;
                }
            }
            if version < PdfVersion::V1_3 {
                if let Some(value) = dict.get("ShadingType") {
                    let value = doc.resolve(value)?;
                    if matches!(
                        value
                            .as_number()
                            .and_then(|number| number.as_u64_checked().ok()),
                        Some(1..=7)
                    ) {
                        return Err(PdfError::UnsupportedFeature(
                            "shading requires PDF 1.3 or later; select an explicit version upgrade"
                                .into(),
                        ));
                    }
                }
                if let Some(value) = dict.get("FunctionType") {
                    let value = doc.resolve(value)?;
                    let minimum = match value
                        .as_number()
                        .and_then(|number| number.as_u64_checked().ok())
                    {
                        Some(0) => Some(PdfVersion::V1_2),
                        Some(2..=4) => Some(PdfVersion::V1_3),
                        _ => None,
                    };
                    if let Some(minimum) = minimum
                        && version < minimum
                    {
                        return Err(PdfError::UnsupportedFeature(format!(
                            "PDF function requires PDF {} or later; select an explicit version upgrade",
                            minimum.as_str()
                        )));
                    }
                }
            }
            if version < PdfVersion::V1_6
                && !stream
                && name(doc, dict, "Type", "Page")?
                && let Some(value) = dict.get("UserUnit")
                && !matches!(&*doc.resolve(value)?, PdfObject::PdfNull)
            {
                return Err(PdfError::UnsupportedFeature(
                    "page UserUnit requires PDF 1.6 or later; select an explicit version upgrade"
                        .into(),
                ));
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
                if version < PdfVersion::V1_5
                    && name(doc, dict, "Subtype", "Image")?
                    && let Some(bits) = dict.get("BitsPerComponent")
                {
                    let bits = doc.resolve(bits)?;
                    if bits
                        .as_number()
                        .is_some_and(|number| number.as_u64_checked().ok() == Some(16))
                    {
                        return Err(PdfError::UnsupportedFeature(
                                "16-bit image requires PDF 1.5 or later; select an explicit version upgrade".into(),
                            ));
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
                push(
                    &mut pending,
                    value,
                    limits.max_objects,
                    &mut _pending_lease,
                    budget,
                )?;
            }
        } else if let PdfObject::PdfArray(array) = object {
            for value in array {
                push(
                    &mut pending,
                    value,
                    limits.max_objects,
                    &mut _pending_lease,
                    budget,
                )?;
            }
        }
    }
    Ok(())
}

fn check_color_space_version<R: Read + Seek>(
    doc: &FeatureDocument<'_, R>,
    value: &PdfObject,
    version: PdfVersion,
) -> PdfResult<()> {
    let mut value = doc.resolve(value)?;
    let mut indexed_base = false;
    let limits = doc.ctx().limits();
    for depth in 0..limits.max_depth {
        if depth >= limits.max_objects {
            return Err(PdfError::ResourceLimit("color feature nodes".into()));
        }
        let array = value.as_array();
        let family_object = if let Some(array) = array {
            let Some(first) = array.get(0) else {
                return Ok(());
            };
            Some(doc.resolve(first)?)
        } else {
            None
        };
        let family = family_object.as_deref().unwrap_or(&value);
        let Some(family) = family.as_name() else {
            return Ok(());
        };
        let minimum = match family.name() {
            "Separation" if indexed_base => Some(PdfVersion::V1_3),
            "Pattern" | "Separation" => Some(PdfVersion::V1_2),
            "ICCBased" | "DeviceN" => Some(PdfVersion::V1_3),
            _ => None,
        };
        if let Some(minimum) = minimum
            && version < minimum
        {
            return Err(PdfError::UnsupportedFeature(format!(
                "{} color space requires PDF {} or later; select an explicit version upgrade",
                family.name(),
                minimum.as_str()
            )));
        }
        let base_index = match family.name() {
            "Indexed" | "I" | "Pattern" => 1,
            "Separation" | "DeviceN" => 2,
            _ => return Ok(()),
        };
        let Some(base) = array.and_then(|array| array.get(base_index)) else {
            return Ok(());
        };
        let next = doc.resolve(base)?;
        indexed_base = matches!(family.name(), "Indexed" | "I");
        drop(family_object);
        value = next;
    }
    Err(PdfError::ResourceLimit("color feature depth".into()))
}

fn check_filters<R: Read + Seek>(
    doc: &FeatureDocument<'_, R>,
    dict: &PdfDict,
    version: PdfVersion,
) -> PdfResult<()> {
    if version < PdfVersion::V1_2 {
        for key in ["F", "FFilter", "FDecodeParms"] {
            if let Some(value) = dict.get(key)
                && !matches!(&*doc.resolve(value)?, PdfObject::PdfNull)
            {
                return Err(PdfError::UnsupportedFeature(
                        "external stream settings require PDF 1.2 or later; select an explicit version upgrade".into(),
                    ));
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
        let filter = doc.resolve(filter)?;
        match &*filter {
            PdfObject::PdfNull => {}
            PdfObject::PdfArray(array) => {
                if array.len() > doc.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("filter feature entries".into()));
                }
                for value in array {
                    let value = doc.resolve(value)?;
                    check(&value)?;
                }
            }
            value => check(value)?,
        }
    }
    Ok(())
}

fn push<'a>(
    pending: &mut Vec<&'a PdfObject>,
    value: &'a PdfObject,
    limit: usize,
    lease: &mut Option<crate::budget::MemoryLease>,
    budget: Option<&crate::budget::OperationBudget>,
) -> PdfResult<()> {
    if pending.len() >= limit {
        return Err(PdfError::ResourceLimit(
            "feature validation pending nodes".into(),
        ));
    }
    super::reserve_validation_slots(pending, 1, lease, budget)?;
    pending.push(value);
    Ok(())
}

fn name<R: Read + Seek>(
    doc: &FeatureDocument<'_, R>,
    dict: &PdfDict,
    key: &str,
    expected: &str,
) -> PdfResult<bool> {
    match dict.get(key) {
        Some(value) => Ok(doc
            .resolve(value)?
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
    doc: &FeatureDocument<'_, R>,
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
            let group = doc.resolve(group)?;
            if let Some(group) = group.as_dict()
                && name(doc, group, "S", "Transparency")?
            {
                return Err(reject());
            }
        }
        if let Some(resources) = dict.get("Resources") {
            let resources = doc.resolve(resources)?;
            if let Some(resources) = resources.as_dict()
                && let Some(states) = resources.get("ExtGState")
            {
                let states = doc.resolve(states)?;
                if matches!(&*states, PdfObject::PdfNull) {
                    return Ok(());
                }
                let states = states.as_dict().ok_or_else(|| {
                    PdfError::PageResourceError("ExtGState resource map must be dictionary".into())
                })?;
                if states.into_iter().len() > doc.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit(
                        "ExtGState feature map entries".into(),
                    ));
                }
                for (_, state) in states {
                    let state = doc.resolve(state)?;
                    if matches!(&*state, PdfObject::PdfNull) {
                        continue;
                    }
                    let state = state.as_dict().ok_or_else(|| {
                        PdfError::PageResourceError("ExtGState resource must be dictionary".into())
                    })?;
                    if transparent_state(state) {
                        return Err(reject());
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
    fn feature_budget_covers_resource_copies_and_failure_cleanup() {
        use crate::{budget::OperationBudget, objects::pdf_string::PdfString};
        let doc = PdfDocument::new_with_version(PdfVersion::V1_3).unwrap();
        let revision = doc.revision();
        let mut state = PdfDict::default();
        state.insert(
            "Opaque".into(),
            PdfObject::PdfString(PdfString::new(vec![b'x'; 4096], false)),
        );
        let mut states = PdfDict::default();
        states.insert("G".into(), PdfObject::PdfDict(state.clone()));
        let mut resources = PdfDict::default();
        resources.insert("ExtGState".into(), PdfObject::PdfDict(states));
        let mut page = PdfDict::default();
        page.insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new("Page".into())),
        );
        page.insert("Resources".into(), PdfObject::PdfDict(resources));
        let root = PdfObject::PdfDict(page);
        validate_feature_versions(&doc, &root, PdfVersion::V1_3).unwrap();
        let pool = OperationBudget::new(1024 * 1024);
        validate_feature_versions_with_budget(&doc, &root, PdfVersion::V1_3, Some(&pool)).unwrap();
        assert_eq!(pool.live_bytes(), 0);
        assert!(pool.peak_bytes() > 3 * 4096);
        let small = OperationBudget::new(pool.peak_bytes() - 1);
        assert!(matches!(
            validate_feature_versions_with_budget(&doc, &root, PdfVersion::V1_3, Some(&small)),
            Err(PdfError::ResourceLimit(_))
        ));
        assert_eq!(small.live_bytes(), 0);
        // Retry uses the same pool after refusal; all temporary owners have gone.
        validate_feature_versions_with_budget(
            &doc,
            &PdfObject::PdfNull,
            PdfVersion::V1_3,
            Some(&small),
        )
        .unwrap();
        state.insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new("ExtGState".into())),
        );
        state.insert("ca".into(), PdfObject::PdfNumber(PdfNumber::new(0.5)));
        let transparent = PdfObject::PdfDict(state);
        assert!(matches!(
            validate_feature_versions_with_budget(
                &doc,
                &transparent,
                PdfVersion::V1_3,
                Some(&pool)
            ),
            Err(PdfError::UnsupportedFeature(_))
        ));
        assert_eq!(pool.live_bytes(), 0);
        let caught = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let checker = FeatureDocument {
                doc: &doc,
                budget: Some(&pool),
            };
            let _owner = checker.resolve(&root).unwrap();
            assert!(pool.live_bytes() > 0);
            panic!("feature owner unwind");
        }));
        assert!(caught.is_err());
        assert_eq!(pool.live_bytes(), 0);
        assert_eq!(doc.revision(), revision);
    }

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
