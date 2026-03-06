use thiserror::Error;

#[derive(Debug, Error)]
pub enum PdfError {
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
