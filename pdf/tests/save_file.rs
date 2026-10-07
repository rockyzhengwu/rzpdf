use pdf::{
    document::PdfDocument,
    objects::{PdfObject, pdf_name::PdfName},
    parser::syntax::ParseMode,
};

struct TestDirectory(std::path::PathBuf);

#[cfg(unix)]
#[test]
fn replacing_existing_file_preserves_permission_bits() {
    use std::os::unix::fs::PermissionsExt;
    let directory = TestDirectory::new();
    let path = directory.0.join("permissions.pdf");
    std::fs::write(&path, b"previous").unwrap();
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o640)).unwrap();
    PdfDocument::new().unwrap().save_to_file(&path).unwrap();
    assert_eq!(
        std::fs::metadata(path).unwrap().permissions().mode() & 0o777,
        0o640
    );
}
impl TestDirectory {
    fn new() -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let stamp = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!(
            "rzpdf-save-test-{}-{stamp}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        for entry in std::fs::read_dir(&self.0).unwrap() {
            let _ = std::fs::remove_file(entry.unwrap().path());
        }
        let _ = std::fs::remove_dir(&self.0);
    }
}

#[test]
fn file_save_creates_and_replaces_without_leaving_temporary_files() {
    let directory = TestDirectory::new();
    let path = directory.0.join("document.pdf");
    let doc = PdfDocument::new().unwrap();
    doc.save_to_file(&path).unwrap();
    doc.add_page(0).unwrap();
    doc.save_to_file(&path).unwrap();
    let reopened =
        PdfDocument::from_bytes_with_mode(std::fs::read(&path).unwrap(), ParseMode::Strict)
            .unwrap();
    assert_eq!(reopened.try_page_count().unwrap(), 1);
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn serialization_failure_preserves_existing_target() {
    let directory = TestDirectory::new();
    let path = directory.0.join("document.pdf");
    std::fs::write(&path, b"existing user file").unwrap();
    let doc = PdfDocument::new().unwrap();
    doc.try_insert_object(PdfObject::PdfName(PdfName::new("bad\0name".into())))
        .unwrap();
    assert!(doc.save_to_file(&path).is_err());
    assert_eq!(std::fs::read(&path).unwrap(), b"existing user file");
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[test]
fn output_budget_failure_preserves_existing_file_without_temporary_output() {
    use pdf::document::save_options::SaveOptions;
    let directory = TestDirectory::new();
    let path = directory.0.join("limited.pdf");
    std::fs::write(&path, b"existing").unwrap();
    let result = PdfDocument::new().unwrap().save_to_file_with_options(
        &path,
        SaveOptions {
            max_output_bytes: 8,
            ..Default::default()
        },
    );
    assert!(matches!(
        result,
        Err(pdf::error::PdfError::ResourceLimit(_))
    ));
    assert_eq!(std::fs::read(&path).unwrap(), b"existing");
    assert_eq!(std::fs::read_dir(&directory.0).unwrap().count(), 1);
}

#[cfg(unix)]
#[test]
fn file_save_rejects_symlinks_without_modifying_their_targets() {
    let directory = TestDirectory::new();
    let original = directory.0.join("original.pdf");
    let link = directory.0.join("link.pdf");
    std::fs::write(&original, b"original").unwrap();
    std::os::unix::fs::symlink(&original, &link).unwrap();
    assert!(PdfDocument::new().unwrap().save_to_file(&link).is_err());
    assert_eq!(std::fs::read(&original).unwrap(), b"original");
    assert!(
        std::fs::symlink_metadata(&link)
            .unwrap()
            .file_type()
            .is_symlink()
    );
}
