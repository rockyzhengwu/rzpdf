//! Synthetic offline interoperability driver, not a private-key storage API.
use pdf::{
    document::{
        Document,
        acroform::{FormTextStyle, FormValue},
    },
    parser::syntax::ParseMode,
    signatures::{SignatureAppearance, SignatureFieldOptions, SignatureOptions},
};
use std::{
    io::{Read, Write},
    process::{Command, Stdio},
};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() < 2 {
        return Err("verify INPUT | sign|append INPUT NEW_OUTPUT CERT KEY rsa|pss|ec FIELD".into());
    }
    let input = &args[1];
    if std::fs::metadata(input)?.len() > 128 * 1024 * 1024 {
        return Err("input limit".into());
    }
    let doc = Document::from_bytes_with_mode(std::fs::read(input)?, ParseMode::Strict)?;
    if args[0] == "audit" && args.len() == 2 {
        println!("constraints={:?}", doc.signature_constraints()?);
        println!("changes={:?}", doc.signature_modifications()?);
        return Ok(());
    }
    if matches!(args[0].as_str(), "fill" | "check" | "square") && args.len() >= 5 {
        let prepared = if args[0] == "square" {
            doc.prepare_signed_square(args[3].parse()?, [20., 20., 80., 80.], &args[4])?
        } else {
            let field = doc
                .form_field_tree()?
                .into_iter()
                .find(|n| n.name == args[3])
                .ok_or("field missing")?;
            let mut style = FormTextStyle::default();
            if let Some(font) = args.get(5) {
                style.font = Some(
                    doc.object_handle(pdf::objects::object_id::ObjectId::new(font.parse()?, 0))?,
                );
            }
            let value = if args[0] == "check" {
                FormValue::Checkbox(args[4] == "on")
            } else {
                FormValue::Text(args[4].clone())
            };
            doc.prepare_signed_form_fill(
                &field.handle,
                value,
                if args[0] == "fill" { Some(style) } else { None },
            )?
        };
        let bytes = doc.complete_signed_update(&prepared)?;
        let mut output = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&args[2])?;
        output.write_all(&bytes)?;
        output.sync_all()?;
        return Ok(());
    }
    if args[0] == "verify" && args.len() == 2 {
        let reports = doc.verify_signatures()?;
        println!("count={}", reports.len());
        for report in reports {
            println!(
                "object={} revision={} bytes={} current={} state={:?} verification={:?} field={:?}",
                report.object_id.obj_num(),
                report.signed_revision,
                report.signed_revision_bytes,
                report.covers_current_document,
                report.source_object_state,
                report.verification,
                report.field.map(|f| f.name)
            );
        }
        return Ok(());
    }
    if !matches!(args[0].as_str(), "sign" | "append" | "field") || !(7..=9).contains(&args.len()) {
        return Err("invalid arguments".into());
    }
    let mut command = Command::new("openssl");
    command.args([
        "cms", "-sign", "-binary", "-md", "sha256", "-signer", &args[3], "-inkey", &args[4],
        "-outform", "DER",
    ]);
    match args[5].as_str() {
        "pss" => {
            command.args([
                "-keyopt",
                "rsa_padding_mode:pss",
                "-keyopt",
                "rsa_pss_saltlen:32",
            ]);
        }
        "rsa" | "ec" => {}
        _ => return Err("unsupported algorithm".into()),
    }
    let options = SignatureOptions {
        field_name: args[6].clone(),
        ..Default::default()
    };
    let prepared = if args[0] == "field" {
        let mut style = FormTextStyle::default();
        if let Some(font) = args.get(8) {
            style.font =
                Some(doc.object_handle(pdf::objects::object_id::ObjectId::new(font.parse()?, 0))?);
        }
        doc.prepare_field_signature(SignatureFieldOptions {
            field_name: args[6].clone(),
            reserved_cms_bytes: 16384,
            appearance: args.get(7).map(|text| SignatureAppearance {
                text: text.clone(),
                style,
            }),
        })?
    } else if args[0] == "append" {
        doc.prepare_additional_signature(options)?
    } else {
        doc.prepare_signature(options)?
    };
    let mut child = command
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()?;
    let stdout = child.stdout.take().ok_or("stdout unavailable")?;
    let drain = std::thread::spawn(move || {
        let mut bytes = Vec::new();
        stdout.take(65537).read_to_end(&mut bytes).map(|_| bytes)
    });
    let mut stdin = child.stdin.take().ok_or("stdin unavailable")?;
    prepared.write_signed_content(&mut stdin)?;
    drop(stdin);
    if !child.wait()?.success() {
        return Err("external signer failed".into());
    }
    let cms = drain.join().map_err(|_| "signer reader failed")??;
    let bytes = doc.complete_signature(&prepared, &cms)?;
    let check = Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict)?;
    let reports = check.verify_signatures()?;
    if reports.is_empty()
        || reports
            .iter()
            .any(|r| r.verification != pdf::signatures::SignatureVerification::Valid)
    {
        return Err("signed output verification failed".into());
    }
    let mut output = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?;
    output.write_all(&bytes)?;
    output.sync_all()?;
    println!(
        "signed and verified {} signatures; no certificate trust assertion",
        reports.len()
    );
    Ok(())
}
