fn main() {
    let mut build = cc::Build::new();
    build.include("vendor/jbig2dec").warnings(false);
    for name in [
        "jbig2_arith",
        "jbig2_arith_int",
        "jbig2_arith_iaid",
        "jbig2_huffman",
        "jbig2_hufftab",
        "jbig2_segment",
        "jbig2_page",
        "jbig2_symbol_dict",
        "jbig2_text",
        "jbig2_halftone",
        "jbig2_generic",
        "jbig2_refinement",
        "jbig2_mmr",
        "jbig2_image",
        "jbig2",
    ] {
        build.file(format!("vendor/jbig2dec/{name}.c"));
    }
    build.compile("rzpdf_jbig2dec");
    println!("cargo:rerun-if-changed=vendor/jbig2dec");
}
