use std::io::{Read, Seek};

use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, pdf_dict::PdfDict, pdf_name::PdfName, pdf_reference::PdfReference,
        pdf_stream::PdfStream, pdf_string::PdfString,
    },
};

/// Standard Info text fields; dates and Trapped use separate PDF value rules.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InfoTextField {
    Title,
    Author,
    Subject,
    Keywords,
    Creator,
    Producer,
}

impl InfoTextField {
    pub fn key(self) -> &'static str {
        match self {
            Self::Title => "Title",
            Self::Author => "Author",
            Self::Subject => "Subject",
            Self::Keywords => "Keywords",
            Self::Creator => "Creator",
            Self::Producer => "Producer",
        }
    }
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Atomically replace both Info and the document Metadata packet.
    /// None removes that side. The caller owns semantic consistency and valid XML.
    /// No automatic field mapping, unknown-XMP removal, or XML entity expansion.
    pub fn set_info_and_metadata(
        &self,
        info: Option<PdfDict>,
        packet: Option<Vec<u8>>,
    ) -> PdfResult<()> {
        self.edit_transaction(|doc| {
            match info {
                Some(dict) => doc.set_info_dict(dict)?,
                None => doc.remove_info_dict()?,
            }
            doc.set_metadata_packet(packet)
        })
    }

    /// Decoded document-level Metadata packet. Does not parse or validate XML/XMP.
    pub fn metadata_packet(&self) -> PdfResult<Option<Vec<u8>>> {
        let root = self.ctx().get_root_owned()?;
        let Some(value) = root.get("Metadata") else {
            return Ok(None);
        };
        let value = self.ctx().resolve(value)?;
        if matches!(value, PdfObject::PdfNull) {
            return Ok(None);
        }
        let stream = value
            .as_stream()
            .ok_or_else(|| PdfError::DocumentError("Metadata must be a stream".into()))?;
        for (key, expected) in [("Type", "Metadata"), ("Subtype", "XML")] {
            let value = stream
                .dict()
                .get(key)
                .ok_or_else(|| PdfError::DocumentError("Metadata stream type missing".into()))?;
            let value = self.ctx().resolve(value)?;
            if value.as_name().map(|name| name.name()) != Some(expected) {
                return Err(PdfError::DocumentError(
                    "invalid Metadata stream type".into(),
                ));
            }
        }
        stream.decode_data(self.ctx()).map(Some)
    }

    /// Replace/unlink a raw XML/XMP packet without altering Info.
    /// The caller supplies valid XML; no XML parsing, entity expansion or XMP synchronization.
    pub fn set_metadata_packet(&self, packet: Option<Vec<u8>>) -> PdfResult<()> {
        if let Some(bytes) = &packet {
            let limits = self.ctx().limits();
            if bytes.len() > limits.max_raw_stream_bytes || bytes.len() > limits.max_decoded_bytes {
                return Err(PdfError::ResourceLimit("Metadata packet bytes".into()));
            }
            if self.effective_version()? < super::save_options::PdfVersion::V1_4 {
                return Err(PdfError::UnsupportedFeature(
                    "Metadata requires PDF 1.4 or later".into(),
                ));
            }
        }
        self.edit_transaction(|doc| {
            let root_id = doc
                .ctx()
                .trailer()
                .get("Root")
                .and_then(PdfObject::as_reference)
                .map(|reference| reference.id())
                .ok_or_else(|| {
                    PdfError::DocumentError("Root must be an indirect reference".into())
                })?;
            let mut root = doc.ctx().get_root_owned()?;
            if let Some(bytes) = packet {
                let mut dict = PdfDict::default();
                dict.try_insert(
                    "Type".into(),
                    PdfObject::PdfName(PdfName::new("Metadata".into())),
                )?;
                dict.try_insert(
                    "Subtype".into(),
                    PdfObject::PdfName(PdfName::new("XML".into())),
                )?;
                let id = doc.try_insert_object(PdfObject::PdfStream(
                    PdfStream::from_decoded_data(dict, bytes)?,
                ))?;
                root.try_insert(
                    "Metadata".into(),
                    PdfObject::PdfReference(PdfReference::from_id(id)),
                )?;
            } else if root.remove("Metadata").is_none() {
                return Ok(());
            }
            doc.update_object(root_id, PdfObject::PdfDict(root))
        })
    }

    pub fn info_text(&self, field: InfoTextField) -> PdfResult<Option<String>> {
        let Some(dict) = self.info_dict()? else {
            return Ok(None);
        };
        let Some(value) = dict.get(field.key()) else {
            return Ok(None);
        };
        let value = self.ctx().resolve(value)?;
        if matches!(value, PdfObject::PdfNull) {
            return Ok(None);
        }
        let text = value
            .as_string()
            .ok_or_else(|| PdfError::DocumentError("Info text field must be a string".into()))?;
        if text.bytes().starts_with(&[0xef, 0xbb, 0xbf])
            && self.effective_version()? < super::save_options::PdfVersion::V2_0
        {
            return Err(PdfError::UnsupportedFeature(
                "UTF-8 BOM Info text requires PDF 2.0".into(),
            ));
        }
        text.text_content_with_limits(&self.ctx().limits())
            .map(Some)
    }

    /// Set or remove one standard text field, preserving all other Info entries.
    /// Does not synchronize XMP; removal is not secure erasure of old object bytes.
    pub fn set_info_text(&self, field: InfoTextField, value: Option<&str>) -> PdfResult<()> {
        let encoded = value
            .map(|text| PdfString::from_text_with_limits(text, &self.ctx().limits()))
            .transpose()?;
        self.edit_transaction(|doc| {
            let mut dict = doc.info_dict()?.unwrap_or_default();
            if let Some(value) = encoded {
                dict.try_insert(field.key().into(), PdfObject::PdfString(value))?;
            } else if dict.remove(field.key()).is_none() {
                return Ok(());
            }
            doc.set_info_dict(dict)
        })
    }

    /// Read the latest Info dictionary, preserving unknown entries.
    pub fn info_dict(&self) -> PdfResult<Option<PdfDict>> {
        let value = self.ctx().trailer().get("Info").cloned();
        match value {
            None | Some(PdfObject::PdfNull) => Ok(None),
            Some(value) => self
                .ctx()
                .resolve(&value)?
                .into_dict()
                .map(Some)
                .ok_or_else(|| PdfError::DocumentError("Info must be a dictionary".into())),
        }
    }

    /// Replace Info using a fresh indirect object; shared source objects are untouched.
    /// Text values should be encoded with PdfString::from_text, not raw UTF-8.
    /// Does not synchronize XMP or validate PDF date syntax.
    pub fn set_info_dict(&self, info: PdfDict) -> PdfResult<()> {
        if (&info).into_iter().count() > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("Info dictionary entries".into()));
        }
        self.edit_transaction(|doc| {
            let id = doc.try_insert_object(PdfObject::PdfDict(info))?;
            let mut trailer = doc.ctx().trailer().clone();
            trailer.try_insert(
                "Info".into(),
                PdfObject::PdfReference(PdfReference::from_id(id)),
            )?;
            doc.ctx().replace_trailer(trailer)
        })
    }

    /// Unlink Info conservatively, retaining any formerly referenced object.
    pub fn remove_info_dict(&self) -> PdfResult<()> {
        self.edit_transaction(|doc| {
            let mut trailer = doc.ctx().trailer().clone();
            trailer.remove("Info");
            doc.ctx().replace_trailer(trailer)
        })
    }
}
