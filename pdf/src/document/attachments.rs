//! Document-level embedded files through a bounded, balanced name tree.

use std::io::{Read, Seek};

use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_reference::PdfReference, pdf_stream::PdfStream, pdf_string::PdfString,
    },
};

fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Address an existing embedded Filespec for FileAttachment annotations.
    /// Reading a payload never launches or opens the embedded file.
    pub fn attachment_filespec(
        &self,
        name: &str,
    ) -> PdfResult<Option<super::identity::ObjectHandle>> {
        for (key, value) in self.attachment_entries()?.2 {
            if key.text_content_with_limits(&self.ctx().limits())? == name {
                let id = value.as_reference().map(PdfReference::id).ok_or_else(|| {
                    PdfError::UnsupportedFeature(
                        "direct attachment Filespec has no stable handle".into(),
                    )
                })?;
                return self.object_handle(id).map(Some);
            }
        }
        Ok(None)
    }
    fn attachment_entries(&self) -> PdfResult<super::trees::NameTreeSnapshot> {
        let root = self.ctx().get_root_owned()?;
        let names = match root.get("Names") {
            Some(value) => self.ctx().resolve(value)?.into_dict().ok_or_else(|| {
                PdfError::DocumentError("catalog Names must be a dictionary".into())
            })?,
            None => PdfDict::default(),
        };
        let Some(tree_value) = names.get("EmbeddedFiles") else {
            return Ok((names, PdfDict::default(), Vec::new()));
        };
        let tree =
            self.ctx().resolve(tree_value)?.into_dict().ok_or_else(|| {
                PdfError::DocumentError("EmbeddedFiles must be a dictionary".into())
            })?;
        let mut entries = Vec::new();
        let mut decoded_names = std::collections::HashSet::new();
        for (key, value) in self.read_tree(tree_value, super::trees::TreeKind::Name)? {
            let super::trees::TreeKey::Name(bytes) = key else {
                unreachable!()
            };
            let key = PdfString::new(bytes, false);
            if !decoded_names.insert(key.text_content_with_limits(&self.ctx().limits())?) {
                return Err(PdfError::DocumentError(
                    "duplicate decoded attachment name".into(),
                ));
            }
            entries.push((key, value));
        }
        Ok((names, tree, entries))
    }

    /// List attachment keys from flat or nested name trees.
    pub fn attachment_names(&self) -> PdfResult<Vec<String>> {
        let (_, _, entries) = self.attachment_entries()?;
        entries
            .into_iter()
            .map(|(name, _)| name.text_content_with_limits(&self.ctx().limits()))
            .collect()
    }

    /// Extract an embedded file. This does not execute or open its payload.
    pub fn attachment_data(&self, name: &str) -> PdfResult<Option<Vec<u8>>> {
        let (_, _, entries) = self.attachment_entries()?;
        for (key, value) in entries {
            if key.text_content_with_limits(&self.ctx().limits())? != name {
                continue;
            }
            let filespec =
                self.ctx().resolve(&value)?.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("Filespec must be a dictionary".into())
                })?;
            let ef = filespec
                .get("EF")
                .ok_or_else(|| PdfError::DocumentError("Filespec EF missing".into()))?;
            let ef = self.ctx().resolve(ef)?.into_dict().ok_or_else(|| {
                PdfError::DocumentError("Filespec EF must be a dictionary".into())
            })?;
            let stream = super::embedded_file::payload_reference(&ef, false)?;
            let stream =
                self.ctx().resolve(stream)?.into_stream().ok_or_else(|| {
                    PdfError::DocumentError("embedded file must be a stream".into())
                })?;
            return stream.decode_data(self.ctx()).map(Some);
        }
        Ok(None)
    }

    /// Add an attachment with a unique name. The name tree is balanced on write.
    /// Removal unlinks data; it is not secure erasure.
    pub fn add_attachment(&self, name: &str, data: Vec<u8>) -> PdfResult<()> {
        if self.effective_version()? < super::save_options::PdfVersion::V1_4 {
            return Err(PdfError::UnsupportedFeature(
                "document attachments require PDF 1.4 or later".into(),
            ));
        }
        if name.is_empty() {
            return Err(PdfError::DocumentError("attachment name is empty".into()));
        }
        if data.len() > self.ctx().limits().max_decoded_bytes
            || data.len() > self.ctx().limits().max_raw_stream_bytes
        {
            return Err(PdfError::ResourceLimit("attachment bytes".into()));
        }
        let key = PdfString::from_text_with_limits(name, &self.ctx().limits())?;
        let fallback_name = if name.is_ascii() && name.bytes().all(|b| (0x20..0x7f).contains(&b)) {
            name
        } else {
            "attachment.bin"
        };
        self.edit_transaction(|doc| {
            let (names, tree, mut entries) = doc.attachment_entries()?;
            for (other, _) in &entries {
                if other.text_content_with_limits(&doc.ctx().limits())? == name {
                    return Err(PdfError::DocumentError(
                        "attachment name already exists".into(),
                    ));
                }
            }
            let mut stream_dict = PdfDict::default();
            stream_dict.try_insert(
                "Type".into(),
                PdfObject::PdfName(PdfName::new("EmbeddedFile".into())),
            )?;
            let stream_id = doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(stream_dict, data)?,
            ))?;
            let mut ef = PdfDict::default();
            ef.try_insert("F".into(), reference(stream_id))?;
            let mut spec = PdfDict::default();
            spec.try_insert(
                "Type".into(),
                PdfObject::PdfName(PdfName::new("Filespec".into())),
            )?;
            spec.try_insert("UF".into(), PdfObject::PdfString(key.clone()))?;
            spec.try_insert(
                "F".into(),
                PdfObject::PdfString(PdfString::new(fallback_name.as_bytes().to_vec(), false)),
            )?;
            spec.try_insert("EF".into(), PdfObject::PdfDict(ef))?;
            let spec_id = doc.try_insert_object(PdfObject::PdfDict(spec))?;
            entries.push((key, reference(spec_id)));
            doc.write_attachment_entries(names, tree, entries)
        })
    }

    pub fn remove_attachment(&self, name: &str) -> PdfResult<bool> {
        self.edit_transaction(|doc| {
            let (names, tree, mut entries) = doc.attachment_entries()?;
            let mut found = None;
            for (index, (key, _)) in entries.iter().enumerate() {
                if key.text_content_with_limits(&doc.ctx().limits())? == name {
                    found = Some(index);
                    break;
                }
            }
            let Some(index) = found else {
                return Ok(false);
            };
            entries.remove(index);
            doc.write_attachment_entries(names, tree, entries)?;
            Ok(true)
        })
    }

    fn write_attachment_entries(
        &self,
        mut names: PdfDict,
        mut tree: PdfDict,
        mut entries: Vec<(PdfString, PdfObject)>,
    ) -> PdfResult<()> {
        if entries.len() > self.ctx().limits().max_objects / 2 {
            return Err(PdfError::ResourceLimit("EmbeddedFiles name entries".into()));
        }
        entries.sort_by(|a, b| a.0.bytes().cmp(b.0.bytes()));
        let entries: Vec<_> = entries
            .into_iter()
            .map(|(key, value)| (super::trees::TreeKey::Name(key.bytes().to_vec()), value))
            .collect();
        tree = self.build_tree(super::trees::TreeKind::Name, &entries, tree)?;
        let tree_id = self.try_insert_object(PdfObject::PdfDict(tree))?;
        names.try_insert("EmbeddedFiles".into(), reference(tree_id))?;
        let mut root = self.ctx().get_root_owned()?;
        root.try_insert("Names".into(), PdfObject::PdfDict(names))?;
        let root_id = self
            .ctx()
            .trailer()
            .get("Root")
            .and_then(PdfObject::as_reference)
            .map(PdfReference::id)
            .ok_or_else(|| PdfError::DocumentError("Root must be indirect".into()))?;
        self.update_object(root_id, PdfObject::PdfDict(root))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        objects::pdf_array::PdfArray,
        parser::options::{Limits, ParseOptions},
    };

    #[test]
    fn attachment_roundtrip_and_remove() {
        let doc = PdfDocument::new().unwrap();
        doc.add_attachment("测试.txt", b"hello".to_vec()).unwrap();
        doc.add_attachment("z.bin", vec![0, 1, 255]).unwrap();
        assert_eq!(doc.attachment_names().unwrap(), vec!["z.bin", "测试.txt"]);
        assert_eq!(
            doc.attachment_data("测试.txt").unwrap(),
            Some(b"hello".to_vec())
        );
        let before = doc.commit_revision();
        assert!(doc.add_attachment("测试.txt", vec![]).is_err());
        assert_eq!(doc.commit_revision(), before);
        let reopened = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(
            reopened.attachment_data("z.bin").unwrap(),
            Some(vec![0, 1, 255])
        );
        assert!(reopened.remove_attachment("z.bin").unwrap());
        assert_eq!(reopened.attachment_data("z.bin").unwrap(), None);
    }

    #[test]
    fn malformed_mixed_attachment_tree_is_rejected_without_write() {
        let doc = PdfDocument::new().unwrap();
        doc.add_attachment("one.txt", b"one".to_vec()).unwrap();
        let root = doc.ctx().get_root_owned().unwrap();
        let names = root.get("Names").unwrap().as_dict().unwrap();
        let tree_id = names
            .get("EmbeddedFiles")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let mut tree = doc.resolve_object(tree_id).unwrap().into_dict().unwrap();
        tree.try_insert("Kids".into(), PdfObject::PdfArray(PdfArray::default()))
            .unwrap();
        doc.update_object(tree_id, PdfObject::PdfDict(tree))
            .unwrap();
        let revision = doc.commit_revision();
        assert!(doc.attachment_names().is_err());
        assert!(doc.add_attachment("two.txt", b"two".to_vec()).is_err());
        assert_eq!(doc.commit_revision(), revision);
    }

    #[test]
    fn malformed_attachment_name_order_is_rejected_without_write() {
        let doc = PdfDocument::new().unwrap();
        doc.add_attachment("one.txt", b"one".to_vec()).unwrap();
        doc.add_attachment("two.txt", b"two".to_vec()).unwrap();
        let root = doc.ctx().get_root_owned().unwrap();
        let names = root.get("Names").unwrap().as_dict().unwrap();
        let tree_id = names
            .get("EmbeddedFiles")
            .unwrap()
            .as_reference()
            .unwrap()
            .id();
        let mut tree = doc.resolve_object(tree_id).unwrap().into_dict().unwrap();
        let original = tree.get("Names").unwrap().as_array().unwrap();
        let mut pairs = PdfArray::default();
        for index in [2, 3, 0, 1] {
            pairs
                .try_add_obj(original.get(index).unwrap().clone())
                .unwrap();
        }
        tree.try_insert("Names".into(), PdfObject::PdfArray(pairs))
            .unwrap();
        doc.update_object(tree_id, PdfObject::PdfDict(tree))
            .unwrap();
        let revision = doc.commit_revision();
        assert!(doc.attachment_names().is_err());
        assert!(doc.remove_attachment("one.txt").is_err());
        assert_eq!(doc.commit_revision(), revision);
    }

    #[test]
    fn attachment_payload_limit_rejects_before_write() {
        let doc = PdfDocument::new_with_options(ParseOptions {
            limits: Limits {
                // Limit the payload stream without also limiting filenames.
                max_raw_stream_bytes: 4,
                ..Default::default()
            },
            ..Default::default()
        })
        .unwrap();
        let revision = doc.commit_revision();
        assert!(matches!(
            doc.add_attachment("too-large.bin", b"12345".to_vec()),
            Err(PdfError::ResourceLimit(message)) if message == "attachment bytes"
        ));
        assert_eq!(doc.commit_revision(), revision);
        doc.add_attachment("within-limit.bin", b"1234".to_vec())
            .unwrap();
        assert_eq!(doc.attachment_names().unwrap(), vec!["within-limit.bin"]);
        assert_eq!(
            doc.attachment_data("within-limit.bin").unwrap(),
            Some(b"1234".to_vec())
        );
    }
}
