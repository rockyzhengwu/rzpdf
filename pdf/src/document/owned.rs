use super::PdfDocument;
use crate::{
    error::{PdfError, PdfResult},
    parser::{options::ParseOptions, syntax::ParseMode},
};
use std::{
    fs::File,
    io::{BufReader, Cursor, Read, Seek},
    ops::Deref,
    path::Path,
    rc::Rc,
};

/// The parser and future incremental writer share one immutable allocation.
/// Rc<Vec<u8>> retains the incoming Vec allocation (Rc<[u8]> would copy it).
#[derive(Clone)]
struct SourceBytes(Rc<Vec<u8>>);

impl AsRef<[u8]> for SourceBytes {
    fn as_ref(&self) -> &[u8] {
        self.0.as_slice()
    }
}

// Retained for the incremental writer, introduced in the next implementation
// slice. Reader/file constructors intentionally do not claim stable snapshots.
struct SourceBaseline {
    bytes: SourceBytes,
    last_xref_offset: u64,
    mode: ParseMode,
    original_trailer: crate::objects::pdf_dict::PdfDict,
}

/// Owned seekable input. No Send/Sync guarantee is imposed by the facade.
pub trait DocumentReader: Read + Seek {}
impl<T: Read + Seek> DocumentReader for T {}
pub type OwnedReader = Box<dyn DocumentReader>;

/// Non-generic entry point for new documents, files, bytes and owned readers.
/// Existing PdfDocument<R> APIs remain available through Deref during migration.
/// Reader inputs must be 'static; borrowed readers can use the legacy API.
pub struct Document {
    inner: PdfDocument<OwnedReader>,
    source_baseline: Option<SourceBaseline>,
}

impl Document {
    /// Append changes to a strict, immutable byte source without rebasing this
    /// document. Reopen the output to append a subsequent revision.
    /// Old content remains in the original prefix; this is not secure erasure.
    /// New documents and file/reader inputs must use full save or be reopened
    /// with `from_bytes_with_mode(bytes, ParseMode::Strict)` first.
    pub fn save_incremental_to_bytes(&self) -> PdfResult<Vec<u8>> {
        self.save_incremental_to_bytes_with_options(Default::default())
    }

    pub fn save_incremental_to_bytes_with_options(
        &self,
        options: super::save_options::IncrementalSaveOptions,
    ) -> PdfResult<Vec<u8>> {
        let budget = self.inner.ctx().limits().operation_budget();
        let bytes = self.save_incremental_with_budget(options, &budget)?;
        let (bytes, _lease) = bytes.into_reserved_parts();
        Ok(bytes)
    }

    pub fn save_incremental_with_budget(
        &self,
        options: super::save_options::IncrementalSaveOptions,
        budget: &crate::budget::OperationBudget,
    ) -> PdfResult<crate::budget::BudgetedBytes> {
        let baseline = self.source_baseline.as_ref().ok_or_else(|| {
            PdfError::UnsupportedFeature(
                "incremental save requires an immutable byte source".into(),
            )
        })?;
        if baseline.mode != ParseMode::Strict {
            return Err(PdfError::UnsupportedFeature(
                "incremental save requires strict source parsing".into(),
            ));
        }
        crate::writer::write_incremental(
            &self.inner,
            baseline.bytes.as_ref(),
            baseline.last_xref_offset,
            &baseline.original_trailer,
            options,
            budget,
        )
    }

    /// Serialization finishes before writing. A failing caller-owned writer
    /// may contain partial output; neither source nor document is committed.
    pub fn save_incremental_to_writer<W: std::io::Write>(&self, output: &mut W) -> PdfResult<()> {
        output.write_all(&self.save_incremental_to_bytes()?)?;
        Ok(())
    }

    /// Atomically replace an independent output file. Never appends in place.
    pub fn save_incremental_to_file<P: AsRef<Path>>(&self, path: P) -> PdfResult<()> {
        crate::writer::save_bytes_atomically(path.as_ref(), &self.save_incremental_to_bytes()?)
    }

    pub fn new() -> PdfResult<Self> {
        Self::new_with_options(ParseOptions::default())
    }
    pub fn new_with_options(options: ParseOptions) -> PdfResult<Self> {
        Ok(Self {
            inner: PdfDocument::create_with_options(options)?,
            source_baseline: None,
        })
    }
    pub fn new_with_version(version: super::save_options::PdfVersion) -> PdfResult<Self> {
        Self::new_with_version_and_options(version, ParseOptions::default())
    }
    pub fn new_with_version_and_options(
        version: super::save_options::PdfVersion,
        options: ParseOptions,
    ) -> PdfResult<Self> {
        Ok(Self {
            inner: PdfDocument::create_with_version_and_options(version, options)?,
            source_baseline: None,
        })
    }
    pub fn from_reader<R: Read + Seek + 'static>(reader: R) -> PdfResult<Self> {
        Self::from_reader_with_options(reader, ParseOptions::default())
    }
    pub fn from_reader_with_options<R: Read + Seek + 'static>(
        reader: R,
        options: ParseOptions,
    ) -> PdfResult<Self> {
        let reader: OwnedReader = Box::new(reader);
        Ok(Self {
            inner: PdfDocument::from_reader_with_options(reader, options)?,
            source_baseline: None,
        })
    }
    pub fn from_reader_with_mode<R: Read + Seek + 'static>(
        reader: R,
        mode: ParseMode,
    ) -> PdfResult<Self> {
        Self::from_reader_with_options(
            reader,
            ParseOptions {
                mode,
                ..Default::default()
            },
        )
    }
    pub fn from_bytes(bytes: Vec<u8>) -> PdfResult<Self> {
        Self::from_bytes_with_options(bytes, ParseOptions::default())
    }
    pub fn from_bytes_with_options(bytes: Vec<u8>, options: ParseOptions) -> PdfResult<Self> {
        let bytes = SourceBytes(Rc::new(bytes));
        let reader: OwnedReader = Box::new(Cursor::new(bytes.clone()));
        let inner = PdfDocument::from_reader_with_options(reader, options)?;
        let last_xref_offset = inner.source_xref_offset.ok_or_else(|| {
            PdfError::DocumentError("parsed source is missing its xref address".into())
        })?;
        let (original_trailer, _lease) = crate::objects::clone_dict_with_budget(
            &inner.ctx().trailer(),
            &options.limits.operation_budget(),
            options.limits,
            0,
        )?;
        Ok(Self {
            inner,
            source_baseline: Some(SourceBaseline {
                bytes,
                last_xref_offset,
                mode: options.mode,
                original_trailer,
            }),
        })
    }
    pub fn from_bytes_with_mode(bytes: Vec<u8>, mode: ParseMode) -> PdfResult<Self> {
        Self::from_bytes_with_options(
            bytes,
            ParseOptions {
                mode,
                ..Default::default()
            },
        )
    }
    pub fn open<P: AsRef<Path>>(path: P) -> PdfResult<Self> {
        Self::open_with_options(path, ParseOptions::default())
    }
    pub fn open_with_options<P: AsRef<Path>>(path: P, options: ParseOptions) -> PdfResult<Self> {
        let file = File::open(path)
            .map_err(|error| PdfError::IOError(format!("open document failed: {error}")))?;
        Self::from_reader_with_options(BufReader::new(file), options)
    }
    pub fn open_with_mode<P: AsRef<Path>>(path: P, mode: ParseMode) -> PdfResult<Self> {
        Self::open_with_options(
            path,
            ParseOptions {
                mode,
                ..Default::default()
            },
        )
    }
    pub fn edit_transaction<T>(
        &self,
        edit: impl FnOnce(&Document) -> PdfResult<T>,
    ) -> PdfResult<T> {
        self.inner.edit_transaction(|_| edit(self))
    }
}

impl Deref for Document {
    type Target = PdfDocument<OwnedReader>;
    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

#[cfg(test)]
mod source_baseline_tests {
    use super::*;
    use crate::document::metadata::InfoTextField;

    fn source() -> Vec<u8> {
        let document = Document::new().unwrap();
        document.add_page(0).unwrap();
        document.save_to_bytes().unwrap()
    }

    #[test]
    fn owned_source_baseline_retains_the_incoming_allocation_and_exact_bytes() {
        let mut bytes = source();
        bytes.extend_from_slice(b" \r\n");
        let address = bytes.as_ptr();
        let expected = bytes.clone();
        let document = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        let baseline = document.source_baseline.as_ref().unwrap();
        assert_eq!(baseline.bytes.as_ref().as_ptr(), address);
        assert_eq!(baseline.bytes.as_ref(), expected);
        assert_eq!(Rc::strong_count(&baseline.bytes.0), 2);
        assert_eq!(baseline.mode, ParseMode::Strict);
        let offset = usize::try_from(baseline.last_xref_offset).unwrap();
        assert!(baseline.bytes.as_ref()[offset..].starts_with(b"xref"));
    }

    #[test]
    fn owned_source_baseline_does_not_rebase_after_edit_rollback_or_full_save() {
        let bytes = source();
        let document = Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).unwrap();
        let baseline_offset = document.source_baseline.as_ref().unwrap().last_xref_offset;
        let result: PdfResult<()> = document.edit_transaction(|doc| {
            doc.set_info_text(InfoTextField::Title, Some("rolled back"))?;
            Err(PdfError::DocumentError("rollback".into()))
        });
        assert!(result.is_err());
        assert_eq!(document.info_text(InfoTextField::Title).unwrap(), None);
        document
            .set_info_text(InfoTextField::Title, Some("edited"))
            .unwrap();
        let stamp = document.stamp();
        let commit = document.commit_revision();
        let rewritten = document.save_to_bytes().unwrap();
        assert!(document.is_stamp_current(&stamp));
        assert_eq!(document.commit_revision(), commit);
        let baseline = document.source_baseline.as_ref().unwrap();
        assert_eq!(baseline.bytes.as_ref(), bytes);
        assert_eq!(baseline.last_xref_offset, baseline_offset);
        assert_ne!(rewritten, bytes);
        let reopened = Document::from_bytes_with_mode(rewritten, ParseMode::Strict).unwrap();
        assert_eq!(
            reopened.info_text(InfoTextField::Title).unwrap().as_deref(),
            Some("edited")
        );
    }

    #[test]
    fn owned_source_baseline_records_latest_revision_not_its_prev() {
        let mut bytes = source();
        let original = Document::from_bytes_with_mode(bytes.clone(), ParseMode::Strict).unwrap();
        let prev = original.source_baseline.as_ref().unwrap().last_xref_offset;
        let trailer = original.ctx().trailer();
        let size = trailer
            .get("Size")
            .unwrap()
            .as_number()
            .unwrap()
            .as_u64_checked()
            .unwrap();
        let root = trailer.get("Root").unwrap().as_object_id().unwrap();
        let latest = bytes.len();
        bytes.extend_from_slice(format!(
            "xref\n0 1\n0000000000 65535 f \ntrailer\n<< /Size {size} /Root {} {} R /Prev {prev} >>\nstartxref\n{latest}\n%%EOF\n",
            root.obj_num(), root.gen_num(),
        ).as_bytes());
        let document = Document::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        assert_eq!(
            document.source_baseline.as_ref().unwrap().last_xref_offset,
            latest as u64
        );
        assert_ne!(latest as u64, prev);
        assert_eq!(document.try_page_count().unwrap(), 1);
    }

    #[test]
    fn owned_source_baseline_does_not_trust_mutable_readers_or_new_documents() {
        assert!(Document::new().unwrap().source_baseline.is_none());
        let bytes = source();
        let document =
            Document::from_reader_with_mode(Cursor::new(bytes.clone()), ParseMode::Strict).unwrap();
        assert!(document.source_baseline.is_none());
        let compatible = Document::from_bytes(bytes.clone()).unwrap();
        assert_eq!(
            compatible.source_baseline.as_ref().unwrap().mode,
            ParseMode::Compatible
        );
        let legacy = PdfDocument::from_bytes_with_mode(bytes, ParseMode::Strict).unwrap();
        assert_eq!(
            compatible.save_to_bytes().unwrap(),
            legacy.save_to_bytes().unwrap()
        );
    }
}
