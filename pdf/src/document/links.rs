//! Limited page link annotations. Unknown annotation types are preserved.

use std::io::{Read, Seek};

use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_string::PdfString,
    },
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinkTarget {
    Page(u32),
    Named(String),
    Uri(String),
}

#[derive(Debug, Clone, PartialEq)]
pub struct PageLink {
    pub rect: [f32; 4],
    pub target: LinkTarget,
}

fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}

fn rectangle(rect: [f32; 4]) -> PdfResult<PdfArray> {
    if rect.iter().any(|v| !v.is_finite()) || rect[0] >= rect[2] || rect[1] >= rect[3] {
        return Err(PdfError::DocumentError(
            "link rectangle must have finite positive area".into(),
        ));
    }
    let mut array = PdfArray::default();
    for number in rect {
        array.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(number)))?;
    }
    Ok(array)
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Return supported `/Link` annotations on a page. Other annotation types
    /// are skipped; malformed `/Link` entries are not silently skipped.
    pub fn page_links(&self, page_index: u32) -> PdfResult<Vec<PageLink>> {
        let page = self.page_location(page_index)?;
        let Some(value) = page.page_dict.get("Annots") else {
            return Ok(Vec::new());
        };
        let resolved = self.ctx().resolve(value)?;
        let array = resolved
            .as_array()
            .ok_or_else(|| PdfError::DocumentError("Annots must be an array".into()))?;
        if array.len() > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("annotation count".into()));
        }
        let pages = self.collect_page_entries()?;
        let mut links = Vec::new();
        for value in array {
            let object = self.ctx().resolve(value)?;
            let dict = object
                .as_dict()
                .ok_or_else(|| PdfError::DocumentError("annotation must be a dictionary".into()))?;
            let subtype = dict
                .get("Subtype")
                .ok_or_else(|| PdfError::DocumentError("annotation Subtype missing".into()))?;
            let subtype = self.ctx().resolve(subtype)?;
            if subtype.as_name().map(PdfName::name) != Some("Link") {
                continue;
            }
            if let Some(owner) = dict.get("P") {
                if owner.as_reference().map(PdfReference::id) != Some(page.page_id) {
                    return Err(PdfError::DocumentError(
                        "link P does not match its page".into(),
                    ));
                }
            }
            if dict.get("Dest").is_some() && dict.get("A").is_some() {
                return Err(PdfError::DocumentError("link has both Dest and A".into()));
            }
            if dict.get("AA").is_some() {
                return Err(PdfError::UnsupportedFeature(
                    "link additional actions are not supported".into(),
                ));
            }
            let rect_obj = dict
                .get("Rect")
                .ok_or_else(|| PdfError::DocumentError("link Rect missing".into()))?;
            let rect =
                crate::geom::rectangle::Rectangle::try_from_with_context(rect_obj, self.ctx())?;
            let rect = [rect.llx(), rect.lly(), rect.urx(), rect.ury()];
            if rect[0] >= rect[2] || rect[1] >= rect[3] {
                return Err(PdfError::DocumentError("empty link Rect".into()));
            }
            let target = if let Some(dest) = dict.get("Dest") {
                let dest = self.ctx().resolve(dest)?;
                if let Some(named) = dest.as_string() {
                    let value = named.text_content_with_limits(&self.ctx().limits())?;
                    if !self
                        .named_destinations()?
                        .iter()
                        .any(|(key, _)| key == &value)
                    {
                        return Err(PdfError::DocumentError(
                            "named link destination is missing".into(),
                        ));
                    }
                    links.push(PageLink {
                        rect,
                        target: LinkTarget::Named(value),
                    });
                    continue;
                }
                let dest = dest.as_array().ok_or_else(|| {
                    PdfError::UnsupportedFeature("link Dest is not a page array".into())
                })?;
                if dest.len() != 2
                    || self
                        .ctx()
                        .resolve(dest.get(1).unwrap())?
                        .as_name()
                        .map(PdfName::name)
                        != Some("Fit")
                {
                    return Err(PdfError::UnsupportedFeature(
                        "only /Fit link destinations are supported".into(),
                    ));
                }
                let id = dest
                    .get(0)
                    .and_then(PdfObject::as_reference)
                    .map(PdfReference::id)
                    .ok_or_else(|| {
                        PdfError::UnsupportedFeature("link page target must be indirect".into())
                    })?;
                let index = pages
                    .iter()
                    .position(|page| page.page_id == id)
                    .ok_or_else(|| PdfError::DocumentError("link target page is missing".into()))?;
                LinkTarget::Page(
                    u32::try_from(index)
                        .map_err(|_| PdfError::ResourceLimit("page index".into()))?,
                )
            } else {
                let action = dict.get("A").ok_or_else(|| {
                    PdfError::UnsupportedFeature("link has no supported target".into())
                })?;
                let action = self.ctx().resolve(action)?;
                let action = action.as_dict().ok_or_else(|| {
                    PdfError::DocumentError("link action must be a dictionary".into())
                })?;
                let kind = action
                    .get("S")
                    .and_then(PdfObject::as_name)
                    .map(PdfName::name);
                if kind != Some("URI") {
                    return Err(PdfError::UnsupportedFeature(
                        "only URI link actions are supported".into(),
                    ));
                }
                let uri = action
                    .get("URI")
                    .and_then(PdfObject::as_string)
                    .ok_or_else(|| PdfError::DocumentError("URI must be a string".into()))?;
                let bytes = uri.bytes();
                if bytes.len() > self.ctx().limits().max_token_bytes
                    || !bytes.is_ascii()
                    || bytes.iter().any(|&b| b < 0x21 || b == 0x7f)
                {
                    return Err(PdfError::UnsupportedFeature(
                        "only printable ASCII URI links are supported".into(),
                    ));
                }
                LinkTarget::Uri(
                    String::from_utf8(bytes.to_vec())
                        .map_err(|_| PdfError::DocumentError("invalid URI".into()))?,
                )
            };
            links.push(PageLink { rect, target });
        }
        Ok(links)
    }

    pub fn add_page_link(
        &self,
        page_index: u32,
        rect: [f32; 4],
        target_page: u32,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let rect = rectangle(rect)?;
        self.edit_transaction(|doc| {
            let target = doc.page_location(target_page)?;
            let mut dest = PdfArray::default();
            dest.try_add_obj(reference(target.page_id))?;
            dest.try_add_obj(PdfObject::PdfName(PdfName::new("Fit".into())))?;
            doc.add_link_inner(page_index, rect, "Dest", PdfObject::PdfArray(dest))
        })
    }

    pub fn add_named_link(
        &self,
        page_index: u32,
        rect: [f32; 4],
        destination: &str,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let rect = rectangle(rect)?;
        let encoded = PdfString::from_text_with_limits(destination, &self.ctx().limits())?;
        self.edit_transaction(|doc| {
            if !doc
                .named_destinations()?
                .iter()
                .any(|(name, _)| name == destination)
            {
                return Err(PdfError::DocumentError(
                    "named destination is missing".into(),
                ));
            }
            doc.add_link_inner(page_index, rect, "Dest", PdfObject::PdfString(encoded))
        })
    }

    pub fn add_uri_link(&self, page_index: u32, rect: [f32; 4], uri: &str) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let rect = rectangle(rect)?;
        if uri.is_empty()
            || uri.len() > self.ctx().limits().max_token_bytes
            || !uri.is_ascii()
            || uri.bytes().any(|b| b < 0x21 || b == 0x7f)
            || !(uri.starts_with("https://")
                || uri.starts_with("http://")
                || uri.starts_with("mailto:"))
        {
            return Err(PdfError::UnsupportedFeature(
                "URI must be printable ASCII http(s) or mailto".into(),
            ));
        }
        let mut action = PdfDict::default();
        action.try_insert("S".into(), PdfObject::PdfName(PdfName::new("URI".into())))?;
        action.try_insert(
            "URI".into(),
            PdfObject::PdfString(PdfString::new(uri.as_bytes().to_vec(), false)),
        )?;
        self.edit_transaction(|doc| {
            doc.add_link_inner(page_index, rect, "A", PdfObject::PdfDict(action))
        })
    }

    fn add_link_inner(
        &self,
        page_index: u32,
        rect: PdfArray,
        target_key: &str,
        target: PdfObject,
    ) -> PdfResult<()> {
        let page = self.page_location(page_index)?;
        let mut array = match page.page_dict.get("Annots") {
            Some(value) => self
                .ctx()
                .resolve(value)?
                .into_array()
                .ok_or_else(|| PdfError::DocumentError("Annots must be an array".into()))?,
            None => PdfArray::default(),
        };
        if array.len() >= self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("annotation count".into()));
        }
        let mut annotation = PdfDict::default();
        annotation.try_insert(
            "Type".into(),
            PdfObject::PdfName(PdfName::new("Annot".into())),
        )?;
        annotation.try_insert(
            "Subtype".into(),
            PdfObject::PdfName(PdfName::new("Link".into())),
        )?;
        annotation.try_insert("Rect".into(), PdfObject::PdfArray(rect))?;
        annotation.try_insert("P".into(), reference(page.page_id))?;
        annotation.try_insert(target_key.into(), target)?;
        let mut border = PdfArray::default();
        for _ in 0..3 {
            border.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(0.0)))?;
        }
        annotation.try_insert("Border".into(), PdfObject::PdfArray(border))?;
        let id = self.try_insert_object(PdfObject::PdfDict(annotation))?;
        array.try_add_obj(reference(id))?;
        let mut page_dict = page.page_dict;
        page_dict.try_insert("Annots".into(), PdfObject::PdfArray(array))?;
        self.update_object(page.page_id, PdfObject::PdfDict(page_dict))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn links_roundtrip_and_invalid_target_rolls_back() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        let rect = [10.0, 20.0, 100.0, 40.0];
        doc.add_page_link(0, rect, 1).unwrap();
        doc.add_uri_link(0, rect, "https://example.org/a").unwrap();
        doc.set_named_destination("next", 1).unwrap();
        doc.add_named_link(0, rect, "next").unwrap();
        let expected = vec![
            PageLink {
                rect,
                target: LinkTarget::Page(1),
            },
            PageLink {
                rect,
                target: LinkTarget::Uri("https://example.org/a".into()),
            },
            PageLink {
                rect,
                target: LinkTarget::Named("next".into()),
            },
        ];
        assert_eq!(doc.page_links(0).unwrap(), expected);
        assert!(doc.remove_named_destination("next").is_err());
        let revision = doc.commit_revision();
        assert!(doc.add_page_link(0, rect, 2).is_err());
        assert!(doc.add_uri_link(0, rect, "javascript:alert(1)").is_err());
        assert_eq!(doc.commit_revision(), revision);
        assert_eq!(doc.page_links(0).unwrap(), expected);
        let reopened = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_links(0).unwrap(), expected);
    }
}
