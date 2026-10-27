//! Separate font-backend attribution, never used for engine speed rankings.
use serde_json::json;
use skia_safe::{FontMgr, Paint, surfaces};
use std::time::Instant;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args().skip(1);
    let path = args.next().ok_or("expected font path [iterations]")?;
    let iterations: usize = args.next().map_or(Ok(5), |v| v.parse())?;
    if !(1..=1000).contains(&iterations) {
        return Err("iterations must be 1..1000".into());
    }
    let data = std::fs::read(path)?;
    let mut rows = Vec::new();
    for iteration in 0..iterations {
        let start = Instant::now();
        let manager = FontMgr::new();
        let manager_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let face = manager.new_from_data(&data, 0).ok_or("font rejected")?;
        let face_ms = start.elapsed().as_secs_f64() * 1000.;
        let start = Instant::now();
        let mut surface = surfaces::raster_n32_premul((128, 64)).ok_or("surface allocation")?;
        let font = skia_safe::Font::new(face, 18.);
        let glyph = font.unichar_to_glyph('H' as i32);
        surface
            .canvas()
            .draw_str("Hello PDF", (0, 24), &font, &Paint::default());
        let draw_ms = start.elapsed().as_secs_f64() * 1000.;
        rows.push(
            json!({"iteration":iteration,"manager_ms":manager_ms,"face_ms":face_ms,
                         "draw_ms":draw_ms,"families":manager.count_families(),"glyph":glyph}),
        );
    }
    let custom = FontMgr::custom_empty();
    let empty = FontMgr::empty();
    println!(
        "{}",
        json!({"rows":rows,"custom_empty_available":custom.is_some(),
        "empty_loads_font":empty.new_from_data(&data,0).is_some()})
    );
    Ok(())
}
