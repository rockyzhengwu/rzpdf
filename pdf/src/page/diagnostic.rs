#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DiagnosticKind {
    UnknownOperator,
    UnsupportedOperator,
}

/// Byte offsets refer to decoded content, not the file offset.
/// For a Form diagnostic, `source` qualifies the leaf offset with its call sites.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ContentDiagnostic {
    pub kind: DiagnosticKind,
    pub operator: String,
    pub byte_offset: Option<u64>,
    pub message: String,
    pub source: Option<super::source_provenance::NestedDrawSource>,
}
