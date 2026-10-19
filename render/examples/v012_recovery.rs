//! Read-only recovery rendering and diagnostics; creates a new directory.
use pdf::{
    document::{Document, recovery::RecoveredDocument},
    parser::syntax::ParseMode,
};
use render::render_device::SkiaRender;
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err("INPUT OUTPUT_DIRECTORY strict|recover".into());
    }
    if std::fs::metadata(&args[0])?.len() > 64 * 1024 * 1024 {
        return Err("input limit".into());
    }
    let bytes = std::fs::read(&args[0])?;
    let lists = match args[2].as_str() {
        "strict" => {
            let doc = Document::from_bytes_with_mode(bytes, ParseMode::Strict)?;
            let mut lists = Vec::new();
            for i in 0..doc.try_page_count()? {
                lists.push(doc.get_page(i as u32)?.display_list()?);
            }
            lists
        }
        "recover" => {
            let doc = RecoveredDocument::from_bytes(bytes)?;
            println!(
                "provenance={:?} scan_steps={} diagnostics={:?}",
                doc.provenance(),
                doc.scan_steps(),
                doc.diagnostics()
            );
            let mut lists = Vec::new();
            for i in 0..doc.try_page_count()? {
                lists.push(doc.display_list(i as u32)?);
            }
            lists
        }
        _ => return Err("unknown mode".into()),
    };
    std::fs::create_dir(&args[1])?;
    for (i, list) in lists.iter().enumerate() {
        let mut device = SkiaRender::new(72., 72.)?;
        list.replay(&mut device)?;
        device.save(
            std::path::Path::new(&args[1])
                .join(format!("page-{}.png", i + 1))
                .to_str()
                .ok_or("output encoding")?,
        )?;
    }
    println!("pages={}", lists.len());
    Ok(())
}
