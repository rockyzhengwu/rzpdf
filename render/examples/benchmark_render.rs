//! JSON worker for scripts/benchmark_pdf.py. Timers exclude serialization and file output.
use pdf::{document::Document, parser::syntax::ParseMode};
use render::render_device::SkiaRender;
use serde_json::{Value, json};
use std::{io::Read, time::Instant};

#[cfg(unix)]
fn usage() -> Option<libc::rusage> {
    let mut value = std::mem::MaybeUninit::<libc::rusage>::uninit();
    // SAFETY: getrusage initializes this correctly sized output on success.
    (unsafe { libc::getrusage(libc::RUSAGE_SELF, value.as_mut_ptr()) } == 0)
        .then(|| unsafe { value.assume_init() })
}
fn cpu_ms() -> Option<f64> {
    #[cfg(unix)]
    {
        usage().map(|v| {
            (v.ru_utime.tv_sec + v.ru_stime.tv_sec) as f64 * 1000.
                + (v.ru_utime.tv_usec + v.ru_stime.tv_usec) as f64 / 1000.
        })
    }
    #[cfg(not(unix))]
    {
        None
    }
}
fn memory_snapshot() -> Value {
    let rss = std::process::Command::new("ps")
        .args(["-o", "rss=", "-p", &std::process::id().to_string()])
        .output()
        .ok()
        .filter(|v| v.status.success())
        .and_then(|v| String::from_utf8(v.stdout).ok())
        .and_then(|v| v.trim().parse::<u64>().ok())
        .map(|v| v * 1024);
    #[cfg(unix)]
    let peak =
        usage().map(|v| v.ru_maxrss as u64 * if cfg!(target_os = "macos") { 1 } else { 1024 });
    #[cfg(not(unix))]
    let peak = None::<u64>;
    json!({"rss_bytes":rss,"peak_rss_bytes":peak})
}
fn cpu_delta(start: Option<f64>, end: Option<f64>) -> Option<f64> {
    start.zip(end).map(|(a, b)| b - a)
}

fn run() -> Result<Value, Box<dyn std::error::Error>> {
    if cfg!(debug_assertions) {
        return Err("benchmark requires --release".into());
    }
    let mut input = String::new();
    std::io::stdin().read_to_string(&mut input)?;
    let job: Value = serde_json::from_str(&input)?;
    let path = job["path"].as_str().ok_or("missing path")?;
    let dpi = job["dpi"].as_f64().ok_or("missing dpi")? as f32;
    let repeats = job["repeats"].as_u64().ok_or("missing repeats")?;
    let warmup = job["warmup"].as_u64().ok_or("missing warmup")?;
    if repeats > 1000 || warmup > 100 || !dpi.is_finite() || !(1.0..=1200.).contains(&dpi) {
        return Err("invalid benchmark parameters".into());
    }
    let mut renderer = SkiaRender::new(dpi, dpi)?;
    let session = renderer.render_session();
    let fresh_renderer = job["fresh_renderer"].as_bool().unwrap_or(false);
    // Attribution only: force cold backend resources while keeping the same
    // prepared display list. Formal engine rankings never enable this flag.
    let clear_session_before_replay = job["clear_session_before_replay"]
        .as_bool()
        .unwrap_or(false);
    let image_policy = match job["image_decode"].as_str().unwrap_or("budgeted") {
        "budgeted" => pdf::document::prepared_page::ImageDecodePolicy::Budgeted,
        "target" => pdf::document::prepared_page::ImageDecodePolicy::TargetResolution,
        _ => return Err("invalid image_decode policy".into()),
    };
    if job["profile_prepare"].as_bool().unwrap_or(false)
        && image_policy != pdf::document::prepared_page::ImageDecodePolicy::Budgeted
    {
        return Err("profile_prepare currently requires budgeted image decoding".into());
    }
    let diagnostics = job["diagnostics"].as_bool().unwrap_or(false);
    let initialized_memory = diagnostics.then(memory_snapshot);
    let first_cpu_start = diagnostics.then(cpu_ms).flatten();
    let start = Instant::now();
    let input_mode = job["input_mode"].as_str().unwrap_or("file");
    let doc = match input_mode {
        "file" => Document::open_with_mode(path, ParseMode::Compatible)?,
        "memory" => Document::from_bytes_with_mode(std::fs::read(path)?, ParseMode::Compatible)?,
        _ => return Err("input_mode must be file or memory".into()),
    };
    let count = doc.try_page_count()?.min(5);
    let open_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    let (page, preparation_profile) = if job["profile_prepare"].as_bool().unwrap_or(false) {
        let (page, profile) = doc.profile_page_preparation(0, None, dpi)?;
        let phases: Vec<_> = pdf::page::preparation_profile::PreparationPhase::ALL.into_iter().map(|phase| {
            let measurement = profile.measurement(phase);
            json!({"phase": phase.name(), "ms": measurement.elapsed.as_secs_f64() * 1000., "calls": measurement.calls})
        }).collect();
        (page, Some(phases))
    } else {
        (
            doc.prepare_page_for_render_with_policy(0, None, dpi, image_policy)?,
            None,
        )
    };
    let prepare_ms = start.elapsed().as_secs_f64() * 1000.;
    let start = Instant::now();
    page.replay(&doc, &mut renderer)?;
    let first_replay_ms = start.elapsed().as_secs_f64() * 1000.;
    let readback = Instant::now();
    let image = renderer.to_rgba()?;
    let first_readback_ms = readback.elapsed().as_secs_f64() * 1000.;
    let raster_ms = start.elapsed().as_secs_f64() * 1000.;
    let first_cpu_ms = cpu_delta(first_cpu_start, diagnostics.then(cpu_ms).flatten());
    let first_memory = diagnostics.then(memory_snapshot);
    let width = image.width;
    let height = image.height;
    let mut outputs = Vec::new();
    if let Some(dir) = job["output_dir"].as_str() {
        let path = std::path::Path::new(dir).join("page-0.rgba");
        std::fs::write(&path, &image.pixels)?;
        outputs.push(json!({"page":0,"width":width,"height":height,"file":path}));
    }
    drop(image);
    let mut warm_ms = Vec::new();
    let mut replay_ms = Vec::new();
    let mut readback_ms = Vec::new();
    let replay_cpu_start = diagnostics.then(cpu_ms).flatten();
    let replay_wall = Instant::now();
    for i in 0..warmup + repeats {
        if clear_session_before_replay {
            session.clear();
        }
        let start = Instant::now();
        if fresh_renderer {
            renderer = session.renderer(&doc, 0, dpi, dpi)?;
        }
        page.replay(&doc, &mut renderer)?;
        let replay = start.elapsed().as_secs_f64() * 1000.;
        let readback = Instant::now();
        let image = renderer.to_rgba()?;
        let readback = readback.elapsed().as_secs_f64() * 1000.;
        let elapsed = start.elapsed().as_secs_f64() * 1000.;
        std::hint::black_box(&image);
        drop(image);
        if i >= warmup {
            warm_ms.push(elapsed);
            replay_ms.push(replay);
            readback_ms.push(readback);
        }
    }
    let replay_wall_ms = replay_wall.elapsed().as_secs_f64() * 1000.;
    let replay_cpu_ms = cpu_delta(replay_cpu_start, diagnostics.then(cpu_ms).flatten());
    let replay_memory = diagnostics.then(memory_snapshot);
    let mut next_ms = Vec::new();
    for index in 1..count {
        let start = Instant::now();
        let page =
            doc.prepare_page_for_render_with_policy(index as u32, None, dpi, image_policy)?;
        page.replay(&doc, &mut renderer)?;
        let image = renderer.to_rgba()?;
        next_ms.push(start.elapsed().as_secs_f64() * 1000.);
        if let Some(dir) = job["output_dir"].as_str() {
            let path = std::path::Path::new(dir).join(format!("page-{index}.rgba"));
            std::fs::write(&path, &image.pixels)?;
            outputs
                .push(json!({"page":index,"width":image.width,"height":image.height,"file":path}));
        }
    }
    Ok(
        json!({"status":"ok","engine":"rzpdf","profile":"release", "input_mode":input_mode,"image_decode":job["image_decode"].as_str().unwrap_or("budgeted"), "open_ms":open_ms,
        "preparation_profile":preparation_profile,"prepare_ms":prepare_ms,"raster_ms":raster_ms,"first_ms":open_ms+prepare_ms+raster_ms,
        "fresh_renderer":fresh_renderer,"clear_session_before_replay":clear_session_before_replay,"warm_ms":warm_ms,"warm_replay_ms":replay_ms,"warm_readback_ms":readback_ms,
        "diagnostics":diagnostics.then(|| json!({"initialized_memory":initialized_memory,
            "after_first_memory":first_memory,"after_replay_memory":replay_memory,
            "first_cpu_ms":first_cpu_ms,"replay_cpu_ms":replay_cpu_ms,"replay_wall_ms":replay_wall_ms,
            "replay_iterations_including_warmup":warmup+repeats})),
        "first_replay_ms":first_replay_ms,"first_readback_ms":first_readback_ms,"next_ms":next_ms,"outputs":outputs,"width":width,"height":height}),
    )
}

fn main() {
    let result = run().unwrap_or_else(|error| json!({"status":"error","error":error.to_string()}));
    println!("{result}");
}
