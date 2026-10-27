//! Backend resources outlive raster targets. Caches are optional, bounded and
//! contain immutable values; neither a document nor a surface is retained.
use super::*;
use pdf::{color::conversion::ColorConversionContext, document::identity::DocumentStamp};
use std::{
    cell::RefCell,
    collections::{BTreeMap, HashMap},
    hash::{Hash, Hasher},
    rc::{Rc, Weak},
};

#[derive(Clone)]
pub struct RenderSession(Rc<RefCell<Resources>>);

/// Estimates include cache keys and owned data, not Skia internals or RSS.
#[derive(Clone, Copy, Debug, Default)]
pub struct RenderSessionStatistics {
    pub estimated_bytes: usize,
    pub byte_limit: usize,
    pub font_entries: usize,
    pub font_manager_entries: usize,
    pub font_manager_hits: u64,
    pub image_entries: usize,
    pub geometry_entries: usize,
    pub font_hits: u64,
    pub image_hits: u64,
    pub geometry_hits: u64,
    pub misses: u64,
    pub evictions: u64,
}

const ENTRY_OVERHEAD: usize = 512;

struct Resources {
    scope: Option<(DocumentStamp, u64)>,
    cache: HashMap<Key, Entry>,
    order: BTreeMap<u64, Key>,
    statistics: RenderSessionStatistics,
    clock: u64,
}
#[derive(Clone, PartialEq, Eq, Hash)]
pub(super) enum Key {
    FontManager,
    Font(usize),
    Image(usize, [u32; 4], bool),
    Outline(PathKey),
    Coverage(PathKey, [i32; 8], bool),
}
#[derive(Clone)]
pub(super) struct PathKey {
    path: skia_safe::Path,
    hash: u64,
}
impl PathKey {
    pub(super) fn new(path: &skia_safe::Path) -> Self {
        let mut hash = std::collections::hash_map::DefaultHasher::new();
        path.verbs().hash(&mut hash);
        (path.fill_type() as u8).hash(&mut hash);
        for point in path.points() {
            // Equality treats -0 and +0 alike.
            for v in [point.x, point.y] {
                (if v == 0. { 0 } else { v.to_bits() }).hash(&mut hash);
            }
        }
        for weight in path.conic_weights() {
            weight.to_bits().hash(&mut hash);
        }
        Self {
            path: path.clone(),
            hash: hash.finish(),
        }
    }
}
impl PartialEq for PathKey {
    fn eq(&self, other: &Self) -> bool {
        self.hash == other.hash && self.path == other.path
    }
}
impl Eq for PathKey {}
impl Hash for PathKey {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.hash.hash(state);
    }
}
#[derive(Clone)]
enum Value {
    FontManager(skia_safe::FontMgr),
    Font(Weak<pdf::font::pdf_font::Font>, skia_safe::Typeface),
    Image(
        Weak<pdf::page::pdf_image::PdfImage>,
        ColorConversionContext,
        skia_safe::Image,
    ),
    Outline(skia_safe::Path, usize),
    Coverage(Rc<Vec<CoverageCell>>, usize),
}
struct Entry {
    value: Value,
    bytes: usize,
    used: u64,
}
pub(super) struct CoverageCell {
    pub x: i32,
    pub y: i32,
    pub rects: Vec<coverage_spans::CoverageRect>,
}

impl RenderSession {
    /// At most 16 MiB, and at most half the decode budget (8 MiB for the
    /// default browser profile). An oversized resource simply isn't cached.
    pub fn new(limits: Limits) -> Self {
        Self::with_cache_limit(limits, 16 * 1024 * 1024)
    }
    /// Restrict optional caching further; zero disables retention.
    pub fn with_cache_limit(limits: Limits, byte_limit: usize) -> Self {
        Self(Rc::new(RefCell::new(Resources {
            scope: None,
            cache: HashMap::new(),
            order: BTreeMap::new(),
            clock: 0,
            statistics: RenderSessionStatistics {
                byte_limit: (limits.max_decoded_bytes / 2)
                    .min(16 * 1024 * 1024)
                    .min(byte_limit),
                ..Default::default()
            },
        })))
    }
    /// Create a request-local target sharing this session's resources. Changing
    /// document, revision or fallback-font epoch invalidates cached resources.
    pub fn renderer(
        &self,
        document: &pdf::document::Document,
        font_epoch: u64,
        xdpi: f32,
        ydpi: f32,
    ) -> PdfResult<SkiaRender> {
        let mut renderer = SkiaRender::with_limits(xdpi, ydpi, document.resource_limits())?;
        let limit = self
            .0
            .borrow()
            .statistics
            .byte_limit
            .min(document.resource_limits().max_decoded_bytes / 2);
        if limit < self.0.borrow().statistics.byte_limit {
            self.clear();
            self.0.borrow_mut().statistics.byte_limit = limit;
        }
        self.bind(document.stamp(), font_epoch);
        renderer.session = self.clone();
        Ok(renderer)
    }
    fn bind(&self, stamp: DocumentStamp, font_epoch: u64) {
        let changed = self.0.borrow().scope.as_ref() != Some(&(stamp.clone(), font_epoch));
        if changed {
            self.clear();
            self.0.borrow_mut().scope = Some((stamp, font_epoch));
        }
    }
    /// Clears optional resources; caller-owned output and active targets remain valid.
    pub fn clear(&self) {
        let mut resources = self.0.borrow_mut();
        resources.cache = HashMap::new();
        resources.order.clear();
        resources.statistics.estimated_bytes = 0;
    }
    pub fn byte_limit(&self) -> usize {
        self.0.borrow().statistics.byte_limit
    }

    pub fn statistics(&self) -> RenderSessionStatistics {
        let r = self.0.borrow();
        let mut s = r.statistics;
        for key in r.cache.keys() {
            match key {
                Key::FontManager => s.font_manager_entries += 1,
                Key::Font(_) => s.font_entries += 1,
                Key::Image(..) => s.image_entries += 1,
                _ => s.geometry_entries += 1,
            }
        }
        s
    }
    fn get(&self, key: &Key) -> Option<Value> {
        let mut r = self.0.borrow_mut();
        r.clock = r.clock.saturating_add(1);
        let clock = r.clock;
        let result = r.cache.get_mut(key).map(|entry| {
            let old = entry.used;
            entry.used = clock;
            (entry.value.clone(), old)
        });
        if let Some((_, old)) = &result {
            r.order.remove(old);
            r.order.insert(clock, key.clone());
        } else {
            r.statistics.misses += 1;
        }
        result.map(|(value, _)| value)
    }
    fn insert(&self, key: Key, value: Value, payload_bytes: usize) {
        let key_bytes = match &key {
            Key::Outline(p) | Key::Coverage(p, ..) => p.path.approximate_bytes_used(),
            _ => 0,
        };
        let bytes = payload_bytes
            .saturating_add(key_bytes)
            .saturating_add(ENTRY_OVERHEAD);
        let mut r = self.0.borrow_mut();
        if bytes > r.statistics.byte_limit || r.cache.try_reserve(1).is_err() {
            return;
        }
        if let Some(old) = r.cache.remove(&key) {
            r.statistics.estimated_bytes -= old.bytes;
            r.order.remove(&old.used);
        }
        while r.statistics.estimated_bytes.saturating_add(bytes) > r.statistics.byte_limit
            || r.cache.len() >= 4096
        {
            let Some((_, oldest)) = r.order.pop_first() else {
                break;
            };
            let entry = r.cache.remove(&oldest).unwrap();
            r.statistics.estimated_bytes -= entry.bytes;
            r.statistics.evictions += 1;
        }
        r.clock = r.clock.saturating_add(1);
        let used = r.clock;
        r.order.insert(used, key.clone());
        r.cache.insert(key, Entry { value, bytes, used });
        r.statistics.estimated_bytes += bytes;
    }
    /// Backend initialization is lazy and happens inside the first font request.
    /// Reuse the manager for later faces without moving cold work out of timers.
    pub(super) fn font_manager(&self) -> skia_safe::FontMgr {
        if let Some(Value::FontManager(manager)) = self.get(&Key::FontManager) {
            self.0.borrow_mut().statistics.font_manager_hits += 1;
            return manager;
        }
        let manager = crate::memory_font_manager::new();
        // Opaque backend storage is an estimate, not an RSS limit. Charge an
        // allowance alongside the same LRU budget as font programs and images.
        self.insert(
            Key::FontManager,
            Value::FontManager(manager.clone()),
            32 * 1024,
        );
        manager
    }
    pub(super) fn font(&self, font: &Rc<pdf::font::pdf_font::Font>) -> Option<skia_safe::Typeface> {
        if let Some(Value::Font(source, face)) = self.get(&Key::Font(Rc::as_ptr(font) as usize))
            && source
                .upgrade()
                .is_some_and(|source| Rc::ptr_eq(&source, font))
        {
            self.0.borrow_mut().statistics.font_hits += 1;
            Some(face)
        } else {
            None
        }
    }
    pub(super) fn insert_font(
        &self,
        font: &Rc<pdf::font::pdf_font::Font>,
        face: skia_safe::Typeface,
        bytes: usize,
    ) {
        if bytes.saturating_add(ENTRY_OVERHEAD) > self.byte_limit() {
            return;
        }
        // Tiny font programs can hide sizable backend face allocations.
        // Preserve the original renderer's independent 128-face ceiling even
        // when compact CMaps allow more source fonts to fit the document cache.
        {
            let mut resources = self.0.borrow_mut();
            if resources
                .cache
                .keys()
                .filter(|key| matches!(key, Key::Font(_)))
                .count()
                >= 128
                && let Some(oldest) = resources
                    .cache
                    .iter()
                    .filter(|(key, _)| matches!(key, Key::Font(_)))
                    .min_by_key(|(_, entry)| entry.used)
                    .map(|(key, _)| key.clone())
            {
                let old = resources.cache.remove(&oldest).unwrap();
                resources.order.remove(&old.used);
                resources.statistics.estimated_bytes -= old.bytes;
                resources.statistics.evictions += 1;
            }
        }
        self.insert(
            Key::Font(Rc::as_ptr(font) as usize),
            Value::Font(Rc::downgrade(font), face),
            bytes,
        );
    }
    pub(super) fn image(
        &self,
        source: &Rc<pdf::page::pdf_image::PdfImage>,
        transform: [u32; 4],
        shape: bool,
        context: &ColorConversionContext,
    ) -> Option<skia_safe::Image> {
        if let Some(Value::Image(original, converted, image)) =
            self.get(&Key::Image(Rc::as_ptr(source) as usize, transform, shape))
            && original
                .upgrade()
                .is_some_and(|original| Rc::ptr_eq(&original, source))
            && converted.rendering_intent == context.rendering_intent
            && converted.icc_fallback == context.icc_fallback
            && converted
                .output_profile()
                .same_profile(context.output_profile())
        {
            self.0.borrow_mut().statistics.image_hits += 1;
            Some(image)
        } else {
            None
        }
    }
    pub(super) fn insert_image(
        &self,
        source: &Rc<pdf::page::pdf_image::PdfImage>,
        transform: [u32; 4],
        shape: bool,
        context: ColorConversionContext,
        image: skia_safe::Image,
    ) {
        let bytes = (image.width() as usize)
            .saturating_mul(image.height() as usize)
            .saturating_mul(4);
        // Include a conservative bound for mip levels (small dimensions too).
        let bytes = if image.has_mipmaps() {
            bytes.saturating_mul(2)
        } else {
            bytes
        };
        self.insert(
            Key::Image(Rc::as_ptr(source) as usize, transform, shape),
            Value::Image(Rc::downgrade(source), context, image),
            bytes,
        );
    }
    pub(super) fn normalized(
        &self,
        path: &skia_safe::Path,
        budget: &mut usize,
    ) -> PdfResult<skia_safe::Path> {
        if self.byte_limit() == 0 {
            return raster_path::normalized(path, budget);
        }
        let key = Key::Outline(PathKey::new(path));
        if let Some(Value::Outline(outline, work)) = self.get(&key) {
            *budget = budget
                .checked_sub(work)
                .ok_or_else(|| PdfError::ResourceLimit("raster curve work budget".into()))?;
            self.0.borrow_mut().statistics.geometry_hits += 1;
            return Ok(outline);
        }
        let before = *budget;
        let outline = raster_path::normalized(path, budget)?;
        self.insert(
            key,
            Value::Outline(outline.clone(), before - *budget),
            outline.approximate_bytes_used(),
        );
        Ok(outline)
    }
    pub(super) fn coverage_key(path: &skia_safe::Path, bounds: [i32; 8], antialias: bool) -> Key {
        Key::Coverage(PathKey::new(path), bounds, antialias)
    }
    pub(super) fn coverage(
        &self,
        key: &Key,
        budget: &mut usize,
    ) -> PdfResult<Option<Rc<Vec<CoverageCell>>>> {
        if let Some(Value::Coverage(cells, work)) = self.get(key) {
            *budget = budget
                .checked_sub(work)
                .ok_or_else(|| PdfError::ResourceLimit("canonical curve coverage work".into()))?;
            self.0.borrow_mut().statistics.geometry_hits += 1;
            Ok(Some(cells))
        } else {
            Ok(None)
        }
    }
    pub(super) fn insert_coverage(&self, key: Key, cells: Vec<CoverageCell>, work: usize) {
        let bytes = cells.capacity() * std::mem::size_of::<CoverageCell>()
            + cells
                .iter()
                .map(|c| c.rects.capacity() * std::mem::size_of::<coverage_spans::CoverageRect>())
                .sum::<usize>();
        self.insert(key, Value::Coverage(Rc::new(cells), work), bytes);
    }
}
