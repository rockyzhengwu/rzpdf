//! Empty signature-field creation reuses the ordinary ownership/path publisher.
use super::graph::{invalid, partial_name};
use super::*;
use crate::document::identity::ObjectHandle;

impl<R: Read + Seek> PdfDocument<R> {
    pub(crate) fn create_signature_field(
        &self,
        full_name: &str,
        widgets: &[FormWidgetSpec],
    ) -> PdfResult<ObjectHandle> {
        self.ctx().require_annotation_permission()?;
        if self.effective_version()? < crate::document::save_options::PdfVersion::V1_3 {
            return Err(PdfError::UnsupportedFeature(
                "signature fields require PDF 1.3".into(),
            ));
        }
        if full_name.len() > 4096 {
            return Err(PdfError::ResourceLimit("signature field name".into()));
        }
        PdfString::from_text_with_limits(full_name, &self.ctx().limits())?;
        for part in full_name.split('.') {
            partial_name(part)?;
        }
        if widgets.is_empty() || widgets.len() > self.ctx().limits().max_objects {
            return Err(PdfError::ResourceLimit("signature Widget count".into()));
        }
        for widget in widgets {
            let r = widget.rect;
            if widget.on_state.is_some()
                || !r.iter().all(|v| v.is_finite())
                || r[2] < r[0]
                || r[3] < r[1]
                || !(r[2] - r[0]).is_finite()
                || !(r[3] - r[1]).is_finite()
            {
                return Err(invalid("signature Widget geometry/state"));
            }
        }
        self.edit_transaction(|doc| {
            let graph = doc.field_graph()?;
            doc.form_reference_guard(&graph)?;
            if graph.nodes.iter().any(|n| n.name == full_name) {
                return Err(invalid("duplicate signature field name"));
            }
            let mut form = graph.form.clone();
            let parent = doc.ensure_form_path(&graph, &mut form, full_name)?;
            let mut field = PdfDict::default();
            field.try_insert(
                "T".into(),
                PdfObject::PdfString(PdfString::from_text_with_limits(
                    full_name.rsplit('.').next().unwrap(),
                    &doc.ctx().limits(),
                )?),
            )?;
            field.try_insert("FT".into(), name("Sig"))?;
            if let Some(parent) = parent {
                field.try_insert("Parent".into(), reference(parent))?;
            }
            let id = doc.try_insert_object(PdfObject::PdfDict(field.clone()))?;
            let mut kids = PdfArray::default();
            for spec in widgets {
                let page = doc.page_location(spec.page_index)?;
                let mut w = if widgets.len() == 1 {
                    field.clone()
                } else {
                    PdfDict::default()
                };
                for (key, value) in [
                    ("Type", name("Annot")),
                    ("Subtype", name("Widget")),
                    ("F", number(4.)),
                    ("P", reference(page.page_id)),
                    ("Rect", PdfObject::PdfArray(numbers(&spec.rect)?)),
                ] {
                    w.try_insert(key.into(), value)?;
                }
                if widgets.len() > 1 {
                    w.try_insert("Parent".into(), reference(id))?;
                }
                let mut ap = PdfDict::default();
                ap.try_insert("Type".into(), name("XObject"))?;
                ap.try_insert("Subtype".into(), name("Form"))?;
                ap.try_insert(
                    "BBox".into(),
                    PdfObject::PdfArray(numbers(&[
                        0.,
                        0.,
                        (spec.rect[2] - spec.rect[0]).max(1.),
                        (spec.rect[3] - spec.rect[1]).max(1.),
                    ])?),
                )?;
                ap.try_insert(
                    "Matrix".into(),
                    PdfObject::PdfArray(numbers(&[1., 0., 0., 1., 0., 0.])?),
                )?;
                ap.try_insert("Resources".into(), PdfObject::PdfDict(PdfDict::default()))?;
                let aid = doc.try_insert_object(PdfObject::PdfStream(
                    crate::objects::pdf_stream::PdfStream::new(ap, Vec::new()),
                ))?;
                let mut appearances = PdfDict::default();
                appearances.try_insert("N".into(), reference(aid))?;
                w.try_insert("AP".into(), PdfObject::PdfDict(appearances))?;
                let wid = if widgets.len() == 1 {
                    doc.update_object(id, PdfObject::PdfDict(w))?;
                    id
                } else {
                    let wid = doc.try_insert_object(PdfObject::PdfDict(w))?;
                    kids.try_add_obj(reference(wid))?;
                    wid
                };
                let mut page_dict = page.page_dict;
                let mut annots = doc.form_array(&page_dict, "Annots")?;
                annots.try_add_obj(reference(wid))?;
                page_dict.try_insert("Annots".into(), PdfObject::PdfArray(annots))?;
                doc.update_object(page.page_id, PdfObject::PdfDict(page_dict))?;
            }
            if widgets.len() > 1 {
                field.try_insert("Kids".into(), PdfObject::PdfArray(kids))?;
                doc.update_object(id, PdfObject::PdfDict(field))?;
            }
            doc.append_field_child(&mut form, parent, id)?;
            doc.publish_form(form)?;
            if doc.field_graph()?.nodes.len() > 256 {
                return Err(PdfError::ResourceLimit(
                    "signature field graph count".into(),
                ));
            }
            doc.object_handle(id)
        })
    }
}
