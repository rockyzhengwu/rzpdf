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
};

/// Owned seekable input. No Send/Sync guarantee is imposed by the facade.
pub trait DocumentReader: Read + Seek {}
impl<T: Read + Seek> DocumentReader for T {}
pub type OwnedReader = Box<dyn DocumentReader>;

/// Non-generic entry point for new documents, files, bytes and owned readers.
/// Existing PdfDocument<R> APIs remain available through Deref during migration.
/// Reader inputs must be 'static; borrowed readers can use the legacy API.
pub struct Document {
    inner: PdfDocument<OwnedReader>,
}

impl Document {
    pub fn new() -> PdfResult<Self> {
        Self::new_with_options(ParseOptions::default())
    }
    pub fn new_with_options(options: ParseOptions) -> PdfResult<Self> {
        Ok(Self {
            inner: PdfDocument::create_with_options(options)?,
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
        Self::from_reader(Cursor::new(bytes))
    }
    pub fn from_bytes_with_options(bytes: Vec<u8>, options: ParseOptions) -> PdfResult<Self> {
        Self::from_reader_with_options(Cursor::new(bytes), options)
    }
    pub fn from_bytes_with_mode(bytes: Vec<u8>, mode: ParseMode) -> PdfResult<Self> {
        Self::from_reader_with_mode(Cursor::new(bytes), mode)
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
