fn main() {
    println!("cargo:rerun-if-changed=native");
    println!("cargo:rustc-check-cfg=cfg(rzpdf_system_font_manager)");
    let mut custom_skia = false;
    for name in [
        "SKIA_DEBUG",
        "SKIA_SOURCE_DIR",
        "SKIA_LIBRARY_SEARCH_PATH",
        "SKIA_GN_ARGS",
    ] {
        println!("cargo:rerun-if-env-changed={name}");
        custom_skia |= std::env::var(name).is_ok_and(|value| name != "SKIA_DEBUG" || value != "0");
    }
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    // The public C++ factory returns sk_sp, whose ABI depends on Skia's GN
    // settings. The version-pinned default release binaries match these
    // headers; custom Skia builds retain the existing system-manager path.
    if custom_skia {
        println!("cargo:rustc-cfg=rzpdf_system_font_manager");
        return;
    }
    cc::Build::new()
        .cpp(true)
        .std("c++17")
        .include("native/skia-headers")
        .define("SK_BUILD_FOR_MAC", None)
        .define("SK_RELEASE", None)
        .file("native/font_manager_mac.cpp")
        .compile("rzpdf_coretext_fonts");
    println!("cargo:rustc-link-lib=framework=CoreText");
    println!("cargo:rustc-link-lib=framework=CoreFoundation");
}
