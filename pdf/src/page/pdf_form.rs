//! Form dictionary preflight; not an interpreter or transparency compositor.
use crate::{
    error::{PdfError, PdfResult},
    geom::{matrix::Matrix, rectangle::Rectangle},
    objects::{PdfObject, pdf_dict::PdfDict},
    pdf_context::PDFContext,
};
use std::io::{Read, Seek};

#[derive(Debug, Clone)]
pub struct PdfForm {
    pub bounds: Rectangle,
    pub matrix: Matrix,
    /// None retains the distinction between omitted and explicitly empty scope.
    pub resources: Option<PdfDict>,
    /// Preserved only; transparency group execution is not implemented here.
    pub group: Option<PdfDict>,
}

impl PdfForm {
    /// Independent invocation state. Caller state is untouched on success/error.
    /// Cloning inherited state is not an operation-wide peak-memory guarantee.
    pub fn invocation_state(
        &self,
        parent: &crate::page::all_state::AllState,
        page_matrix: &Matrix,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<crate::page::all_state::AllState> {
        if self.group.is_some() {
            return Err(PdfError::UnsupportedFeature("Form group execution".into()));
        }
        let ctm = self.invocation_matrix(parent.ctm())?;
        let operand = self.bbox_clip_element(parent.ctm(), page_matrix, limits)?;
        let mut state = parent.clone();
        state
            .graphic_state
            .clip_path
            .try_add_element(operand, limits)?;
        state.ctm = ctm;
        Ok(state)
    }
    /// BBox intersection operand, already in display coordinates. Existing
    /// clipping operands remain separate; renderers must intersect all of them.
    pub fn bbox_clip_element(
        &self,
        parent_ctm: &Matrix,
        page_matrix: &Matrix,
        limits: &crate::parser::options::Limits,
    ) -> PdfResult<crate::page::clip_path::ClipElement> {
        use crate::path::pdf_path::{PdfPath, Segment, SubPath};
        if limits.max_path_segments < 5
            || limits.max_objects < 1
            || limits.max_decoded_bytes
                < 5 * std::mem::size_of::<Segment>() + std::mem::size_of::<SubPath>()
        {
            return Err(PdfError::ResourceLimit("Form BBox clipping storage".into()));
        }
        let points = self.bbox_display_corners(parent_ctm, page_matrix)?;
        let mut segments = Vec::new();
        segments
            .try_reserve_exact(5)
            .map_err(|_| PdfError::ResourceLimit("Form BBox clip allocation".into()))?;
        segments.extend([
            Segment::MoveTo(points[0].clone()),
            Segment::LineTo(points[1].clone()),
            Segment::LineTo(points[2].clone()),
            Segment::LineTo(points[3].clone()),
            Segment::Close,
        ]);
        let mut path = PdfPath::default();
        path.try_add_subpath(SubPath::new(segments, true))?;
        Ok(crate::page::clip_path::ClipElement::new(
            path,
            crate::page::graphic_state::FillType::Winding,
            Matrix::identity(),
        ))
    }

    /// Form-to-user matrix for this invocation; does not alter the caller CTM.
    pub fn invocation_matrix(&self, parent_ctm: &Matrix) -> PdfResult<Matrix> {
        self.matrix.checked_mul(parent_ctm)
    }

    /// Actual transformed BBox corners, not its axis-aligned enclosing box.
    /// The interpreter must intersect this polygon with the inherited clip.
    pub fn bbox_display_corners(
        &self,
        parent_ctm: &Matrix,
        page_matrix: &Matrix,
    ) -> PdfResult<[crate::geom::point::Point; 4]> {
        use crate::geom::point::Point;
        let matrix = self
            .invocation_matrix(parent_ctm)?
            .checked_mul(page_matrix)?;
        let corners = [
            Point::new(self.bounds.llx(), self.bounds.lly()),
            Point::new(self.bounds.urx(), self.bounds.lly()),
            Point::new(self.bounds.urx(), self.bounds.ury()),
            Point::new(self.bounds.llx(), self.bounds.ury()),
        ];
        Ok([
            matrix.checked_transform(&corners[0])?,
            matrix.checked_transform(&corners[1])?,
            matrix.checked_transform(&corners[2])?,
            matrix.checked_transform(&corners[3])?,
        ])
    }

    /// Select the Form's entire scope, or an explicitly supplied legacy page
    /// scope if Resources is absent. Never merge missing names from a parent.
    pub fn resource_scope<'a>(&'a self, legacy_page_resources: &'a PdfDict) -> &'a PdfDict {
        self.resources.as_ref().unwrap_or(legacy_page_resources)
    }

    /// Resolve a named entry inside the selected scope. The caller controls
    /// legacy page fallback; this does not execute the Form or recurse.
    pub fn named_resource<R: Read + Seek>(
        &self,
        legacy_page_resources: &PdfDict,
        category: &str,
        name: &str,
        ctx: &PDFContext<R>,
    ) -> PdfResult<PdfObject> {
        let scope = self.resource_scope(legacy_page_resources);
        let entries = scope.get(category).ok_or_else(|| {
            PdfError::PageResourceError(format!("Form resource category {category} missing"))
        })?;
        let entries = ctx.resolve(entries)?;
        let entries = entries.as_dict().ok_or_else(|| {
            PdfError::PageResourceError(format!(
                "Form resource category {category} must be a dictionary"
            ))
        })?;
        let value = entries.get(name).ok_or_else(|| {
            PdfError::PageResourceError(format!("Form resource {category}/{name} missing"))
        })?;
        let value = ctx.resolve(value)?;
        if matches!(value, PdfObject::PdfNull) {
            return Err(PdfError::PageResourceError(format!(
                "Form resource {category}/{name} is null"
            )));
        }
        Ok(value)
    }

    pub fn try_new<R: Read + Seek>(object: &PdfObject, ctx: &PDFContext<R>) -> PdfResult<Self> {
        let object = ctx.resolve(object)?;
        let stream = object
            .as_stream()
            .ok_or_else(|| PdfError::PageResourceError("Form must be a stream".into()))?;
        let dict = stream.dict();
        let subtype = ctx.resolve(
            dict.get("Subtype")
                .ok_or_else(|| PdfError::PageResourceError("Form Subtype missing".into()))?,
        )?;
        if subtype.as_name().is_none_or(|name| name.name() != "Form") {
            return Err(PdfError::PageResourceError("Expected Form subtype".into()));
        }
        if let Some(kind) = dict.get("FormType") {
            let kind = ctx.resolve(kind)?;
            if kind.as_f32() != Some(1.) {
                return Err(PdfError::UnsupportedFeature("Unsupported FormType".into()));
            }
        }
        let bounds = Rectangle::try_from_with_context(
            dict.get("BBox")
                .ok_or_else(|| PdfError::PageResourceError("Form BBox missing".into()))?,
            ctx,
        )?;
        let matrix = if let Some(value) = dict.get("Matrix") {
            let value = ctx.resolve(value)?;
            let array = value.as_array().ok_or_else(|| {
                PdfError::PageResourceError("Form Matrix must be an array".into())
            })?;
            if array.len() != 6 {
                return Err(PdfError::PageResourceError(
                    "Form Matrix length must be six".into(),
                ));
            }
            let mut direct = crate::objects::pdf_array::PdfArray::default();
            for entry in array {
                direct.try_add_obj(ctx.resolve(entry)?)?;
            }
            Matrix::try_from(&PdfObject::PdfArray(direct))?
        } else {
            Matrix::identity()
        };
        let dictionary = |key: &str| -> PdfResult<Option<PdfDict>> {
            match dict.get(key) {
                None => Ok(None),
                Some(value) => {
                    let value = ctx.resolve(value)?;
                    if matches!(value, PdfObject::PdfNull) {
                        return Ok(None);
                    }
                    value.as_dict().cloned().map(Some).ok_or_else(|| {
                        PdfError::PageResourceError(format!("Form {key} must be a dictionary"))
                    })
                }
            }
        };
        Ok(Self {
            bounds,
            matrix,
            resources: dictionary("Resources")?,
            group: dictionary("Group")?,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn invocation_state_preserves_parent_and_rejects_unimplemented_groups() {
        let mut form = PdfForm {
            bounds: Rectangle::new(0., 0., 10., 20.),
            matrix: Matrix::new(1., 0., 0., 1., 3., 4.),
            resources: None,
            group: None,
        };
        let limits = crate::parser::options::Limits::default();
        let mut parent = crate::page::all_state::AllState::default();
        parent.ctm = Matrix::new(2., 0., 0., 2., 10., 20.);
        parent.text_rise = 7.;
        let mut child = form
            .invocation_state(&parent, &Matrix::identity(), &limits)
            .unwrap();
        assert_eq!((child.ctm.e(), child.ctm.f()), (16., 28.));
        assert_eq!(child.text_rise, 7.);
        assert_eq!(child.graphic_state.clip_path.elements.len(), 1);
        child.text_rise = 99.;
        child.ctm = Matrix::identity();
        assert_eq!(parent.text_rise, 7.);
        assert_eq!((parent.ctm.e(), parent.ctm.f()), (10., 20.));
        assert!(parent.graphic_state.clip_path.is_empty());
        let nested = form
            .invocation_state(&child, &Matrix::identity(), &limits)
            .unwrap();
        assert_eq!(nested.graphic_state.clip_path.elements.len(), 2);
        assert!(
            form.invocation_state(
                &child,
                &Matrix::identity(),
                &crate::parser::options::Limits {
                    max_path_segments: 5,
                    ..limits
                }
            )
            .is_err()
        );
        assert_eq!(child.graphic_state.clip_path.elements.len(), 1);
        form.group = Some(PdfDict::default());
        assert!(matches!(
            form.invocation_state(&parent, &Matrix::identity(), &limits),
            Err(PdfError::UnsupportedFeature(_))
        ));
    }
    #[test]
    fn form_bbox_keeps_transformed_polygon_and_checks_overflow() {
        let mut form = PdfForm {
            bounds: Rectangle::new(0., 0., 10., 20.),
            matrix: Matrix::new(1., 0.5, -0.25, 1., 3., 4.),
            resources: None,
            group: None,
        };
        let parent = Matrix::new(0., 1., -1., 0., 10., 20.);
        let page = Matrix::new(1., 0., 0., -1., 0., 100.);
        let points = form.bbox_display_corners(&parent, &page).unwrap();
        let actual = points.map(|p| [p.x(), p.y()]);
        assert_eq!(actual, [[6., 77.], [1., 67.], [-19., 72.], [-14., 82.]]);
        let limits = crate::parser::options::Limits::default();
        let operand = form.bbox_clip_element(&parent, &page, &limits).unwrap();
        let segments = operand.path.subpaths()[0].segments();
        assert_eq!(segments.len(), 5);
        assert!(matches!(
            segments[0],
            crate::path::pdf_path::Segment::MoveTo(_)
        ));
        assert!(matches!(segments[4], crate::path::pdf_path::Segment::Close));
        assert_eq!((operand.matrix.a(), operand.matrix.d()), (1., 1.));
        let mut inherited = crate::page::clip_path::ClipPath::default();
        inherited.try_add_element(operand.clone(), &limits).unwrap();
        let mut isolated = inherited.clone();
        isolated.try_add_element(operand.clone(), &limits).unwrap();
        assert_eq!(inherited.elements.len(), 1);
        assert_eq!(isolated.elements.len(), 2);
        for bounded in [
            crate::parser::options::Limits {
                max_path_segments: 10,
                ..limits
            },
            crate::parser::options::Limits {
                max_objects: 2,
                ..limits
            },
            crate::parser::options::Limits {
                max_decoded_bytes: 1,
                ..limits
            },
        ] {
            assert!(isolated.try_add_element(operand.clone(), &bounded).is_err());
            assert_eq!(isolated.elements.len(), 2);
        }
        assert!(
            form.bbox_clip_element(
                &parent,
                &page,
                &crate::parser::options::Limits {
                    max_path_segments: 4,
                    ..limits
                }
            )
            .is_err()
        );
        assert_eq!((parent.e(), parent.f()), (10., 20.));
        form.matrix = Matrix::new(0., 0., 0., 0., 3., 4.);
        let points = form.bbox_display_corners(&parent, &page).unwrap();
        assert!(points.iter().all(|p| p.x() == 6. && p.y() == 77.));
        form.matrix = Matrix::new(f32::MAX, 0., 0., 1., 0., 0.);
        assert!(
            form.bbox_display_corners(&Matrix::identity(), &Matrix::identity())
                .is_err()
        );
        form.matrix = Matrix::new(f32::NAN, 0., 0., 1., 0., 0.);
        assert!(form.invocation_matrix(&parent).is_err());
    }
    #[test]
    fn indirect_resources_and_geometry_survive_strict_save_reopen() {
        use crate::{
            document::{PdfDocument, resource_manager::ResourceKind},
            io::stream_reader::StreamReader,
            objects::{pdf_reference::PdfReference, pdf_stream::PdfStream},
            parser::syntax::{ParseMode, SyntaxParser},
        };
        use std::io::Cursor;
        let parse = |source: &str| {
            SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            )
            .get_object()
            .unwrap()
        };
        let doc = PdfDocument::new().unwrap();
        doc.add_page(0).unwrap();
        let coordinate = doc.try_insert_object(parse("3")).unwrap();
        let bounds = doc.try_insert_object(parse("[0 0 10 20]")).unwrap();
        let matrix = doc
            .try_insert_object(parse(&format!("[1 0 0 1 {} 0 R 4]", coordinate.obj_num())))
            .unwrap();
        let value = doc
            .try_insert_object(parse("<< /Marker /Local >>"))
            .unwrap();
        let entries = doc
            .try_insert_object(parse(&format!("<< /Shared {} 0 R >>", value.obj_num())))
            .unwrap();
        let scope = doc
            .try_insert_object(parse(&format!("<< /XObject {} 0 R >>", entries.obj_num())))
            .unwrap();
        let form_dict = parse(&format!(
            "<< /Type /XObject /Subtype /Form /BBox {} 0 R /Matrix {} 0 R /Resources {} 0 R >>",
            bounds.obj_num(),
            matrix.obj_num(),
            scope.obj_num()
        ))
        .into_dict()
        .unwrap();
        let form_id = doc
            .try_insert_object(PdfObject::PdfStream(
                PdfStream::from_decoded_data(form_dict, b"q Q".to_vec()).unwrap(),
            ))
            .unwrap();
        assert!(
            doc.resources()
                .register_existing(0, ResourceKind::Form, value)
                .is_err()
        );
        let name = doc
            .resources()
            .register_existing(0, ResourceKind::Form, form_id)
            .unwrap();
        assert!(name.starts_with("Fm"));
        let target = PdfDocument::new().unwrap();
        target.add_page(0).unwrap();
        let source_handle = doc.object_handle(form_id).unwrap();
        assert!(
            target
                .resources()
                .register_handle(0, ResourceKind::Form, &source_handle)
                .is_err()
        );
        let imported = target
            .import_resource_from(&doc, &source_handle, ResourceKind::Form)
            .unwrap();
        let imported_name = target
            .resources()
            .register_handle(0, ResourceKind::Form, &imported)
            .unwrap();
        let reopened =
            PdfDocument::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        let reference = PdfObject::PdfReference(PdfReference::from_id(form_id));
        for document in [&doc, &reopened] {
            assert!(
                document
                    .get_page(0)
                    .unwrap()
                    .get_xobject(&name)
                    .unwrap()
                    .as_stream()
                    .is_some()
            );
            let form = PdfForm::try_new(&reference, document.ctx()).unwrap();
            assert_eq!((form.matrix.e(), form.matrix.f()), (3., 4.));
            let parent = parse("<< /XObject << /Shared << /Marker /Parent >> /OnlyParent 1 >> >>")
                .into_dict()
                .unwrap();
            let resolved = form
                .named_resource(&parent, "XObject", "Shared", document.ctx())
                .unwrap();
            assert_eq!(
                resolved
                    .get_attr("Marker")
                    .unwrap()
                    .as_name()
                    .unwrap()
                    .name(),
                "Local"
            );
            assert!(
                form.named_resource(&parent, "XObject", "OnlyParent", document.ctx())
                    .is_err()
            );
        }
        drop(reopened);
        drop(doc);
        let target_reopened =
            PdfDocument::from_bytes_with_mode(target.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        for document in [&target, &target_reopened] {
            let object = document
                .get_page(0)
                .unwrap()
                .get_xobject(&imported_name)
                .unwrap();
            assert_eq!(
                object
                    .as_stream()
                    .unwrap()
                    .decode_data(document.ctx())
                    .unwrap(),
                b"q Q"
            );
            let form = PdfForm::try_new(&object, document.ctx()).unwrap();
            assert_eq!((form.matrix.e(), form.matrix.f()), (3., 4.));
            let resource = form
                .named_resource(&PdfDict::default(), "XObject", "Shared", document.ctx())
                .unwrap();
            assert_eq!(
                resource
                    .get_attr("Marker")
                    .unwrap()
                    .as_name()
                    .unwrap()
                    .name(),
                "Local"
            );
        }
    }
    #[test]
    fn form_dictionary_validates_geometry_and_preserves_resource_scope() {
        use crate::{
            document::cross_ref_table::CrossRefTable,
            io::stream_reader::StreamReader,
            objects::pdf_stream::PdfStream,
            parser::{parser::PdfParser, syntax::SyntaxParser},
        };
        use std::{collections::HashMap, io::Cursor};
        let ctx = PDFContext::try_new(
            PdfParser::new(SyntaxParser::new(
                StreamReader::try_new(Cursor::new(Vec::<u8>::new())).unwrap(),
            )),
            CrossRefTable::new(HashMap::new(), PdfDict::default()),
        )
        .unwrap();
        let parse = |source: &str| {
            let mut parser = SyntaxParser::new(
                StreamReader::try_new(Cursor::new(source.as_bytes().to_vec())).unwrap(),
            );
            let dict = parser.get_object().unwrap().into_dict().unwrap();
            PdfObject::PdfStream(PdfStream::from_decoded_data(dict, Vec::new()).unwrap())
        };
        let base =
            PdfForm::try_new(&parse("<< /Subtype /Form /BBox [0 0 10 20] >>"), &ctx).unwrap();
        assert!(base.resources.is_none());
        assert_eq!(base.matrix.a(), 1.);
        assert_eq!(base.matrix.d(), 1.);
        let parent = parse(
            "<< /Subtype /Form /BBox [0 0 1 1] /Resources << /XObject << /Shared 12 /ParentOnly 13 >> >> >>",
        );
        let parent = PdfForm::try_new(&parent, &ctx).unwrap();
        let page_resources = parent.resources.as_ref().unwrap();
        assert_eq!(
            base.named_resource(page_resources, "XObject", "Shared", &ctx)
                .unwrap()
                .as_f32(),
            Some(12.)
        );
        let own = PdfForm::try_new(&parse("<< /Subtype /Form /BBox [0 0 1 1] /Resources << /XObject << /Shared 22 /Null null >> >> >>"),&ctx).unwrap();
        assert_eq!(
            own.named_resource(page_resources, "XObject", "Shared", &ctx)
                .unwrap()
                .as_f32(),
            Some(22.)
        );
        assert!(
            own.named_resource(page_resources, "XObject", "ParentOnly", &ctx)
                .is_err()
        );
        assert!(
            own.named_resource(page_resources, "XObject", "Null", &ctx)
                .is_err()
        );
        assert!(
            own.named_resource(page_resources, "Font", "Shared", &ctx)
                .is_err()
        );
        let empty = PdfForm::try_new(
            &parse("<< /Subtype /Form /BBox [0 0 1 1] /Resources << >> >>"),
            &ctx,
        )
        .unwrap();
        assert!(
            empty
                .named_resource(page_resources, "XObject", "Shared", &ctx)
                .is_err()
        );
        let malformed = PdfForm::try_new(
            &parse("<< /Subtype /Form /BBox [0 0 1 1] /Resources << /XObject 3 >> >>"),
            &ctx,
        )
        .unwrap();
        assert!(
            malformed
                .named_resource(page_resources, "XObject", "Shared", &ctx)
                .is_err()
        );
        let explicit = PdfForm::try_new(&parse("<< /Subtype /Form /FormType 1 /BBox [0 0 10 20] /Matrix [1 0 0 1 3 4] /Resources << >> /Group << /S /Transparency >> >>"),&ctx).unwrap();
        assert!(explicit.resources.is_some());
        assert!(explicit.group.is_some());
        assert_eq!((explicit.matrix.e(), explicit.matrix.f()), (3., 4.));
        for source in [
            "<< /Subtype /Form >>",
            "<< /Subtype /Image /BBox [0 0 1 1] >>",
            "<< /Subtype /Form /BBox [0 0 1] >>",
            "<< /Subtype /Form /BBox [0 0 1 1] /Matrix [1 0 0] >>",
            "<< /Subtype /Form /BBox [0 0 1 1] /Matrix [1 0 0 1 /bad 0] >>",
            "<< /Subtype /Form /FormType 2 /BBox [0 0 1 1] >>",
            "<< /Subtype /Form /BBox [0 0 1 1] /Resources 3 >>",
            "<< /Subtype /Form /BBox [0 0 1 1] /Group /bad >>",
        ] {
            assert!(PdfForm::try_new(&parse(source), &ctx).is_err(), "{source}");
        }
        assert!(PdfForm::try_new(&PdfObject::PdfNull, &ctx).is_err());
    }
}
