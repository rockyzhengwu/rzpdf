//! One-document probe. The parent runner provides process isolation/timeouts.
use pdf::{
    document::{PdfDocument, prepared_page::ImageDecodePolicy},
    error::{PdfError, PdfResult},
    parser::options::{Limits, ParseOptions},
};
use render::render_device::SkiaRender;
use serde_json::{Value, json};
use std::io::{self, Write};

fn emit(value: Value) {
    println!("{value}");
    io::stdout().flush().unwrap();
}
fn error(error: PdfError) -> Value {
    let resource = match error.root_cause() {
        PdfError::ResourceLimitExceeded {
            reason,
            stage,
            limit,
            actual,
        } => json!({"reason":reason,"stage":stage,"limit":limit,"actual":actual}),
        _ => Value::Null,
    };
    json!({"status":"failed","message":error.to_string(),"resource":resource})
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if args.len() != 3 {
        return Err("usage: compatibility_probe INPUT.pdf OUTPUT_DIRECTORY".into());
    }
    std::fs::create_dir_all(&args[2])?;
    let limits = Limits {
        max_objects: 100_000,
        max_depth: 64,
        max_decoded_bytes: 16 * 1024 * 1024,
        max_image_source_bytes: 128 * 1024 * 1024,
        max_image_decode_work: 512 * 1024 * 1024,
        max_display_list_bytes: 32 * 1024 * 1024,
        max_operation_bytes: 64 * 1024 * 1024,
        max_image_pixels: 4_000_000,
        max_render_pixels: 4_000_000,
        max_render_bytes: 16_000_000,
        max_raw_stream_bytes: 64 * 1024 * 1024,
        max_cached_stream_bytes: 16 * 1024 * 1024,
        max_content_operations: 100_000,
        max_text_chars: 100_000,
        max_path_segments: 100_000,
        max_token_bytes: 1024 * 1024,
        ..Default::default()
    };
    let doc = match PdfDocument::open_with_options(
        &args[1],
        ParseOptions {
            limits,
            ..Default::default()
        },
    ) {
        Ok(doc) => doc,
        Err(e) => {
            emit(json!({"operation":"open","result":error(e)}));
            return Ok(());
        }
    };
    let pages = doc.try_page_count()?;
    emit(
        json!({"operation":"open","result":{"status":"passed","pages":pages},"fileBacked":true,"actualWasm":false}),
    );
    for index in 0..pages as u32 {
        emit(json!({"operation":"page-start","page":index}));
        let start = std::time::Instant::now();
        let rendered = (|| -> PdfResult<Value> {
            let geometry = doc.get_page(index)?.geometry()?;
            // A bounded 512px overview, not a high-DPI compatibility claim.
            let dpi = (512. * 72. / geometry.display_width.max(geometry.display_height)).clamp(1.,72.);
            let mut renderer = SkiaRender::with_limits(dpi,dpi,limits)?;
            doc.prepare_page_for_render_with_policy(index,None,dpi,ImageDecodePolicy::Budgeted)?.replay(&doc,&mut renderer)?;
            let image = renderer.to_rgba()?;
            let mut hash = 14695981039346656037u64;
            for byte in &image.pixels { hash = (hash ^ u64::from(*byte)).wrapping_mul(1099511628211); }
            renderer.save(&format!("{}/{index}.png",args[2]))?;
            Ok(json!({"status":"passed","dpi":dpi,"width":image.width,"height":image.height,"pixelFingerprint":format!("{hash:016x}")}))
        })().unwrap_or_else(error);
        emit(
            json!({"operation":"render","page":index,"result":rendered,"milliseconds":start.elapsed().as_millis()}),
        );
        let text = doc
            .extract_characters_for_display(index)
            .map(|report| json!({"status":"passed","characters":report.characters.len()}))
            .unwrap_or_else(error);
        emit(json!({"operation":"text","page":index,"result":text}));
        if index % 32 == 31 {
            doc.clear_render_caches();
            doc.clear_source_stream_cache();
        }
    }
    emit(json!({"operation":"complete","pages":pages}));
    Ok(())
}
