//! Synthetic interoperability driver; credentials are never taken from argv.
use pdf::{
    document::{Document, metadata::InfoTextField},
    error::{PdfError, PdfResult},
    parser::{options::ParseOptions, syntax::ParseMode},
};
use zeroize::Zeroizing;
fn main() -> PdfResult<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 {
        return Err(PdfError::DocumentError(
            "input.pdf new-output.pdf title".into(),
        ));
    }
    let password = Zeroizing::new(
        std::env::var("RZPDF_INPUT_PASSWORD")
            .map_err(|_| PdfError::DocumentError("missing input credential".into()))?,
    );
    if std::fs::metadata(&args[0])?.len() > 512 * 1024 * 1024 {
        return Err(PdfError::ResourceLimit("example input bytes".into()));
    }
    let source = std::fs::read(&args[0])?;
    let doc = Document::from_bytes_with_password_and_options(
        source.clone(),
        &password,
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )?;
    doc.set_info_text(InfoTextField::Title, Some(&args[2]))?;
    doc.add_attachment(
        &format!("v011-{}.bin", args[2]),
        b"incremental-private-payload".to_vec(),
    )?;
    let stamp = doc.stamp();
    let output = doc.save_incremental_to_bytes()?;
    if !output.starts_with(&source) || !doc.is_stamp_current(&stamp) {
        return Err(PdfError::DocumentError(
            "incremental source contract".into(),
        ));
    }
    let reopened = Document::from_bytes_with_password_and_options(
        output.clone(),
        &password,
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )?;
    if reopened.info_text(InfoTextField::Title)?.as_deref() != Some(&args[2]) {
        return Err(PdfError::DocumentError("strict title roundtrip".into()));
    }
    use std::io::Write;
    std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?
        .write_all(&output)?;
    println!(
        "encrypted_incremental pages={} prefix={} output={} strict_reopen=ok",
        doc.try_page_count()?,
        source.len(),
        output.len()
    );
    Ok(())
}
