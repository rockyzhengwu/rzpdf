#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum PdfVersion {
    V1_0,
    V1_1,
    V1_2,
    V1_3,
    V1_4,
    V1_5,
    V1_6,
    V1_7,
    V2_0,
}

impl PdfVersion {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::V1_0 => "1.0",
            Self::V1_1 => "1.1",
            Self::V1_2 => "1.2",
            Self::V1_3 => "1.3",
            Self::V1_4 => "1.4",
            Self::V1_5 => "1.5",
            Self::V1_6 => "1.6",
            Self::V1_7 => "1.7",
            Self::V2_0 => "2.0",
        }
    }
    pub(crate) fn parse(value: &str) -> Option<Self> {
        [
            Self::V1_0,
            Self::V1_1,
            Self::V1_2,
            Self::V1_3,
            Self::V1_4,
            Self::V1_5,
            Self::V1_6,
            Self::V1_7,
            Self::V2_0,
        ]
        .into_iter()
        .find(|version| version.as_str() == value)
    }
}

/// Full-rewrite configuration. Changing the header does not establish PDF/A,
/// PDF/UA or PDF 2.0 conformance; unsupported features remain unsupported.
#[derive(Debug, Clone, Copy)]
pub struct SaveOptions {
    /// None preserves the source header and Catalog override. Explicit versions
    /// must not downgrade the effective Header/Catalog version. Unknown or
    /// malformed Catalog versions are rejected during save.
    pub version: Option<PdfVersion>,
    pub compression: StreamCompression,
    /// Maximum final PDF byte length; temporary object buffers are separate.
    pub max_output_bytes: usize,
}

impl Default for SaveOptions {
    fn default() -> Self {
        Self {
            version: None,
            compression: StreamCompression::Preserve,
            max_output_bytes: 512 * 1024 * 1024,
        }
    }
}

/// Append-only save configuration. Original bytes, including old text and
/// attachments, remain recoverable: this is NOT redaction or secure erasure.
#[derive(Debug, Clone, Copy)]
pub struct IncrementalSaveOptions {
    /// Includes the original byte prefix and the appended revision.
    pub max_output_bytes: usize,
}

impl Default for IncrementalSaveOptions {
    fn default() -> Self {
        Self {
            max_output_bytes: 512 * 1024 * 1024,
        }
    }
}

#[derive(Debug, Clone, Copy, Default)]
pub enum StreamCompression {
    #[default]
    Preserve,
    /// Compress streams without a Filter; encoded and external-file streams
    /// remain unchanged.
    FlateUnencoded,
}

/// Unsigned append-only layout. New object streams are never generated.
/// Signature and controlled-update workflows keep their existing private layout.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum IncrementalSaveLayout {
    #[default]
    Classic,
    /// A plaintext, sparse cross-reference stream; requires effective PDF 1.5+.
    /// Changed compressed members are appended as ordinary generation-zero objects.
    XrefStream,
}

/// Full-save container layout, independent of ordinary stream compression.
/// Existing save APIs always select Classic. No object-stream packing is
/// implied by XrefStream; protection and incremental combinations are separate.
#[non_exhaustive]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum SaveLayout {
    #[default]
    Classic,
    /// PDF 1.5+ with a new, uncompressed cross-reference stream. Existing R6
    /// protection is preserved; adding R6 follows its PDF 2.0 requirements.
    XrefStream,
    /// PDF 1.5+; eligible generation-zero objects are grouped in
    /// new object streams (100 members / 1 MiB decoded per group at most).
    /// Extends collections or active references to replaced containers are
    /// rejected rather than silently discarded. Existing object IDs survive.
    /// With R6, members remain plaintext until the containing stream is encrypted.
    XrefStreamWithObjectStreams,
}
