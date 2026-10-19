//! Original AF edit/save/page-operation interoperability driver. New output only.
use pdf::{
    budget::OperationBudget,
    document::{
        Document,
        associated_files::{AssociatedFileDefinition, AssociatedFileRelationship},
        save_options::{
            IncrementalSaveLayout, PdfVersion, SaveLayout, SaveOptions, StreamCompression,
        },
    },
    parser::{options::ParseOptions, syntax::ParseMode},
    security::{EncryptionOptions, SaveSecurity},
};
use std::{
    fs::OpenOptions,
    io::Write,
    path::{Path, PathBuf},
};
fn write(out: &Path, label: &str, data: &[u8]) -> Result<(), Box<dyn std::error::Error>> {
    OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(out.join(format!("{label}.pdf")))?
        .write_all(data)?;
    Ok(())
}
fn open(bytes: Vec<u8>, password: &str) -> Result<Document, Box<dyn std::error::Error>> {
    Ok(Document::from_bytes_with_password_and_options(
        bytes,
        password,
        ParseOptions {
            mode: ParseMode::Strict,
            ..Default::default()
        },
    )?)
}
fn edit_append(
    source: &[u8],
    password: &str,
    out: &Path,
    label: &str,
    b: &OperationBudget,
) -> Result<(), Box<dyn std::error::Error>> {
    let d = open(source.to_vec(), password)?;
    let owner = d.associated_file_owner(Some(0), b)?;
    let list = d.associated_files(&owner, b)?;
    let file = list.files()[1].filespec().unwrap().clone();
    drop(list);
    let stamp = d.stamp();
    assert!(!d.bind_associated_file(&owner, &file, b)?);
    assert!(d.is_stamp_current(&stamp));
    assert_eq!(d.save_incremental_to_bytes()?, source);
    d.unbind_associated_file(&owner, &file, b)?;
    for (layout_name, layout) in [
        ("classic", IncrementalSaveLayout::Classic),
        ("xref", IncrementalSaveLayout::XrefStream),
    ] {
        let bytes = d.save_incremental_to_bytes_with_layout(Default::default(), layout)?;
        assert!(bytes.starts_with(source));
        let r = open(bytes.clone(), if password.is_empty() { "" } else { "user" })?;
        let target = r.associated_file_owner(Some(0), b)?;
        assert_eq!(r.associated_files(&target, b)?.files().len(), 1);
        write(out, &format!("{label}-append-{layout_name}"), &bytes)?;
    }
    Ok(())
}
fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = std::env::args_os().collect();
    if args.len() != 3 {
        return Err("usage: v014_associated_files INPUT NEW_DIRECTORY".into());
    }
    let d = Document::from_bytes_with_mode(std::fs::read(&args[1])?, ParseMode::Strict)?;
    let b = OperationBudget::new(128 * 1024 * 1024);
    let owner = d.associated_file_owner(None, &b)?;
    let list = d.associated_files(&owner, &b)?;
    assert_eq!(
        d.associated_file_data(&list, 0, &b)?.as_slice(),
        b"AF fixture\n"
    );
    let old = list.files()[0].filespec().unwrap().clone();
    drop(list);
    let cloned = d.clone_associated_file(
        &old,
        &AssociatedFileDefinition::new(
            "复用源文件.txt",
            "text/plain",
            AssociatedFileRelationship::Data,
        ),
        &b,
    )?;
    let created = d.create_associated_file(
        &AssociatedFileDefinition::new(
            "新附件.bin",
            "application/octet-stream",
            AssociatedFileRelationship::Supplement,
        ),
        b"new\0\xffdata",
        &b,
    )?;
    for page in [None, Some(0)] {
        let owner = d.associated_file_owner(page, &b)?;
        d.bind_associated_file(&owner, &cloned, &b)?;
    }
    for page in [Some(0), Some(1)] {
        let owner = d.associated_file_owner(page, &b)?;
        d.bind_associated_file(&owner, &created, &b)?;
    }
    let owner = d.associated_file_owner(Some(0), &b)?;
    d.unbind_associated_file(&owner, &created, &b)?;
    let out = PathBuf::from(&args[2]);
    std::fs::create_dir(&out)?;
    let mut encryption = EncryptionOptions::new("user", "owner")?;
    encryption.permissions.modify = false;
    for (layout_name, layout) in [
        ("classic", SaveLayout::Classic),
        ("xref", SaveLayout::XrefStream),
        ("objects", SaveLayout::XrefStreamWithObjectStreams),
    ] {
        for (encoding, compression) in [
            ("preserve", StreamCompression::Preserve),
            ("flate", StreamCompression::FlateUnencoded),
        ] {
            let options = SaveOptions {
                compression,
                ..Default::default()
            };
            let label = format!("{layout_name}-{encoding}");
            let bytes = d.save_to_bytes_with_layout(options, layout)?;
            write(&out, &label, &bytes)?;
            edit_append(&bytes, "", &out, &label, &b)?;
            let protected = d.save_to_bytes_with_layout_and_security(
                options,
                layout,
                SaveSecurity::Set(&encryption),
            )?;
            assert!(open(protected.clone(), "wrong").is_err());
            let user = open(protected.clone(), "user")?;
            let target = user.associated_file_owner(None, &b)?;
            let list = user.associated_files(&target, &b)?;
            let h = list.files()[0].filespec().unwrap().clone();
            drop(list);
            let stamp = user.stamp();
            assert!(user.unbind_associated_file(&target, &h, &b).is_err());
            assert!(user.bind_associated_file(&target, &h, &b).is_err());
            assert!(user.is_stamp_current(&stamp));
            write(&out, &format!("{label}-r6"), &protected)?;
            edit_append(&protected, "owner", &out, &format!("{label}-r6"), &b)?;
        }
    }
    let baseline = d.save_to_bytes()?;
    let copied = open(baseline.clone(), "")?;
    copied.copy_page(0, 2)?;
    write(&out, "page-copy", &copied.save_to_bytes()?)?;
    let reordered = open(baseline.clone(), "")?;
    reordered.reorder_pages(&[1, 0])?;
    write(&out, "page-reorder", &reordered.save_to_bytes()?)?;
    let deleted = open(baseline, "")?;
    deleted.delete_page(1)?;
    write(&out, "page-delete", &deleted.save_to_bytes()?)?;
    let imported = Document::new_with_version(PdfVersion::V2_0)?;
    imported.import_pages_from(&d, &[0, 1], 0)?;
    write(&out, "page-import", &imported.save_to_bytes()?)?;
    let extracted = d.extract_pages(&[1])?;
    write(&out, "page-extract", &extracted.save_to_bytes()?)?;
    assert_eq!(b.live_bytes(), 0);
    println!(
        "cloned={} created={} tracked_peak={}",
        cloned.object_id().obj_num(),
        created.object_id().obj_num(),
        b.peak_bytes()
    );
    Ok(())
}
