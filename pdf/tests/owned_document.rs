use pdf::{
    document::{Document, PdfDocument},
    error::{PdfError, PdfResult},
    parser::{
        options::{Limits, ParseOptions},
        syntax::ParseMode,
    },
};
use std::io::Cursor;

#[test]
fn one_non_generic_type_supports_creation_bytes_and_owned_reader() {
    let created: Document = Document::new().unwrap();
    created.add_page(0).unwrap();
    created
        .replace_page_contents(0, b"0 0 10 10 re f\n".to_vec())
        .unwrap();
    let bytes = created.save_to_bytes().unwrap();
    let from_bytes = Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).unwrap();
    let from_reader =
        Document::from_reader_with_mode(Cursor::new(bytes.clone()), ParseMode::Strict).unwrap();
    let legacy = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
    assert_eq!(legacy.try_page_count().unwrap(), 1);
    for doc in [created, from_bytes, from_reader] {
        assert_eq!(doc.try_page_count().unwrap(), 1);
        assert_eq!(
            doc.get_page(0)
                .unwrap()
                .display_list()
                .unwrap()
                .objects()
                .len(),
            1
        );
        let reopened =
            Document::from_bytes_with_mode(doc.save_to_bytes().unwrap(), ParseMode::Strict)
                .unwrap();
        assert_eq!(reopened.try_page_count().unwrap(), 1);
        assert_ne!(doc.identity(), reopened.identity());
    }
}

#[test]
fn facade_transactions_keep_the_non_generic_api_and_roll_back() {
    let doc = Document::new().unwrap();
    let original = doc.stamp();
    let result: PdfResult<()> = doc.edit_transaction(|doc: &Document| {
        doc.add_page(0)?;
        doc.edit_transaction(|doc: &Document| {
            doc.add_page(1)?;
            Ok(())
        })?;
        assert_eq!(doc.try_page_count()?, 2);
        Err(PdfError::DocumentError("rollback".into()))
    });
    assert!(result.is_err());
    assert_eq!(doc.try_page_count().unwrap(), 0);
    assert!(!doc.is_stamp_current(&original));
    doc.add_page(0).unwrap();
}

#[test]
fn creation_options_are_not_lost_by_reader_erasure() {
    assert!(matches!(
        Document::new_with_options(ParseOptions {
            limits: Limits {
                max_objects: 1,
                ..Default::default()
            },
            ..Default::default()
        }),
        Err(PdfError::ResourceLimit(_))
    ));
}

#[test]
fn file_open_and_byte_open_share_the_same_public_type() {
    struct TestFile(std::path::PathBuf);
    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = std::fs::remove_file(&self.0);
        }
    }
    let stamp = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_nanos();
    let path = std::env::temp_dir().join(format!(
        "rzpdf-owned-test-{}-{stamp}.pdf",
        std::process::id()
    ));
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&path)
        .unwrap();
    let artifact = TestFile(path);
    let source = Document::new().unwrap();
    source.add_page(0).unwrap();
    source.save_to_writer(&mut file).unwrap();
    drop(file);
    let from_file = Document::open_with_mode(&artifact.0, ParseMode::Strict).unwrap();
    let from_bytes = Document::from_bytes(source.save_to_bytes().unwrap()).unwrap();
    for document in [from_file, from_bytes] {
        assert_eq!(document.try_page_count().unwrap(), 1);
        document.add_page(1).unwrap();
        assert_eq!(
            Document::from_bytes(document.save_to_bytes().unwrap())
                .unwrap()
                .try_page_count()
                .unwrap(),
            2
        );
    }
}
