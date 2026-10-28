use crate::document::identity::ObjectHandle;

/// Coordinates are in default, unrotated PDF page user space.
#[derive(Debug, Clone, PartialEq)]
pub enum AnnotationKind {
    Square,
    Circle,
    Text {
        icon: String,
        open: bool,
    },
    Popup {
        open: bool,
    },
    /// Legacy APIs use ASCII Courier. Explicit-font APIs support the bounded
    /// Unicode profile; both use LF lines and no automatic wrapping.
    FreeText {
        font_size: f32,
        alignment: TextAlignment,
    },
    Line {
        points: [f32; 4],
    },
    Polygon {
        vertices: Vec<[f32; 2]>,
    },
    PolyLine {
        vertices: Vec<[f32; 2]>,
    },
    /// QuadPoints in Adobe Z order (TL, TR, BL, BR) or spec perimeter order.
    Highlight {
        quads: Vec<[f32; 8]>,
    },
    Underline {
        quads: Vec<[f32; 8]>,
    },
    Squiggly {
        quads: Vec<[f32; 8]>,
    },
    StrikeOut {
        quads: Vec<[f32; 8]>,
    },
    Ink {
        strokes: Vec<Vec<[f32; 2]>>,
    },
    Stamp {
        name: String,
    },
    FileAttachment {
        file: ObjectHandle,
        icon: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TextAlignment {
    Left,
    Center,
    Right,
}

impl AnnotationKind {
    pub fn subtype(&self) -> &'static str {
        match self {
            Self::Square => "Square",
            Self::Circle => "Circle",
            Self::Text { .. } => "Text",
            Self::Popup { .. } => "Popup",
            Self::FreeText { .. } => "FreeText",
            Self::Line { .. } => "Line",
            Self::Polygon { .. } => "Polygon",
            Self::PolyLine { .. } => "PolyLine",
            Self::Highlight { .. } => "Highlight",
            Self::Underline { .. } => "Underline",
            Self::Squiggly { .. } => "Squiggly",
            Self::StrikeOut { .. } => "StrikeOut",
            Self::Ink { .. } => "Ink",
            Self::Stamp { .. } => "Stamp",
            Self::FileAttachment { .. } => "FileAttachment",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct Annotation {
    pub kind: AnnotationKind,
    pub rect: [f32; 4],
    pub color: [f32; 3],
    pub interior_color: Option<[f32; 3]>,
    pub contents: String,
    /// PDF annotation flags. Default: Print. Locked flags prevent later edits.
    pub flags: u32,
    pub opacity: f32,
    pub stroke_width: f32,
}

impl Annotation {
    pub fn new(kind: AnnotationKind, rect: [f32; 4]) -> Self {
        Self {
            kind,
            rect,
            color: [1., 0., 0.],
            interior_color: None,
            contents: String::new(),
            flags: 4,
            opacity: 1.,
            stroke_width: 1.,
        }
    }
}

/// A unified query includes unknown/Link/Widget entries without interpreting or
/// replacing them. Only supported kinds have an editable typed `annotation`.
#[derive(Debug, Clone)]
pub struct AnnotationRecord {
    pub handle: Option<ObjectHandle>,
    pub subtype: String,
    pub annotation: Option<Annotation>,
    /// Unsupported typed subset; the original object remains addressable.
    pub unsupported_reason: Option<String>,
    pub popup: Option<ObjectHandle>,
    pub popup_parent: Option<ObjectHandle>,
    pub reply_to: Option<ObjectHandle>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AnnotationRemoval {
    RejectRelated,
    Cascade,
}
