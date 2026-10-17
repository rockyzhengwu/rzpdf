//! Opt-in annotation composition, isolated from Contents provenance and extraction.
use super::{PdfPage, display_list::DisplayList, pdf_form::PdfForm};
use crate::{
    device::Device,
    document::appearance::name,
    error::{PdfError, PdfResult},
    geom::matrix::Matrix,
    objects::{PdfObject, pdf_dict::PdfDict, pdf_stream::PdfStream},
};
use std::io::{Read, Seek};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationRenderIntent {
    Screen,
    Print,
}
#[derive(Debug, Clone, Copy)]
pub struct AnnotationRenderOptions {
    pub intent: AnnotationRenderIntent,
    pub include_widgets: bool,
    /// Viewer magnification, separate from device pixel density. The caller
    /// scales its output device accordingly; NoZoom compensates this factor.
    pub view_zoom: f32,
}
impl Default for AnnotationRenderOptions {
    fn default() -> Self {
        Self {
            intent: AnnotationRenderIntent::Screen,
            include_widgets: true,
            view_zoom: 1.,
        }
    }
}

/// AP lists are private: they cannot be mistaken for editable page content.
/// `contents()` exposes only the original page list with its original provenance.
#[derive(Debug, Clone)]
pub struct AnnotatedDisplayList {
    contents: DisplayList,
    appearances: Vec<DisplayList>,
}
impl AnnotatedDisplayList {
    pub fn contents(&self) -> &DisplayList {
        &self.contents
    }
    pub fn appearance_count(&self) -> usize {
        self.appearances.len()
    }
    pub fn replay(&self, device: &mut dyn Device) -> PdfResult<()> {
        device.start_page(self.contents.page_width(), self.contents.page_height())?;
        self.contents.replay_contents(device)?;
        for appearance in &self.appearances {
            appearance.replay_contents(device)?;
        }
        device.end_page()
    }
}

fn invalid(s: &str) -> PdfError {
    PdfError::PageResourceError(s.into())
}

impl<R: Read + Seek> PdfPage<'_, R> {
    pub fn display_with_annotations(
        &mut self,
        device: &mut dyn Device,
        options: AnnotationRenderOptions,
    ) -> PdfResult<()> {
        self.display_list_with_annotations(options)?.replay(device)
    }

    /// Preflight all visible APs before touching a device. Omitted/hidden entries
    /// do not cause resource decoding. No document objects or revisions change.
    pub fn display_list_with_annotations(
        &mut self,
        options: AnnotationRenderOptions,
    ) -> PdfResult<AnnotatedDisplayList> {
        self.display_list_with_annotations_selected(options,None)
    }

    pub fn display_list_with_annotations_and_optional_content(
        &mut self,
        options: AnnotationRenderOptions,
        visibility: &crate::document::optional_content::OptionalContentVisibility,
    ) -> PdfResult<AnnotatedDisplayList> {
        visibility.validate(self.ctx)?;
        self.display_list_with_annotations_selected(options,Some(visibility))
    }

    fn display_list_with_annotations_selected(
        &mut self,
        options: AnnotationRenderOptions,
        visibility: Option<&crate::document::optional_content::OptionalContentVisibility>,
    ) -> PdfResult<AnnotatedDisplayList> {
        if !options.view_zoom.is_finite() || options.view_zoom <= 0. {
            return Err(invalid("annotation view zoom must be finite and positive"));
        }
        let geometry = self.geometry()?;
        let work = super::execution_budget::ExecutionBudget::new();
        let contents = self.display_list_for_annotation_composition(work.clone(),visibility)?;
        let page = self.pagedict()?;
        let Some(annots) = page.get("Annots") else {
            return Ok(AnnotatedDisplayList {
                contents,
                appearances: vec![],
            });
        };
        let annots = self
            .ctx
            .resolve(annots)?
            .into_array()
            .ok_or_else(|| invalid("Annots must be an array"))?;
        let limits = self.ctx.limits();
        if annots.len() > limits.max_objects {
            return Err(PdfError::ResourceLimit("render annotation count".into()));
        }
        let mut appearances = Vec::new();
        let mut total_objects = contents.objects().len();
        let (mut total_bytes, mut total_segments) = contents.collection_usage();
        for annot in &annots {
            let dict = self
                .ctx
                .resolve(annot)?
                .into_dict()
                .ok_or_else(|| invalid("annotation dictionary expected"))?;
            if let Some(owner) = dict.get("P") {
                if self.page_node.as_object_id().is_some()
                    && owner.as_object_id() != self.page_node.as_object_id()
                {
                    return Err(invalid("annotation P does not match rendered page"));
                }
            }
            let subtype = self.ctx.resolve(
                dict.get("Subtype")
                    .ok_or_else(|| invalid("annotation Subtype missing"))?,
            )?;
            let subtype = subtype
                .as_name()
                .ok_or_else(|| invalid("annotation subtype name expected"))?
                .name();
            if subtype == "Popup" || (subtype == "Widget" && !options.include_widgets) {
                continue;
            }
            let flags = dict
                .get("F")
                .map(|v| {
                    self.ctx
                        .resolve(v)?
                        .as_number()
                        .ok_or_else(|| invalid("annotation flags integer expected"))?
                        .as_u32_checked()
                })
                .transpose()?
                .unwrap_or(0);
            let standard = matches!(
                subtype,
                "Text"
                    | "Link"
                    | "FreeText"
                    | "Line"
                    | "Square"
                    | "Circle"
                    | "Polygon"
                    | "PolyLine"
                    | "Highlight"
                    | "Underline"
                    | "Squiggly"
                    | "StrikeOut"
                    | "Stamp"
                    | "Caret"
                    | "Ink"
                    | "Popup"
                    | "FileAttachment"
                    | "Sound"
                    | "Movie"
                    | "Widget"
                    | "Screen"
                    | "PrinterMark"
                    | "TrapNet"
                    | "Watermark"
                    | "3D"
                    | "Redact"
            );
            if flags & 2 != 0
                || (flags & 1 != 0 && !standard)
                || match options.intent {
                    AnnotationRenderIntent::Screen => flags & 32 != 0,
                    AnnotationRenderIntent::Print => flags & 4 == 0,
                }
            {
                continue;
            }
            if let Some(value) = dict.get("OC") {
                let visibility = visibility.ok_or_else(|| PdfError::UnsupportedFeature("annotation optional-content visibility requires an explicit selection".into()))?;
                if !visibility.visible(self.ctx,value)? { continue; }
            }
            let Some(ap) = dict.get("AP") else {
                if subtype == "Link" {
                    continue;
                }
                return Err(PdfError::UnsupportedFeature(format!(
                    "visible {subtype} annotation has no normal appearance"
                )));
            };
            let ap = self
                .ctx
                .resolve(ap)?
                .into_dict()
                .ok_or_else(|| invalid("annotation AP must be dictionary"))?;
            let normal = ap
                .get("N")
                .ok_or_else(|| invalid("annotation normal appearance missing"))?;
            let normal = self.ctx.resolve(normal)?;
            let normal = if let Some(states) = normal.as_dict() {
                let state = self.ctx.resolve(
                    dict.get("AS")
                        .ok_or_else(|| invalid("stateful AP requires AS"))?,
                )?;
                let state = state
                    .as_name()
                    .ok_or_else(|| invalid("annotation AS must be name"))?
                    .name();
                self.ctx.resolve(
                    states
                        .get(state)
                        .ok_or_else(|| invalid("AS selects missing appearance state"))?,
                )?
            } else {
                normal
            };
            if normal.as_stream().is_none() {
                return Err(invalid("normal appearance must be Form stream"));
            }
            let form = PdfForm::try_new(&normal, self.ctx)?;
            let transformed = form.matrix.checked_transform_rect(&form.bounds)?;
            let rect = crate::geom::rectangle::Rectangle::try_from_with_context(
                dict.get("Rect")
                    .ok_or_else(|| invalid("annotation Rect missing"))?,
                self.ctx,
            )?;
            let (w, h) = (rect.urx() - rect.llx(), rect.ury() - rect.lly());
            let (fw, fh) = (
                transformed.urx() - transformed.llx(),
                transformed.ury() - transformed.lly(),
            );
            if ![w, h, fw, fh].iter().all(|v| v.is_finite() && *v > 0.) {
                return Err(invalid("degenerate AP/annotation bounds"));
            }
            let (sx, sy) = (w / fw, h / fh);
            let mut placement = Matrix::new(
                sx,
                0.,
                0.,
                sy,
                rect.llx() - transformed.llx() * sx,
                rect.lly() - transformed.lly() * sy,
            );
            let no_zoom = flags & 8 != 0 && options.intent == AnnotationRenderIntent::Screen;
            let no_rotate = flags & 16 != 0;
            if (no_rotate && geometry.rotation != 0) || no_zoom {
                let anchor = geometry
                    .user_to_display
                    .checked_transform(&crate::geom::point::Point::new(rect.llx(), rect.ury()))?;
                let (mut c, mut s) = match if no_rotate { geometry.rotation } else { 0 } {
                    0 => (1., 0.),
                    90 => (0., -1.),
                    180 => (-1., 0.),
                    270 => (0., 1.),
                    _ => return Err(invalid("page rotation")),
                };
                if no_zoom {
                    c /= options.view_zoom;
                    s /= options.view_zoom;
                }
                let counter = Matrix::new(
                    c,
                    s,
                    -s,
                    c,
                    anchor.x() - c * anchor.x() + s * anchor.y(),
                    anchor.y() - s * anchor.x() - c * anchor.y(),
                );
                placement = placement
                    .checked_mul(&geometry.user_to_display)?
                    .checked_mul(&counter)?
                    .checked_mul(&geometry.user_to_display.checked_inverse()?)?;
            }
            if !placement.is_finite() {
                return Err(invalid("AP placement matrix overflow"));
            }
            // A temporary page invokes a direct Form, retaining BBox clipping,
            // local resources, Matrix, groups, masks and nested Form behavior.
            let mut resources = self.resource()?;
            let mut xobjects = match resources.get("XObject") {
                Some(v) => self
                    .ctx
                    .resolve(v)?
                    .into_dict()
                    .ok_or_else(|| invalid("page XObject resources must be dictionary"))?,
                None => PdfDict::default(),
            };
            if (&xobjects).into_iter().count() >= limits.max_objects {
                return Err(PdfError::ResourceLimit("AP resource scope entries".into()));
            }
            let mut serial = 0usize;
            let resource_name = loop {
                let candidate = format!("RzAnnotation{serial}");
                if xobjects.get(&candidate).is_none() {
                    break candidate;
                }
                serial += 1;
            };
            xobjects.try_insert(resource_name.clone(), normal)?;
            resources.try_insert("XObject".into(), PdfObject::PdfDict(xobjects))?;
            let mut synthetic = PdfDict::default();
            synthetic.try_insert("Type".into(), name("Page"))?;
            for key in ["MediaBox", "CropBox", "Rotate"] {
                if let Some(value) = self.get_pageattr(key)? {
                    synthetic.try_insert(key.into(), value)?;
                }
            }
            if let Some(value) = page.get("UserUnit") {
                synthetic.try_insert("UserUnit".into(), value.clone())?;
            }
            synthetic.try_insert("Resources".into(), PdfObject::PdfDict(resources))?;
            let bytes = format!(
                "q {} {} {} {} {} {} cm /{resource_name} Do Q",
                placement.a(),
                placement.b(),
                placement.c(),
                placement.d(),
                placement.e(),
                placement.f()
            )
            .into_bytes();
            synthetic.try_insert(
                "Contents".into(),
                PdfObject::PdfStream(PdfStream::from_decoded_data(PdfDict::default(), bytes)?),
            )?;
            let mut temporary = PdfPage::new(PdfObject::PdfDict(synthetic), self.ctx);
            let mut list = temporary.display_list_for_annotation_composition(work.clone(),visibility)?;
            list.clear_edit_provenance();
            let (bytes, segments) = list.collection_usage();
            total_bytes = total_bytes
                .checked_add(bytes)
                .ok_or_else(|| PdfError::ResourceLimit("annotation retained bytes".into()))?;
            total_segments = total_segments
                .checked_add(segments)
                .ok_or_else(|| PdfError::ResourceLimit("annotation retained segments".into()))?;
            if total_bytes > limits.max_decoded_bytes || total_segments > limits.max_path_segments {
                return Err(PdfError::ResourceLimit(
                    "annotation retained display payload".into(),
                ));
            }
            total_objects = total_objects
                .checked_add(list.objects().len())
                .ok_or_else(|| PdfError::ResourceLimit("annotation render objects".into()))?;
            if total_objects > limits.max_objects {
                return Err(PdfError::ResourceLimit("annotation render objects".into()));
            }
            appearances.push(list);
        }
        Ok(AnnotatedDisplayList {
            contents,
            appearances,
        })
    }
}
