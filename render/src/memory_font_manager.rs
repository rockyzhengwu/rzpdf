//! Managers for loading supplied programs. Font selection remains explicit.
use skia_safe::FontMgr;

pub(crate) fn new() -> FontMgr {
    #[cfg(all(target_os = "macos", not(rzpdf_system_font_manager)))]
    if let Some(manager) = coretext() {
        return manager;
    }
    FontMgr::custom_empty().unwrap_or_default()
}

#[cfg(all(target_os = "macos", not(rzpdf_system_font_manager)))]
fn coretext() -> Option<FontMgr> {
    use skia_safe::wrapper::PointerWrapper;
    unsafe extern "C" {
        fn rzpdf_coretext_memory_font_manager() -> *mut skia_bindings::SkFontMgr;
    }
    // SAFETY: the bridge returns either null or a new owned SkFontMgr reference
    // from the version-pinned public factory. wrap takes ownership without
    // incrementing it; Skia's normal handle releases the reference on drop.
    FontMgr::wrap(unsafe { rzpdf_coretext_memory_font_manager() })
}

#[cfg(all(test, target_os = "macos", not(rzpdf_system_font_manager)))]
mod tests {
    use super::*;

    #[test]
    fn bounded_coretext_collection_loads_memory_fonts_without_family_substitution() {
        let manager = coretext().expect("bounded CoreText collection");
        assert_eq!(manager.count_families(), 1);
        let data = include_bytes!("../../pdf/tests/fixtures/v03/NotoSansSC-static-subset.ttf");
        let face = manager.new_from_data(data, 0).unwrap();
        assert_ne!(face.unichar_to_glyph('H' as i32), 0);
        assert!(manager.new_from_data(b"invalid program", 0).is_none());
    }
}
