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

#[derive(Debug, Clone, PartialEq)]
pub enum LinkTarget {
    Page(u32),
    /// Full view; page identity survives page reordering.
    Destination(super::destination::Destination),
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
    /// Stable identities of the Link entries, in page order. Direct annotations
    /// remain readable through page_links but cannot be edited by address.
    pub fn page_link_handles(
        &self,
        page_index: u32,
    ) -> PdfResult<Vec<super::identity::ObjectHandle>> {
        self.page_links(page_index)?;
        let page = self.page_location(page_index)?;
        let Some(annots) = page.page_dict.get("Annots") else {
            return Ok(Vec::new());
        };
        let annots = self
            .ctx()
            .resolve(annots)?
            .into_array()
            .ok_or_else(|| PdfError::DocumentError("Annots must be an array".into()))?;
        let mut result = Vec::new();
        for object in &annots {
            let dict =
                self.ctx().resolve(object)?.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("annotation must be a dictionary".into())
                })?;
            if dict
                .get("Subtype")
                .and_then(PdfObject::as_name)
                .map(PdfName::name)
                == Some("Link")
            {
                let id = object.as_reference().map(PdfReference::id).ok_or_else(|| {
                    PdfError::UnsupportedFeature("direct link has no stable handle".into())
                })?;
                result.push(self.object_handle(id)?);
            }
        }
        Ok(result)
    }

    fn encoded_link_target(&self, target: &LinkTarget) -> PdfResult<(&'static str, PdfObject)> {
        match target {
            LinkTarget::Page(page) => Ok((
                "Dest",
                self.encode_destination(
                    &self.destination(*page, super::destination::DestinationView::Fit)?,
                )?,
            )),
            LinkTarget::Destination(dest) => Ok(("Dest", self.encode_destination(dest)?)),
            LinkTarget::Named(name) => {
                if !self
                    .named_destinations()?
                    .iter()
                    .any(|(key, _)| key == name)
                {
                    return Err(PdfError::DocumentError("named destination missing".into()));
                }
                Ok((
                    "Dest",
                    PdfObject::PdfString(PdfString::from_text_with_limits(
                        name,
                        &self.ctx().limits(),
                    )?),
                ))
            }
            LinkTarget::Uri(uri) => {
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
                Ok(("A", PdfObject::PdfDict(action)))
            }
        }
    }

    pub fn add_link(
        &self,
        page_index: u32,
        link: &PageLink,
    ) -> PdfResult<super::identity::ObjectHandle> {
        self.ctx().require_annotation_permission()?;
        let rect = rectangle(link.rect)?;
        self.edit_transaction(|doc| {
            let (key, target) = doc.encoded_link_target(&link.target)?;
            doc.add_link_inner(page_index, rect, key, target)?;
            let page = doc.page_location(page_index)?;
            let array = doc
                .ctx()
                .resolve(page.page_dict.get("Annots").unwrap())?
                .into_array()
                .unwrap();
            let id = array
                .get(array.len() - 1)
                .and_then(PdfObject::as_reference)
                .unwrap()
                .id();
            doc.object_handle(id)
        })
    }

    /// Replace only rectangle/target. Unknown keys, flags, border and AP survive.
    pub fn update_page_link(
        &self,
        page_index: u32,
        handle: &super::identity::ObjectHandle,
        link: &PageLink,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let id = self.validate_handle(handle)?;
        let rect = rectangle(link.rect)?;
        self.edit_transaction(|doc| {
            doc.check_link_edit_owner(page_index, id)?;
            if !doc
                .page_link_handles(page_index)?
                .iter()
                .any(|h| h == handle)
            {
                return Err(PdfError::DocumentError(
                    "link is not owned by this page".into(),
                ));
            }
            let (key, target) = doc.encoded_link_target(&link.target)?;
            let mut dict = doc
                .resolve_object(id)?
                .into_dict()
                .ok_or_else(|| PdfError::DocumentError("link must be a dictionary".into()))?;
            dict.remove("A");
            dict.remove("Dest");
            dict.try_insert("Rect".into(), PdfObject::PdfArray(rect))?;
            dict.try_insert(key.into(), target)?;
            doc.update_object(id, PdfObject::PdfDict(dict))
        })
    }

    /// Detach the Link from its page; do not delete an object that another
    /// object may still reference. This is not secure erasure.
    pub fn remove_page_link(
        &self,
        page_index: u32,
        handle: &super::identity::ObjectHandle,
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        let id = self.validate_handle(handle)?;
        self.edit_transaction(|doc| {
            doc.check_link_edit_owner(page_index, id)?;
            if !doc
                .page_link_handles(page_index)?
                .iter()
                .any(|h| h == handle)
            {
                return Err(PdfError::DocumentError(
                    "link is not owned by this page".into(),
                ));
            }
            let page = doc.page_location(page_index)?;
            let old = doc
                .ctx()
                .resolve(page.page_dict.get("Annots").unwrap())?
                .into_array()
                .unwrap();
            let mut annots = PdfArray::default();
            for object in &old {
                if object.as_reference().map(PdfReference::id) != Some(id) {
                    annots.try_add_obj(object.clone())?;
                }
            }
            let mut dict = page.page_dict;
            dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
            doc.update_object(page.page_id, PdfObject::PdfDict(dict))
        })
    }

    pub(super) fn check_link_edit_owner(&self, page_index: u32, id: ObjectId) -> PdfResult<()> {
        let page = self.page_location(page_index)?;
        let dict = self
            .resolve_object(id)?
            .into_dict()
            .ok_or_else(|| PdfError::DocumentError("link must be a dictionary".into()))?;
        if dict
            .get("P")
            .is_some_and(|p| p.as_reference().map(PdfReference::id) != Some(page.page_id))
        {
            return Err(PdfError::DocumentError("link P does not match page".into()));
        }
        let flags = dict
            .get("F")
            .map(|f| {
                self.ctx()
                    .resolve(f)?
                    .as_number()
                    .ok_or_else(|| PdfError::DocumentError("annotation F must be integer".into()))?
                    .as_u32_checked()
            })
            .transpose()?
            .unwrap_or(0);
        if flags & (64 | 128 | 512) != 0 {
            return Err(PdfError::UnsupportedFeature(
                "read-only or locked link".into(),
            ));
        }
        let mut owners = 0usize;
        for page in self.collect_page_entries()? {
            if let Some(annots) = page.page_dict.get("Annots") {
                let annots = self
                    .ctx()
                    .resolve(annots)?
                    .into_array()
                    .ok_or_else(|| PdfError::DocumentError("Annots must be an array".into()))?;
                if annots.len() > self.ctx().limits().max_objects {
                    return Err(PdfError::ResourceLimit("annotation count".into()));
                }
                for value in &annots {
                    if value.as_reference().map(PdfReference::id) == Some(id) {
                        owners += 1;
                    }
                }
            }
        }
        if owners != 1 {
            return Err(PdfError::UnsupportedFeature(
                "link has shared or missing page ownership".into(),
            ));
        }
        Ok(())
    }

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
                let destination = self.decode_destination(&dest)?;
                if destination.view == super::destination::DestinationView::Fit {
                    LinkTarget::Page(self.destination_page_index(&destination)?)
                } else {
                    LinkTarget::Destination(destination)
                }
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
                if action.get("Next").is_some() {
                    return Err(PdfError::UnsupportedFeature("chained link actions".into()));
                }
                if kind == Some("GoTo") {
                    let target = action
                        .get("D")
                        .ok_or_else(|| PdfError::DocumentError("GoTo D missing".into()))?;
                    let target = self.ctx().resolve(target)?;
                    let target = if let Some(s) = target.as_string() {
                        let name = s.text_content_with_limits(&self.ctx().limits())?;
                        if !self.named_destinations()?.iter().any(|(n, _)| n == &name) {
                            return Err(PdfError::DocumentError(
                                "named GoTo destination missing".into(),
                            ));
                        }
                        LinkTarget::Named(name)
                    } else {
                        let dest = self.decode_destination(&target)?;
                        if dest.view == super::destination::DestinationView::Fit {
                            LinkTarget::Page(self.destination_page_index(&dest)?)
                        } else {
                            LinkTarget::Destination(dest)
                        }
                    };
                    links.push(PageLink { rect, target });
                    continue;
                }
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
    fn locked_and_shared_links_reject_edits_and_named_rename() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        doc.add_page(1).unwrap();
        doc.set_named_destination("target", 1).unwrap();
        let link = PageLink {
            rect: [10., 20., 30., 40.],
            target: LinkTarget::Named("target".into()),
        };
        let handle = doc.add_link(0, &link).unwrap();
        let mut dict = doc.resolve_handle(&handle).unwrap().into_dict().unwrap();
        dict.try_insert("F".into(), PdfObject::PdfNumber(PdfNumber::new(128.)))
            .unwrap();
        doc.update_handle(&handle, PdfObject::PdfDict(dict.clone()))
            .unwrap();
        let revision = doc.commit_revision();
        assert!(doc.update_page_link(0, &handle, &link).is_err());
        assert!(doc.remove_page_link(0, &handle).is_err());
        assert!(doc.rename_named_destination("target", "renamed").is_err());
        assert_eq!(doc.commit_revision(), revision);
        assert_eq!(doc.named_destinations().unwrap()[0].0, "target");
        dict.remove("F");
        dict.remove("P");
        doc.update_handle(&handle, PdfObject::PdfDict(dict))
            .unwrap();
        let page = doc.page_location(1).unwrap();
        let mut dict = page.page_dict;
        let mut annots = PdfArray::default();
        annots.try_add_obj(reference(handle.object_id())).unwrap();
        dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))
            .unwrap();
        doc.update_object(page.page_id, PdfObject::PdfDict(dict))
            .unwrap();
        let revision = doc.commit_revision();
        assert!(doc.update_page_link(0, &handle, &link).is_err());
        assert!(doc.remove_page_link(0, &handle).is_err());
        assert!(doc.rename_named_destination("target", "renamed").is_err());
        assert_eq!(doc.commit_revision(), revision);
    }

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
