use std::{fs::OpenOptions, io::Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let mut args = std::env::args_os().skip(1);
    let path = args
        .next()
        .ok_or("usage: generate_document_seed <new-output-path>")?;
    if args.next().is_some() {
        return Err("expected one output path".into());
    }
    let bytes = rzpdf_fuzz::document_seed()?;
    // Preserve existing corpora and user files; callers create the destination directory.
    let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
    file.write_all(&bytes)?;
    file.sync_all()?;
    Ok(())
}
