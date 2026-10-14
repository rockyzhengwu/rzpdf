use std::io::{Read, Seek};

use super::{PdfPage, pdf_image::PdfImage};
use crate::{
    error::{PdfError, PdfResult},
    objects::pdf_stream::PdfStream,
};

/// An image XObject source snapshot, not a decoded pixel buffer.
/// The stream dictionary preserves Filter/DecodeParms and other source entries.
/// Indirect dictionary references still belong to the source document; this is
/// not a portable cross-document resource or an encoded standalone image file.
#[derive(Debug, Clone)]
pub struct ExtractedImageSource {
    pub resource_name: String,
    pub width: u32,
    pub height: u32,
    pub bits_per_component: Option<u32>,
    pub stream: PdfStream,
}

/// Images plus interpretation warnings. An empty diagnostic list is not proof
/// of universal PDF feature coverage, but warnings must not be silently ignored.
#[derive(Debug, Clone)]
pub struct ImageExtractionReport {
    pub images: Vec<super::image_object::ImageObject>,
    pub diagnostics: Vec<super::diagnostic::ContentDiagnostic>,
    provenance: super::source_provenance::PageSourceProvenance,
}

impl ImageExtractionReport {
    /// Document revision that produced these events. A reopened document differs.
    pub fn source_stamp(&self) -> &crate::document::identity::DocumentStamp {
        &self.provenance.stamp
    }

    /// Require the same document, revision and page before using draw indices.
    /// Offline programs and pages without an indirect identity cannot match.
    /// Two historical snapshots can still match; this does not prove either is
    /// current or validate nested leaf indices against the page's top-level ops.
    pub fn matches_program(&self, program: &super::content_program::ContentProgram) -> bool {
        self.provenance.matches_program(program)
    }
}

impl<R: Read + Seek> PdfPage<'_, R> {
    /// Read a top-level inline payload from a current, page-bound source snapshot.
    /// Nested draws use nested_content_program and the same Operator accessor.
    pub fn extract_inline_image_source(
        &self,
        program: &super::content_program::ContentProgram,
        operation: usize,
    ) -> PdfResult<super::inline_image::InlineImage> {
        if !self.source_provenance().matches_program(program) {
            return Err(PdfError::ObjectError("inline image source is foreign or stale".into()));
        }
        program.operations().get(operation)
            .and_then(|o| o.operator.inline_image()).cloned()
            .ok_or_else(|| PdfError::ObjectError("source operation is not an inline image".into()))
    }

    /// Return interpreted image draws in paint order, including repeated draws.
    /// The matrix maps the unit image square into cropped/rotated display space
    /// (top-left origin, y downward). Payloads are decoded samples, not sources.
    /// Coverage matches display_list: unsupported image/Group paths may fail.
    /// This convenience API discards diagnostics; prefer the report API when
    /// completeness matters, especially under compatible parsing.
    pub fn extract_images_paint_order(
        &mut self,
    ) -> PdfResult<Vec<super::image_object::ImageObject>> {
        // Convenience API intentionally drops warnings; use the report API for them.
        Ok(self.extract_images_paint_order_report()?.images)
    }

    /// Interpret once and retain both image draws and compatibility diagnostics.
    pub fn extract_images_paint_order_report(&mut self) -> PdfResult<ImageExtractionReport> {
        let provenance = self.source_provenance();
        let limits = self.ctx.limits();
        let mut images = Vec::new();
        let mut sample_bytes = 0usize;
        let (objects, diagnostics) = self.display_list()?.into_objects_with_diagnostics();
        for object in objects {
            let super::page_object::PageObject::Image(image) = object else {
                continue;
            };
            if images.len() >= limits.max_objects {
                return Err(PdfError::ResourceLimit("extracted image draws".into()));
            }
            let source_bytes = image
                .nested_source()
                .map(|source| source.retained_bytes())
                .transpose()?
                .unwrap_or(0);
            sample_bytes = sample_bytes
                .checked_add(image.image.retained_sample_bytes()?)
                .and_then(|bytes| bytes.checked_add(source_bytes))
                .filter(|total| *total <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("extracted image sample bytes".into()))?;
            images
                .try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("extracted image draws allocation".into()))?;
            images.push(image);
        }
        Ok(ImageExtractionReport {
            images,
            diagnostics,
            provenance,
        })
    }

    /// List direct page image XObject resource names, sorted lexicographically.
    /// Includes inherited and unused resources; does not traverse Form contents.
    /// Malformed XObject entries fail explicitly rather than silently disappearing.
    pub fn image_resource_names(&self) -> PdfResult<Vec<String>> {
        let resources = self.resource()?;
        let Some(objects) = resources.get("XObject") else {
            return Ok(Vec::new());
        };
        let objects = self.ctx.resolve(objects)?;
        let objects = objects.as_dict().ok_or(PdfError::PageXobjectIsNotDict)?;
        let limits = self.ctx.limits();
        let mut names = Vec::new();
        let mut name_bytes = 0usize;
        for (index, (name, object)) in objects.into_iter().enumerate() {
            if index >= limits.max_objects {
                return Err(PdfError::ResourceLimit(
                    "image resource scan entries".into(),
                ));
            }
            let object = self.ctx.resolve(object)?;
            let stream = object.as_stream().ok_or_else(|| {
                PdfError::PageResourceError("XObject resource must be a stream".into())
            })?;
            let subtype = stream
                .dict()
                .get("Subtype")
                .ok_or_else(|| PdfError::PageResourceError("XObject Subtype is missing".into()))?;
            let subtype = self.ctx.resolve(subtype)?;
            let subtype = subtype.as_name().ok_or_else(|| {
                PdfError::PageResourceError("XObject Subtype must be a name".into())
            })?;
            if subtype.name() != "Image" {
                continue;
            }
            if name.len() > limits.max_token_bytes {
                return Err(PdfError::ResourceLimit("image resource name bytes".into()));
            }
            name_bytes = name_bytes
                .checked_add(name.len())
                .filter(|total| *total <= limits.max_decoded_bytes)
                .ok_or_else(|| PdfError::ResourceLimit("image resource names bytes".into()))?;
            let mut owned = String::new();
            owned
                .try_reserve_exact(name.len())
                .map_err(|_| PdfError::ResourceLimit("image resource name allocation".into()))?;
            owned.push_str(name);
            names
                .try_reserve(1)
                .map_err(|_| PdfError::ResourceLimit("image resource names allocation".into()))?;
            names.push(owned);
        }
        names.sort_unstable();
        Ok(names)
    }

    /// Extract an image by its page resource name without decoding its filters.
    /// Includes unused named resources, but not images nested in Form XObjects
    /// or inline images. Raw bytes remain available for unsupported encodings.
    pub fn extract_image_source(&self, name: &str) -> PdfResult<ExtractedImageSource> {
        if name.len() > self.ctx.limits().max_token_bytes {
            return Err(PdfError::ResourceLimit(
                "extracted image resource name".into(),
            ));
        }
        let object = self.get_xobject(name)?;
        let stream = object
            .into_stream()
            .ok_or_else(|| PdfError::PageResourceError("image resource must be a stream".into()))?;
        let subtype = stream.dict().get("Subtype").ok_or_else(|| {
            PdfError::PageResourceError("image resource Subtype is missing".into())
        })?;
        let subtype = self.ctx.resolve(subtype)?;
        if subtype.as_name().map(|value| value.name()) != Some("Image") {
            return Err(PdfError::PageResourceError(
                "resource is not an image XObject".into(),
            ));
        }
        let integer = |key: &str| -> PdfResult<Option<u32>> {
            stream
                .dict()
                .get(key)
                .map(|value| {
                    let value = self.ctx.resolve(value)?;
                    value
                        .as_number()
                        .ok_or_else(|| {
                            PdfError::ObjectError(format!("image {key} must be an integer"))
                        })?
                        .as_u32_checked()
                        .map_err(|_| PdfError::ObjectError(format!("invalid image {key}")))
                })
                .transpose()
        };
        let width = integer("Width")?
            .filter(|value| *value > 0)
            .ok_or_else(|| PdfError::ObjectError("image Width must be positive".into()))?;
        let height = integer("Height")?
            .filter(|value| *value > 0)
            .ok_or_else(|| PdfError::ObjectError("image Height must be positive".into()))?;
        if stream.raw_data().len() > self.ctx.limits().max_raw_stream_bytes {
            return Err(PdfError::ResourceLimit(
                "extracted image source bytes".into(),
            ));
        }
        let bits_per_component = integer("BitsPerComponent")?;
        let mut resource_name = String::new();
        resource_name
            .try_reserve_exact(name.len())
            .map_err(|_| PdfError::ResourceLimit("image resource name allocation".into()))?;
        resource_name.push_str(name);
        Ok(ExtractedImageSource {
            resource_name,
            width,
            height,
            bits_per_component,
            stream,
        })
    }

    /// Decode a named image resource to the existing normalized sample model.
    /// `data()` returns normalized samples; `rgba_buffer()` also applies SMask.
    /// Unsupported decoding fails explicitly, independently of raw extraction.
    pub fn extract_decoded_image(&self, name: &str) -> PdfResult<PdfImage> {
        let source = self.extract_image_source(name)?;
        PdfImage::try_new(
            source.resource_name,
            &crate::objects::PdfObject::PdfStream(source.stream),
            self.ctx,
        )
    }
}
