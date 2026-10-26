//! Opt-in, exclusive preparation timings. Enable `prepare-diagnostics` and
//! explicitly call the profiling API. Default builds contain no clock reads.
use crate::{
    error::{PdfError, PdfResult},
    pdf_context::PDFContext,
};
use std::{
    io::{Read, Seek},
    time::Duration,
};

#[derive(Debug, Clone, Copy)]
#[repr(usize)]
pub enum PreparationPhase {
    PageSetup,
    ContentStreams,
    OperatorParsing,
    Fonts,
    Text,
    XObjects,
    Collection,
    OtherOperators,
    CacheAccounting,
}
impl PreparationPhase {
    pub const ALL: [Self; 9] = [
        Self::PageSetup,
        Self::ContentStreams,
        Self::OperatorParsing,
        Self::Fonts,
        Self::Text,
        Self::XObjects,
        Self::Collection,
        Self::OtherOperators,
        Self::CacheAccounting,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::PageSetup => "page_setup",
            Self::ContentStreams => "content_streams",
            Self::OperatorParsing => "operator_parsing",
            Self::Fonts => "fonts",
            Self::Text => "text",
            Self::XObjects => "xobjects",
            Self::Collection => "collection",
            Self::OtherOperators => "other_operators",
            Self::CacheAccounting => "cache_accounting",
        }
    }
}
#[derive(Debug, Default, Clone, Copy)]
pub struct PreparationMeasurement {
    pub elapsed: Duration,
    pub calls: u64,
}
#[derive(Debug, Default, Clone)]
pub struct PreparationProfile {
    pub phases: [PreparationMeasurement; 9],
}
impl PreparationProfile {
    pub const fn available() -> bool {
        cfg!(feature = "prepare-diagnostics")
    }
    pub fn measurement(&self, phase: PreparationPhase) -> PreparationMeasurement {
        self.phases[phase as usize]
    }
}

#[cfg(feature = "prepare-diagnostics")]
mod enabled {
    use super::*;
    use std::{cell::RefCell, time::Instant};
    #[derive(Debug)]
    pub(crate) struct Recorder {
        profile: PreparationProfile,
        active: PreparationPhase,
        since: Instant,
    }
    impl Recorder {
        pub(crate) fn new() -> Self {
            Self {
                profile: Default::default(),
                active: PreparationPhase::PageSetup,
                since: Instant::now(),
            }
        }
        fn switch(&mut self, next: PreparationPhase) -> PreparationPhase {
            let now = Instant::now();
            self.profile.phases[self.active as usize].elapsed += now - self.since;
            self.since = now;
            std::mem::replace(&mut self.active, next)
        }
        pub(crate) fn finish(mut self) -> PreparationProfile {
            self.switch(PreparationPhase::PageSetup);
            self.profile
        }
    }
    pub(crate) struct Span<'a> {
        recorder: &'a RefCell<Option<Recorder>>,
        previous: Option<PreparationPhase>,
    }
    impl<'a> Span<'a> {
        pub(crate) fn new(
            recorder: &'a RefCell<Option<Recorder>>,
            phase: PreparationPhase,
        ) -> Self {
            let previous = recorder.borrow_mut().as_mut().map(|r| {
                r.profile.phases[phase as usize].calls += 1;
                r.switch(phase)
            });
            Self { recorder, previous }
        }
    }
    impl Drop for Span<'_> {
        fn drop(&mut self) {
            if let Some(previous) = self.previous
                && let Some(recorder) = self.recorder.borrow_mut().as_mut()
            {
                recorder.switch(previous);
            }
        }
    }
}
#[cfg(feature = "prepare-diagnostics")]
pub(crate) use enabled::{Recorder, Span};
#[cfg(not(feature = "prepare-diagnostics"))]
pub(crate) struct Span<'a>(std::marker::PhantomData<&'a ()>);

impl<R: Read + Seek> PDFContext<R> {
    #[inline]
    pub(crate) fn preparation_span(&self, phase: PreparationPhase) -> Span<'_> {
        #[cfg(feature = "prepare-diagnostics")]
        {
            Span::new(&self.preparation_profile, phase)
        }
        #[cfg(not(feature = "prepare-diagnostics"))]
        {
            let _ = phase;
            Span(std::marker::PhantomData)
        }
    }
    pub(crate) fn profile_preparation<T>(
        &self,
        action: impl FnOnce() -> PdfResult<T>,
    ) -> PdfResult<(T, PreparationProfile)> {
        #[cfg(feature = "prepare-diagnostics")]
        {
            struct Reset<'a>(&'a std::cell::RefCell<Option<Recorder>>);
            impl Drop for Reset<'_> {
                fn drop(&mut self) {
                    self.0.borrow_mut().take();
                }
            }
            if self.preparation_profile.borrow().is_some() {
                return Err(PdfError::DocumentError(
                    "nested preparation profiling request".into(),
                ));
            }
            *self.preparation_profile.borrow_mut() = Some(Recorder::new());
            let _reset = Reset(&self.preparation_profile);
            let result = action()?;
            let profile = self
                .preparation_profile
                .borrow_mut()
                .take()
                .unwrap()
                .finish();
            Ok((result, profile))
        }
        #[cfg(not(feature = "prepare-diagnostics"))]
        {
            let _ = action;
            Err(PdfError::UnsupportedFeature(
                "build with prepare-diagnostics to profile page preparation".into(),
            ))
        }
    }
}

// Collection work is charged exclusively even when called from a text or
// nested-form operator. The wrapper exists only in diagnostic builds.
#[cfg(feature = "prepare-diagnostics")]
pub(crate) struct Collector<'a, R: Read + Seek> {
    pub context: &'a PDFContext<R>,
    pub device: &'a mut dyn crate::device::Device,
}
#[cfg(feature = "prepare-diagnostics")]
macro_rules! forward {
    ($name:ident $(,$arg:ident:$ty:ty)*) => {
        fn $name(&mut self $(,$arg:$ty)*) -> PdfResult<()> {
            let _span = self.context.preparation_span(PreparationPhase::Collection);
            self.device.$name($($arg),*)
        }
    };
}
#[cfg(feature = "prepare-diagnostics")]
impl<R: Read + Seek> crate::device::Device for Collector<'_, R> {
    forward!(start_page, width:f32, height:f32);
    forward!(end_page);
    forward!(begin_group, group:crate::page::transparency::GroupPaint);
    forward!(end_group, commit:bool);
    forward!(do_image, object:crate::page::image_object::ImageObject);
    forward!(show_text, object:crate::page::text_object::TextObject);
    forward!(stroke_path, object:crate::page::path_object::PathObject);
    forward!(fill_path, object:crate::page::path_object::PathObject);
    forward!(fill_and_stroke_path, object:crate::page::path_object::PathObject);
    forward!(clip_path, object:crate::page::path_object::PathObject);
    forward!(do_shading, object:crate::page::shading_object::ShadingObject);
    fn preflight_draw(
        &self,
        state: &crate::page::graphic_state::GraphicState,
        path: Option<&crate::path::pdf_path::PdfPath>,
    ) -> PdfResult<()> {
        let _span = self.context.preparation_span(PreparationPhase::Collection);
        self.device.preflight_draw(state, path)
    }
}

#[cfg(all(test, feature = "prepare-diagnostics"))]
mod tests {
    use super::*;
    use crate::document::{Document, resource_manager::StandardFont};
    use crate::page::page_builder::PageBuilder;

    #[test]
    fn profiling_partitions_nested_work_and_reports_cache_hits_without_changing_pages() {
        let doc = Document::new().unwrap();
        doc.add_page_with_builder(0, PageBuilder::new().size(40., 40.))
            .unwrap();
        let font = doc
            .resources()
            .register_standard_font(0, StandardFont::Helvetica)
            .unwrap();
        doc.replace_page_contents(
            0,
            format!("BT /{font} 12 Tf 2 20 Td (AB) Tj ET").into_bytes(),
        )
        .unwrap();
        let (page, first) = doc.profile_page_preparation(0, None, 72.).unwrap();
        assert!(PreparationProfile::available());
        assert_eq!(first.measurement(PreparationPhase::Fonts).calls, 1);
        assert_eq!(first.measurement(PreparationPhase::Text).calls, 1);
        assert!(first.measurement(PreparationPhase::Collection).calls > 0);
        let (cached, second) = doc.profile_page_preparation(0, None, 72.).unwrap();
        assert!(std::rc::Rc::ptr_eq(&page, &cached));
        assert_eq!(
            second.measurement(PreparationPhase::OperatorParsing).calls,
            0
        );
        assert_eq!(second.measurement(PreparationPhase::Fonts).calls, 0);
        assert!(doc.is_stamp_current(page.source_stamp()));
    }
    #[test]
    fn failed_and_unwound_profiles_restore_the_next_request() {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"BT (no font) Tj ET".to_vec())
            .unwrap();
        assert!(doc.profile_page_preparation(0, None, 72.).is_err());
        let panic = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let _: PdfResult<((), PreparationProfile)> =
                doc.ctx().profile_preparation(|| panic!("injected"));
        }));
        assert!(panic.is_err());
        doc.replace_page_contents(0, b"0 0 20 20 re f".to_vec())
            .unwrap();
        let (_, profile) = doc.profile_page_preparation(0, None, 72.).unwrap();
        assert!(profile.measurement(PreparationPhase::OtherOperators).calls > 0);
    }
}
