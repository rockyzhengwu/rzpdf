use crate::document::{annotations::TextAlignment, identity::ObjectHandle};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FormFieldKind {
    Text {
        multiline: bool,
    },
    Checkbox,
    Radio {
        no_toggle_to_off: bool,
        in_unison: bool,
    },
    Choice {
        combo: bool,
        editable: bool,
        multiple: bool,
    },
    PushButton,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChoiceOption {
    pub export: String,
    pub display: String,
}

/// Explicit horizontal font and size. None selects standard Helvetica;
/// supplied fonts must belong to this document and be embedded or bundled.
#[derive(Debug, Clone)]
pub struct FormTextStyle {
    pub font: Option<ObjectHandle>,
    pub font_size: f32,
    pub alignment: TextAlignment,
}
impl Default for FormTextStyle {
    fn default() -> Self {
        Self {
            font: None,
            font_size: 12.,
            alignment: TextAlignment::Left,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormWidgetSpec {
    pub page_index: u32,
    pub rect: [f32; 4],
    /// Checkbox/Radio on-state PDF name; Off is reserved. Required for Radio.
    /// With button_exports, this must equal the zero-based widget index.
    pub on_state: Option<String>,
}
impl FormWidgetSpec {
    pub fn new(page_index: u32, rect: [f32; 4]) -> Self {
        Self {
            page_index,
            rect,
            on_state: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct FormFieldDefinition {
    pub kind: FormFieldKind,
    pub value: super::FormValue,
    pub options: Vec<ChoiceOption>,
    pub button_exports: Option<Vec<String>>,
    pub text_style: FormTextStyle,
    pub max_len: Option<u32>,
    pub caption: String,
}
impl FormFieldDefinition {
    pub fn new(kind: FormFieldKind, value: super::FormValue) -> Self {
        Self {
            kind,
            value,
            options: Vec::new(),
            button_exports: None,
            text_style: Default::default(),
            max_len: None,
            caption: String::new(),
        }
    }
}

/// Effective type and value plus type-specific choices. Radio/ButtonState
/// identifiers are PDF /V names, not /Opt export labels or widget indices.
#[derive(Debug, Clone, PartialEq)]
pub struct FormFieldDetails {
    pub handle: ObjectHandle,
    pub name: String,
    pub kind: FormFieldKind,
    pub value: super::FormValue,
    pub options: Vec<ChoiceOption>,
    pub button_states: Vec<String>,
    pub button_exports: Option<Vec<String>>,
}
