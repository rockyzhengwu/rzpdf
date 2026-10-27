//! Opt-in inclusive replay timings for attribution, never engine rankings.
use std::{
    cell::RefCell,
    rc::Rc,
    time::{Duration, Instant},
};

#[derive(Clone, Copy, Debug)]
pub enum RenderPhase {
    Text,
    FontManager,
    TypefaceLoad,
    GlyphSelection,
    GlyphPaint,
    GlyphBatch,
    PreparedPath,
    CoverageRaster,
}
impl RenderPhase {
    pub const ALL: [Self; 8] = [
        Self::Text,
        Self::FontManager,
        Self::TypefaceLoad,
        Self::GlyphSelection,
        Self::GlyphPaint,
        Self::GlyphBatch,
        Self::PreparedPath,
        Self::CoverageRaster,
    ];
    pub fn name(self) -> &'static str {
        match self {
            Self::Text => "text",
            Self::FontManager => "font_manager",
            Self::TypefaceLoad => "typeface_load",
            Self::GlyphSelection => "glyph_selection",
            Self::GlyphPaint => "glyph_paint",
            Self::GlyphBatch => "glyph_batch",
            Self::PreparedPath => "prepared_path",
            Self::CoverageRaster => "coverage_raster",
        }
    }
}
#[derive(Clone, Copy, Debug, Default)]
pub struct Measurement {
    pub elapsed: Duration,
    pub calls: u64,
}
#[derive(Clone, Default)]
pub struct RenderProfile(Rc<RefCell<[Measurement; 8]>>);
impl RenderProfile {
    pub fn measurement(&self, phase: RenderPhase) -> Measurement {
        self.0.borrow()[phase as usize]
    }
    pub(crate) fn span(&self, phase: RenderPhase) -> Span {
        Span {
            profile: self.clone(),
            phase,
            start: Instant::now(),
        }
    }
}
pub(crate) struct Span {
    profile: RenderProfile,
    phase: RenderPhase,
    start: Instant,
}
impl Drop for Span {
    fn drop(&mut self) {
        let elapsed = self.start.elapsed();
        let mut measurements = self.profile.0.borrow_mut();
        let measurement = &mut measurements[self.phase as usize];
        measurement.elapsed += elapsed;
        measurement.calls += 1;
    }
}
