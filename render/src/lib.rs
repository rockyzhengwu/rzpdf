pub mod render_device;

pub mod font_provider;

pub mod font_preflight;

/// Set process-wide Skia cache limits for an isolated WebAssembly module.
/// Strike bytes exclude the referenced font program and FreeType face, so a
/// byte limit alone does not bound repeated memory-font renders. Explicit
/// registered faces remain owned by their provider independently of this cache.
pub fn configure_isolated_web_caches() {
    skia_safe::graphics::set_font_cache_limit(2 * 1024 * 1024);
    skia_safe::graphics::set_font_cache_count_limit(2);
    skia_safe::graphics::set_typeface_cache_count_limit(0);
    skia_safe::graphics::set_resource_cache_total_bytes_limit(4 * 1024 * 1024);
}

/// Release unused process-wide backend caches when an isolated module disposes.
pub fn purge_isolated_web_caches() {
    skia_safe::graphics::purge_all_caches();
}
