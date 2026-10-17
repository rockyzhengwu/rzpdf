//! Shared self-contained normal appearance assembly. No page resource mutation.
use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    objects::{
        PdfObject, object_id::ObjectId, pdf_array::PdfArray, pdf_dict::PdfDict, pdf_name::PdfName,
        pdf_number::PdfNumber, pdf_reference::PdfReference, pdf_stream::PdfStream,
    },
};
use std::io::{Read, Seek};

pub(crate) fn nums(values: &[f32]) -> PdfResult<PdfObject> {
    let mut array = PdfArray::default();
    for &n in values {
        if !n.is_finite() {
            return Err(PdfError::DocumentError(
                "non-finite appearance number".into(),
            ));
        }
        array.try_add_obj(PdfObject::PdfNumber(PdfNumber::new(n)))?;
    }
    Ok(PdfObject::PdfArray(array))
}
pub(crate) fn name(s: &str) -> PdfObject {
    PdfObject::PdfName(PdfName::new(s.into()))
}
pub(crate) fn reference(id: ObjectId) -> PdfObject {
    PdfObject::PdfReference(PdfReference::from_id(id))
}
pub(crate) fn courier() -> PdfResult<PdfDict> {
    let mut font = PdfDict::default();
    font.try_insert("Type".into(), name("Font"))?;
    font.try_insert("Subtype".into(), name("Type1"))?;
    font.try_insert("BaseFont".into(), name("Courier"))?;
    let mut fonts = PdfDict::default();
    fonts.try_insert("F0".into(), PdfObject::PdfDict(font))?;
    let mut resources = PdfDict::default();
    resources.try_insert("Font".into(), PdfObject::PdfDict(fonts))?;
    Ok(resources)
}

impl<R: Read + Seek> PdfDocument<R> {
    pub(crate) fn normal_appearance(
        &self,
        width: f32,
        height: f32,
        resources: PdfDict,
        content: Vec<u8>,
        opacity: f32,
        blend: &str,
    ) -> PdfResult<ObjectId> {
        if !width.is_finite()
            || !height.is_finite()
            || width <= 0.
            || height <= 0.
            || !opacity.is_finite()
            || !(0. ..=1.).contains(&opacity)
        {
            return Err(PdfError::DocumentError(
                "invalid appearance bounds/opacity".into(),
            ));
        }
        if content.len() > self.ctx().limits().max_raw_stream_bytes
            || content.len() > self.ctx().limits().max_decoded_bytes
        {
            return Err(PdfError::ResourceLimit(
                "annotation appearance bytes".into(),
            ));
        }
        let mut dict = PdfDict::default();
        dict.try_insert("Type".into(), name("XObject"))?;
        dict.try_insert("Subtype".into(), name("Form"))?;
        dict.try_insert("BBox".into(), nums(&[0., 0., width, height])?)?;
        dict.try_insert("Resources".into(), PdfObject::PdfDict(resources))?;
        if opacity == 1. && blend == "Normal" {
            return self.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
                dict, content,
            )?));
        }
        // Apply opacity to the whole isolated artwork, not to each intersecting
        // line/shape. The outer AP owns alpha; readers need not apply /CA again.
        let mut group = PdfDict::default();
        group.try_insert("S".into(), name("Transparency"))?;
        group.try_insert(
            "I".into(),
            PdfObject::PdfBool(crate::objects::pdf_bool::PdfBool::new(b"true")),
        )?;
        group.try_insert("CS".into(), name("DeviceRGB"))?;
        dict.try_insert("Group".into(), PdfObject::PdfDict(group))?;
        let inner = self.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dict.clone(),
            content,
        )?))?;
        dict.remove("Group");
        let mut xobjects = PdfDict::default();
        xobjects.try_insert("Art".into(), reference(inner))?;
        let mut state = PdfDict::default();
        state.try_insert("ca".into(), PdfObject::PdfNumber(PdfNumber::new(opacity)))?;
        state.try_insert("CA".into(), PdfObject::PdfNumber(PdfNumber::new(opacity)))?;
        state.try_insert("BM".into(), name(blend))?;
        let mut states = PdfDict::default();
        states.try_insert("Opacity".into(), PdfObject::PdfDict(state))?;
        let mut resources = PdfDict::default();
        resources.try_insert("XObject".into(), PdfObject::PdfDict(xobjects))?;
        resources.try_insert("ExtGState".into(), PdfObject::PdfDict(states))?;
        dict.try_insert("Resources".into(), PdfObject::PdfDict(resources))?;
        let outer = b"q /Opacity gs /Art Do Q".to_vec();
        if outer.len() > self.ctx().limits().max_raw_stream_bytes
            || outer.len() > self.ctx().limits().max_decoded_bytes
        {
            return Err(PdfError::ResourceLimit(
                "annotation appearance bytes".into(),
            ));
        }
        self.try_insert_object(PdfObject::PdfStream(PdfStream::from_decoded_data(
            dict, outer,
        )?))
    }
}
