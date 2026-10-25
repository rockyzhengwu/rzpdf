//! Browser policy ceilings and observable, non-RSS cache estimates.
use super::*;
use serde_json::{Value, json};

pub const MAX_FULL_PAGE_DPI: f32 = 288.;
pub const MAX_REGION_DPI: f32 = 1152.;

pub fn capabilities(limits: Limits) -> Value {
    json!({"abi":2,"renderRegion":true,"regionRasterStatus":"experimental",
        "maxDpi":MAX_FULL_PAGE_DPI,"maxFullPageDpi":MAX_FULL_PAGE_DPI,"maxRegionDpi":MAX_REGION_DPI,
        "maxRegionSide":1024,"maxRasterSide":32768,
        "maxRenderPixels":limits.max_render_pixels,"maxRenderBytes":limits.max_render_bytes,
        "maxImagePixels":limits.max_image_pixels,"maxDecodedBytes":limits.max_decoded_bytes,
        "maxOperationBytes":limits.max_operation_bytes,"maxCachedStreamBytes":limits.max_cached_stream_bytes,
        "maxIccTransformEstimatedBytes":pdf::color::icc_profile::MAX_WASM_ICC_TRANSFORM_ESTIMATED_BYTES,
        "maxHandles":8,"maxPreparedPages":2,
        "maxPreparedPageEstimatedBytes":limits.max_operation_bytes.min(32*1024*1024),
        "maxFontCacheEstimatedBytes":limits.max_operation_bytes.min(16*1024*1024),
        "maxInputBytes":null,"maxBufferBytes":BUFFER_LIMIT,"maxJsonBytes":16*1024*1024,"maxWasmMemoryBytes":256*1024*1024,
        "effectiveProfile":if limits.max_operation_bytes==32*1024*1024 {"conservative"} else {"default"},
        "resourceProfiles":["default","conservative"],"cacheStatistics":true,
        "imageDecode":{"jpegTargetResolution":true,"jpxTargetResolution":false,
        "maxJpegSourcePixels":16000000,"jpegProcesses":["sequential"],"maskedReduction":false,
        "policy":"target-if-full-decode-exceeds-limits",
        "fallback":"bounded-full-resolution-or-error"}})
}

pub fn profile(name: &str) -> ApiResult<Limits> {
    let mut policy = limits();
    match name {
        "default" => {}
        "conservative" => {
            policy.max_image_pixels = 2_000_000;
            policy.max_decoded_bytes = 8 * 1024 * 1024;
            policy.max_operation_bytes = 32 * 1024 * 1024;
            policy.max_render_pixels = 2_000_000;
            policy.max_render_bytes = 8_000_000;
            policy.max_cached_stream_bytes = 8 * 1024 * 1024;
        }
        _ => return Err("unknown resource profile".into()),
    }
    Ok(policy)
}

pub fn cache_statistics(document: &Document) -> Value {
    let c = document.render_cache_statistics();
    let [font_bytes, font_limit, resource_bytes, resource_limit] =
        render::backend_cache_statistics();
    json!({"pageHits":c.page_hits,"pageMisses":c.page_misses,"pageEntries":c.page_entries,
        "pageEstimatedBytes":c.page_estimated_bytes,"fontHits":c.font_hits,"fontMisses":c.font_misses,
        "fontEntries":c.font_entries,"fontEstimatedBytes":c.font_estimated_bytes,
        "byteAccounting":"estimated-owned-storage-not-rss","linearMemoryMayRemainGrown":true,
        "callerBuffersReleased":false,"skiaCacheScope":"module-on-wasm",
        "skiaFontBytes":font_bytes,"skiaFontLimit":font_limit,
        "skiaResourceBytes":resource_bytes,"skiaResourceLimit":resource_limit})
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::api::{rz_open_with_options, rz_query};

    fn input(value: &[u8]) -> u32 {
        STATE.with(|s| {
            s.borrow_mut()
                .insert(Resource::Input(value.to_vec()))
                .unwrap()
        })
    }
    fn json_input(value: Value) -> u32 {
        input(&serde_json::to_vec(&value).unwrap())
    }
    fn read(document: u32, operation: &str) -> Value {
        let request = json_input(json!({"operation":operation}));
        let result = rz_query(document, request);
        assert_ne!(result, 0);
        let value = STATE.with(|s| {
            serde_json::from_slice(s.borrow().resources[&result].bytes().unwrap()).unwrap()
        });
        assert_eq!(rz_release(request), 1);
        assert_eq!(rz_release(result), 1);
        value
    }
    fn document(profile: &str) -> u32 {
        let doc = Document::new().unwrap();
        doc.add_page(0).unwrap();
        doc.replace_page_contents(0, b"0 0 1000 1000 re f".to_vec())
            .unwrap();
        let source = input(&doc.save_to_bytes().unwrap());
        let options = json_input(json!({"resourceProfile":profile}));
        let result = rz_open_with_options(source, options);
        // Read errors before releasing request handles (which clears error state).
        if profile == "invalid" {
            assert_eq!(result, 0);
            assert_eq!(crate::api::rz_error_code(), 9);
        } else {
            assert_ne!(result, 0);
        }
        rz_release(source);
        rz_release(options);
        result
    }

    #[test]
    fn capabilities_before_open_and_immutable_profiles() {
        let caps = read(0, "capabilities");
        assert_eq!(caps["maxRegionDpi"], 1152.);
        assert_eq!(caps["maxFullPageDpi"], 288.);
        assert_eq!(caps["maxWasmMemoryBytes"], 256 * 1024 * 1024);
        assert_eq!(document("invalid"), 0);
        let doc = document("conservative");
        let caps = read(doc, "capabilities");
        assert_eq!(caps["maxImagePixels"], 2_000_000);
        assert_eq!(caps["maxDecodedBytes"], 8 * 1024 * 1024);
        assert_eq!(read(0, "capabilities")["maxImagePixels"], 4_000_000);
        rz_release(doc);
    }

    #[test]
    fn high_dpi_tiles_cache_release_and_structured_output_errors() {
        let doc = document("default");
        for dpi in [576., 1152.] {
            let image = rz_render_region(doc, 0, dpi, 0, 1., 0, 0, 64, 64);
            assert_ne!(image, 0);
            assert_eq!(rz_image_width(image), 64);
            STATE.with(|s| {
                assert!(
                    s.borrow().resources[&image]
                        .bytes()
                        .unwrap()
                        .as_chunks::<4>()
                        .0
                        .iter()
                        .all(|p| *p == [0, 0, 0, 255])
                )
            });
            let next = rz_render_region(doc, 0, dpi, 0, 1., 8, 8, 32, 32);
            assert_ne!(next, 0);
            rz_release(next);
            let before = read(doc, "cacheStatistics");
            assert!(before["pageEntries"].as_u64().unwrap() > 0);
            assert!(before["pageHits"].as_u64().unwrap() > 0);
            let after = read(doc, "clearRenderCaches");
            assert_eq!(after["pageEntries"], 0);
            // Cache release does not release caller-owned output images.
            assert_eq!(rz_image_width(image), 64);
            rz_release(image);
        }
        for dpi in [f32::NAN, f32::INFINITY, 0., 1153.] {
            assert_eq!(rz_render_region(doc, 0, dpi, 0, 1., 0, 0, 1, 1), 0);
            assert_eq!(crate::api::rz_error_code(), 9);
        }
        assert_eq!(rz_render(doc, 0, 576.), 0);
        assert_eq!(crate::api::rz_error_code(), 9);
        assert_eq!(rz_render(doc, 0, 288.), 0);
        assert_eq!(crate::api::rz_error_code(), 7);
        let error: Value = STATE.with(|s| serde_json::from_str(&s.borrow().error_details).unwrap());
        assert_eq!(error["reason"], "render_pixels");
        assert_eq!(error["stage"], "raster_output");
        assert_eq!(error["limit"], 4_000_000);
        assert!(error["actual"].as_u64().unwrap() > 4_000_000);
        assert!(rz_error_details_len() > 0);
        assert_ne!(rz_error_details_ptr(), 0);
        rz_release(doc);
    }

    #[test]
    fn typed_image_errors_do_not_require_string_matching() {
        let error = ApiError::from(pdf::error::PdfError::ResourceLimitExceeded {
            reason: "image_pixels",
            stage: "image_decode",
            limit: 4_000_000,
            actual: 8_750_000,
        });
        assert_eq!(error.code, 7);
        assert_eq!(error.details.unwrap()["actual"], 8_750_000);
        let unknown = ApiError::from(pdf::error::PdfError::ResourceLimit("legacy reason".into()));
        assert_eq!(
            unknown.details.unwrap(),
            json!({"reason":"unclassified","stage":"unknown"})
        );
    }
}
