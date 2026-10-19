//! Synthetic preservation gate driver. Never overwrites an existing output.
use pdf::{
    document::{
        Document,
        metadata::InfoTextField,
        save_options::{IncrementalSaveLayout, PdfVersion, SaveLayout},
    },
    parser::{options::ParseOptions, syntax::ParseMode},
};
use std::{fs::OpenOptions, io::Write};
use zeroize::Zeroizing;

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 {
        return Err("INPUT OUTPUT preserve|edit|copy|delete|import|select|delete-retry|import-retry classic|stream|objects|incremental-classic|incremental-stream".into());
    }
    if std::fs::metadata(&args[0])?.len() > 512 * 1024 * 1024 {
        return Err("example input limit".into());
    }
    let password = Zeroizing::new(std::env::var("RZPDF_INPUT_PASSWORD").unwrap_or_default());
    let source = Document::from_bytes_with_password_and_options(
        std::fs::read(&args[0])?,
        &password,
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )?;
    let mut target = None;
    match args[2].as_str() {
        "preserve" => {}
        "edit" => source.set_info_text(InfoTextField::Title, Some("preservation edit"))?,
        "copy" => {
            source.copy_page(0, 2)?;
        }
        "delete" => source.delete_page(0)?,
        "delete-retry" => {
            let before = source.save_to_bytes()?;
            let commit = source.commit_revision();
            assert!(source.delete_page(0).is_err());
            assert_eq!(source.commit_revision(), commit);
            assert_eq!(source.save_to_bytes()?, before);
            source.delete_page(1)?;
        }
        "import" | "select" | "import-retry" => {
            let doc = Document::new_with_version(PdfVersion::V2_0)?;
            if args[2] != "select" {
                doc.add_page(0)?;
            }
            doc.set_info_text(InfoTextField::Title, Some("target title"))?;
            doc.set_metadata_packet(Some(b"target packet".to_vec()))?;
            if args[2] == "import-retry" {
                let before = doc.save_to_bytes()?;
                let commit = doc.commit_revision();
                assert!(doc.import_pages_from(&source, &[0], 0).is_err());
                assert_eq!(doc.commit_revision(), commit);
                assert_eq!(doc.save_to_bytes()?, before);
            }
            let indices: &[u32] = if args[2] == "select" {
                &[0]
            } else if args[2] == "import-retry" {
                &[0, 1]
            } else {
                &[1, 0]
            };
            doc.import_pages_from(&source, indices, 0)?;
            target = Some(doc);
        }
        _ => return Err("unknown action".into()),
    }
    let doc = target.as_ref().unwrap_or(&source);
    let stamp = doc.stamp();
    let commit = doc.commit_revision();
    let bytes = match args[3].as_str() {
        "classic" => doc.save_to_bytes_with_layout(Default::default(), SaveLayout::Classic)?,
        "stream" => doc.save_to_bytes_with_layout(Default::default(), SaveLayout::XrefStream)?,
        "objects" => doc.save_to_bytes_with_layout(
            Default::default(),
            SaveLayout::XrefStreamWithObjectStreams,
        )?,
        "incremental-classic" => doc.save_incremental_to_bytes_with_layout(
            Default::default(),
            IncrementalSaveLayout::Classic,
        )?,
        "incremental-stream" => doc.save_incremental_to_bytes_with_layout(
            Default::default(),
            IncrementalSaveLayout::XrefStream,
        )?,
        _ => return Err("unknown layout".into()),
    };
    assert!(doc.is_stamp_current(&stamp));
    assert_eq!(doc.commit_revision(), commit);
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    println!("bytes={}", bytes.len());
    Ok(())
}
