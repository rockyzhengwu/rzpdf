#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    UnknownOperator,
    UnsupportedOperator,
}

/// Byte offsets refer to the concatenated page content, not the file offset.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDiagnostic {
    pub kind: DiagnosticKind,
    pub operator: String,
    pub byte_offset: Option<u64>,
    pub message: String,
}
