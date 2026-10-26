//! Diagnostic-only preparation worker. Build with pdf/prepare-diagnostics;
//! use benchmark_render without that feature for formal speed comparisons.
use pdf::{document::Document, parser::syntax::ParseMode};
use serde_json::{Value, json};
use std::io::Read;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    if !pdf::page::preparation_profile::PreparationProfile::available() {
        return Err("enable pdf/prepare-diagnostics to use profile_prepare".into());
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let job: Value = serde_json::from_str(&input)?;
    let path = job["path"].as_str().ok_or("missing path")?;
    let mode = job["input_mode"].as_str().unwrap_or("file");
    let repeats = job["repeats"].as_u64().unwrap_or(1);
    let dpi = job["dpi"].as_f64().unwrap_or(144.) as f32;
    if !(1..=1000).contains(&repeats) || !dpi.is_finite() || !(1.0..=1200.).contains(&dpi) {
        return Err("invalid repeats or DPI".into());
    }
    let doc = match mode {
        "file" => Document::open_with_mode(path, ParseMode::Compatible)?,
        "memory" => Document::from_bytes_with_mode(std::fs::read(path)?, ParseMode::Compatible)?,
        _ => return Err("input_mode must be file or memory".into()),
    };
    doc.try_page_count()?;
    let mut profiles = Vec::new();
    for _ in 0..repeats {
        doc.clear_render_caches();
        let (_, profile) = doc.profile_page_preparation(0, None, dpi)?;
        profiles.push(pdf::page::preparation_profile::PreparationPhase::ALL.into_iter().map(|phase| {
            let value = profile.measurement(phase);
            json!({"phase":phase.name(),"ms":value.elapsed.as_secs_f64()*1000.,"calls":value.calls})
        }).collect::<Vec<_>>());
    }
    println!(
        "{}",
        json!({"input_mode":mode,"profiles":profiles,
        "source_objects_reused_after_first":repeats>1,"diagnostics":true})
    );
    Ok(())
}
