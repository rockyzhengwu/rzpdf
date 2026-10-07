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

#[derive(Debug, Clone, Copy, Default)]
pub enum StreamCompression {
    #[default]
    Preserve,
    /// Compress streams without a Filter; encoded and external-file streams
    /// remain unchanged.
    FlateUnencoded,
}
