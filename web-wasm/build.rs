fn main() {
    println!("cargo:rerun-if-changed=src/blob_source.c");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("emscripten") {
        cc::Build::new()
            .file("src/blob_source.c")
            .compile("rzpdf_blob_source");
    }
}
