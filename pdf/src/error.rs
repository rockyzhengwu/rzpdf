use thiserror::Error;

/// Stable diagnostic context for a failed render-time image decode.
#[derive(Debug, Clone)]
pub struct ImageDecodeDetails {
    pub encoding: &'static str,
    pub width: u32,
    pub height: u32,
    pub target_width: Option<u32>,
    pub target_height: Option<u32>,
    pub target_decode_supported: bool,
    pub unsupported_reason: Option<&'static str>,
    pub jpeg_process: Option<&'static str>,
}

#[derive(Debug, Error)]
pub enum PdfError {
    #[error("{source}")]
    ImageDecode {
        details: Box<ImageDecodeDetails>,
        #[source]
        source: Box<PdfError>,
    },

    #[error("malformed signature: {0}")]
    MalformedSignature(String),

    #[error("signature digest or cryptographic verification failed: {0}")]
    SignatureMismatch(String),

    #[error("document password required")]
    PasswordRequired,

    #[error("invalid document password")]
    InvalidPassword,

    #[error("unsupported document security: {0}")]
    UnsupportedSecurity(String),

    #[error("malformed document security: {0}")]
    MalformedSecurity(String),

    #[error("document permission denied: {0}")]
    PermissionDenied(String),

    #[error("cryptographic randomness unavailable")]
    RandomnessUnavailable,

    #[error("unsupported feature: {0}")]
    UnsupportedFeature(String),

    #[error("invalid reference: {0}")]
    InvalidReference(String),

    #[error("resource limit exceeded: {0}")]
    ResourceLimit(String),

    /// Stable machine-readable resource failure. Legacy unclassified failures
    /// remain ResourceLimit; callers must not infer reasons from their text.
    #[error("resource limit exceeded: {stage}/{reason} (limit {limit}, actual {actual})")]
    ResourceLimitExceeded {
        reason: &'static str,
        stage: &'static str,
        limit: u64,
        actual: u64,
    },

    #[error("IO Error '{0}' ")]
    IOError(String),

    #[error("Eeach End OF File")]
    EndofFile,

    #[error("Reader Error: '{0}'")]
    ReaderError(String),

    #[error("Parser Error:'{0}'")]
    ParserError(String),

    #[error("Object Error:'{0}'")]
    ObjectError(String),

    #[error("Filter Error:'{0}'")]
    FilterError(String),

    #[error("Document Error: '{0}'")]
    DocumentError(String),

    // Page
    #[error("Page not exist")]
    PageNotExist,

    #[error("Page content need a stream")]
    PageContentIsNotStream,

    #[error("Page Parent is not a dict")]
    PageParentIsNotDict,

    #[error("Page Resource is not dict")]
    PageResourcesIsNotDict,

    #[error("Page Resource Error :`{0}`")]
    PageResourceError(String),

    #[error("Page Xobject is not dict")]
    PageXobjectIsNotDict,

    #[error("Page Xobject not found")]
    PageXobjectNotFound,

    #[error("Page operator error:`{0}`")]
    PageOperatorError(String),

    #[error("Content Parser error: `{0}` ")]
    ContentParseError(String),

    #[error("PdfObject is not Rectangle:`{0}` ")]
    RectangleFromPdfObjectError(String),

    #[error("PdfObject is not Matrix: `{0}`")]
    MatrixFromPdfObjectError(String),

    // Color
    #[error("Color error: `{0}`")]
    ColorError(String),

    #[error("Function error: `{0}`")]
    FunctionError(String),

    #[error("Font error:`{0}`")]
    FontError(String),
}

pub type PdfResult<T> = std::result::Result<T, PdfError>;

impl From<std::io::Error> for PdfError {
    fn from(error: std::io::Error) -> Self {
        Self::IOError(error.to_string())
    }
}

impl PdfError {
    /// The original error category remains available under image diagnostics.
    pub fn root_cause(&self) -> &Self {
        match self {
            Self::ImageDecode { source, .. } => source.root_cause(),
            error => error,
        }
    }
    pub fn image_decode_details(&self) -> Option<&ImageDecodeDetails> {
        match self {
            Self::ImageDecode { details, .. } => Some(details),
            _ => None,
        }
    }
}
