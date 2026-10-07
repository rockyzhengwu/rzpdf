use crate::{
    geom::matrix::Matrix,
    page::graphic_state::{FillType, GraphicState},
    path::pdf_path::PdfPath,
};

/// Top-level path provenance in the decoded page Contents of one revision.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PathDrawSource {
    /// Explicit m/l/c/v/y/re/h operations; implicit closing by s/b/b* is painting.
    pub geometry_operation_indices: Vec<usize>,
    pub painting_operation_index: usize,
    pub painting_byte_offset: Option<u64>,
}

#[derive(Debug, Clone, Default)]
pub struct PathObject {
    path: PdfPath,
    matrix: Matrix,
    graphic_state: GraphicState,
    fill_type: FillType,
    stroke: bool,
    source: Option<PathDrawSource>,
    nested_source: Option<super::source_provenance::NestedDrawSource>,
}

impl PathObject {
    pub fn new(
        path: PdfPath,
        matrix: Matrix,
        graphic_state: GraphicState,
        fill_type: FillType,
        stroke: bool,
    ) -> Self {
        PathObject {
            matrix,
            path,
            graphic_state,
            fill_type,
            stroke,
            source: None,
            nested_source: None,
        }
    }
    pub fn matrix(&self) -> &Matrix {
        &self.matrix
    }
    pub fn source(&self) -> Option<&PathDrawSource> {
        self.source.as_ref()
    }
    pub fn nested_source(&self) -> Option<&super::source_provenance::NestedDrawSource> {
        self.nested_source.as_ref()
    }
    pub(crate) fn with_nested_source(
        mut self,
        source: Option<super::source_provenance::NestedDrawSource>,
    ) -> Self {
        self.nested_source = source;
        self
    }

    pub(crate) fn with_source(mut self, source: PathDrawSource) -> Self {
        self.source = Some(source);
        self
    }
    pub fn graphic_state(&self) -> &GraphicState {
        &self.graphic_state
    }
    pub fn fill_type(&self) -> &FillType {
        &self.fill_type
    }

    pub fn stroke(&self) -> bool {
        self.stroke
    }

    pub fn pdf_path(&self) -> &PdfPath {
        &self.path
    }
}
