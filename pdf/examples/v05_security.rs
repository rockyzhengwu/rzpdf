//! Synthetic interoperability CLI. Passwords come from RZPDF_* environment
//! variables, not command-line arguments. Never use production passwords in
//! checked-in scripts or logs. inspect does not create OUTPUT.
use pdf::{
    document::{Document, acroform::FormValue},
    parser::{options::ParseOptions, syntax::ParseMode},
    security::{EncryptionOptions, SaveSecurity},
};
use zeroize::Zeroizing;

fn password(name: &str) -> Result<Zeroizing<String>, Box<dyn std::error::Error>> {
    std::env::var(name)
        .map(Zeroizing::new)
        .map_err(|_| format!("missing {name}").into())
}

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if !(3..=4).contains(&args.len()) {
        return Err(
            "usage: v05_security INPUT OUTPUT encrypt|preserve|remove|replace|inspect [--fill]"
                .into(),
        );
    }
    let mode = args[2].as_str();
    if !["encrypt", "preserve", "remove", "replace", "inspect"].contains(&mode) {
        return Err("unknown mode".into());
    }
    if mode != "inspect" && std::path::Path::new(&args[1]).exists() {
        return Err("refusing to overwrite existing output".into());
    }
    if std::fs::metadata(&args[0])?.len() > 512 * 1024 * 1024 {
        return Err("input exceeds example's 512 MiB ceiling".into());
    }
    let bytes = std::fs::read(&args[0])?;
    let options = ParseOptions {
        mode: ParseMode::Strict,
        ..Default::default()
    };
    let input_password = std::env::var("RZPDF_INPUT_PASSWORD")
        .ok()
        .map(Zeroizing::new);
    let document = match &input_password {
        Some(password) => Document::from_bytes_with_password_and_options(bytes, password, options)?,
        None => Document::from_bytes_with_options(bytes, options)?,
    };
    if let Some(flag) = args.get(3) {
        if flag != "--fill" {
            return Err("unknown fourth argument".into());
        }
        document.set_form_value("external_text", FormValue::Text("Protected update".into()))?;
    }
    // Read actual content, not only the Catalog, to exercise decryption and
    // downstream filtering/fonts before handing the file off.
    for page in 0..document.try_page_count()? {
        document.get_page(page as u32)?.display_list()?;
    }
    if mode == "inspect" {
        println!(
            "pages={} security={:?}",
            document.try_page_count()?,
            document.security_report()
        );
        return Ok(());
    }
    let settings = if matches!(mode, "encrypt" | "replace") {
        let user = password("RZPDF_OUTPUT_USER_PASSWORD")?;
        let owner = password("RZPDF_OUTPUT_OWNER_PASSWORD")?;
        let mut settings = EncryptionOptions::new(&user, &owner)?;
        settings.encrypt_metadata = std::env::var("RZPDF_CLEAR_METADATA").as_deref() != Ok("1");
        if std::env::var("RZPDF_RESTRICT_MODIFY").as_deref() == Ok("1") {
            settings.permissions.modify = false;
            settings.permissions.copy = false;
        }
        if std::env::var("RZPDF_RESTRICT_FILL").as_deref() == Ok("1") {
            settings.permissions.fill_forms = false;
        }
        Some(settings)
    } else {
        None
    };
    match mode {
        "preserve" => document.save_to_file(&args[1])?,
        "remove" => document.save_to_file_with_security(
            &args[1],
            Default::default(),
            SaveSecurity::Remove,
        )?,
        _ => document.save_to_file_with_security(
            &args[1],
            Default::default(),
            SaveSecurity::Set(settings.as_ref().unwrap()),
        )?,
    }
    Ok(())
}
