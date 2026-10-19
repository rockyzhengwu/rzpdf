//! Synthetic unsigned incremental interoperability driver; credentials are
//! read from the host environment, never argv. Creates a new output file.
use pdf::{
    document::{Document, metadata::InfoTextField, save_options::IncrementalSaveLayout},
    objects::{PdfObject, object_id::ObjectId, pdf_string::PdfString},
    parser::{options::ParseOptions, syntax::ParseMode},
};
use std::{fs::OpenOptions, io::Write};
use zeroize::Zeroizing;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 6 {
        return Err(
            "INPUT OUTPUT classic|stream TITLE|noop MEMBER_NUMBER|none MEMBER_GENERATION".into(),
        );
    }
    if std::fs::metadata(&args[0])?.len() > 512 * 1024 * 1024 {
        return Err("example input limit".into());
    }
    let password = Zeroizing::new(std::env::var("RZPDF_INPUT_PASSWORD").unwrap_or_default());
    let original = std::fs::read(&args[0])?;
    let doc = Document::from_bytes_with_password_and_options(
        original.clone(),
        &password,
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )?;
    let layout = match args[2].as_str() {
        "classic" => IncrementalSaveLayout::Classic,
        "stream" => IncrementalSaveLayout::XrefStream,
        _ => return Err("unknown layout".into()),
    };
    if args[3] != "noop" {
        doc.set_info_text(InfoTextField::Title, Some(&args[3]))?;
        if args[4] != "none" {
            doc.update_object(
                ObjectId::new(args[4].parse()?, args[5].parse()?),
                PdfObject::PdfString(PdfString::new(args[3].as_bytes().to_vec(), false)),
            )?;
        }
    }
    let stamp = doc.stamp();
    let revision = doc.commit_revision();
    let output = doc.save_incremental_to_bytes_with_layout(Default::default(), layout)?;
    assert!(output.starts_with(&original));
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(revision, doc.commit_revision());
    if args[3] == "noop" {
        assert_eq!(output, original);
    }
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    file.write_all(&output)?;
    file.sync_all()?;
    println!("bytes={}", output.len());
    Ok(())
}
