//! Synthetic/local external signing demonstration, not a production key store.
//! The PDF core never accepts a private key; OpenSSL is this example's backend.
use pdf::{document::Document, parser::syntax::ParseMode, signatures::SignatureOptions};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
};

fn sign(
    first: &[u8],
    second: &[u8],
    cert: &str,
    key: &str,
) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
    let mut child = Command::new("openssl")
        .args([
            "cms", "-sign", "-binary", "-md", "sha256", "-signer", cert, "-inkey", key, "-outform",
            "DER",
        ])
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().ok_or("missing signer stdout")?;
    let drain = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(65537).read_to_end(&mut bytes).map(|_| bytes)
    });
    let mut input = child.stdin.take().ok_or("missing signer stdin")?;
    input.write_all(first)?;
    input.write_all(second)?;
    input.flush()?;
    drop(input);
    if !child.wait()?.success() {
        return Err("external CMS backend failed".into());
    }
    Ok(drain.join().map_err(|_| "backend reader failed")??)
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() == 2 && args[0] == "--inspect" {
        let document = Document::from_bytes_with_mode(std::fs::read(&args[1])?, ParseMode::Strict)?;
        for report in document.signature_reports()? {
            println!(
                "object={} revision_bytes={} covers_current={} verification=NotPerformed",
                report.object_id.obj_num(),
                report.signed_revision_bytes,
                report.covers_current_document
            );
        }
        if document.save_to_bytes().is_ok() {
            return Err("signature-bearing rewrite unexpectedly allowed".into());
        }
        if document.add_page(0).is_ok() {
            return Err("signature-bearing edit unexpectedly allowed".into());
        }
        return Ok(());
    }
    let self_test = args.len() == 5 && args[4] == "--self-test";
    if args.len() != 4 && !self_test {
        return Err(
            "usage: v05_sign INPUT OUTPUT CERT_PEM KEY_PEM [--self-test] | --inspect INPUT".into(),
        );
    }
    if std::path::Path::new(&args[1]).exists() {
        return Err("refusing to overwrite output".into());
    }
    if std::fs::metadata(&args[0])?.len() > 128 * 1024 * 1024 {
        return Err("example input limit".into());
    }
    let document = Document::from_bytes_with_mode(std::fs::read(&args[0])?, ParseMode::Strict)?;
    let prepared = document.prepare_signature(SignatureOptions::default())?;
    let (first, second) = prepared.signed_ranges();
    let cms = sign(first, second, &args[2], &args[3])?;
    if self_test {
        let mut damaged = cms.clone();
        *damaged.last_mut().ok_or("empty CMS")? ^= 1;
        assert!(document.complete_signature(&prepared, &damaged).is_err());
        assert!(
            document
                .complete_signature(&prepared, &cms[..cms.len() - 1])
                .is_err()
        );
        let wrong_digest = sign(b"wrong detached document", b"", &args[2], &args[3])?;
        assert!(
            document
                .complete_signature(&prepared, &wrong_digest)
                .is_err()
        );
        assert!(
            document
                .complete_signature_with_budget(
                    &prepared,
                    &cms,
                    &pdf::budget::OperationBudget::new(1)
                )
                .is_err()
        );
        let foreign = Document::new()?;
        assert!(foreign.complete_signature(&prepared, &cms).is_err());
        let small = document.prepare_signature(SignatureOptions {
            reserved_cms_bytes: 1024,
            ..Default::default()
        })?;
        assert!(cms.len() > 1024);
        assert!(document.complete_signature(&small, &cms).is_err());
    }
    let signed = document.complete_signature(&prepared, &cms)?;
    if self_test {
        assert!(document.complete_signature(&prepared, &cms).is_err());
        let stale = document.prepare_signature(Default::default())?;
        let _: pdf::error::PdfResult<()> = document.edit_transaction(|doc| {
            doc.set_info_text(
                pdf::document::metadata::InfoTextField::Title,
                Some("rolled back"),
            )?;
            Err(pdf::error::PdfError::DocumentError(
                "injected rollback".into(),
            ))
        });
        assert!(document.complete_signature(&stale, &cms).is_err());
        println!("negative completion and retry checks passed");
    }
    // Serialize/validate completely before opening a new independent output.
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[1])?;
    output.write_all(&signed)?;
    output.sync_all()?;
    Ok(())
}
