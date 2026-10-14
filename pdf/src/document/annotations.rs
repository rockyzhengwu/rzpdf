//! Limited square annotations with self-contained normal appearance streams.

use std::io::{Read, Seek};

use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream,
    },
};

#[derive(Debug, Clone, PartialEq)]
pub struct SquareAnnotation {
    pub rect: [f32; 4],
    pub color: [f32; 3],
}

fn ref_obj(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
fn nums(values: &[f32]) -> PdfResult<PdfArray> {
    let mut array = PdfArray::default();
    for &v in values {
        array.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(v)))?;
    }
    Ok(array)
}

impl<R: Read + Seek> PdfDocument<R> {
    /// Read supported square annotations; unrelated annotation types are kept
    /// in the page and omitted from this view.
    pub fn page_square_annotations(&self, page_index: u32) -> PdfResult<Vec<SquareAnnotation>> {
        let page = self.page_location(page_index)?;
        let Some(value) = page.page_dict.get("Annots") else {
            return Ok(Vec::new());
        };
        let array = self
            .ctx()
            .resolve(value)?
            .into_array()
            .ok_or_else(|| PdfError::DocumentError("Annots must be an array".into()))?;
        if array.len() > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("annotation count".into()));
        }
        let mut found = Vec::new();
        for value in &array {
            let dict =
                self.ctx().resolve(value)?.into_dict().ok_or_else(|| {
                    PdfError::DocumentError("annotation must be a dictionary".into())
                })?;
            let subtype = dict
                .get("Subtype")
                .ok_or_else(|| PdfError::DocumentError("annotation Subtype missing".into()))?;
            if self.ctx().resolve(subtype)?.as_name().map(PdfName::name) != Some("Square") {
                continue;
            }
            if let Some(owner) = dict.get("P") {
                if owner.as_reference().map(PdfReference::id) != Some(page.page_id) {
                    return Err(PdfError::DocumentError(
                        "Square P does not match its page".into(),
                    ));
                }
            }
            let rect = crate::geom::rectangle::Rectangle::try_from_with_context(
                dict.get("Rect")
                    .ok_or_else(|| PdfError::DocumentError("Square Rect missing".into()))?,
                self.ctx(),
            )?;
            let color = dict
                .get("C")
                .ok_or_else(|| PdfError::DocumentError("Square color missing".into()))?;
            let color =
                self.ctx().resolve(color)?.into_array().ok_or_else(|| {
                    PdfError::DocumentError("Square color must be an array".into())
                })?;
            if color.len() != 3 {
                return Err(PdfError::DocumentError("Square color must be RGB".into()));
            }
            let mut rgb = [0.0; 3];
            for (i, value) in (&color).into_iter().enumerate() {
                rgb[i] = self
                    .ctx()
                    .resolve(value)?
                    .as_number()
                    .ok_or_else(|| {
                        PdfError::DocumentError("Square color must contain numbers".into())
                    })?
                    .value();
                if !rgb[i].is_finite() || !(0.0..=1.0).contains(&rgb[i]) {
                    return Err(PdfError::DocumentError("Square color out of range".into()));
                }
            }
            found.push(SquareAnnotation {
                rect: [rect.llx(), rect.lly(), rect.urx(), rect.ury()],
                color: rgb,
            });
        }
        Ok(found)
    }

    /// Add a visible, unfilled square/rectangle annotation. Color is RGB 0..1.
    pub fn add_square_annotation(
        &self,
        page_index: u32,
        rect: [f32; 4],
        color: [f32; 3],
    ) -> PdfResult<()> {
        self.ctx().require_annotation_permission()?;
        if self.effective_version()? < super::save_options::PdfVersion::V1_4 {
            return Err(PdfError::UnsupportedFeature(
                "Square annotation writing requires PDF 1.4 or later".into(),
            ));
        }
        if rect.iter().any(|v| !v.is_finite())
            || rect[0] >= rect[2]
            || rect[1] >= rect[3]
            || color
                .iter()
                .any(|v| !v.is_finite() || !(0.0..=1.0).contains(v))
        {
            return Err(PdfError::DocumentError(
                "invalid Square rectangle or color".into(),
            ));
        }
        let width = rect[2] - rect[0];
        let height = rect[3] - rect[1];
        let content = format!(
            "q {} {} {} RG 1 w 0.5 0.5 {} {} re S Q",
            color[0],
            color[1],
            color[2],
            width - 1.0,
            height - 1.0
        )
        .into_bytes();
        if width < 1.0
            || height < 1.0
            || content.len() > self.ctx().limits().max_raw_stream_bytes
            || content.len() > self.ctx().limits().max_decoded_bytes
        {
            return Err(PdfError::ResourceLimit("Square appearance size".into()));
        }
        self.edit_transaction(|doc| {
            let page = doc.page_location(page_index)?;
            let mut array = match page.page_dict.get("Annots") {
                Some(value) => doc
                    .ctx()
                    .resolve(value)?
                    .into_array()
                    .ok_or_else(|| PdfError::DocumentError("Annots must be an array".into()))?,
                None => PdfArray::default(),
            };
            if array.len() >= doc.ctx().limits().max_objects {
                return Err(PdfError::ResourceLimit("annotation count".into()));
            }
            let mut appearance = PdfDict::default();
            appearance.try_insert(
                "Type".into(),
                PdfObject::PdfName(PdfName::new("XObject".into())),
            )?;
            appearance.try_insert(
                "Subtype".into(),
                PdfObject::PdfName(PdfName::new("Form".into())),
            )?;
            appearance.try_insert(
                "BBox".into(),
                PdfObject::PdfArray(nums(&[0.0, 0.0, width, height])?),
            )?;
            let ap_id = doc.try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(appearance, content)?,
            ))?;
            let mut ap = PdfDict::default();
            ap.try_insert("N".into(), ref_obj(ap_id))?;
            let mut annotation = PdfDict::default();
            annotation.try_insert(
                "Type".into(),
                PdfObject::PdfName(PdfName::new("Annot".into())),
            )?;
            annotation.try_insert(
                "Subtype".into(),
                PdfObject::PdfName(PdfName::new("Square".into())),
            )?;
            annotation.try_insert("Rect".into(), PdfObject::PdfArray(nums(&rect)?))?;
            annotation.try_insert("C".into(), PdfObject::PdfArray(nums(&color)?))?;
            annotation.try_insert("P".into(), ref_obj(page.page_id))?;
            annotation.try_insert("AP".into(), PdfObject::PdfDict(ap))?;
            let id = doc.try_insert_object(PdfObject::PdfDict(annotation))?;
            array.try_add_obj(ref_obj(id))?;
            let mut page_dict = page.page_dict;
            page_dict.try_insert("Annots".into(), PdfObject::PdfArray(array))?;
            doc.update_object(page.page_id, PdfObject::PdfDict(page_dict))
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn square_appearance_roundtrip() {
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let expected = SquareAnnotation {
            rect: [10.0, 20.0, 80.0, 40.0],
            color: [1.0, 0.0, 0.0],
        };
        doc.add_square_annotation(0, expected.rect, expected.color)
            .unwrap();
        let revision = doc.commit_revision();
        assert!(
            doc.add_square_annotation(0, expected.rect, [2.0, 0.0, 0.0])
                .is_err()
        );
        assert_eq!(doc.commit_revision(), revision);
        assert_eq!(
            doc.page_square_annotations(0).unwrap(),
            vec![expected.clone()]
        );
        let reopened = PdfDocument::from_bytes(doc.save_to_bytes().unwrap()).unwrap();
        assert_eq!(reopened.page_square_annotations(0).unwrap(), vec![expected]);
    }
}
