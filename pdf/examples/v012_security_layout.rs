//! Synthetic interoperability driver. Credentials use host environment, never
//! argv or logs. The example deliberately creates a new output file.
use pdf::{
    document::{
        Document,
        save_options::{SaveLayout, SaveOptions, StreamCompression},
    },
    parser::{options::ParseOptions, syntax::ParseMode},
    security::{EncryptionOptions, SaveSecurity},
};
use std::{fs::OpenOptions, io::Write};
use zeroize::Zeroizing;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(4..=5).contains(&args.len()) {
        return Err(
            "INPUT OUTPUT classic|stream|objects preserve|remove|set|set-clear [flate]".into(),
        );
    }
    let layout = match args[2].as_str() {
        "classic" => SaveLayout::Classic,
        "stream" => SaveLayout::XrefStream,
        "objects" => SaveLayout::XrefStreamWithObjectStreams,
        _ => return Err("unknown layout".into()),
    };
    let input = Zeroizing::new(std::env::var("RZPDF_INPUT_PASSWORD").unwrap_or_default());
    if std::fs::metadata(&args[0])?.len() > 512 * 1024 * 1024 {
        return Err("example input limit".into());
    }
    let doc = Document::from_bytes_with_password_and_options(
        std::fs::read(&args[0])?,
        &input,
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )?;
    // Optional synthetic fixture mutation, never enabled by ordinary usage.
    // It allows independent checks of string-vs-stream Identity defaults.
    if let Ok(title) = std::env::var("RZPDF_FIXTURE_TITLE") {
        doc.set_info_text(pdf::document::metadata::InfoTextField::Title, Some(&title))?;
    }
    let settings = if matches!(args[3].as_str(), "set" | "set-clear") {
        let user = Zeroizing::new(
            std::env::var("RZPDF_OUTPUT_USER_PASSWORD")
                .map_err(|_| "missing output user credential")?,
        );
        let owner = Zeroizing::new(
            std::env::var("RZPDF_OUTPUT_OWNER_PASSWORD")
                .map_err(|_| "missing output owner credential")?,
        );
        let mut settings = EncryptionOptions::new(&user, &owner)?;
        settings.encrypt_metadata = args[3] != "set-clear";
        Some(settings)
    } else {
        None
    };
    let policy = match args[3].as_str() {
        "preserve" => SaveSecurity::Preserve,
        "remove" => SaveSecurity::Remove,
        "set" | "set-clear" => SaveSecurity::Set(settings.as_ref().unwrap()),
        _ => return Err("unknown protection policy".into()),
    };
    let compression = match args.get(4).map(String::as_str) {
        None => StreamCompression::Preserve,
        Some("flate") => StreamCompression::FlateUnencoded,
        _ => return Err("unknown compression".into()),
    };
    let stamp = doc.stamp();
    let output = doc.save_to_bytes_with_layout_and_security(
        SaveOptions {
            compression,
            ..Default::default()
        },
        layout,
        policy,
    )?;
    assert!(doc.is_stamp_current(&stamp));
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    file.write_all(&output)?;
    file.sync_all()?;
    println!("bytes={}", output.len());
    Ok(())
}
