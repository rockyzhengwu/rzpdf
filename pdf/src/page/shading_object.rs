use std::sync::Arc;

use crate::{color::shading::Shading, geom::matrix::Matrix, page::graphic_state::GraphicState};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShadingDrawSource {
    pub operation_index: usize,
    pub operator_byte_offset: u64,
}

/// A shading invocation in display coordinates, retaining its original state.
#[derive(Debug, Clone)]
pub struct ShadingObject {
    pub matrix: Matrix,
    pub shading: Arc<Shading>,
    pub graphic_state: GraphicState,
    pub resource_name: String,
    source: Option<ShadingDrawSource>,
    nested_source: Option<super::source_provenance::NestedDrawSource>,
    pattern_background: bool,
}

impl ShadingObject {
    pub fn new(
        resource_name: String,
        matrix: Matrix,
        shading: Shading,
        graphic_state: GraphicState,
    ) -> Self {
        Self {
            resource_name,
            matrix,
            shading: Arc::new(shading),
            graphic_state,
            source: None,
            nested_source: None,
            pattern_background: false,
        }
    }
    pub fn source(&self) -> Option<ShadingDrawSource> {
        self.source
    }
    pub fn uses_pattern_background(&self) -> bool {
        self.pattern_background
    }
    pub(crate) fn with_pattern_background(mut self) -> Self {
        self.pattern_background = true;
        self
    }
    pub fn nested_source(&self) -> Option<&super::source_provenance::NestedDrawSource> {
        self.nested_source.as_ref()
    }
    pub(crate) fn with_source(mut self, source: Option<ShadingDrawSource>) -> Self {
        self.source = source;
        self
    }
    pub(crate) fn with_nested_source(
        mut self,
        source: Option<super::source_provenance::NestedDrawSource>,
    ) -> Self {
        self.nested_source = source;
        self
    }
    pub(crate) fn retained_payload_bytes(&self) -> crate::error::PdfResult<usize> {
        self.shading
            .retained_payload_bytes()
            .checked_add(self.resource_name.capacity())
            .ok_or_else(|| {
                crate::error::PdfError::ResourceLimit("shading draw payload overflow".into())
            })
    }
}
