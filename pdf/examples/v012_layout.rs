//! Strict input, full rewrite, create-new host output. No protection downgrade.
use pdf::{
    document::{
        Document,
        save_options::{SaveLayout, SaveOptions, StreamCompression},
    },
    parser::syntax::ParseMode,
};
use std::{fs::OpenOptions, io::Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().collect();
    if !(4..=5).contains(&args.len()) {
        return Err("usage: v012_layout INPUT OUTPUT classic|stream|objects [flate]".into());
    }
    let layout = match args[3].as_str() {
        "classic" => SaveLayout::Classic,
        "stream" => SaveLayout::XrefStream,
        "objects" => SaveLayout::XrefStreamWithObjectStreams,
        _ => return Err("unknown layout".into()),
    };
    let compression = match args.get(4).map(String::as_str) {
        None => StreamCompression::Preserve,
        Some("flate") => StreamCompression::FlateUnencoded,
        _ => return Err("unknown compression".into()),
    };
    let document = Document::from_bytes_with_mode(std::fs::read(&args[1])?, ParseMode::Strict)?;
    let output = document.save_to_bytes_with_layout(
        SaveOptions {
            compression,
            ..Default::default()
        },
        layout,
    )?;
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    file.write_all(&output)?;
    file.sync_all()?;
    println!("bytes={}", output.len());
    Ok(())
}
