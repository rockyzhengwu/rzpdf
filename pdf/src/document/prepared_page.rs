//! Reusable interpretation snapshots. Tile rectangles are not interpretation
//! inputs. Render-only reduced image decoding adds DPI to snapshot identity;
//! annotation magnification/rotation remain inputs as well.
use super::{PdfDocument, identity::DocumentStamp};
use crate::{
    device::Device,
    error::{PdfError, PdfResult},
    page::{
        annotations::{AnnotatedDisplayList, AnnotationRenderOptions},
        display_list::DisplayList,
    },
};
use std::{
    io::{Read, Seek},
    rc::Rc,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct PageKey {
    page: u32,
    annotations: Option<(bool, bool, u32, u16)>,
    image_dpi: Option<u32>,
}
#[derive(Debug)]
enum List {
    Contents(DisplayList),
    Annotated(AnnotatedDisplayList),
}
/// Immutable prepared page. Replay validates both document identity and revision.
/// Font substitutions are selected by the output device, not baked into pixels.
#[derive(Debug)]
pub struct PreparedPage {
    stamp: DocumentStamp,
    list: List,
    estimated_bytes: Option<usize>,
}
impl PreparedPage {
    pub fn source_stamp(&self) -> &DocumentStamp {
        &self.stamp
    }
    pub fn estimated_cache_bytes(&self) -> Option<usize> {
        self.estimated_bytes
    }
    pub fn replay<R: Read + Seek>(
        &self,
        document: &PdfDocument<R>,
        device: &mut dyn Device,
    ) -> PdfResult<()> {
        if !document.is_stamp_current(&self.stamp) {
            return Err(PdfError::InvalidReference(
                "prepared page belongs to a different document or revision".into(),
            ));
        }
        match &self.list {
            List::Contents(v) => v.replay(device),
            List::Annotated(v) => v.replay(device),
        }
    }
}
#[derive(Debug, Clone, Copy)]
pub struct RenderCacheStatistics {
    pub page_hits: u64,
    pub page_misses: u64,
    pub page_entries: usize,
    pub page_estimated_bytes: usize,
    pub font_hits: u64,
    pub font_misses: u64,
    pub font_entries: usize,
    pub font_estimated_bytes: usize,
}
impl<R: Read + Seek> PdfDocument<R> {
    /// Display text geometry with reduced incidental images and default layer
    /// visibility. Character data and source positions are not downsampled.
    pub fn extract_characters_for_display(
        &self,
        page: u32,
    ) -> PdfResult<crate::page::text_extraction::TextExtractionReport> {
        self.ctx().with_render_image_dpi(72., || {
            let mut page = self.get_page(page)?;
            if self.optional_content()?.is_some() {
                page.extract_characters_with_optional_content_report(
                    &self.optional_content_visibility(&Default::default())?,
                )
            } else {
                page.extract_characters_paint_order_report()
            }
        })
    }
    /// Render-only interpretation. DPI is part of the image cache identity;
    /// tile position is not. Extraction and editing keep full source precision.
    pub fn prepare_page_for_render(
        &self,
        page: u32,
        annotations: Option<AnnotationRenderOptions>,
        dpi: f32,
    ) -> PdfResult<Rc<PreparedPage>> {
        if !dpi.is_finite() || dpi <= 0.0 {
            return Err(PdfError::PageResourceError(
                "invalid image decode DPI".into(),
            ));
        }
        self.ctx()
            .with_render_image_dpi(dpi, || self.prepare_page(page, annotations))
    }
    /// Prepare the current default optional-content selection. Keeps at most two
    /// pages / 32 MiB of estimated owned Rust storage per document. Oversized or
    /// Type3 snapshots are returned without retention; parse errors are not cached.
    pub fn prepare_page(
        &self,
        page: u32,
        annotations: Option<AnnotationRenderOptions>,
    ) -> PdfResult<Rc<PreparedPage>> {
        if let Some(options) = annotations
            && (!options.view_zoom.is_finite()
                || options.view_zoom <= 0.
                || !matches!(options.view_rotation, 0 | 90 | 180 | 270))
        {
            return Err(PdfError::PageResourceError(
                "invalid prepared-page view options".into(),
            ));
        }
        let key = PageKey {
            page,
            image_dpi: self.ctx().render_image_dpi().map(f32::to_bits),
            annotations: annotations.map(|o| {
                (
                    o.intent == crate::page::annotations::AnnotationRenderIntent::Print,
                    o.include_widgets,
                    o.view_zoom.to_bits(),
                    o.view_rotation,
                )
            }),
        };
        if let Some(page) = self.prepared_pages.borrow_mut().get(self.revision(), &key) {
            return Ok(page);
        }
        let mut source = self.get_page(page)?;
        let visibility = self
            .optional_content()?
            .map(|_| self.optional_content_visibility(&Default::default()))
            .transpose()?;
        let list = match (annotations, visibility.as_ref()) {
            (Some(options), Some(visibility)) => List::Annotated(
                source.display_list_with_annotations_and_optional_content(options, visibility)?,
            ),
            (Some(options), None) => {
                List::Annotated(source.display_list_with_annotations(options)?)
            }
            (None, Some(visibility)) => {
                List::Contents(source.display_list_with_optional_content(visibility)?)
            }
            (None, None) => List::Contents(source.display_list()?),
        };
        let estimated_bytes = match &list {
            List::Contents(v) => v.cache_bytes(),
            List::Annotated(v) => v.cache_bytes(),
        };
        let result = Rc::new(PreparedPage {
            stamp: self.stamp(),
            list,
            estimated_bytes,
        });
        if let Some(bytes) = estimated_bytes {
            self.prepared_pages
                .borrow_mut()
                .insert(self.revision(), key, result.clone(), bytes);
        }
        Ok(result)
    }
    /// Releases cache-owned references; caller-held snapshots remain valid until
    /// the next edit and remain the caller's responsibility.
    pub fn clear_render_caches(&self) {
        self.prepared_pages.borrow_mut().clear();
        self.ctx.clear_derived_caches();
    }
    pub fn render_cache_statistics(&self) -> RenderCacheStatistics {
        let p = self.prepared_pages.borrow();
        let (fh, fm, fe, fb) = self.ctx.font_cache_statistics();
        RenderCacheStatistics {
            page_hits: p.hits,
            page_misses: p.misses,
            page_entries: p.len(),
            page_estimated_bytes: p.bytes(),
            font_hits: fh,
            font_misses: fm,
            font_entries: fe,
            font_estimated_bytes: fb,
        }
    }
}
