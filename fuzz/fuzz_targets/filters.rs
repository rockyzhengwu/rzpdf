#![no_main]
libfuzzer_sys::fuzz_target!(|data: &[u8]| rzpdf_fuzz::filters(data));
